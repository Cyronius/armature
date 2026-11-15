/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Line clamping across the block formatting context of a line-clamp container.
//!
//! <https://drafts.csswg.org/css-overflow-4/#line-clamp>
//!
//! The lines of an inline formatting context that is a direct child of the container
//! are clamped during inline layout. This handles lines in descendant blocks that
//! participate in the same block formatting context, by clamping the laid out fragments.

use app_units::Au;
use style::Zero;
use style::values::generics::length::GenericSize;
use style::computed_values::position::T as Position;
use style::properties::ComputedValues;
use style::values::generics::box_::BlockEllipsis;
use style::values::specified::box_::DisplayInside as StyloDisplayInside;

use crate::context::LayoutContext;
use crate::flow::inline::place_block_ellipsis;
use crate::flow::inline::text_run::get_font_for_first_font_for_style;
use crate::fragment_tree::{BaseFragmentInfo, BoxFragment, Fragment, PositioningFragment};
use crate::style_ext::{ComputedValuesExt, LayoutStyle};
use crate::ContainingBlock;

/// A line box found in the block formatting context, by its path of child indices.
struct LineBox {
    path: Vec<usize>,
    block_end: Au,
}

/// Whether the block-level `fragment` participates in the same block formatting context
/// as its parent, so that its lines are counted by the line-clamp container.
fn is_same_formatting_context_block(box_fragment: &BoxFragment) -> bool {
    let style = box_fragment.style();
    style.get_box().display.inside() == StyloDisplayInside::Flow &&
        !matches!(
            style.get_box().position,
            Position::Absolute | Position::Fixed
        ) &&
        !box_fragment.is_replaced() &&
        !style.establishes_block_formatting_context(box_fragment.base.flags)
}

fn collect_lines(
    fragments: &[Fragment],
    offset: Au,
    path: &mut Vec<usize>,
    lines: &mut Vec<LineBox>,
) {
    for (index, fragment) in fragments.iter().enumerate() {
        path.push(index);
        match fragment {
            Fragment::Positioning(positioning) => {
                let rect = positioning.base.rect();
                if positioning.is_line_box() && !contains_block_level(&positioning.children) {
                    // Empty line boxes, such as phantom lines, aren't counted.
                    if rect.size.height.is_zero() {
                        path.pop();
                        continue;
                    }
                    lines.push(LineBox {
                        path: path.clone(),
                        block_end: offset + rect.max_y(),
                    });
                } else {
                    collect_lines(&positioning.children, offset + rect.origin.y, path, lines);
                }
            },
            Fragment::Box(box_fragment)
                if is_same_formatting_context_block(box_fragment) ||
                    is_inline_box(box_fragment) =>
            {
                let rect = box_fragment.base.rect();
                collect_lines(&box_fragment.children, offset + rect.origin.y, path, lines);
            },
            _ => {},
        }
        path.pop();
    }
}

fn is_inline_box(box_fragment: &BoxFragment) -> bool {
    box_fragment
        .style()
        .is_inline_box(box_fragment.base.flags)
}

/// Whether the inline content `fragments` contains a block-level box, which then splits
/// the line around it.
fn contains_block_level(fragments: &[Fragment]) -> bool {
    fragments.iter().any(|fragment| match fragment {
        Fragment::Box(box_fragment) if is_inline_box(box_fragment) => {
            contains_block_level(&box_fragment.children)
        },
        Fragment::Box(box_fragment) => is_same_formatting_context_block(box_fragment),
        _ => false,
    })
}

/// The block end of the margin boxes of the floats in the block formatting context
/// `fragments`.
pub(crate) fn floats_block_end(fragments: &[Fragment], offset: Au) -> Au {
    let mut end = Au::zero();
    for fragment in fragments {
        match fragment {
            Fragment::Float(float) => end = end.max(offset + float.margin_rect().max_y()),
            Fragment::Positioning(positioning) => {
                let rect = positioning.base.rect();
                end = end.max(floats_block_end(&positioning.children, offset + rect.origin.y));
            },
            Fragment::Box(box_fragment)
                if is_same_formatting_context_block(box_fragment) ||
                    is_inline_box(box_fragment) =>
            {
                let rect = box_fragment.base.rect();
                end = end.max(floats_block_end(&box_fragment.children, offset + rect.origin.y));
            },
            _ => {},
        }
    }
    end
}

/// Collect the placeholders of hoisted out-of-flow boxes within `fragments`, which are
/// still painted after the content that contains them is clamped, unless their
/// containing block is clamped as well.
pub(super) fn collect_hoisted_placeholders(fragments: &[Fragment], result: &mut Vec<Fragment>) {
    for fragment in fragments {
        match fragment {
            Fragment::AbsoluteOrFixedPositionedPlaceholder(_) => result.push(fragment.clone()),
            Fragment::Positioning(positioning) => {
                collect_hoisted_placeholders(&positioning.children, result)
            },
            Fragment::Box(box_fragment)
                if !box_fragment
                    .style()
                    .establishes_containing_block_for_absolute_descendants(
                        box_fragment.base.flags,
                    ) =>
            {
                collect_hoisted_placeholders(&box_fragment.children, result)
            },
            _ => {},
        }
    }
}

