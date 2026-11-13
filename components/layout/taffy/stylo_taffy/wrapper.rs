/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::ops::Deref;

use style::logical_geometry::WritingMode;
use style::properties::ComputedValues;
use style::values::CustomIdent;
use style::values::computed::{BorderSideWidth, GridTemplateAreas, LengthPercentage};
use style::values::generics::grid::{TrackListValue, TrackRepeat, TrackSize};
use style::values::specified::BorderStyle;
use style::values::specified::position::NamedArea;
use style::{Atom, OwnedSlice};

use super::{as_clamped_i16, convert, stylo};
use crate::fragment_tree::FragmentFlags;
use crate::style_ext::ComputedValuesExt;

/// A wrapper struct for anything that Deref's to a [`ComputedValues`], which
/// implements Taffy's layout traits and can used with Taffy's layout algorithms.
pub struct TaffyStyloStyle<T: Deref<Target = ComputedValues>> {
    pub style: T,
    pub fragment_flags: FragmentFlags,
    grid_container_direction: Option<taffy::Direction>,
    grid_container_writing_mode: Option<WritingMode>,
    grid_lanes_stacking_axis_is_horizontal: Option<bool>,
}

impl<T: Deref<Target = ComputedValues>> TaffyStyloStyle<T> {
    pub fn new(style: T, fragment_flags: FragmentFlags) -> Self {
        Self {
            style,
            fragment_flags,
            grid_container_direction: None,
            grid_container_writing_mode: None,
            grid_lanes_stacking_axis_is_horizontal: None,
        }
    }

    pub fn with_grid_container_direction(mut self, direction: taffy::Direction) -> Self {
        self.grid_container_direction = Some(direction);
        self
    }

    pub fn with_grid_container_writing_mode(mut self, writing_mode: WritingMode) -> Self {
        self.grid_container_writing_mode = Some(writing_mode);
        self
    }

    fn grid_axes_are_vertical(&self) -> bool {
        // Orthogonal children are measured in their own physical axes by compute_child_layout.
        self.grid_container_writing_mode
            .unwrap_or(self.style.writing_mode)
            .is_vertical()
            && (self.grid_container_writing_mode.is_none() || self.style.writing_mode.is_vertical())
    }

    fn rect_in_grid_axes<U: Copy>(&self, rect: taffy::Rect<U>) -> taffy::Rect<U> {
        let writing_mode = self
            .grid_container_writing_mode
            .unwrap_or(self.style.writing_mode);
        if !self.grid_axes_are_vertical() {
            return rect;
        }
        taffy::Rect {
            left: rect.top,
            right: rect.bottom,
            top: if writing_mode.is_vertical_rl() {
                rect.right
            } else {
                rect.left
            },
            bottom: if writing_mode.is_vertical_rl() {
                rect.left
            } else {
                rect.right
            },
        }
    }

    pub fn with_grid_lanes_stacking_axis(mut self, horizontal: bool) -> Self {
        self.grid_lanes_stacking_axis_is_horizontal = Some(horizontal);
        self
    }

    fn grid_self_alignment(
        &self,
        input: stylo::AlignFlags,
        horizontal: bool,
    ) -> Option<taffy::AlignSelf> {
        if self.grid_lanes_stacking_axis_is_horizontal == Some(horizontal)
            && matches!(
                input.value(),
                stylo::AlignFlags::AUTO | stylo::AlignFlags::NORMAL
            )
        {
            return Some(taffy::AlignSelf::START);
        }
        let mut alignment = convert::self_alignment(input)?;
        let Some(container_direction) = self.grid_container_direction else {
            return Some(alignment);
        };

        let self_start = match alignment.keyword {
            taffy::AlignItemsKeyword::SelfStart => true,
            taffy::AlignItemsKeyword::SelfEnd => false,
            _ => return Some(alignment),
        };
        // Taffy resolves self-relative alignment using direction only. Resolve it here so
        // vertical item writing modes choose the correct physical side as well.
        let writing_mode = self.style.writing_mode;
        let container_mode = self.grid_container_writing_mode.unwrap_or(writing_mode);
        let item_start_at_end = if container_mode.is_vertical() {
            if horizontal {
                writing_mode.is_vertical() && !writing_mode.is_inline_tb()
            } else {
                let item_start_at_right = if writing_mode.is_vertical() {
                    writing_mode.is_vertical_rl()
                } else {
                    !writing_mode.is_bidi_ltr()
                };
                item_start_at_right == container_mode.is_vertical_lr()
            }
        } else if horizontal {
            if writing_mode.is_vertical() {
                writing_mode.is_vertical_rl()
            } else {
                !writing_mode.is_bidi_ltr()
            }
        } else {
            writing_mode.is_vertical() && !writing_mode.is_inline_tb()
        };
        let container_start_at_end = horizontal && container_direction == taffy::Direction::Rtl;
        alignment.keyword = if self_start == (item_start_at_end == container_start_at_end) {
            taffy::AlignItemsKeyword::Start
        } else {
            taffy::AlignItemsKeyword::End
        };
        Some(alignment)
    }
}

