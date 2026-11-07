/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The geometry of SVG shapes, computed from their attributes, for the methods of the SVG
//! DOM that measure them: `getBBox()`, `getTotalLength()`, `getPointAtLength()`,
//! `isPointInFill()` and `isPointInStroke()`.
//!
//! Servo renders SVG by handing the serialized document fragment to resvg, so layout knows
//! nothing about the geometry of SVG elements; this module recomputes it from the
//! attributes.

use html5ever::{LocalName, local_name, ns};
use js::context::{JSContext, NoGC};
use kurbo::{Affine, BezPath, Ellipse, Line, ParamCurveArclen, PathSeg, Point, Rect, Shape};

use crate::dom::bindings::codegen::Bindings::NodeBinding::NodeMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::element::Element;
use crate::dom::node::{Node, NodeTraits};
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svglength::resolve_length;
use crate::dom::svg::svgpointlist::parse_point_list;
use crate::dom::svg::svgtransformlist::parse_transform_items;
use crate::dom::svg::svgtypes::{
    SVGLengthDirection, SVGLengthValue, is_svg_whitespace, parse_number_prefix,
};

/// The accuracy of lengths along curves, in user units.
const ACCURACY: f64 = 1e-4;

/// The geometry of a basic shape or path.
pub(crate) enum SVGShapeGeometry {
    Path(BezPath),
    Ellipse(Ellipse),
    Line(Line),
}

impl SVGShapeGeometry {
    pub(crate) fn bounding_box(&self) -> Rect {
        match self {
            Self::Path(path) => path.bounding_box(),
            Self::Ellipse(ellipse) => ellipse.bounding_box(),
            Self::Line(line) => line.bounding_box(),
        }
    }

    pub(crate) fn total_length(&self) -> f64 {
        match self {
            Self::Path(path) => path.perimeter(ACCURACY),
            Self::Ellipse(ellipse) => ellipse.perimeter(ACCURACY),
            Self::Line(line) => line.perimeter(ACCURACY),
        }
    }

    pub(crate) fn winding(&self, point: Point) -> i32 {
        match self {
            Self::Path(path) => path.winding(point),
            Self::Ellipse(ellipse) => ellipse.winding(point),
            Self::Line(_) => 0,
        }
    }

    fn segments(&self) -> Vec<PathSeg> {
        match self {
            Self::Path(path) => path.segments().collect(),
            Self::Ellipse(ellipse) => ellipse.path_segments(ACCURACY).collect(),
            Self::Line(line) => vec![PathSeg::Line(*line)],
        }
    }

    /// The point at `distance` along the shape, which is clamped to its length.
    ///
    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGGeometryElement__getPointAtLength>
    pub(crate) fn point_at_length(&self, distance: f64) -> Point {
        let segments = self.segments();
        let Some(first) = segments.first() else {
            return Point::ZERO;
        };
        let mut remaining = distance.max(0.0);
        for segment in &segments {
            let length = segment.arclen(ACCURACY);
            if remaining <= length {
                let t = segment.inv_arclen(remaining, ACCURACY);
                return kurbo::ParamCurve::eval(segment, t);
            }
            remaining -= length;
        }
        let last = segments.last().unwrap_or(first);
        kurbo::ParamCurve::end(last)
    }

    /// The distance from `point` to the outline of the shape.
    pub(crate) fn distance_to_outline(&self, point: Point) -> f64 {
        self.segments()
            .iter()
            .map(|segment| {
                kurbo::ParamCurveNearest::nearest(segment, point, ACCURACY)
                    .distance_sq
                    .sqrt()
            })
            .fold(f64::INFINITY, f64::min)
    }
}

/// The value of a length attribute of `element` in user units.
fn length(
    cx: &mut JSContext,
    element: &SVGElement,
    name: LocalName,
    direction: SVGLengthDirection,
) -> f64 {
    resolve_length(cx, element, &name, direction, SVGLengthValue::number(0.0)) as f64
}

