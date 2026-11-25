/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

mod paint_style;

use std::collections::HashMap;
use std::sync::Arc;

use layout_api::{LayoutElement, LayoutNode};
use log::debug;
use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};
use servo_arc::Arc as ServoArc;
use servo_svg::document::{
    AspectAlign, AspectRatio, MeetOrSlice, SvgTree, SvgViewport, ViewBox, ViewportInfo,
};
use servo_svg::element::{Container, SvgNode, SvgTag};
use servo_svg::transform::TransformOp;
use servo_svg::units::{Id, Length};
use style::properties::ComputedValues;
use style::values::computed::box_::Zoom;
use style::values::computed::{Length as CssLength, Overflow};
use web_atoms::{local_name, ns};

use crate::context::LayoutContext;
use crate::svg::primitives::attrs::{get_attr, parse_length_resolved};
use crate::svg::primitives::geometry::{build_shape, diagonal, resolve, size};

pub(crate) struct SvgTreeBuilder<'dom, 'a> {
    root_node: ServoLayoutNode<'dom>,
    context: &'a LayoutContext<'a>,
    root_width: f32,
    root_height: f32,
}

impl<'dom, 'a> SvgTreeBuilder<'dom, 'a> {
    pub(crate) fn new(
        node: ServoLayoutNode<'dom>,
        context: &'a LayoutContext<'a>,
        root_width: f32,
        root_height: f32,
    ) -> Self {
        SvgTreeBuilder {
            root_node: node,
            context,
            root_width,
            root_height,
        }
    }

    pub(crate) fn build(self) -> Option<Arc<SvgTree>> {
        let root_style = self.computed_style(self.root_node);
        let zoom = root_style
            .as_ref()
            .map_or(Zoom::ONE, |style| style.effective_zoom);
        let viewport = viewport_info(
            &self.root_node.as_element()?,
            root_style.as_deref(),
            zoom.unzoom(self.root_width),
            zoom.unzoom(self.root_height),
        );
        let root = self.build_node(self.root_node, user_size(&viewport))?;

        let root = Arc::try_unwrap(root).expect("the root `<svg>` is never cached");

        let tree = SvgTree {
            root,
            viewport,
            gradients: HashMap::new(),  // TODO
            clip_paths: HashMap::new(), // TODO
            patterns: HashMap::new(),   // TODO
            masks: HashMap::new(),      // TODO
            filters: HashMap::new(),    // TODO
            markers: HashMap::new(),    // TODO
        };
        Some(Arc::new(tree))
    }

    fn build_node(
        &self,
        node: ServoLayoutNode<'dom>,
        parent_size: (f32, f32),
    ) -> Option<Arc<SvgNode>> {
        let element = node.as_element()?;
        if !element.is_svg_element() {
            return None;
        }
        // Follow usvg's conditional processing: this renderer supports no extensions.
        if get_attr(&element, &local_name!("requiredExtensions")).is_some() {
            return None;
        }
        let is_root = node == self.root_node;

        let computed_style = self.computed_style(node);
        let version = node.inclusive_descendants_version();
        let computed = computed_style.as_deref();
        let (width, height) = parent_size;
        let is_svg = element.local_name() == &local_name!("svg");
        let is_group = element.local_name() == &local_name!("g");
        let viewport =
            (!is_root && is_svg).then(|| nested_viewport(&element, computed, width, height));
        let child_size = viewport
            .as_ref()
            .map_or(parent_size, |viewport| user_size(&viewport.viewport));
        let mut children = Vec::new();
        if is_svg || is_group {
            children.extend(
                node.dom_children()
                    .filter_map(|child| self.build_node(child, child_size)),
            );
        }

        if !is_root {
            match self.context.svg_subtree_cache.read().lookup(
                node.opaque(),
                version,
                computed_style.as_ref(),
                parent_size,
                &children,
            ) {
                Ok(hit) => {
                    debug!(target: "svg", "build_node: reuse {}", describe(node));
                    return Some(hit);
                },
                Err(reason) => {
                    debug!(target: "svg", "build_node: build {} [{reason:?}]", describe(node))
                },
            }
        }

        let tag = build_tag(&element, computed, width, height)?;
        let id = extract_id(&element);

        let built = Arc::new(SvgNode {
            id,
            tag,
            style: paint_style::node_style(computed, diagonal(width, height)),
            transforms: computed
                .filter(|_| !is_root)
                .map_or_else(Vec::new, |style| transforms(style, width, height)),
            viewport,
            children,
        });

        if !is_root {
            self.context.svg_subtree_cache.write().store(
                node.opaque(),
                version,
                computed_style,
                parent_size,
                Arc::clone(&built),
            );
        }
        Some(built)
    }

    fn computed_style(
        &self,
        node: ServoLayoutNode<'dom>,
    ) -> Option<ServoArc<style::properties::ComputedValues>> {
        let element = node.as_element()?;
        element.style_data()?;
        Some(node.style(&self.context.style_context))
    }
}

fn build_tag(
    element: &ServoLayoutElement,
    computed: Option<&style::properties::ComputedValues>,
    vw: f32,
    vh: f32,
) -> Option<SvgTag> {
    match element.local_name() {
        &local_name!("svg") => Some(SvgTag::Container(Container::Svg)),
        &local_name!("g") => Some(SvgTag::Container(Container::Group)),
        &local_name!("defs") => None,
        &local_name!("use") => None,
        &local_name!("symbol") => None,
        &local_name!("switch") => None,
        &local_name!("text") => None,
        &local_name!("tspan") => None,
        &local_name!("image") => None,
        tag => build_shape(element, tag, computed, vw, vh).map(SvgTag::Shape),
    }
}