impl<T: Deref<Target = ComputedValues>> taffy::CoreStyle for TaffyStyloStyle<T> {
    type CustomIdent = Atom;

    #[inline]
    fn box_generation_mode(&self) -> taffy::BoxGenerationMode {
        convert::box_generation_mode(self.style.get_box().display)
    }

    #[inline]
    fn direction(&self) -> taffy::Direction {
        convert::direction(self.style.slow_clone_direction())
    }

    #[inline]
    fn is_block(&self) -> bool {
        convert::is_block(self.style.get_box().display)
    }

    #[inline]
    fn is_compressible_replaced(&self) -> bool {
        self.fragment_flags.is_compressible_replaced()
    }

    #[inline]
    fn is_replaced(&self) -> bool {
        self.fragment_flags.contains(FragmentFlags::IS_REPLACED)
    }

    #[inline]
    fn box_sizing(&self) -> taffy::BoxSizing {
        convert::box_sizing(self.style.get_position().box_sizing)
    }

    #[inline]
    fn overflow(&self) -> taffy::Point<taffy::Overflow> {
        let box_styles = self.style.get_box();
        let overflow = taffy::Point {
            x: convert::overflow(box_styles.overflow_x),
            y: convert::overflow(box_styles.overflow_y),
        };
        if self.grid_axes_are_vertical() {
            taffy::Point {
                x: overflow.y,
                y: overflow.x,
            }
        } else {
            overflow
        }
    }

    #[inline]
    fn scrollbar_width(&self) -> f32 {
        0.0
    }

    #[inline]
    fn position(&self) -> taffy::Position {
        convert::position(self.style.get_box().position)
    }

    #[inline]
    fn is_containing_block(&self) -> taffy::ContainingBlockClaims {
        let fixed = self
            .style
            .establishes_containing_block_for_all_descendants(self.fragment_flags);
        let absolute = fixed ||
            self.style
                .establishes_containing_block_for_absolute_descendants(self.fragment_flags);
        taffy::ContainingBlockClaims { absolute, fixed }
    }

    #[inline]
    fn inset(&self) -> taffy::Rect<taffy::LengthPercentageAuto> {
        let position_styles = self.style.get_position();
        self.rect_in_grid_axes(taffy::Rect {
            left: convert::inset(&position_styles.left),
            right: convert::inset(&position_styles.right),
            top: convert::inset(&position_styles.top),
            bottom: convert::inset(&position_styles.bottom),
        })
    }

    #[inline]
    fn size(&self) -> taffy::Size<taffy::Dimension> {
        let position_styles = self.style.get_position();
        let size = taffy::Size {
            width: convert::dimension(&position_styles.width),
            height: convert::dimension(&position_styles.height),
        };
        if self.grid_axes_are_vertical() {
            taffy::Size {
                width: size.height,
                height: size.width,
            }
        } else {
            size
        }
    }

    #[inline]
    fn min_size(&self) -> taffy::Size<taffy::LengthPercentageAuto> {
        let position_styles = self.style.get_position();
        let size = taffy::Size {
            width: convert::min_size(&position_styles.min_width),
            height: convert::min_size(&position_styles.min_height),
        };
        if self.grid_axes_are_vertical() {
            taffy::Size {
                width: size.height,
                height: size.width,
            }
        } else {
            size
        }
    }