/// The geometry of `element`, when it is a basic shape or a path.
pub(crate) fn shape_geometry(cx: &mut JSContext, element: &SVGElement) -> Option<SVGShapeGeometry> {
    use SVGLengthDirection::{Horizontal, Other, Vertical};
    let local_name = element.upcast::<Element>().local_name().clone();
    Some(match local_name {
        local_name!("rect") => {
            let x = length(cx, element, local_name!("x"), Horizontal);
            let y = length(cx, element, local_name!("y"), Vertical);
            let width = length(cx, element, local_name!("width"), Horizontal);
            let height = length(cx, element, local_name!("height"), Vertical);
            if width <= 0.0 || height <= 0.0 {
                return Some(SVGShapeGeometry::Path(BezPath::new()));
            }
            SVGShapeGeometry::Path(Rect::new(x, y, x + width, y + height).to_path(ACCURACY))
        },
        local_name!("circle") => {
            let cx_ = length(cx, element, local_name!("cx"), Horizontal);
            let cy = length(cx, element, local_name!("cy"), Vertical);
            let r = length(cx, element, local_name!("r"), Other);
            if r <= 0.0 {
                return Some(SVGShapeGeometry::Path(BezPath::new()));
            }
            SVGShapeGeometry::Ellipse(Ellipse::new((cx_, cy), (r, r), 0.0))
        },
        local_name!("ellipse") => {
            let cx_ = length(cx, element, local_name!("cx"), Horizontal);
            let cy = length(cx, element, local_name!("cy"), Vertical);
            let rx = length(cx, element, local_name!("rx"), Horizontal);
            let ry = length(cx, element, local_name!("ry"), Vertical);
            if rx <= 0.0 || ry <= 0.0 {
                return Some(SVGShapeGeometry::Path(BezPath::new()));
            }
            SVGShapeGeometry::Ellipse(Ellipse::new((cx_, cy), (rx, ry), 0.0))
        },
        local_name!("line") => {
            let x1 = length(cx, element, local_name!("x1"), Horizontal);
            let y1 = length(cx, element, local_name!("y1"), Vertical);
            let x2 = length(cx, element, local_name!("x2"), Horizontal);
            let y2 = length(cx, element, local_name!("y2"), Vertical);
            SVGShapeGeometry::Line(Line::new((x1, y1), (x2, y2)))
        },
        local_name!("polyline") | local_name!("polygon") => {
            let points = parse_point_list(element.svg_attribute(&local_name!("points")).as_deref());
            let mut path = BezPath::new();
            for (index, (x, y)) in points.iter().enumerate() {
                if index == 0 {
                    path.move_to((*x, *y));
                } else {
                    path.line_to((*x, *y));
                }
            }
            if local_name == local_name!("polygon") && !points.is_empty() {
                path.close_path();
            }
            SVGShapeGeometry::Path(path)
        },
        local_name!("path") => {
            let data = element.svg_attribute(&local_name!("d")).unwrap_or_default();
            SVGShapeGeometry::Path(parse_path_data(&data))
        },
        _ => return None,
    })
}

/// Parse path data. When the data has an error, the path up to the error is used.
///
/// <https://svgwg.org/svg2-draft/paths.html#PathDataErrorHandling>
fn parse_path_data(data: &str) -> BezPath {
    if let Ok(path) = BezPath::from_svg(data) {
        return path;
    }
    // Find the longest prefix that ends before a command letter and parses.
    let boundaries: Vec<usize> = data
        .char_indices()
        .filter(|(_, c)| c.is_ascii_alphabetic() && !matches!(c, 'e' | 'E'))
        .map(|(index, _)| index)
        .collect();
    for end in boundaries.into_iter().rev() {
        if let Ok(path) = BezPath::from_svg(&data[..end]) {
            return path;
        }
    }
    BezPath::new()
}

/// Parse a transform list, the value of a `transform` attribute, into its matrix.
pub(crate) fn parse_transform_list(value: &str) -> Option<Affine> {
    Some(
        parse_transform_items(value)?
            .iter()
            .fold(Affine::IDENTITY, |matrix, item| matrix * item.matrix),
    )
}

