/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Native SVG layout: builds a `servo_svg::document::SvgTree` for an outer `<svg>` from the
//! computed style of its subtree, behind the `layout_servo_svg_enabled` pref.
//!
//! Based on servo/servo#46688 (the builder, the subtree cache and `Fragment::Svg`). The plan
//! for the rest is in `docs/armature/svg-native.md`.

mod builder;
mod cache;
mod primitives;
mod raster_cache;

use std::collections::HashMap;
use std::sync::Arc;

pub(crate) use cache::SvgSubtreeCache;
use layout_api::{LayoutElement, LayoutNode};
pub(crate) use raster_cache::SvgRasterCache;
use script::layout_dom::ServoLayoutNode;
use servo_svg::document::SvgTree;
use style::dom::OpaqueNode;
use web_atoms::local_name;

use crate::context::LayoutContext;

pub(crate) fn outer_svg_nodes(
    root: ServoLayoutNode<'_>,
) -> HashMap<OpaqueNode, ServoLayoutNode<'_>> {
    let mut roots = HashMap::new();
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if node.as_element().is_some_and(|element| {
            element.is_svg_element() && *element.local_name() == local_name!("svg")
        }) {
            roots.insert(node.opaque(), node);
        } else {
            pending.extend(node.flat_tree_children());
        }
    }
    roots
}

/// The entry point for building an SvgTree.
pub(crate) fn build_svg_tree<'dom>(
    node: ServoLayoutNode<'dom>,
    context: &LayoutContext,
    viewport_width: f32,
    viewport_height: f32,
) -> Option<Arc<SvgTree>> {
    builder::SvgTreeBuilder::new(node, context, viewport_width, viewport_height).build()
}
