/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `text-overflow`: eliding inline content that overflows the end edge of a line.
//!
//! <https://drafts.csswg.org/css-overflow-3/#text-overflow>

use std::sync::Arc;

use app_units::Au;
use fonts::{FontRef, ShapedTextSlice, ShapedTextSlicer, ShapingOptions, TrailingWhiteSpace};
use servo_arc::Arc as ServoArc;
use servo_base::text::Utf32CodeUnits;
use style::Zero;
use style::computed_values::overflow_x::T as Overflow;
use style::properties::ComputedValues;
use style::values::specified::text::TextOverflowSide;

use crate::context::LayoutContext;
use crate::geom::{PhysicalPoint, PhysicalRect, PhysicalSize};
use crate::style_ext::ComputedValuesExt;
use crate::cell::ArcRefCell;
use crate::flow::inline::SharedInlineStyles;
use crate::flow::inline::text_run::{FontAndScriptInfo, SharedTextRunData};
use crate::fragment_tree::{
    BaseFragment, BaseFragmentInfo, BoxFragment, Fragment, PositioningFragment, TextFragment,
};

/// Shape the string used to mark elided content.
fn shape_marker(font: &FontRef, text: &str) -> Option<Arc<ShapedTextSlice>> {
    let font_info = FontAndScriptInfo::simple_for_font(font.clone());
    let shaped = font.shape_text(text, &ShapingOptions::from(&font_info));
    ShapedTextSlicer::new(shaped).slice_until_character_offset(
        Utf32CodeUnits(u32::try_from(text.chars().count()).unwrap_or(u32::MAX)),
        TrailingWhiteSpace {
            hangable: Utf32CodeUnits(0),
            removable: Utf32CodeUnits(0),
        },
        false,
    )
}

/// The furthest inline end of the content of `fragments`, offset by `offset`.
fn content_end(fragments: &[Fragment], offset: Au) -> Au {
    let mut end = Au::zero();
    for fragment in fragments {
        match fragment {
            Fragment::Text(text) => {
                let rect = text.base.rect();
                end = end.max(offset + rect.origin.x + rect.size.width);
            },
            Fragment::Box(box_fragment) => {
                let rect = box_fragment.base.rect();
                if box_fragment.style().is_inline_box(box_fragment.base.flags) {
                    end = end.max(content_end(&box_fragment.children, offset + rect.origin.x));
                } else {
                    end = end.max(offset + box_fragment.margin_rect().max_x());
                }
            },
            Fragment::Image(image) => {
                let rect = image.base.rect();
                end = end.max(offset + rect.origin.x + rect.size.width);
            },
            Fragment::Svg(svg) => {
                let rect = svg.base.rect();
                end = end.max(offset + rect.origin.x + rect.size.width);
            },
            _ => {},
        }
    }
    end
}

struct Elider {
    /// Where content must end to leave room for the marker.
    available: Au,
    /// The end of the content kept so far.
    end: Au,
    /// Whether the cut has happened; later inline content is hidden.
    done: bool,
    /// Whether any content of the line has been kept.
    kept_any: bool,
    /// The last text fragment seen, whose text data the marker reuses.
    last_text: Option<Arc<TextFragment>>,
}