/// Parse numbers separated by whitespace and optionally a comma.
fn parse_numbers(value: &str) -> Option<Vec<f64>> {
    let mut numbers = Vec::new();
    let mut rest = value.trim_matches(is_svg_whitespace);
    while !rest.is_empty() {
        let (number, after) = parse_number_prefix(rest)?;
        numbers.push(number as f64);
        rest = after.trim_start_matches(is_svg_whitespace);
        if let Some(after_comma) = rest.strip_prefix(',') {
            rest = after_comma.trim_start_matches(is_svg_whitespace);
            if rest.is_empty() {
                return None;
            }
        }
    }
    Some(numbers)
}

/// The transform that the `transform` attribute of `element` gives it.
pub(crate) fn element_transform(element: &SVGElement) -> Affine {
    element
        .svg_attribute(&local_name!("transform"))
        .and_then(|value| parse_transform_list(&value))
        .unwrap_or(Affine::IDENTITY)
}

/// Whether `element` does not take part in rendering because of its `display`.
///
/// The computed style of the elements inside an `svg` element always has `display: none`
/// in Servo, which does not lay them out, so this looks at the `display` presentation
/// attribute and at the `style` attribute instead.
fn is_display_none(element: &SVGElement) -> bool {
    let element = element.upcast::<Element>();
    if element
        .get_attribute_string_value_with_namespace(&ns!(), &local_name!("display"))
        .is_some_and(|value| value.trim() == "none")
    {
        return true;
    }
    element
        .get_attribute_string_value_with_namespace(&ns!(), &local_name!("style"))
        .is_some_and(|value| {
            value.split(';').any(|declaration| {
                let mut parts = declaration.splitn(2, ':');
                parts.next().is_some_and(|name| name.trim() == "display") &&
                    parts.next().is_some_and(|value| value.trim() == "none")
            })
        })
}

/// The bounding box of `element` in its user space, which does not include its own
/// transform, or `None` when it has no geometry.
///
/// <https://svgwg.org/svg2-draft/coords.html#BoundingBoxes>
pub(crate) fn bounding_box(cx: &mut JSContext, element: &SVGElement) -> Option<Rect> {
    if let Some(geometry) = shape_geometry(cx, element) {
        return Some(geometry.bounding_box());
    }
    let local_name = element.upcast::<Element>().local_name().clone();
    match local_name {
        local_name!("text") | local_name!("tspan") | local_name!("textPath") => {
            // Text that is not displayed has no geometry.
            let hidden = element
                .upcast::<Node>()
                .inclusive_ancestors(crate::dom::iterators::ShadowIncluding::No)
                .filter_map(DomRoot::downcast::<SVGElement>)
                .any(|ancestor| is_display_none(&ancestor));
            if hidden {
                return Some(Rect::ZERO);
            }
            let characters = crate::dom::svg::svgtext::element_characters(cx, element);
            Some(
                characters
                    .iter()
                    .map(|character| character.extent())
                    .reduce(|union, extent| union.union(extent))
                    .unwrap_or_default(),
            )
        },
        local_name!("use") => {
            let x = length(
                cx,
                element,
                local_name!("x"),
                SVGLengthDirection::Horizontal,
            );
            let y = length(cx, element, local_name!("y"), SVGLengthDirection::Vertical);
            let referenced = referenced_element(cx.no_gc(), element)?;
            let bbox = bounding_box(cx, &referenced)?;
            Some(
                (Affine::translate((x, y)) * element_transform(&referenced))
                    .transform_rect_bbox(bbox),
            )
        },
        local_name!("g") |
        local_name!("svg") |
        local_name!("a") |
        local_name!("switch") |
        local_name!("symbol") |
        local_name!("defs") => {
            let mut union: Option<Rect> = None;
            let children: Vec<DomRoot<SVGElement>> = element
                .upcast::<Node>()
                .children()
                .filter_map(DomRoot::downcast::<SVGElement>)
                .collect();
            for child in children {
                if is_display_none(&child) {
                    continue;
                }
                let Some(bbox) = bounding_box(cx, &child) else {
                    continue;
                };
                let bbox = element_transform(&child).transform_rect_bbox(bbox);
                union = Some(match union {
                    Some(union) => union.union(bbox),
                    None => bbox,
                });
            }
            Some(union.unwrap_or_default())
        },
        _ => None,
    }
}