    #[inline]
    fn max_size(&self) -> taffy::Size<taffy::LengthPercentageAuto> {
        let position_styles = self.style.get_position();
        let size = taffy::Size {
            width: convert::max_size(&position_styles.max_width),
            height: convert::max_size(&position_styles.max_height),
        };
        if self.grid_axes_are_vertical() {
            taffy::Size {
                width: size.height,
                height: size.width,
            }
        } else {
            size
        }
    }

    #[inline]
    fn aspect_ratio(&self) -> Option<f32> {
        convert::aspect_ratio(self.style.get_position().aspect_ratio)
    }

    #[inline]
    fn margin(&self) -> taffy::Rect<taffy::LengthPercentageAuto> {
        let margin_styles = self.style.get_margin();
        self.rect_in_grid_axes(taffy::Rect {
            left: convert::margin(&margin_styles.margin_left),
            right: convert::margin(&margin_styles.margin_right),
            top: convert::margin(&margin_styles.margin_top),
            bottom: convert::margin(&margin_styles.margin_bottom),
        })
    }

    #[inline]
    fn padding(&self) -> taffy::Rect<taffy::LengthPercentage> {
        let padding_styles = self.style.get_padding();
        self.rect_in_grid_axes(taffy::Rect {
            left: convert::length_percentage(&padding_styles.padding_left.0),
            right: convert::length_percentage(&padding_styles.padding_right.0),
            top: convert::length_percentage(&padding_styles.padding_top.0),
            bottom: convert::length_percentage(&padding_styles.padding_bottom.0),
        })
    }

    #[inline]
    fn border(&self) -> taffy::Rect<taffy::LengthPercentage> {
        let border = self.style.get_border();
        let resolve = |width: &BorderSideWidth, style: BorderStyle| {
            taffy::LengthPercentage::length(if style.none_or_hidden() {
                0.0
            } else {
                width.0.to_f32_px()
            })
        };
        self.rect_in_grid_axes(taffy::Rect {
            left: resolve(&border.border_left_width, border.border_left_style),
            right: resolve(&border.border_right_width, border.border_right_style),
            top: resolve(&border.border_top_width, border.border_top_style),
            bottom: resolve(&border.border_bottom_width, border.border_bottom_style),
        })
    }
}

type SliceMapIter<'a, Input, Output> =
    core::iter::Map<core::slice::Iter<'a, Input>, for<'c> fn(&'c Input) -> Output>;
type SliceMapRefIter<'a, Input, Output> =
    core::iter::Map<core::slice::Iter<'a, Input>, for<'c> fn(&'c Input) -> &'c Output>;

// Line name iterator type aliases
type LineNameSetIter<'a> = SliceMapRefIter<'a, CustomIdent, Atom>;
type LineNameIter<'a> = core::iter::Map<
    core::slice::Iter<'a, OwnedSlice<CustomIdent>>,
    fn(&OwnedSlice<CustomIdent>) -> LineNameSetIter<'_>,
>;

#[derive(Clone)]
pub struct StyloLineNameIter<'a>(LineNameIter<'a>);
impl<'a> StyloLineNameIter<'a> {
    fn new(names: &'a OwnedSlice<OwnedSlice<CustomIdent>>) -> Self {
        Self(names.iter().map(|names| names.iter().map(|ident| &ident.0)))
    }
}
impl<'a> Iterator for StyloLineNameIter<'a> {
    type Item = core::iter::Map<core::slice::Iter<'a, CustomIdent>, fn(&CustomIdent) -> &Atom>;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}
impl ExactSizeIterator for StyloLineNameIter<'_> {}
impl<'a> taffy::TemplateLineNames<'a, Atom> for StyloLineNameIter<'a> {
    type LineNameSet<'b>
        = SliceMapRefIter<'b, CustomIdent, Atom>
    where
        Self: 'b;
}

pub struct RepetitionWrapper<'a>(&'a TrackRepeat<LengthPercentage, i32>);