/// Remove the content of `fragments` after the line at `path`, returning the new block
/// end of the content, without and then with the block-end margin of the last box,
/// which may collapse with the margins of its ancestors.
fn truncate_after(
    fragments: &mut Vec<Fragment>,
    path: &[usize],
    ellipsis: Option<&str>,
    layout_context: &LayoutContext,
) -> (Au, Au) {
    let index = path[0];
    let mut hoisted = Vec::new();
    collect_hoisted_placeholders(&fragments[index + 1..], &mut hoisted);
    fragments.truncate(index + 1);

    let block_end = match &fragments[index] {
        Fragment::Positioning(positioning) if path.len() == 1 => {
            let block_end = positioning.base.rect().max_y();
            if let Some(ellipsis) = ellipsis {
                let style = positioning.style.borrow().clone();
                if let Some(font) =
                    get_font_for_first_font_for_style(&style, &layout_context.font_context) &&
                    let Some(line) =
                        place_block_ellipsis(positioning, ellipsis, &font, layout_context)
                {
                    fragments[index] = line;
                }
            }
            (block_end, Au::zero())
        },
        Fragment::Positioning(positioning) => {
            let mut children = positioning.children.clone();
            let (child_end, child_margin) =
                truncate_after(&mut children, &path[1..], ellipsis, layout_context);
            let mut rect = positioning.base.rect();
            let is_line_box = positioning.is_line_box();
            if !is_line_box {
                rect.size.height = child_end;
            }
            let style = positioning.style.borrow().clone();
            fragments[index] = Fragment::Positioning(PositioningFragment::new_anonymous(
                style,
                rect,
                children,
                is_line_box,
            ));
            (rect.origin.y + child_end.max(rect.size.height), child_margin)
        },
        Fragment::Box(box_fragment) => {
            let mut children = box_fragment.children.clone();
            let (child_end, child_margin) =
                truncate_after(&mut children, &path[1..], ellipsis, layout_context);
            let style = box_fragment.style().clone();
            let mut rect = box_fragment.base.rect();
            let inline_box = is_inline_box(box_fragment);
            // Without block-end padding and border, the block-end margin of the last
            // child collapses through this box.
            let margins_collapse = !inline_box &&
                (box_fragment.padding.bottom + box_fragment.border.bottom).is_zero();
            let mut margin = box_fragment.margin;
            if margins_collapse {
                margin.bottom = margin.bottom.max(child_margin);
            }
            if !inline_box && style.get_position().height.is_auto() {
                rect.size.height = if margins_collapse {
                    child_end
                } else {
                    child_end + child_margin
                };
                if let GenericSize::LengthPercentage(min) = &style.get_position().min_height &&
                    let Some(min) = min.0.to_length()
                {
                    rect.size.height = rect.size.height.max(Au::from(min));
                }
            }
            let mut new_box = BoxFragment::new(
                BaseFragmentInfo {
                    tag: box_fragment.base.tag,
                    flags: box_fragment.base.flags,
                },
                style,
                children,
                rect,
                box_fragment.padding,
                box_fragment.border,
                box_fragment.margin,
                None,
            )
            .with_baselines(box_fragment.baselines(box_fragment.style().writing_mode));
            if let Some(info) = &box_fragment.block_level_layout_info {
                new_box = new_box.with_block_level_layout_info(
                    info.block_margins_collapsed_with_children.clone(),
                    info.clearance,
                );
            }
            let block_end = if inline_box {
                (rect.origin.y + child_end, child_margin)
            } else {
                (new_box.border_rect().max_y(), margin.bottom)
            };
            fragments[index] = Fragment::Box(new_box.into());
            block_end
        },
        _ => unreachable!("Line paths only go through boxes and positioning fragments"),
    };
    fragments.extend(hoisted);
    block_end
}

/// Clamp the lines of the block formatting context of the line-clamp container with
/// `style`, after the lines of its inline formatting contexts are laid out.
pub(crate) fn clamp_block_formatting_context(
    fragments: &mut Vec<Fragment>,
    content_block_size: &mut Au,
    style: &ComputedValues,
    containing_block: &ContainingBlock,
    layout_context: &LayoutContext,
) {
    if !style.line_clamp_applies() ||
        style.get_column().is_multicol() ||
        style.writing_mode.is_vertical() ||
        !style.writing_mode.is_bidi_ltr()
    {
        return;
    }
    let line_clamp = &style.get_box().line_clamp;

    let mut lines = Vec::new();
    collect_lines(fragments, Au::zero(), &mut Vec::new(), &mut lines);

    let mut clamp_after = None;
    if let Some(max_lines) = line_clamp
        .max_lines
        .lines_value()
        .and_then(|lines| usize::try_from(lines.0).ok()) &&
        max_lines > 0 &&
        lines.len() > max_lines
    {
        clamp_after = Some(max_lines - 1);
    }
    if !line_clamp.max_lines.kw.is_none() {
        let block_sizes = LayoutStyle::Default(style)
            .content_box_sizes_and_padding_border_margin(&containing_block.into())
            .content_box_sizes
            .block;
        let max_block_size = block_sizes
            .preferred
            .to_numeric()
            .or_else(|| block_sizes.max.to_numeric())
            .map(|size| {
                size.min(block_sizes.max.to_numeric().unwrap_or(size))
                    .max(block_sizes.min.to_numeric().unwrap_or_default())
            });
        if let Some(max_block_size) = max_block_size &&
            let Some(first_overflowing) = lines
                .iter()
                .position(|line| line.block_end > max_block_size) &&
            first_overflowing > 0
        {
            clamp_after = Some(
                clamp_after.map_or(first_overflowing - 1, |index| index.min(first_overflowing - 1)),
            );
        }
    }
    let Some(clamp_after) = clamp_after else {
        return;
    };

    let ellipsis = match &line_clamp.block_ellipsis {
        BlockEllipsis::Ellipsis => Some("…"),
        BlockEllipsis::String(text) => Some(text.as_ref()),
        BlockEllipsis::NoEllipsis => None,
    }
    .filter(|text| !text.is_empty());
    let (block_end, margin) =
        truncate_after(fragments, &lines[clamp_after].path, ellipsis, layout_context);
    *content_block_size = block_end + margin;
}