/// The element that the `href` of a `use` element refers to.
fn referenced_element(no_gc: &NoGC, element: &SVGElement) -> Option<DomRoot<SVGElement>> {
    let element_ref = element.upcast::<Element>();
    let href = element_ref
        .get_attribute_string_value_with_namespace(&ns!(), &local_name!("href"))
        .or_else(|| {
            element_ref.get_attribute_string_value_with_namespace(&ns!(xlink), &local_name!("href"))
        })?;
    let id = href.trim().strip_prefix('#')?;
    let document = element.upcast::<Node>().owner_doc();
    let referenced = document.get_element_by_id(no_gc, &stylo_atoms::Atom::from(id))?;
    DomRoot::downcast::<SVGElement>(referenced)
}

/// Parse a `viewBox` attribute: four numbers, of which the width and height must be
/// positive for the view box to apply.
pub(crate) fn parse_view_box(element: &SVGElement) -> Option<Rect> {
    let value = element.svg_attribute(&local_name!("viewBox"))?;
    match parse_numbers(&value)?.as_slice() {
        [x, y, width, height] if *width > 0.0 && *height > 0.0 => {
            Some(Rect::new(*x, *y, x + width, y + height))
        },
        _ => None,
    }
}

/// The transform from the user space inside the view box of `element` to its viewport of
/// size `width` by `height`.
///
/// <https://svgwg.org/svg2-draft/coords.html#ComputingAViewportsTransform>
pub(crate) fn view_box_transform(element: &SVGElement, width: f64, height: f64) -> Affine {
    let Some(view_box) = parse_view_box(element) else {
        return Affine::IDENTITY;
    };
    let (align, meet_or_slice) = parse_preserve_aspect_ratio(
        element
            .svg_attribute(&local_name!("preserveAspectRatio"))
            .as_deref(),
    );
    let mut scale_x = width / view_box.width();
    let mut scale_y = height / view_box.height();
    // Alignment 1 is `none`, which scales each axis independently.
    if align != 1 {
        let scale = if meet_or_slice == 2 {
            scale_x.max(scale_y)
        } else {
            scale_x.min(scale_y)
        };
        scale_x = scale;
        scale_y = scale;
    }
    let mut translate_x = -view_box.x0 * scale_x;
    let mut translate_y = -view_box.y0 * scale_y;
    if align != 1 {
        // The alignments from 2 to 10 are xMinYMin, xMidYMin, xMaxYMin, xMinYMid, ...
        let x_align = (align - 2) % 3;
        let y_align = (align - 2) / 3;
        let extra_x = width - view_box.width() * scale_x;
        let extra_y = height - view_box.height() * scale_y;
        translate_x += extra_x * x_align as f64 / 2.0;
        translate_y += extra_y * y_align as f64 / 2.0;
    }
    Affine::new([scale_x, 0.0, 0.0, scale_y, translate_x, translate_y])
}

/// Parse a `preserveAspectRatio` attribute into its alignment and meet-or-slice types.
pub(crate) fn parse_preserve_aspect_ratio(value: Option<&str>) -> (u16, u16) {
    const ALIGN_KEYWORDS: [&str; 10] = [
        "none", "xMinYMin", "xMidYMin", "xMaxYMin", "xMinYMid", "xMidYMid", "xMaxYMid", "xMinYMax",
        "xMidYMax", "xMaxYMax",
    ];
    let default = (6, 1);
    let Some(value) = value else {
        return default;
    };
    let mut tokens = value
        .split(is_svg_whitespace)
        .filter(|token| !token.is_empty());
    let Some(align) = tokens
        .next()
        .and_then(|first| ALIGN_KEYWORDS.iter().position(|keyword| *keyword == first))
    else {
        return default;
    };
    let meet_or_slice = match tokens.next() {
        None | Some("meet") => 1,
        Some("slice") => 2,
        Some(_) => return default,
    };
    if tokens.next().is_some() {
        return default;
    }
    (align as u16 + 1, meet_or_slice)
}