fn describe<'dom>(node: ServoLayoutNode<'dom>) -> String {
    let Some(element) = node.as_element() else {
        return "<node>".to_owned();
    };
    let tag = element.local_name().to_string();
    match element.attribute_as_str(&ns!(), &local_name!("id")) {
        Some(id) => format!("<{tag}#{id}>"),
        None => format!("<{tag}>"),
    }
}

fn extract_id(element: &ServoLayoutElement) -> Option<Id> {
    element
        .attribute_as_str(&ns!(), &local_name!("id"))
        .map(Id::new)
}

fn user_size(viewport: &ViewportInfo) -> (f32, f32) {
    viewport.view_box.as_ref().map_or_else(
        || (viewport.width.get(), viewport.height.get()),
        |view_box| (view_box.width.get(), view_box.height.get()),
    )
}

fn viewport_info(
    element: &ServoLayoutElement,
    computed: Option<&ComputedValues>,
    width: f32,
    height: f32,
) -> ViewportInfo {
    let view_box = get_attr(element, &local_name!("viewBox")).and_then(|attr| {
        let mut values = svgtypes::NumberListParser::from(attr);
        let mut next = || {
            values
                .next()?
                .ok()
                .map(|value| value as f32)
                .filter(|value| value.is_finite())
        };
        let [x, y, w, h] = [next()?, next()?, next()?, next()?];
        (w >= 0.0 && h >= 0.0 && values.next().is_none()).then(|| ViewBox {
            min_x: Length::new(x),
            min_y: Length::new(y),
            width: Length::new(w),
            height: Length::new(h),
        })
    });
    let aspect_ratio = get_attr(element, &local_name!("preserveAspectRatio"))
        .and_then(|attr| attr.parse::<svgtypes::AspectRatio>().ok())
        .map(|value| AspectRatio {
            align: match value.align {
                svgtypes::Align::None => AspectAlign::None,
                svgtypes::Align::XMinYMin => AspectAlign::XMinYMin,
                svgtypes::Align::XMidYMin => AspectAlign::XMidYMin,
                svgtypes::Align::XMaxYMin => AspectAlign::XMaxYMin,
                svgtypes::Align::XMinYMid => AspectAlign::XMinYMid,
                svgtypes::Align::XMidYMid => AspectAlign::XMidYMid,
                svgtypes::Align::XMaxYMid => AspectAlign::XMaxYMid,
                svgtypes::Align::XMinYMax => AspectAlign::XMinYMax,
                svgtypes::Align::XMidYMax => AspectAlign::XMidYMax,
                svgtypes::Align::XMaxYMax => AspectAlign::XMaxYMax,
            },
            meet_or_slice: if value.slice {
                MeetOrSlice::Slice
            } else {
                MeetOrSlice::Meet
            },
        });
    ViewportInfo {
        width: Length::new(width),
        height: Length::new(height),
        view_box,
        aspect_ratio,
        overflow_visible: computed.is_some_and(|style| {
            style.get_box().overflow_x == Overflow::Visible
                && style.get_box().overflow_y == Overflow::Visible
        }),
    }
}

fn nested_viewport(
    element: &ServoLayoutElement,
    computed: Option<&ComputedValues>,
    width: f32,
    height: f32,
) -> SvgViewport {
    let zoom = computed.map_or(Zoom::ONE, |style| style.effective_zoom);
    let font_size = computed.map_or(16.0, |style| {
        zoom.unzoom(style.get_font().font_size.computed_size().px())
    });
    let position = computed.map(|style| style.get_position());
    let vw = size(
        position.map(|style| &style.width),
        element,
        local_name!("width"),
        font_size,
        width,
        zoom,
    )
    .map_or(width, Length::get);
    let vh = size(
        position.map(|style| &style.height),
        element,
        local_name!("height"),
        font_size,
        height,
        zoom,
    )
    .map_or(height, Length::get);
    let (x, y) = if let Some(style) = computed {
        let svg = style.get_svg();
        (resolve(&svg.x, width, zoom), resolve(&svg.y, height, zoom))
    } else {
        (
            parse_length_resolved(&local_name!("x"), element, font_size, width).unwrap_or_default(),
            parse_length_resolved(&local_name!("y"), element, font_size, height)
                .unwrap_or_default(),
        )
    };
    SvgViewport {
        x: Length::new(x),
        y: Length::new(y),
        viewport: viewport_info(element, computed, vw, vh),
    }
}

fn transforms(style: &ComputedValues, width: f32, height: f32) -> Vec<TransformOp> {
    let box_style = style.get_box();
    if box_style.transform.0.is_empty() {
        return Vec::new();
    }
    let zoom = style.effective_zoom;
    let reference = euclid::rect(
        CssLength::new(0.0),
        CssLength::new(0.0),
        CssLength::new(zoom.zoom(width)),
        CssLength::new(zoom.zoom(height)),
    );
    let Ok((matrix, _)) = box_style.transform.to_transform_3d_matrix(Some(&reference)) else {
        return Vec::new();
    };
    let origin = &box_style.transform_origin;
    let ox = resolve(&origin.horizontal, width, zoom);
    let oy = resolve(&origin.vertical, height, zoom);
    vec![TransformOp::Matrix([
        matrix.m11,
        matrix.m12,
        matrix.m21,
        matrix.m22,
        zoom.unzoom(matrix.m41) + ox - matrix.m11 * ox - matrix.m21 * oy,
        zoom.unzoom(matrix.m42) + oy - matrix.m12 * ox - matrix.m22 * oy,
    ])]
}