impl taffy::GenericRepetition for RepetitionWrapper<'_> {
    type CustomIdent = Atom;

    type RepetitionTrackList<'a>
        = SliceMapIter<'a, stylo::TrackSize<LengthPercentage>, taffy::TrackSizingFunction>
    where
        Self: 'a;

    type TemplateLineNames<'a>
        = StyloLineNameIter<'a>
    where
        Self: 'a;

    fn count(&self) -> taffy::RepetitionCount {
        convert::track_repeat(self.0.count)
    }

    fn tracks(&self) -> Self::RepetitionTrackList<'_> {
        self.0.track_sizes.iter().map(convert::track_size)
    }

    fn lines_names(&self) -> Self::TemplateLineNames<'_> {
        StyloLineNameIter::new(&self.0.line_names)
    }
}

impl<T: Deref<Target = ComputedValues>> taffy::GridContainerStyle for TaffyStyloStyle<T> {
    type Repetition<'a>
        = RepetitionWrapper<'a>
    where
        Self: 'a;

    type TemplateTrackList<'a>
        = core::iter::Map<
        core::slice::Iter<'a, TrackListValue<LengthPercentage, i32>>,
        fn(
            &'a TrackListValue<LengthPercentage, i32>,
        ) -> taffy::GenericGridTemplateComponent<Atom, RepetitionWrapper<'a>>,
    >
    where
        Self: 'a;

    type AutoTrackList<'a>
        = SliceMapIter<'a, TrackSize<LengthPercentage>, taffy::TrackSizingFunction>
    where
        Self: 'a;

    type TemplateLineNames<'a>
        = StyloLineNameIter<'a>
    where
        Self: 'a;
    type GridTemplateAreas<'a>
        = SliceMapIter<'a, NamedArea, taffy::GridTemplateArea<Atom>>
    where
        Self: 'a;