/// Whether `element` is an outermost `svg` element, whose parent is not an SVG element.
fn is_outermost_svg(element: &SVGElement) -> bool {
    element.upcast::<Element>().local_name() == &local_name!("svg") &&
        !element
            .upcast::<Node>()
            .GetParentNode()
            .is_some_and(|parent| parent.is::<SVGElement>())
}

/// The size of the viewport of the `svg` element `svg`, in the user units of its parent.
pub(crate) fn svg_viewport_size(cx: &mut JSContext, svg: &SVGElement) -> (f64, f64) {
    if is_outermost_svg(svg) {
        return svg
            .upcast::<Node>()
            .content_box(cx)
            .map(|rect| (rect.size.width.to_f64_px(), rect.size.height.to_f64_px()))
            .unwrap_or_default();
    }
    let width = resolve_length(
        cx,
        svg,
        &local_name!("width"),
        SVGLengthDirection::Horizontal,
        SVGLengthValue::percentage(100.0),
    );
    let height = resolve_length(
        cx,
        svg,
        &local_name!("height"),
        SVGLengthDirection::Vertical,
        SVGLengthValue::percentage(100.0),
    );
    (width as f64, height as f64)
}

/// The transform from the user space of the `svg` element `svg` to the user space of its
/// parent: its position, which outermost `svg` elements do not have, and its view box.
fn svg_element_transform(cx: &mut JSContext, svg: &SVGElement) -> Affine {
    let (width, height) = svg_viewport_size(cx, svg);
    let view_box = view_box_transform(svg, width, height);
    if is_outermost_svg(svg) {
        return view_box;
    }
    let x = length(cx, svg, local_name!("x"), SVGLengthDirection::Horizontal);
    let y = length(cx, svg, local_name!("y"), SVGLengthDirection::Vertical);
    Affine::translate((x, y)) * view_box
}

/// The current transformation matrix of `element`: the transform from its user space to
/// the viewport of its nearest viewport element, or with `to_screen`, to the viewport of
/// the document.
///
/// <https://svgwg.org/svg2-draft/types.html#__svg__SVGGraphicsElement__getCTM>
pub(crate) fn current_transform(
    cx: &mut JSContext,
    element: &SVGElement,
    to_screen: bool,
) -> Affine {
    let is_svg = element.upcast::<Element>().local_name() == &local_name!("svg");
    let mut transform = if is_svg {
        svg_element_transform(cx, element)
    } else {
        element_transform(element)
    };
    if is_svg && is_outermost_svg(element) {
        if to_screen {
            transform = screen_offset(cx, element) * transform;
        }
        return transform;
    }
    let ancestors: Vec<DomRoot<SVGElement>> = element
        .upcast::<Node>()
        .ancestors()
        .map_while(DomRoot::downcast::<SVGElement>)
        .collect();
    for ancestor in ancestors {
        if ancestor.upcast::<Element>().local_name() == &local_name!("svg") {
            if !to_screen {
                // The nearest viewport element establishes the coordinate system.
                let (width, height) = svg_viewport_size(cx, &ancestor);
                return view_box_transform(&ancestor, width, height) * transform;
            }
            transform = svg_element_transform(cx, &ancestor) * transform;
            if is_outermost_svg(&ancestor) {
                return screen_offset(cx, &ancestor) * transform;
            }
            continue;
        }
        transform = element_transform(&ancestor) * transform;
    }
    transform
}

