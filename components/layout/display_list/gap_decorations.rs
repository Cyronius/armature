/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Gap decorations (`column-rule` and `row-rule` painted in the gaps of grid
//! and flex containers).
//!
//! <https://drafts.csswg.org/css-gaps-1/>

use std::ops::Range;

use style::color::AbsoluteColor;
use style::properties::ComputedValues;
use style::values::computed::{
    BorderStyle, GapRuleBreak, GapRuleInset, GapRuleVisibilityItems, Length, PositiveInteger,
    RuleOverlap,
};
use style::values::generics::gaps::{GenericGapRuleList, GenericGapRuleListItem};

/// A gap decoration ready to paint: a rule along a gap's centerline.
#[derive(Debug)]
pub(crate) struct GapRuleSegment {
    /// The cross-axis center of the rule, in the container's border box space.
    pub center: f32,
    /// Start and end of the rule along the gap, in the container's border box space.
    pub start: f32,
    pub end: f32,
    /// Whether the rule is vertical, i.e. a rule in a column gap.
    pub vertical: bool,
    pub width: f32,
    pub style: BorderStyle,
    pub color: AbsoluteColor,
}

/// Expands a gap decoration value list into one value per gap.
///
/// <https://drafts.csswg.org/css-gaps-1/#gap-decoration-value-assignment>
pub(crate) fn expand_values<T: Clone>(
    list: &GenericGapRuleList<T, PositiveInteger>,
    gap_count: usize,
) -> Vec<T> {
    let mut leading = Vec::new();
    let mut auto_values: Option<Vec<T>> = None;
    let mut trailing = Vec::new();
    for item in list.items() {
        let target = if auto_values.is_some() {
            &mut trailing
        } else {
            &mut leading
        };
        match item {
            GenericGapRuleListItem::Value(value) => target.push(value.clone()),
            GenericGapRuleListItem::Repeat(count, values) => {
                let count = count.0;
                for _ in 0..count.max(1) {
                    target.extend(values.iter().cloned());
                }
            },
            GenericGapRuleListItem::AutoRepeat(values) => {
                auto_values = Some(values.iter().cloned().collect());
            },
        }
    }

    let cycle = |values: &[T], count: usize| -> Vec<T> {
        if values.is_empty() {
            return Vec::new();
        }
        (0..count).map(|i| values[i % values.len()].clone()).collect()
    };

    let Some(auto_values) = auto_values else {
        return cycle(&leading, gap_count);
    };

    let leading_count = leading.len().min(gap_count);
    let trailing_count = trailing.len().min(gap_count - leading_count);
    let auto_count = gap_count - leading_count - trailing_count;
    let mut result: Vec<T> = leading[..leading_count].to_vec();
    result.extend(cycle(&auto_values, auto_count));
    result.extend(trailing[..trailing_count].iter().cloned());
    result
}

/// Per-axis gap decoration properties, resolved for each gap of that axis.
pub(crate) struct AxisRules {
    pub widths: Vec<f32>,
    pub styles: Vec<BorderStyle>,
    pub colors: Vec<AbsoluteColor>,
    pub rule_break: GapRuleBreak,
    pub visibility: GapRuleVisibilityItems,
    pub inset_cap_start: GapRuleInset,
    pub inset_cap_end: GapRuleInset,
    pub inset_junction_start: GapRuleInset,
    pub inset_junction_end: GapRuleInset,
}

impl AxisRules {
    pub(crate) fn new(style: &ComputedValues, columns: bool, gap_count: usize) -> Self {
        let column = style.get_column();
        let (widths, styles, colors) = if columns {
            (
                &column.column_rule_width,
                &column.column_rule_style,
                &column.column_rule_color,
            )
        } else {
            (
                &column.row_rule_width,
                &column.row_rule_style,
                &column.row_rule_color,
            )
        };
        let widths = expand_values(widths, gap_count)
            .iter()
            .map(|width| width.0.to_f32_px())
            .collect();
        // Like collapsed table borders, rules paint `inset` as `ridge` and `outset` as
        // `groove`.
        let styles = expand_values(styles, gap_count)
            .into_iter()
            .map(|style| match style {
                BorderStyle::Inset => BorderStyle::Ridge,
                BorderStyle::Outset => BorderStyle::Groove,
                style => style,
            })
            .collect();
        let colors = expand_values(colors, gap_count)
            .iter()
            .map(|color| style.resolve_color(color))
            .collect();
        if columns {
            Self {
                widths,
                styles,
                colors,
                rule_break: column.column_rule_break,
                visibility: column.column_rule_visibility_items,
                inset_cap_start: column.column_rule_inset_cap_start.clone(),
                inset_cap_end: column.column_rule_inset_cap_end.clone(),
                inset_junction_start: column.column_rule_inset_junction_start.clone(),
                inset_junction_end: column.column_rule_inset_junction_end.clone(),
            }
        } else {
            Self {
                widths,
                styles,
                colors,
                rule_break: column.row_rule_break,
                visibility: column.row_rule_visibility_items,
                inset_cap_start: column.row_rule_inset_cap_start.clone(),
                inset_cap_end: column.row_rule_inset_cap_end.clone(),
                inset_junction_start: column.row_rule_inset_junction_start.clone(),
                inset_junction_end: column.row_rule_inset_junction_end.clone(),
            }
        }
    }

    fn paints(&self, gap: usize) -> bool {
        !matches!(self.styles[gap], BorderStyle::None | BorderStyle::Hidden) &&
            self.widths[gap] > 0.
    }
}

/// Resolves an inset at a segment endpoint. `crossing_gap` is the size of the
/// junction, or `None` at a cap. `crossing_rule_width` is the width of the
/// rule in the crossing gap.
fn resolve_inset(inset: &GapRuleInset, crossing_gap: Option<f32>, crossing_rule_width: f32) -> f32 {
    match inset {
        GapRuleInset::LengthPercentage(length_percentage) => length_percentage
            .resolve(Length::new(crossing_gap.unwrap_or(0.)))
            .px(),
        GapRuleInset::OverlapJoin => match crossing_gap {
            Some(gap) => -(gap / 2. + crossing_rule_width / 2.),
            None => 0.,
        },
    }
}

/// A grid-like layout of tracks used to compute gap decorations: grid
/// containers, where both axes have tracks and items span cells.
pub(crate) struct GridGapLayout {
    /// Start and end of each column track, in border box space.
    pub columns: Vec<(f32, f32)>,
    /// Start and end of each row track, in border box space.
    pub rows: Vec<(f32, f32)>,
    /// The cells each in-flow item covers: row and column index ranges.
    pub items: Vec<(Range<usize>, Range<usize>)>,
}

impl GridGapLayout {
    /// Whether an item covers the cell at `row` and `column`.
    fn occupied(&self, row: usize, column: usize) -> bool {
        self.items
            .iter()
            .any(|(rows, columns)| rows.contains(&row) && columns.contains(&column))
    }

    /// Computes the gap decorations for the gaps between `tracks`, which cross the
    /// tracks in `crossing`. When `columns` is true, the gaps are column gaps.
    fn axis_segments(
        &self,
        columns: bool,
        rules: &AxisRules,
        crossing_rules: &AxisRules,
        painted_matrix: &[Vec<bool>],
        crossing_painted_matrix: &[Vec<bool>],
        out: &mut Vec<GapRuleSegment>,
    ) {
        let (tracks, crossing) = if columns {
            (&self.columns, &self.rows)
        } else {
            (&self.rows, &self.columns)
        };
        if crossing.is_empty() {
            return;
        }
        // Whether an item spans main tracks `t` and `t + 1` in crossing tracks `range`.
        let crosses_gap = |t: usize, range: Range<usize>| {
            self.items.iter().any(|(rows, cols)| {
                let (main, cross) = if columns { (cols, rows) } else { (rows, cols) };
                main.start <= t && main.end > t + 1 && cross.start < range.end && range.start < cross.end
            })
        };
        // Whether the item covering crossing track `c` and main track `t` also covers `c + 1`.
        let spans_crossing_gap = |c: usize, t: usize| {
            self.items.iter().any(|(rows, cols)| {
                let (main, cross) = if columns { (cols, rows) } else { (rows, cols) };
                main.contains(&t) && cross.start <= c && cross.end > c + 1
            })
        };

        for gap in 0..tracks.len().saturating_sub(1) {
            if !rules.paints(gap) {
                continue;
            }
            let center = (tracks[gap].1 + tracks[gap + 1].0) / 2.;
            let segment_count = crossing.len();
            // Gap segments: one per crossing track.
            let segment_start = |c: usize| crossing[c].0;
            let segment_end = |c: usize| crossing[c].1;
            let painted = |c: usize| painted_matrix[gap][c];
            // Whether another painted gap segment touches the junction between crossing
            // tracks `junction` and `junction + 1`, apart from segment `own`.
            let is_junction = |junction: usize, own: usize| {
                let other = if own == junction { junction + 1 } else { junction };
                painted_matrix[gap].get(other).copied().unwrap_or(false) ||
                    crossing_painted_matrix
                        .get(junction)
                        .is_some_and(|row| row[gap] || row[gap + 1])
            };

            // Determine gap decoration segments as ranges of gap segments.
            let mut decoration_segments: Vec<Range<usize>> = Vec::new();
            if rules.rule_break == GapRuleBreak::None {
                decoration_segments.push(0..segment_count);
            } else {
                let mut next = 0;
                while next < segment_count {
                    let first = next;
                    next += 1;
                    if crosses_gap(gap, first..first + 1) {
                        continue;
                    }
                    let mut last = first;
                    while next < segment_count {
                        if crosses_gap(gap, first..next + 1) {
                            break;
                        }
                        if rules.rule_break == GapRuleBreak::Intersection &&
                            !(spans_crossing_gap(last, gap) && spans_crossing_gap(last, gap + 1))
                        {
                            break;
                        }
                        last = next;
                        next += 1;
                    }
                    decoration_segments.push(first..last + 1);
                }
            }

            for segments in decoration_segments {
                // Paint each run of consecutive painted gap segments.
                let mut c = segments.start;
                while c < segments.end {
                    if !painted(c) {
                        c += 1;
                        continue;
                    }
                    let run_start = c;
                    while c < segments.end && painted(c) {
                        c += 1;
                    }
                    let run_end = c;

                    // A start endpoint at the first crossing track is a cap; otherwise it
                    // sits in the junction with the crossing gap before it.
                    let start_junction = (run_start > 0 && is_junction(run_start - 1, run_start))
                        .then(|| {
                        let junction = run_start - 1;
                        (
                            crossing[run_start].0 - crossing[junction].1,
                            crossing_rules.widths.get(junction).copied().unwrap_or(0.),
                        )
                    });
                    let end_junction = (run_end < segment_count &&
                        is_junction(run_end - 1, run_end - 1))
                    .then(|| {
                        let junction = run_end - 1;
                        (
                            crossing[run_end].0 - crossing[junction].1,
                            crossing_rules.widths.get(junction).copied().unwrap_or(0.),
                        )
                    });
                    let start_inset = match start_junction {
                        Some((size, width)) => {
                            resolve_inset(&rules.inset_junction_start, Some(size), width)
                        },
                        None => resolve_inset(&rules.inset_cap_start, None, 0.),
                    };
                    let end_inset = match end_junction {
                        Some((size, width)) => {
                            resolve_inset(&rules.inset_junction_end, Some(size), width)
                        },
                        None => resolve_inset(&rules.inset_cap_end, None, 0.),
                    };
                    let start = segment_start(run_start) + start_inset;
                    let end = segment_end(run_end - 1) - end_inset;
                    if end > start {
                        out.push(GapRuleSegment {
                            center,
                            start,
                            end,
                            vertical: columns,
                            width: rules.widths[gap],
                            style: rules.styles[gap],
                            color: rules.colors[gap],
                        });
                    }
                }
            }
        }
    }

    /// For each gap of an axis, whether each of its gap segments (one per crossing
    /// track) is painted.
    ///
    /// <https://drafts.csswg.org/css-gaps-1/#visibility>
    fn painted_matrix(&self, columns: bool, rules: &AxisRules) -> Vec<Vec<bool>> {
        let (tracks, crossing) = if columns {
            (&self.columns, &self.rows)
        } else {
            (&self.rows, &self.columns)
        };
        let occupied = |c: usize, t: usize| {
            if columns {
                self.occupied(c, t)
            } else {
                self.occupied(t, c)
            }
        };
        (0..tracks.len().saturating_sub(1))
            .map(|gap| {
                (0..crossing.len())
                    .map(|c| {
                        // Unless decorations ignore items (`rule-break: none`), a gap segment
                        // covered by an item spanning the gap is not painted.
                        let covered = rules.rule_break != GapRuleBreak::None &&
                            self.items.iter().any(|(rows, cols)| {
                            let (main, cross) = if columns { (cols, rows) } else { (rows, cols) };
                            main.start <= gap && main.end > gap + 1 && cross.contains(&c)
                        });
                        rules.paints(gap) &&
                            !covered &&
                            match rules.visibility {
                                GapRuleVisibilityItems::All | GapRuleVisibilityItems::Normal => {
                                    true
                                },
                                GapRuleVisibilityItems::Around => {
                                    occupied(c, gap) || occupied(c, gap + 1)
                                },
                                GapRuleVisibilityItems::Between => {
                                    occupied(c, gap) && occupied(c, gap + 1)
                                },
                            }
                    })
                    .collect()
            })
            .collect()
    }

    /// Computes all gap decorations of the grid, in paint order.
    pub(crate) fn segments(&self, style: &ComputedValues) -> Vec<GapRuleSegment> {
        let column_gaps = self.columns.len().saturating_sub(1);
        let row_gaps = self.rows.len().saturating_sub(1);
        let column_rules = AxisRules::new(style, true, column_gaps);
        let row_rules = AxisRules::new(style, false, row_gaps);
        let column_painted = self.painted_matrix(true, &column_rules);
        let row_painted = self.painted_matrix(false, &row_rules);
        let mut columns = Vec::new();
        let mut rows = Vec::new();
        self.axis_segments(
            true,
            &column_rules,
            &row_rules,
            &column_painted,
            &row_painted,
            &mut columns,
        );
        self.axis_segments(
            false,
            &row_rules,
            &column_rules,
            &row_painted,
            &column_painted,
            &mut rows,
        );
        match style.get_column().rule_overlap {
            RuleOverlap::RowOverColumn => {
                columns.extend(rows);
                columns
            },
            RuleOverlap::ColumnOverRow => {
                rows.extend(columns);
                rows
            },
        }
    }
}