impl Elider {
    fn elide(&mut self, children: &[Fragment], offset: Au) -> Vec<Fragment> {
        let mut result = Vec::with_capacity(children.len());
        for child in children {
            match child {
                Fragment::Text(text) => {
                    if self.done {
                        continue;
                    }
                    self.last_text = Some(text.clone());
                    let rect = text.base.rect();
                    let start = offset + rect.origin.x;
                    if start + rect.size.width <= self.available {
                        self.end = self.end.max(start + rect.size.width);
                        self.kept_any |= !text.glyphs.is_empty();
                        result.push(child.clone());
                        continue;
                    }
                    self.done = true;
                    let mut remaining = self.available - start;
                    let mut glyphs = Vec::new();
                    let mut width = Au::zero();
                    for slice in &text.glyphs {
                        let advance = slice.total_advance();
                        if advance <= remaining {
                            glyphs.push(slice.clone());
                            remaining -= advance;
                            width += advance;
                        } else {
                            if let Some(truncated) = slice.truncated_to_advance(remaining) {
                                width += truncated.total_advance();
                                glyphs.push(truncated);
                            }
                            break;
                        }
                    }
                    // The first character of a line is never hidden; the marker is
                    // clipped instead.
                    if glyphs.is_empty() && !self.kept_any {
                        if let Some(first) = text.glyphs.first() &&
                            let Some(glyph) = first.glyphs().next() &&
                            let Some(truncated) = first.truncated_to_advance(glyph.advance())
                        {
                            width = truncated.total_advance();
                            glyphs.push(truncated);
                        }
                    }
                    self.end = self.end.max(start + width);
                    if glyphs.is_empty() {
                        continue;
                    }
                    self.kept_any = true;
                    let mut new_rect = rect;
                    new_rect.size.width = width;
                    result.push(Fragment::Text(Arc::new(TextFragment {
                        base: BaseFragment::new(
                            BaseFragmentInfo {
                                tag: text.base.tag,
                                flags: text.base.flags,
                            },
                            new_rect,
                        ),
                        run_data: text.run_data.clone(),
                        font_metrics: text.font_metrics.clone(),
                        font_key: text.font_key,
                        glyphs,
                        justification_adjustment: text.justification_adjustment,
                        character_range_in_dom_node: text.character_range_in_dom_node.clone(),
                        is_empty_for_text_cursor: text.is_empty_for_text_cursor,
                    })));
                },
                Fragment::Box(box_fragment) if box_fragment.style().is_inline_box(box_fragment.base.flags) => {
                    if self.done {
                        continue;
                    }
                    let rect = box_fragment.base.rect();
                    let children = self.elide(&box_fragment.children, offset + rect.origin.x);
                    let mut content_rect = rect;
                    if self.done {
                        content_rect.size.width =
                            (self.end - offset - rect.origin.x).max(Au::zero());
                    }
                    let new_box = BoxFragment::new(
                        BaseFragmentInfo {
                            tag: box_fragment.base.tag,
                            flags: box_fragment.base.flags,
                        },
                        box_fragment.style().clone(),
                        children,
                        content_rect,
                        box_fragment.padding,
                        box_fragment.border,
                        box_fragment.margin,
                        None,
                    );
                    result.push(Fragment::Box(Arc::new(new_box)));
                },
                Fragment::Box(_) | Fragment::Image(_) | Fragment::Svg(_) | Fragment::IFrame(_) => {
                    if self.done {
                        continue;
                    }
                    // Atomic inline-level content is hidden as a whole.
                    let end = match child {
                        Fragment::Box(box_fragment) => offset + box_fragment.margin_rect().max_x(),
                        _ => child.base().map_or(offset, |base| {
                            let rect = base.rect();
                            offset + rect.origin.x + rect.size.width
                        }),
                    };
                    if end <= self.available {
                        self.end = self.end.max(end);
                        self.kept_any = true;
                        result.push(child.clone());
                    } else {
                        self.done = true;
                        // The first item of a line is never hidden.
                        if !self.kept_any {
                            self.end = end;
                            self.kept_any = true;
                            result.push(child.clone());
                        }
                    }
                },
                _ => result.push(child.clone()),
            }
        }
        result
    }
}

/// Elide the inline content of each line of a block container that overflows its end
/// edge, replacing it with the `text-overflow` marker. Only horizontal, left-to-right
/// lines are handled.
pub(super) fn apply_text_overflow(
    fragments: &mut [Fragment],
    lines: impl Iterator<Item = (usize, Au)>,
    style: &ServoArc<ComputedValues>,
    default_font: Option<&FontRef>,
    layout_context: &LayoutContext,
) {
    if style.writing_mode.is_vertical() || !style.writing_mode.is_bidi_ltr() {
        return;
    }
    // Scroll containers would need the marker to follow the scroll position; only
    // clipping containers are handled.
    if !matches!(style.get_box().overflow_x, Overflow::Hidden | Overflow::Clip) {
        return;
    }
    let marker = match &style.get_text().text_overflow.second {
        TextOverflowSide::Clip => return,
        TextOverflowSide::Ellipsis => "…".to_owned(),
        TextOverflowSide::String(text) => text.to_string(),
    };
    let Some(font) = default_font else {
        return;
    };
    let marker_glyphs = if marker.is_empty() {
        None
    } else {
        shape_marker(font, &marker)
    };
    let marker_width = marker_glyphs
        .as_ref()
        .map_or(Au::zero(), |glyphs| glyphs.total_advance());

    for (index, baseline) in lines {
        let Fragment::Positioning(line) = &fragments[index] else {
            continue;
        };
        let rect = line.base.rect();
        if content_end(&line.children, Au::zero()) <= rect.size.width {
            continue;
        }
        let mut elider = Elider {
            available: (rect.size.width - marker_width).max(Au::zero()),
            end: Au::zero(),
            done: false,
            kept_any: false,
            last_text: None,
        };
        let mut children = elider.elide(&line.children, Au::zero());
        if let (Some(glyphs), Some(text)) = (marker_glyphs.clone(), elider.last_text.as_ref()) {
            // The marker sits on the line's baseline, in the container's first font.
            let metrics = font.metrics();
            let mut marker_rect = text.base.rect();
            marker_rect.origin.x = elider.end;
            marker_rect.origin.y = baseline - rect.origin.y - metrics.ascent;
            marker_rect.size.width = glyphs.total_advance();
            marker_rect.size.height = metrics.ascent + metrics.descent;
            let end = text.character_range_in_dom_node.end;
            children.push(Fragment::Text(Arc::new(TextFragment {
                base: BaseFragment::new(BaseFragmentInfo::anonymous(), marker_rect),
                run_data: text.run_data.clone(),
                font_metrics: font.metrics().clone(),
                font_key: font.key(layout_context.painter_id, &layout_context.font_context),
                glyphs: vec![glyphs],
                justification_adjustment: Au::zero(),
                character_range_in_dom_node: end..end,
                is_empty_for_text_cursor: false,
            })));
        }
        fragments[index] = Fragment::Positioning(PositioningFragment::new_anonymous(
            style.clone(),
            rect,
            children,
            true,
        ));
    }
}