/// The translation from the viewport of the outermost `svg` element `svg` to the viewport
/// of the document: the position of its content box.
fn screen_offset(cx: &mut JSContext, svg: &SVGElement) -> Affine {
    let node = svg.upcast::<Node>();
    let Some(content_box) = node.content_box(cx) else {
        return Affine::IDENTITY;
    };
    let scroll = node.owner_window().scroll_offset();
    Affine::translate((
        content_box.origin.x.to_f64_px() - scroll.x as f64,
        content_box.origin.y.to_f64_px() - scroll.y as f64,
    ))
}

/// Whether `element` is a basic shape or a path.
pub(crate) fn shape_geometry_kind(element: &SVGElement) -> bool {
    matches!(
        element.upcast::<Element>().local_name(),
        &local_name!("rect") |
            &local_name!("circle") |
            &local_name!("ellipse") |
            &local_name!("line") |
            &local_name!("polyline") |
            &local_name!("polygon") |
            &local_name!("path")
    )
}

/// The transform from the user space of `element` to the user space of its ancestor
/// `ancestor`.
pub(crate) fn transform_to_user_space_of(element: &SVGElement, ancestor: &SVGElement) -> Affine {
    let mut transform = element_transform(element);
    for node in element.upcast::<Node>().ancestors() {
        if &*node == ancestor.upcast::<Node>() {
            break;
        }
        if let Some(svg_element) = node.downcast::<SVGElement>() {
            transform = element_transform(svg_element) * transform;
        }
    }
    transform
}

/// The computed value of the property `name` of `element`, serialized. Elements inside an
/// `svg` element may not have been styled by layout, so this uses a style query, which
/// styles them when needed.
pub(crate) fn computed_value(element: &SVGElement, name: &str) -> String {
    let Ok(property) = style::properties::PropertyId::parse_enabled_for_all_content(name) else {
        return String::new();
    };
    let node = element.upcast::<Node>();
    String::from(node.owner_window().resolved_style_query(
        node.to_trusted_node_address(),
        None,
        property,
    ))
}

/// Whether the point `point`, in the user space of `element`, hits it, given what its
/// `pointer-events`, `visibility`, `fill` and `stroke` make sensitive.
///
/// <https://svgwg.org/svg2-draft/interact.html#PointerEventsProperty>
fn hits_element(cx: &mut JSContext, element: &SVGElement, point: Point) -> bool {
    // Find out where the point is relative to the geometry first, which is cheap, so that
    // style is only queried for the elements near the point.
    let local_name = element.upcast::<Element>().local_name().clone();
    let is_text = matches!(
        local_name,
        local_name!("text") | local_name!("tspan") | local_name!("textPath")
    );
    let geometry = if is_text || local_name == local_name!("image") {
        None
    } else {
        let Some(geometry) = shape_geometry(cx, element) else {
            return false;
        };
        Some(geometry)
    };
    let in_box = if is_text {
        crate::dom::svg::svgtext::element_characters(cx, element)
            .iter()
            .any(|character| character.extent().contains(point))
    } else if geometry.is_none() {
        bounding_box(cx, element).is_some_and(|bbox| bbox.contains(point))
    } else {
        false
    };
    let (winding, distance) = match &geometry {
        Some(geometry) => (geometry.winding(point), geometry.distance_to_outline(point)),
        None => (0, f64::INFINITY),
    };
    // Strokes wider than this are not considered.
    const MAXIMUM_HALF_STROKE_WIDTH: f64 = 50.0;
    let in_bounding_box = geometry
        .as_ref()
        .is_some_and(|geometry| geometry.bounding_box().contains(point));
    if !in_box && winding == 0 && distance > MAXIMUM_HALF_STROKE_WIDTH && !in_bounding_box {
        return false;
    }

    let pointer_events = computed_value(element, "pointer-events").to_ascii_lowercase();
    if pointer_events == "none" {
        return false;
    }
    let visible = computed_value(element, "visibility") == "visible";
    let fill_painted = computed_value(element, "fill") != "none";
    let stroke_painted = computed_value(element, "stroke") != "none";
    let (use_visibility, fill_sensitive, stroke_sensitive) = match pointer_events.as_str() {
        "visiblefill" => (true, true, false),
        "visiblestroke" => (true, false, true),
        "visible" => (true, true, true),
        "painted" => (false, fill_painted, stroke_painted),
        "fill" => (false, true, false),
        "stroke" => (false, false, true),
        "all" | "bounding-box" => (false, true, true),
        // `auto` and `visiblePainted`.
        _ => (true, fill_painted, stroke_painted),
    };
    if use_visibility && !visible {
        return false;
    }
    if geometry.is_none() {
        return in_box &&
            (fill_sensitive || stroke_sensitive || local_name == local_name!("image"));
    }
    if pointer_events == "bounding-box" {
        return in_bounding_box;
    }
    if fill_sensitive && winding != 0 {
        let inside = if computed_value(element, "fill-rule") == "evenodd" {
            winding % 2 != 0
        } else {
            true
        };
        if inside {
            return true;
        }
    }
    if stroke_sensitive {
        let stroke_width = computed_value(element, "stroke-width")
            .trim_end_matches("px")
            .parse::<f64>()
            .unwrap_or(1.0);
        if stroke_width > 0.0 && distance <= stroke_width / 2.0 {
            return true;
        }
    }
    false
}