    #[inline]
    fn grid_template_rows(&self) -> Option<Self::TemplateTrackList<'_>> {
        match &self.style.get_position().grid_template_rows {
            stylo::GenericGridTemplateComponent::None => None,
            stylo::GenericGridTemplateComponent::TrackList(list) => {
                Some(list.values.iter().map(|track| match track {
                    stylo::TrackListValue::TrackSize(size) => {
                        taffy::GenericGridTemplateComponent::Single(convert::track_size(size))
                    },
                    stylo::TrackListValue::TrackRepeat(repeat) => {
                        taffy::GenericGridTemplateComponent::Repeat(RepetitionWrapper(repeat))
                    },
                }))
            },

            // TODO: Implement subgrid
            stylo::GenericGridTemplateComponent::Subgrid(_) => None,
        }
    }

    #[inline]
    fn grid_template_columns(&self) -> Option<Self::TemplateTrackList<'_>> {
        match &self.style.get_position().grid_template_columns {
            stylo::GenericGridTemplateComponent::None => None,
            stylo::GenericGridTemplateComponent::TrackList(list) => {
                Some(list.values.iter().map(|track| match track {
                    stylo::TrackListValue::TrackSize(size) => {
                        taffy::GenericGridTemplateComponent::Single(convert::track_size(size))
                    },
                    stylo::TrackListValue::TrackRepeat(repeat) => {
                        taffy::GenericGridTemplateComponent::Repeat(RepetitionWrapper(repeat))
                    },
                }))
            },

            // TODO: Implement subgrid
            stylo::GenericGridTemplateComponent::Subgrid(_) => None,
        }
    }

    #[inline]
    fn grid_auto_rows(&self) -> Self::AutoTrackList<'_> {
        self.style
            .get_position()
            .grid_auto_rows
            .0
            .iter()
            .map(convert::track_size)
    }

    #[inline]
    fn grid_auto_columns(&self) -> Self::AutoTrackList<'_> {
        self.style
            .get_position()
            .grid_auto_columns
            .0
            .iter()
            .map(convert::track_size)
    }

    fn grid_template_areas(&self) -> Option<Self::GridTemplateAreas<'_>> {
        match &self.style.get_position().grid_template_areas {
            GridTemplateAreas::Areas(areas) => {
                Some(areas.0.areas.iter().map(|area| taffy::GridTemplateArea {
                    name: area.name.clone(),
                    row_start: area.rows.start as u16,
                    row_end: area.rows.end as u16,
                    column_start: area.columns.start as u16,
                    column_end: area.columns.end as u16,
                }))
            },
            GridTemplateAreas::None => None,
        }
    }

    fn grid_template_area_row_count(&self) -> u16 {
        match &self.style.get_position().grid_template_areas {
            GridTemplateAreas::Areas(areas) => as_clamped_i16(areas.0.strings.len() as i32) as u16,
            GridTemplateAreas::None => 0,
        }
    }

    fn grid_template_area_column_count(&self) -> u16 {
        match &self.style.get_position().grid_template_areas {
            GridTemplateAreas::Areas(areas) => as_clamped_i16(areas.0.width as i32) as u16,
            GridTemplateAreas::None => 0,
        }
    }

    fn grid_template_column_names(&self) -> Option<Self::TemplateLineNames<'_>> {
        match &self.style.get_position().grid_template_columns {
            stylo::GenericGridTemplateComponent::None => None,
            stylo::GenericGridTemplateComponent::TrackList(list) => {
                Some(StyloLineNameIter::new(&list.line_names))
            },
            // TODO: Implement subgrid
            stylo::GenericGridTemplateComponent::Subgrid(_) => None,
        }
    }

    fn grid_template_row_names(&self) -> Option<Self::TemplateLineNames<'_>> {
        match &self.style.get_position().grid_template_rows {
            stylo::GenericGridTemplateComponent::None => None,
            stylo::GenericGridTemplateComponent::TrackList(list) => {
                Some(StyloLineNameIter::new(&list.line_names))
            },
            // TODO: Implement subgrid
            stylo::GenericGridTemplateComponent::Subgrid(_) => None,
        }
    }

    #[inline]
    fn grid_auto_flow(&self) -> taffy::GridAutoFlow {
        convert::grid_auto_flow(self.style.get_position().grid_auto_flow)
    }

    #[inline]
    fn gap(&self) -> taffy::Size<taffy::LengthPercentage> {
        let position_styles = self.style.get_position();
        taffy::Size {
            width: convert::gap(&position_styles.column_gap),
            height: convert::gap(&position_styles.row_gap),
        }
    }

    #[inline]
    fn align_content(&self) -> taffy::AlignContent {
        convert::content_alignment(self.style.get_position().align_content)
    }

    #[inline]
    fn justify_content(&self) -> taffy::JustifyContent {
        convert::content_alignment(self.style.get_position().justify_content)
    }

    #[inline]
    fn align_items(&self) -> taffy::AlignItems {
        convert::items_alignment(self.style.get_position().align_items.0)
    }

    #[inline]
    fn justify_items(&self) -> taffy::AlignItems {
        convert::items_alignment(self.style.get_position().justify_items.computed.0.0)
    }
}

impl<T: Deref<Target = ComputedValues>> taffy::GridItemStyle for TaffyStyloStyle<T> {
    #[inline]
    fn grid_row(&self) -> taffy::Line<taffy::GridPlacement<Atom>> {
        let position_styles = self.style.get_position();
        taffy::Line {
            start: convert::grid_line(&position_styles.grid_row_start),
            end: convert::grid_line(&position_styles.grid_row_end),
        }
    }

    #[inline]
    fn grid_column(&self) -> taffy::Line<taffy::GridPlacement<Atom>> {
        let position_styles = self.style.get_position();
        taffy::Line {
            start: convert::grid_line(&position_styles.grid_column_start),
            end: convert::grid_line(&position_styles.grid_column_end),
        }
    }

    #[inline]
    fn align_self(&self) -> Option<taffy::AlignSelf> {
        self.grid_self_alignment(self.style.get_position().align_self.0, false)
    }

    #[inline]
    fn justify_self(&self) -> Option<taffy::AlignSelf> {
        self.grid_self_alignment(self.style.get_position().justify_self.0, true)
    }
}