/// Place a block ellipsis (`marker`) at the end of the content of `line`, eliding the
/// content that it would otherwise overlap. Returns `None` when `line` has no text
/// whose data the marker can reuse.
pub(crate) fn place_block_ellipsis(
    line: &PositioningFragment,
    marker: &str,
    font: &FontRef,
    layout_context: &LayoutContext,
) -> Option<Fragment> {
    let glyphs = shape_marker(font, marker)?;
    let marker_width = glyphs.total_advance();
    let rect = line.base.rect();

    // Trailing white space that is removed at the end of the line doesn't count.
    let mut end = content_end(&line.children, Au::zero());
    if let Some(Fragment::Text(text)) = line.children.last() {
        let text_rect = text.base.rect();
        if text_rect.origin.x + text_rect.size.width == end {
            end -= text
                .glyphs
                .last()
                .map_or(Au::zero(), |slice| slice.removable_advance());
        }
    }

    let mut elider = Elider {
        available: (rect.size.width - marker_width).max(Au::zero()),
        end: Au::zero(),
        done: false,
        kept_any: false,
        last_text: None,
    };
    let (mut children, marker_start) = if end + marker_width <= rect.size.width {
        let mut last_text = None;
        find_last_text(&line.children, &mut last_text);
        elider.last_text = last_text;
        (line.children.clone(), end)
    } else {
        let children = elider.elide(&line.children, Au::zero());
        let end = elider.end;
        (children, end)
    };
    let text = elider.last_text.as_ref()?;

    let text_rect = text.base.rect();
    let baseline = text_rect.origin.y + text.font_metrics.ascent;
    let metrics = font.metrics();
    let marker_rect = PhysicalRect::new(
        PhysicalPoint::new(marker_start, baseline - metrics.ascent),
        PhysicalSize::new(marker_width, metrics.ascent + metrics.descent),
    );
    let character_end = text.character_range_in_dom_node.end;
    let line_style = line.style.borrow().clone();
    let inline_styles = SharedInlineStyles {
        style: ArcRefCell::new(line_style.clone()),
        selected: ArcRefCell::new(line_style),
    };
    children.push(Fragment::Text(Arc::new(TextFragment {
        base: BaseFragment::new(BaseFragmentInfo::anonymous(), marker_rect),
        run_data: run_data_with_styles(&text.run_data, inline_styles),
        font_metrics: metrics.clone(),
        font_key: font.key(layout_context.painter_id, &layout_context.font_context),
        glyphs: vec![glyphs],
        justification_adjustment: Au::zero(),
        character_range_in_dom_node: character_end..character_end,
        is_empty_for_text_cursor: false,
    })));
    Some(Fragment::Positioning(PositioningFragment::new_anonymous(
        line.style.borrow().clone(),
        rect,
        children,
        true,
    )))
}

/// Find the last text fragment within the inline content `fragments`.
fn find_last_text(fragments: &[Fragment], result: &mut Option<Arc<TextFragment>>) {
    for fragment in fragments {
        match fragment {
            Fragment::Text(text) => *result = Some(text.clone()),
            Fragment::Box(box_fragment)
                if box_fragment.style().is_inline_box(box_fragment.base.flags) =>
            {
                find_last_text(&box_fragment.children, result)
            },
            _ => {},
        }
    }
}

/// A copy of `run_data` whose text is styled by `inline_styles`, for markers that are
/// styled by their block container rather than by the text they follow.
pub(crate) fn run_data_with_styles(
    run_data: &SharedTextRunData,
    inline_styles: SharedInlineStyles,
) -> Arc<SharedTextRunData> {
    Arc::new(SharedTextRunData {
        text_content: run_data.text_content.clone(),
        inline_styles,
        character_range_in_ifc_text: run_data.character_range_in_ifc_text.clone(),
        original_offset: run_data.original_offset,
        selection: Default::default(),
        paints_caret: false,
        needs_caret_placeholder: false,
        offset_map: run_data.offset_map.clone(),
    })
}