/// Whether `element` is inside an element whose content is never rendered directly, like
/// `defs` or `clipPath`, or is not displayed.
fn is_never_rendered(element: &SVGElement) -> bool {
    element
        .upcast::<Node>()
        .inclusive_ancestors(crate::dom::iterators::ShadowIncluding::No)
        .filter_map(DomRoot::downcast::<SVGElement>)
        .any(|ancestor| {
            matches!(
                ancestor.upcast::<Element>().local_name(),
                &local_name!("defs") |
                    &local_name!("clipPath") |
                    &local_name!("mask") |
                    &local_name!("marker") |
                    &local_name!("pattern") |
                    &local_name!("symbol") |
                    &local_name!("linearGradient") |
                    &local_name!("radialGradient") |
                    &local_name!("filter")
            ) || is_display_none(&ancestor)
        })
}

/// Hit test the content of the outermost `svg` element `svg` at `point`, in the coordinates
/// of the viewport. Layout treats the `svg` element as one replaced box, so its hit test
/// result is refined here into the topmost graphics element under the point.
pub(crate) fn hit_test_svg(
    cx: &mut JSContext,
    svg: &SVGElement,
    point: Point,
) -> Option<DomRoot<SVGElement>> {
    if !is_outermost_svg(svg) {
        return None;
    }
    let candidates: Vec<DomRoot<SVGElement>> = svg
        .upcast::<Node>()
        .traverse_preorder(crate::dom::iterators::ShadowIncluding::No)
        .skip(1)
        .filter_map(DomRoot::downcast::<SVGElement>)
        .filter(|element| {
            shape_geometry_kind(element) ||
                matches!(
                    element.upcast::<Element>().local_name(),
                    &local_name!("text") | &local_name!("image") | &local_name!("use")
                )
        })
        .collect();
    // Later elements are painted on top of earlier ones.
    for element in candidates.iter().rev() {
        if is_never_rendered(element) {
            continue;
        }
        let transform = current_transform(cx, element, true);
        if element.upcast::<Element>().local_name() == &local_name!("use") {
            let Some(referenced) = referenced_element(cx.no_gc(), element) else {
                continue;
            };
            if !shape_geometry_kind(&referenced) {
                continue;
            }
            let x = length(
                cx,
                element,
                local_name!("x"),
                SVGLengthDirection::Horizontal,
            );
            let y = length(cx, element, local_name!("y"), SVGLengthDirection::Vertical);
            let transform = transform * Affine::translate((x, y)) * element_transform(&referenced);
            if transform.determinant() == 0.0 {
                continue;
            }
            if hits_element(cx, &referenced, transform.inverse() * point) {
                return Some(element.clone());
            }
            continue;
        }
        if transform.determinant() == 0.0 {
            continue;
        }
        if hits_element(cx, element, transform.inverse() * point) {
            return Some(element.clone());
        }
    }
    None
}
