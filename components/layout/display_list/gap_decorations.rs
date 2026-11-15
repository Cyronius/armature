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

/// Whether a container's style can paint any gap decoration.
pub(crate) fn style_has_gap_rules(style: &ComputedValues) -> bool {
    let column = style.get_column();
    let paints = |list: &GenericGapRuleList<BorderStyle, PositiveInteger>| {
        list.items().iter().any(|item| match item {
            GenericGapRuleListItem::Value(style) => !style.none_or_hidden(),
            GenericGapRuleListItem::Repeat(_, styles) |
            GenericGapRuleListItem::AutoRepeat(styles) => {
                styles.iter().any(|style| !style.none_or_hidden())
            },
        })
    };
    paints(&column.column_rule_style) || paints(&column.row_rule_style)
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

/// Computes the gap decorations of a flex container, in paint order, relative to its
/// content box. Each flex line has its own main-axis gaps, between its items; the
/// cross-axis gaps lie between lines.
pub(crate) fn flex_segments(
    style: &ComputedValues,
    info: &crate::flexbox::FlexGapInfo,
) -> Vec<GapRuleSegment> {
    let px = |au: app_units::Au| au.to_f32_px();
    let horizontal = info.main_axis_is_horizontal;
    let lines = &info.lines;

    // The physical start and end of the gap between two ranges in placement order.
    let between = |a: (f32, f32), b: (f32, f32)| {
        if a.1 <= b.0 {
            (a.1, b.0)
        } else {
            (b.1, a.0)
        }
    };

    // Main-axis gaps, numbered across lines.
    let mut main_gaps = Vec::new();
    for (line_index, line) in lines.iter().enumerate() {
        for pair in line.items.windows(2) {
            let gap = between((px(pair[0].0), px(pair[0].1)), (px(pair[1].0), px(pair[1].1)));
            main_gaps.push((line_index, gap));
        }
    }
    let cross_gaps: Vec<(f32, f32)> = lines
        .windows(2)
        .map(|pair| {
            between(
                (px(pair[0].cross.0), px(pair[0].cross.1)),
                (px(pair[1].cross.0), px(pair[1].cross.1)),
            )
        })
        .collect();

    // In a row flex container, main-axis gaps are column gaps.
    let main_rules = AxisRules::new(style, horizontal, main_gaps.len());
    let cross_rules = AxisRules::new(style, !horizontal, cross_gaps.len());

    let mut main_segments = Vec::new();
    for (index, (line_index, gap)) in main_gaps.iter().enumerate() {
        if !main_rules.paints(index) {
            continue;
        }
        let line = &lines[*line_index];
        let (cross_start, cross_end) = (px(line.cross.0), px(line.cross.1));
        // The line's edges abut the cross-axis gaps before and after it, if any.
        let junction = |cross_gap: Option<usize>| {
            cross_gap
                .filter(|gap| cross_rules.paints(*gap))
                .map(|gap| (cross_gaps[gap].1 - cross_gaps[gap].0, cross_rules.widths[gap]))
        };
        let before = junction(line_index.checked_sub(1));
        let after = junction((*line_index + 1 < lines.len()).then_some(*line_index));
        // Lines may be placed in reverse; find which junction is at the physical start.
        let reversed = lines.len() > 1 && lines[0].cross.0 > lines[lines.len() - 1].cross.0;
        let (start_junction, end_junction) = if reversed {
            (after, before)
        } else {
            (before, after)
        };
        let start_inset = match start_junction {
            Some((size, width)) => resolve_inset(&main_rules.inset_junction_start, Some(size), width),
            None => resolve_inset(&main_rules.inset_cap_start, None, 0.),
        };
        let end_inset = match end_junction {
            Some((size, width)) => resolve_inset(&main_rules.inset_junction_end, Some(size), width),
            None => resolve_inset(&main_rules.inset_cap_end, None, 0.),
        };
        let (start, end) = (cross_start + start_inset, cross_end - end_inset);
        if end > start {
            main_segments.push(GapRuleSegment {
                center: (gap.0 + gap.1) / 2.,
                start,
                end,
                vertical: horizontal,
                width: main_rules.widths[index],
                style: main_rules.styles[index],
                color: main_rules.colors[index],
            });
        }
    }

    let mut cross_segments = Vec::new();
    let main_size = px(info.main_size);
    for (index, gap) in cross_gaps.iter().enumerate() {
        if !cross_rules.paints(index) {
            continue;
        }
        // Junctions are where main-axis gaps of the lines on either side abut this gap.
        let mut junctions: Vec<(f32, f32, f32)> = main_gaps
            .iter()
            .enumerate()
            .filter(|(main_index, (line_index, _))| {
                (*line_index == index || *line_index == index + 1) && main_rules.paints(*main_index)
            })
            .map(|(main_index, (_, main_gap))| (main_gap.0, main_gap.1, main_rules.widths[main_index]))
            .collect();
        junctions.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut merged: Vec<(f32, f32, f32)> = Vec::new();
        for junction in junctions {
            match merged.last_mut() {
                Some(last) if junction.0 <= last.1 => {
                    last.1 = last.1.max(junction.1);
                    last.2 = last.2.max(junction.2);
                },
                _ => merged.push(junction),
            }
        }

        // With `intersection`, segments break at every junction; otherwise the rule spans
        // the whole gap.
        let mut pieces: Vec<(f32, Option<(f32, f32)>, f32, Option<(f32, f32)>)> = Vec::new();
        if cross_rules.rule_break == GapRuleBreak::Intersection {
            let mut start = 0.;
            let mut start_junction = None;
            for (junction_start, junction_end, width) in &merged {
                let size = junction_end - junction_start;
                pieces.push((start, start_junction, *junction_start, Some((size, *width))));
                start = *junction_end;
                start_junction = Some((size, *width));
            }
            pieces.push((start, start_junction, main_size, None));
        } else {
            pieces.push((0., None, main_size, None));
        }
        for (start, start_junction, end, end_junction) in pieces {
            let start_inset = match start_junction {
                Some((size, width)) => {
                    resolve_inset(&cross_rules.inset_junction_start, Some(size), width)
                },
                None => resolve_inset(&cross_rules.inset_cap_start, None, 0.),
            };
            let end_inset = match end_junction {
                Some((size, width)) => resolve_inset(&cross_rules.inset_junction_end, Some(size), width),
                None => resolve_inset(&cross_rules.inset_cap_end, None, 0.),
            };
            let (start, end) = (start + start_inset, end - end_inset);
            if end > start {
                cross_segments.push(GapRuleSegment {
                    center: (gap.0 + gap.1) / 2.,
                    start,
                    end,
                    vertical: !horizontal,
                    width: cross_rules.widths[index],
                    style: cross_rules.styles[index],
                    color: cross_rules.colors[index],
                });
            }
        }
    }

    // Row rules paint over column rules by default.
    let (mut columns, rows) = if horizontal {
        (main_segments, cross_segments)
    } else {
        (cross_segments, main_segments)
    };
    match style.get_column().rule_overlap {
        RuleOverlap::RowOverColumn => {
            columns.extend(rows);
            columns
        },
        RuleOverlap::ColumnOverRow => {
            let mut rows = rows;
            rows.extend(columns);
            rows
        },
    }
}

/// The geometry of a grid lanes container used to compute gap decorations, in a
/// coordinate space where `u` runs across the grid tracks and `v` along the stacking
/// axis.
pub(crate) struct GridLanesGapLayout {
    /// Start and end of each grid track along `u`, in placement order.
    pub tracks: Vec<(f32, f32)>,
    /// The margin box of each item: `u` start and end, then `v` start and end.
    pub items: Vec<(f32, f32, f32, f32)>,
    /// The start and end of the content box along `v`.
    pub stacking_extent: (f32, f32),
    /// The used gap between items along the stacking axis.
    pub stacking_gap: f32,
    /// Whether the grid tracks are rows (so that `u` is vertical).
    pub tracks_are_rows: bool,
}

impl GridLanesGapLayout {
    /// The tracks an item occupies.
    fn item_tracks(&self, item: &(f32, f32, f32, f32)) -> Range<usize> {
        let mut first = None;
        let mut last = 0;
        for (index, track) in self.tracks.iter().enumerate() {
            if item.0 < track.1 - 0.5 && track.0 + 0.5 < item.1 {
                first.get_or_insert(index);
                last = index;
            }
        }
        match first {
            Some(first) => first..last + 1,
            None => 0..0,
        }
    }

    /// The ranges along the stacking axis covered by the items in a track, with the
    /// stacking gaps between consecutive items included.
    fn occupied_ranges(&self, item_tracks: &[Range<usize>], track: usize) -> Vec<(f32, f32)> {
        let ranges = self
            .items
            .iter()
            .zip(item_tracks)
            .filter(|(_, tracks)| tracks.contains(&track))
            .map(|(item, _)| (item.2, item.3 + self.stacking_gap + 0.01))
            .collect();
        union_ranges(ranges)
            .into_iter()
            .map(|(start, end)| (start, end - self.stacking_gap - 0.01))
            .collect()
    }

    /// Computes all gap decorations of the container, in paint order. Segment
    /// coordinates are in the container's border box space.
    pub(crate) fn segments(&self, style: &ComputedValues) -> Vec<GapRuleSegment> {
        let grid_gaps = self.tracks.len().saturating_sub(1);
        // Rules in the gaps between tracks: column rules when the tracks are columns.
        let grid_rules = AxisRules::new(style, !self.tracks_are_rows, grid_gaps);
        let item_tracks: Vec<Range<usize>> =
            self.items.iter().map(|item| self.item_tracks(item)).collect();

        let mut grid_segments = Vec::new();
        for gap in 0..grid_gaps {
            if !grid_rules.paints(gap) {
                continue;
            }
            let center = (self.tracks[gap].1 + self.tracks[gap + 1].0) / 2.;
            // Items spanning the gap interrupt the rule, keeping the stacking gap clear.
            let mut blocked: Vec<(f32, f32)> = self
                .items
                .iter()
                .zip(&item_tracks)
                .filter(|(_, tracks)| tracks.start <= gap && tracks.end > gap + 1)
                .map(|(item, _)| (item.2 - self.stacking_gap, item.3 + self.stacking_gap))
                .collect();
            blocked.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut pieces = Vec::new();
            let mut cursor = self.stacking_extent.0;
            if grid_rules.rule_break == GapRuleBreak::None {
                pieces.push((cursor, self.stacking_extent.1));
            } else {
                for (start, end) in blocked {
                    if start > cursor {
                        pieces.push((cursor, start));
                    }
                    cursor = cursor.max(end);
                }
                if cursor < self.stacking_extent.1 {
                    pieces.push((cursor, self.stacking_extent.1));
                }
            }
            // <https://drafts.csswg.org/css-gaps-1/#visibility>
            // Paint only where items sit on one (`around`) or both (`between`) sides. The
            // stacking gaps between consecutive items do not count as empty.
            let pieces = match grid_rules.visibility {
                GapRuleVisibilityItems::All | GapRuleVisibilityItems::Normal => pieces,
                visibility => {
                    let before = self.occupied_ranges(&item_tracks, gap);
                    let after = self.occupied_ranges(&item_tracks, gap + 1);
                    let visible = if visibility == GapRuleVisibilityItems::Around {
                        union_ranges(before.into_iter().chain(after).collect())
                    } else {
                        intersect_ranges(&before, &after)
                    };
                    intersect_ranges(&pieces, &visible)
                },
            };
            for (start, end) in pieces {
                let start = start + resolve_inset(&grid_rules.inset_cap_start, None, 0.);
                let end = end - resolve_inset(&grid_rules.inset_cap_end, None, 0.);
                if end > start {
                    grid_segments.push(GapRuleSegment {
                        center,
                        start,
                        end,
                        vertical: !self.tracks_are_rows,
                        width: grid_rules.widths[gap],
                        style: grid_rules.styles[gap],
                        color: grid_rules.colors[gap],
                    });
                }
            }
        }

        // Rules between consecutive items in each track. The value list restarts in each
        // track.
        let mut stacking_segments = Vec::new();
        for track in 0..self.tracks.len() {
            let mut items: Vec<(f32, f32)> = self
                .items
                .iter()
                .zip(&item_tracks)
                .filter(|(_, tracks)| tracks.contains(&track))
                .map(|(item, _)| (item.2, item.3))
                .collect();
            items.sort_by(|a, b| a.0.total_cmp(&b.0));
            if items.len() < 2 {
                continue;
            }
            let rules = AxisRules::new(style, self.tracks_are_rows, items.len() - 1);
            // The track's edges abut the gaps between tracks, if any.
            let junction = |gap: Option<usize>| {
                gap.filter(|gap| grid_rules.paints(*gap)).map(|gap| {
                    (
                        self.tracks[gap + 1].0 - self.tracks[gap].1,
                        grid_rules.widths[gap],
                    )
                })
            };
            let start_junction = junction(track.checked_sub(1));
            let end_junction = junction((track + 1 < self.tracks.len()).then_some(track));
            for (index, pair) in items.windows(2).enumerate() {
                if !rules.paints(index) {
                    continue;
                }
                let start_inset = match start_junction {
                    Some((size, width)) => {
                        resolve_inset(&rules.inset_junction_start, Some(size), width)
                    },
                    None => resolve_inset(&rules.inset_cap_start, None, 0.),
                };
                let end_inset = match end_junction {
                    Some((size, width)) => resolve_inset(&rules.inset_junction_end, Some(size), width),
                    None => resolve_inset(&rules.inset_cap_end, None, 0.),
                };
                let start = self.tracks[track].0 + start_inset;
                let end = self.tracks[track].1 - end_inset;
                if end > start {
                    stacking_segments.push(GapRuleSegment {
                        center: (pair[0].1 + pair[1].0) / 2.,
                        start,
                        end,
                        vertical: self.tracks_are_rows,
                        width: rules.widths[index],
                        style: rules.styles[index],
                        color: rules.colors[index],
                    });
                }
            }
        }

        let (mut columns, rows) = if self.tracks_are_rows {
            (stacking_segments, grid_segments)
        } else {
            (grid_segments, stacking_segments)
        };
        match style.get_column().rule_overlap {
            RuleOverlap::RowOverColumn => {
                columns.extend(rows);
                columns
            },
            RuleOverlap::ColumnOverRow => {
                let mut rows = rows;
                rows.extend(columns);
                rows
            },
        }
    }
}

/// Merges overlapping ranges, returning them sorted.
fn union_ranges(mut ranges: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
    ranges.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut merged: Vec<(f32, f32)> = Vec::new();
    for range in ranges {
        match merged.last_mut() {
            Some(last) if range.0 <= last.1 => last.1 = last.1.max(range.1),
            _ => merged.push(range),
        }
    }
    merged
}

/// Intersects two sorted lists of disjoint ranges.
fn intersect_ranges(a: &[(f32, f32)], b: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let mut result = Vec::new();
    for x in a {
        for y in b {
            let start = x.0.max(y.0);
            let end = x.1.min(y.1);
            if end > start {
                result.push((start, end));
            }
        }
    }
    result.sort_by(|p, q| p.0.total_cmp(&q.0));
    result
}
