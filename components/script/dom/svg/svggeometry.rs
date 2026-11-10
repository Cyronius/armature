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

use std::collections::HashSet;
use std::fmt::Write;

use app_units::Au;
use euclid::default::{Point2D as CSSPoint, Rect as CSSRect, Size2D as CSSSize};
use html5ever::{LocalName, local_name, ns};
use js::context::{JSContext, NoGC};
use kurbo::{
    Affine, BezPath, Cap, Ellipse, Join, Line, ParamCurve, ParamCurveArclen, ParamCurveDeriv,
    PathEl, PathSeg, Point, Rect, Shape, Stroke, StrokeOpts, stroke,
};
use layout_api::BoxAreaType;
use style::values::computed::transform::TransformBox;
use style::values::computed::{DProperty, Length as CSSLength};
use style::values::generics::basic_shape::GenericShapeCommand;
use style::values::specified::SVGPathData;
use style::values::specified::box_::Display;
use style::values::specified::svg_path::CoordPair;
use style_traits::CssWriter;

use crate::dom::bindings::codegen::Bindings::NodeBinding::NodeMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::element::Element;
use crate::dom::node::{Node, NodeTraits};
use crate::dom::svg::svganimationelement::{SVGAnimationElement, trim_one_trailing_semicolon};
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svgimageelement::SVGImageElement;
use crate::dom::svg::svglength::{resolve_length, resolve_value};
use crate::dom::svg::svgpointlist::parse_point_list;
use crate::dom::svg::svgtransformlist::animated_transform_values;
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
            Self::Path(path) => {
                let elements = path.elements();
                // A move-only path has a position even though kurbo gives it an
                // empty drawing bounds. An earlier move without a segment is
                // superseded by the last move.
                if elements
                    .iter()
                    .all(|element| matches!(element, PathEl::MoveTo(_) | PathEl::ClosePath))
                {
                    if let Some(PathEl::MoveTo(point)) = elements
                        .iter()
                        .rev()
                        .find(|element| matches!(element, PathEl::MoveTo(_)))
                    {
                        return Rect::new(point.x, point.y, point.x, point.y);
                    }
                }
                path.bounding_box()
            },
            Self::Ellipse(ellipse) => ellipse.bounding_box(),
            Self::Line(line) => line.bounding_box(),
        }
    }

    fn stroke_bounding_box(&self, element: &SVGElement) -> Option<Rect> {
        if computed_value(element, "stroke") == "none" {
            return None;
        }
        let width = computed_value(element, "stroke-width")
            .trim_end_matches("px")
            .parse::<f64>()
            .ok()
            .filter(|width| *width > 0.0 && width.is_finite())?;
        let cap = match computed_value(element, "stroke-linecap").as_str() {
            "round" => Cap::Round,
            "square" => Cap::Square,
            _ => Cap::Butt,
        };
        let join = match computed_value(element, "stroke-linejoin").as_str() {
            "round" => Join::Round,
            "bevel" => Join::Bevel,
            _ => Join::Miter,
        };
        let miter_limit = computed_value(element, "stroke-miterlimit")
            .parse::<f64>()
            .unwrap_or(4.0);
        let style = Stroke::new(width)
            .with_caps(cap)
            .with_join(join)
            .with_miter_limit(miter_limit);
        let path = match self {
            Self::Path(path) => path.clone(),
            Self::Ellipse(ellipse) => ellipse.to_path(ACCURACY),
            Self::Line(line) => {
                let mut path = BezPath::new();
                path.move_to(line.p0);
                path.line_to(line.p1);
                path
            },
        };
        let outline = stroke(
            path.elements().iter().copied(),
            &style,
            &StrokeOpts::default(),
            ACCURACY,
        );
        Some(if outline.elements().is_empty() {
            let bbox = self.bounding_box();
            if cap != Cap::Butt &&
                path.elements().iter().any(|element| {
                    matches!(
                        element,
                        PathEl::LineTo(_) | PathEl::QuadTo(_, _) | PathEl::CurveTo(_, _, _)
                    )
                }) &&
                bbox.width() == 0.0 &&
                bbox.height() == 0.0
            {
                let radius = width / 2.0;
                Rect::new(
                    bbox.x0 - radius,
                    bbox.y0 - radius,
                    bbox.x1 + radius,
                    bbox.y1 + radius,
                )
            } else {
                bbox
            }
        } else {
            outline.bounding_box()
        })
    }

    pub(crate) fn total_length(&self) -> f64 {
        let length = match self {
            Self::Path(path) => path.perimeter(ACCURACY),
            Self::Ellipse(ellipse) => ellipse.perimeter(ACCURACY),
            Self::Line(line) => line.perimeter(ACCURACY),
        };
        if length == 0.0 { 0.0 } else { length }
    }

    pub(crate) fn winding(&self, point: Point) -> i32 {
        match self {
            Self::Path(path) => path.winding(point),
            Self::Ellipse(ellipse) => ellipse.winding(point),
            Self::Line(_) => 0,
        }
    }

    /// The winding number is undefined on the outline; test it explicitly for fill queries.
    pub(crate) fn is_on_fill_boundary(&self, point: Point) -> bool {
        if !point.x.is_finite() || !point.y.is_finite() {
            return false;
        }
        match self {
            Self::Path(path) => path.segments().any(|segment| {
                kurbo::ParamCurveNearest::nearest(&segment, point, ACCURACY).distance_sq <= 1e-12
            }),
            Self::Ellipse(ellipse) => {
                let bounds = ellipse.bounding_box();
                let x = (2.0 * point.x - bounds.x0 - bounds.x1) / bounds.width();
                let y = (2.0 * point.y - bounds.y0 - bounds.y1) / bounds.height();
                (x * x + y * y - 1.0).abs() <= 1e-12
            },
            Self::Line(line) => {
                kurbo::ParamCurveNearest::nearest(line, point, ACCURACY).distance_sq <= 1e-12
            },
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
    pub(crate) fn point_at_length(&self, distance: f64) -> Option<Point> {
        let segments = self.segments();
        let Some(first) = segments.first() else {
            // A move-only path has a point but no segments. An empty shape has neither.
            return match self {
                Self::Path(path) => match path.elements().first() {
                    Some(PathEl::MoveTo(point)) => Some(*point),
                    _ => None,
                },
                _ => None,
            };
        };
        let mut remaining = distance.max(0.0);
        for segment in &segments {
            let length = segment.arclen(ACCURACY);
            if length == 0.0 {
                continue;
            }
            if remaining <= length {
                let t = segment.inv_arclen(remaining, ACCURACY);
                return Some(kurbo::ParamCurve::eval(segment, t));
            }
            remaining -= length;
        }
        if segments
            .iter()
            .all(|segment| segment.arclen(ACCURACY) == 0.0)
        {
            return Some(first.start());
        }
        let last = segments.last().unwrap_or(first);
        Some(kurbo::ParamCurve::end(last))
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

    /// Test the painted stroke shape, including caps, joins and dash gaps. The stroke paint
    /// itself does not affect `isPointInStroke()`.
    pub(crate) fn is_point_in_stroke(
        &self,
        cx: &mut JSContext,
        element: &SVGElement,
        point: Point,
    ) -> bool {
        let width = computed_value(element, "stroke-width")
            .trim_end_matches("px")
            .parse::<f64>()
            .unwrap_or(1.0);
        if !width.is_finite() || width <= 0.0 || !point.x.is_finite() || !point.y.is_finite() {
            return false;
        }
        let cap = match computed_value(element, "stroke-linecap").as_str() {
            "round" => Cap::Round,
            "square" => Cap::Square,
            _ => Cap::Butt,
        };
        let join = match computed_value(element, "stroke-linejoin").as_str() {
            "round" => Join::Round,
            "bevel" => Join::Bevel,
            _ => Join::Miter,
        };
        let miter_limit = computed_value(element, "stroke-miterlimit")
            .parse::<f64>()
            .unwrap_or(4.0);
        let mut style = Stroke::new(width)
            .with_caps(cap)
            .with_join(join)
            .with_miter_limit(miter_limit);

        let dash_array = computed_value(element, "stroke-dasharray");
        if dash_array != "none" {
            let dashes: Option<Vec<f64>> = dash_array
                .split(|character: char| character == ',' || character.is_ascii_whitespace())
                .filter(|part| !part.is_empty())
                .map(|part| part.trim_end_matches("px").parse::<f64>().ok())
                .collect();
            if let Some(dashes) = dashes.filter(|dashes| {
                !dashes.is_empty() &&
                    dashes.iter().all(|dash| dash.is_finite() && *dash >= 0.0) &&
                    dashes.iter().any(|dash| *dash > 0.0)
            }) {
                let author_length = computed_value(element, "path-length")
                    .trim_end_matches("px")
                    .parse::<f64>()
                    .ok()
                    .filter(|length| *length > 0.0 && length.is_finite());
                let scale = author_length.map_or(1.0, |length| self.total_length() / length);
                let offset = computed_value(element, "stroke-dashoffset")
                    .trim_end_matches("px")
                    .parse::<f64>()
                    .unwrap_or(0.0);
                style =
                    style.with_dashes(offset * scale, dashes.into_iter().map(|dash| dash * scale));
            }
        }

        let path = match self {
            Self::Path(path) => path.clone(),
            Self::Ellipse(ellipse) => ellipse.to_path(ACCURACY),
            Self::Line(line) => {
                let mut path = BezPath::new();
                path.move_to(line.p0);
                path.line_to(line.p1);
                path
            },
        };
        let transform = if computed_value(element, "vector-effect") == "non-scaling-stroke" {
            current_transform(cx, element, true)
        } else {
            Affine::IDENTITY
        };
        let path = transform * path;
        let point = transform * point;
        stroke(
            path.elements().iter().copied(),
            &style,
            &StrokeOpts::default(),
            ACCURACY,
        )
        .winding(point) !=
            0
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

/// The used width and height of an SVG image's positioning rectangle.
/// <https://svgwg.org/svg2-draft/embedded.html#Placement>
pub(crate) fn image_size(cx: &mut JSContext, element: &SVGElement) -> (f64, f64) {
    use SVGLengthDirection::{Horizontal, Vertical};
    let specified = |cx: &mut JSContext, name: &str, direction| {
        let value = SVGLengthValue::parse(&computed_value(element, name))?;
        Some(resolve_value(cx, element, value, direction) as f64)
    };
    let width = specified(cx, "width", Horizontal);
    let height = specified(cx, "height", Vertical);
    let natural = element
        .downcast::<SVGImageElement>()
        .and_then(|image| image.natural_dimensions())
        .unwrap_or_default();
    let natural_width = natural.width.map(f64::from);
    let natural_height = natural.height.map(f64::from);
    let ratio = natural
        .ratio
        .map(f64::from)
        .filter(|ratio| *ratio > 0.0 && ratio.is_finite());
    match (width, height) {
        (Some(width), Some(height)) => (width, height),
        (Some(width), None) => (
            width,
            ratio.map_or(natural_height.unwrap_or(150.0), |r| width / r),
        ),
        (None, Some(height)) => (
            ratio.map_or(natural_width.unwrap_or(300.0), |r| height * r),
            height,
        ),
        (None, None) => match (natural_width, natural_height, ratio) {
            (Some(width), Some(height), _) => (width, height),
            (Some(width), None, Some(ratio)) => (width, width / ratio),
            (None, Some(height), Some(ratio)) => (height * ratio, height),
            (Some(width), None, None) => (width, 150.0),
            (None, Some(height), None) => (300.0, height),
            (None, None, Some(ratio)) => {
                let width = 300.0_f64.min(150.0 * ratio);
                (width, width / ratio)
            },
            (None, None, None) => (300.0, 150.0),
        },
    }
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
            let specified_rx = element
                .svg_attribute(&local_name!("rx"))
                .and_then(|value| SVGLengthValue::parse(&value))
                .filter(|value| value.value >= 0.0);
            let specified_ry = element
                .svg_attribute(&local_name!("ry"))
                .and_then(|value| SVGLengthValue::parse(&value))
                .filter(|value| value.value >= 0.0);
            let rx = if specified_rx.is_some() {
                length(cx, element, local_name!("rx"), Horizontal)
            } else {
                length(cx, element, local_name!("ry"), Vertical)
            };
            let ry = if specified_ry.is_some() {
                length(cx, element, local_name!("ry"), Vertical)
            } else {
                rx
            };
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
            let attribute = element.svg_attribute(&local_name!("d"));
            let data = match (computed_path_data(element), attribute) {
                // An invalid XML path still renders its valid prefix. Stylo rejects the
                // presentation hint as a whole, so use the original data for recovery.
                (Some(computed), Some(attribute))
                    if computed.is_empty() && !SVGPathData::parse_bytes(attribute.as_bytes()).1 =>
                {
                    attribute
                },
                (Some(computed), _) => computed,
                (None, Some(attribute)) => attribute,
                (None, None) => String::new(),
            };
            let data = super::svgpathbearing::expand_bearing(&data).unwrap_or(data);
            SVGShapeGeometry::Path(parse_path_data(&data))
        },
        _ => return None,
    })
}

/// The CSS `d` property supplies the rendered path, including for `<mpath>` targets.
/// Keep DOM path data APIs on the XML attribute's base value.
fn computed_path_data(element: &SVGElement) -> Option<String> {
    let style = element.upcast::<Element>().style()?;
    let DProperty::Path(path) = &style.get_svg().d else {
        return Some(String::new());
    };

    // CSS serialization rounds path numbers to a few decimal places. Geometry needs the
    // computed coordinates themselves so a style query cannot change the measured path.
    let normalized = path.normalize(false);
    let mut data = String::new();
    for command in normalized.commands() {
        use GenericShapeCommand::*;
        if !data.is_empty() {
            data.push(' ');
        }
        match *command {
            Move { point } => {
                let point = CoordPair::from(point);
                write!(data, "M {} {}", point.x, point.y).unwrap();
            },
            Line { point } => {
                let point = CoordPair::from(point);
                write!(data, "L {} {}", point.x, point.y).unwrap();
            },
            HLine { x } => write!(data, "H {}", f32::from(x)).unwrap(),
            VLine { y } => write!(data, "V {}", f32::from(y)).unwrap(),
            CubicCurve {
                point,
                control1,
                control2,
            } => {
                let point = CoordPair::from(point);
                let control1 = CoordPair::from(control1);
                let control2 = CoordPair::from(control2);
                write!(
                    data,
                    "C {} {} {} {} {} {}",
                    control1.x, control1.y, control2.x, control2.y, point.x, point.y
                )
                .unwrap();
            },
            QuadCurve { point, control1 } => {
                let point = CoordPair::from(point);
                let control1 = CoordPair::from(control1);
                write!(
                    data,
                    "Q {} {} {} {}",
                    control1.x, control1.y, point.x, point.y
                )
                .unwrap();
            },
            SmoothCubic { point, control2 } => {
                let point = CoordPair::from(point);
                let control2 = CoordPair::from(control2);
                write!(
                    data,
                    "S {} {} {} {}",
                    control2.x, control2.y, point.x, point.y
                )
                .unwrap();
            },
            SmoothQuad { point } => {
                let point = CoordPair::from(point);
                write!(data, "T {} {}", point.x, point.y).unwrap();
            },
            Arc {
                point,
                radii,
                arc_sweep,
                arc_size,
                rotate,
            } => {
                let point = CoordPair::from(point);
                let ry = radii.ry.as_ref().copied().unwrap_or(radii.rx);
                write!(
                    data,
                    "A {} {} {} {} {} {} {}",
                    radii.rx, ry, rotate, arc_size as u8, arc_sweep as u8, point.x, point.y
                )
                .unwrap();
            },
            Close => data.push('Z'),
        }
    }
    Some(data)
}

/// Parse path data. When the data has an error, the path up to the error is used.
///
/// <https://svgwg.org/svg2-draft/paths.html#PathDataErrorHandling>
fn parse_path_data(data: &str) -> BezPath {
    let (parsed, valid) = SVGPathData::parse_bytes(data.as_bytes());
    if valid {
        if let Ok(path) = BezPath::from_svg(data) {
            return path;
        }
    }

    // Stylo stops at the first invalid parameter and keeps every complete
    // segment before it, including repeated coordinate pairs within a command.
    let mut prefix = String::new();
    let _ = parsed.to_css(&mut CssWriter::new(&mut prefix), false);
    BezPath::from_svg(&prefix).unwrap_or_else(|_| BezPath::new())
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

fn motion_point(value: &str) -> Option<Point> {
    let numbers = parse_numbers(value)?;
    let [x, y] = numbers.as_slice() else {
        return None;
    };
    Some(Point::new(*x, *y))
}

fn motion_path(cx: &mut JSContext, animation: &SVGAnimationElement) -> Option<SVGShapeGeometry> {
    for child in animation.upcast::<Node>().child_elements() {
        if child.local_name() == &local_name!("mpath") {
            let mpath = child.downcast::<SVGElement>()?;
            let referenced = referenced_element(cx.no_gc(), mpath)?;
            return shape_geometry(cx, &referenced);
        }
    }
    animation
        .upcast::<SVGElement>()
        .svg_attribute(&local_name!("path"))
        .map(|data| SVGShapeGeometry::Path(parse_path_data(&data)))
}

fn motion_tangent(segment: &PathSeg, t: f64) -> f64 {
    let derivative = match segment {
        PathSeg::Line(line) => line.deriv().eval(t),
        PathSeg::Quad(quad) => quad.deriv().eval(t),
        PathSeg::Cubic(cubic) => cubic.deriv().eval(t),
    };
    derivative.y.atan2(derivative.x)
}

fn point_and_tangent_at_length(geometry: &SVGShapeGeometry, distance: f64) -> Option<(Point, f64)> {
    let segments = geometry.segments();
    if segments.is_empty() {
        return Some((geometry.point_at_length(0.0)?, 0.0));
    }
    let mut remaining = distance.max(0.0);
    for segment in &segments {
        let length = segment.arclen(ACCURACY);
        if length == 0.0 {
            continue;
        }
        if remaining <= length {
            let t = segment.inv_arclen(remaining, ACCURACY);
            return Some((segment.eval(t), motion_tangent(segment, t)));
        }
        remaining -= length;
    }
    let Some(last) = segments
        .iter()
        .rev()
        .find(|segment| segment.arclen(ACCURACY) > 0.0)
    else {
        return Some((geometry.point_at_length(0.0)?, 0.0));
    };
    Some((last.end(), motion_tangent(last, 1.0)))
}

fn discrete_path_point(
    path: &BezPath,
    animation: &SVGAnimationElement,
    progress: f64,
) -> Option<(Point, f64)> {
    let mut points = Vec::new();
    let mut subpath_start = Point::ZERO;
    for element in path.elements() {
        let point = match element {
            PathEl::MoveTo(point) => {
                subpath_start = *point;
                *point
            },
            PathEl::LineTo(point) | PathEl::QuadTo(_, point) | PathEl::CurveTo(_, _, point) => {
                *point
            },
            PathEl::ClosePath => subpath_start,
        };
        points.push(point);
    }
    let index = if points.len() == 1 {
        0
    } else {
        let (index, fraction) = animation.sample_value_interval_f64(points.len(), progress)?;
        index + usize::from(fraction >= 1.0)
    };
    let point = *points.get(index)?;
    let direction = if index + 1 < points.len() {
        points[index + 1] - point
    } else if index > 0 {
        point - points[index - 1]
    } else {
        kurbo::Vec2::ZERO
    };
    Some((point, direction.y.atan2(direction.x)))
}

/// Sample the supplemental transform of active animateMotion children.
pub(crate) fn motion_transform(cx: &mut JSContext, element: &SVGElement) -> Option<Affine> {
    let mut result = Affine::IDENTITY;
    let mut sampled = false;
    for child in element.upcast::<Node>().child_elements() {
        if child.local_name() != &local_name!("animateMotion") {
            continue;
        }
        let Some(animation) = child.downcast::<SVGAnimationElement>() else {
            continue;
        };
        let Some((progress, iteration)) = animation.sample_iteration_f64() else {
            continue;
        };
        let attributes = animation.upcast::<SVGElement>();
        let mode = attributes
            .svg_attribute(&local_name!("calcMode"))
            .unwrap_or_else(|| "paced".into());
        let path = motion_path(cx, animation);
        let (mut position, tangent, by_only) = if let Some(path) = path {
            if mode == "discrete" &&
                attributes
                    .svg_attribute(&local_name!("keyPoints"))
                    .is_none() &&
                let SVGShapeGeometry::Path(ref bezier) = path
            {
                let Some((position, tangent)) = discrete_path_point(bezier, animation, progress)
                else {
                    continue;
                };
                (position, tangent, false)
            } else {
                let total = path.total_length();
                let key_points = attributes.svg_attribute(&local_name!("keyPoints"));
                let distance = if let Some(key_points) = key_points {
                    let points: Option<Vec<f64>> = trim_one_trailing_semicolon(&key_points)
                        .split(';')
                        .map(|point| point.trim().parse::<f64>().ok())
                        .collect();
                    let Some(points) = points.filter(|points| {
                        points.len() >= 2 && points.iter().all(|point| (0.0..=1.0).contains(point))
                    }) else {
                        continue;
                    };
                    let Some((index, fraction)) =
                        animation.sample_value_interval_f64(points.len(), progress)
                    else {
                        continue;
                    };
                    (points[index] + (points[index + 1] - points[index]) * fraction) * total
                } else if mode == "paced" || total == 0.0 {
                    progress * total
                } else {
                    let segments = path.segments();
                    let mut distances = vec![0.0];
                    for segment in segments {
                        distances.push(
                            distances.last().copied().unwrap_or(0.0) + segment.arclen(ACCURACY),
                        );
                    }
                    if distances.len() == 1 {
                        0.0
                    } else {
                        let Some((index, fraction)) =
                            animation.sample_value_interval_f64(distances.len(), progress)
                        else {
                            continue;
                        };
                        distances[index] + (distances[index + 1] - distances[index]) * fraction
                    }
                };
                let Some((position, tangent)) = point_and_tangent_at_length(&path, distance) else {
                    continue;
                };
                (position, tangent, false)
            }
        } else {
            let values = attributes.svg_attribute(&local_name!("values"));
            let points = if let Some(values) = values.as_deref() {
                trim_one_trailing_semicolon(values)
                    .split(';')
                    .map(motion_point)
                    .collect::<Option<Vec<_>>>()
            } else {
                let from = attributes.svg_attribute(&local_name!("from"));
                let to = attributes.svg_attribute(&local_name!("to"));
                let by = attributes.svg_attribute(&local_name!("by"));
                match (
                    from.as_deref().and_then(motion_point),
                    to.as_deref().and_then(motion_point),
                    by.as_deref().and_then(motion_point),
                ) {
                    (Some(from), Some(to), _) => Some(vec![from, to]),
                    (Some(from), None, Some(by)) => Some(vec![from, from + by.to_vec2()]),
                    (None, None, Some(by)) => Some(vec![Point::ZERO, by]),
                    (None, Some(to), _) => Some(vec![Point::ZERO, to]),
                    _ => None,
                }
            };
            let Some(points) = points.filter(|points| !points.is_empty()) else {
                continue;
            };
            let by_only = values.is_none() &&
                attributes.svg_attribute(&local_name!("from")).is_none() &&
                attributes.svg_attribute(&local_name!("to")).is_none() &&
                attributes.svg_attribute(&local_name!("by")).is_some();
            if points.len() == 1 {
                (points[0], 0.0, by_only)
            } else {
                let Some((index, fraction)) =
                    animation.sample_value_interval_f64(points.len(), progress)
                else {
                    continue;
                };
                let from = points[index];
                let to = points[index + 1];
                let position = from + (to - from) * fraction;
                let delta = to - from;
                (position, delta.y.atan2(delta.x), by_only)
            }
        };
        if iteration > 0 &&
            attributes
                .svg_attribute(&local_name!("accumulate"))
                .as_deref() ==
                Some("sum")
        {
            let final_position = if let Some(path) = motion_path(cx, animation) {
                path.point_at_length(path.total_length())
            } else if let Some(values) = attributes.svg_attribute(&local_name!("values")) {
                trim_one_trailing_semicolon(&values)
                    .split(';')
                    .last()
                    .and_then(motion_point)
            } else {
                attributes
                    .svg_attribute(&local_name!("to"))
                    .as_deref()
                    .and_then(motion_point)
                    .or_else(|| {
                        attributes
                            .svg_attribute(&local_name!("by"))
                            .as_deref()
                            .and_then(motion_point)
                    })
            };
            if let Some(final_position) = final_position {
                position += final_position.to_vec2() * f64::from(iteration);
            }
        }
        let angle = match attributes.svg_attribute(&local_name!("rotate")).as_deref() {
            Some("auto") => tangent,
            Some("auto-reverse") => tangent + std::f64::consts::PI,
            Some(value) => value.trim().parse::<f64>().unwrap_or(0.0).to_radians(),
            None => 0.0,
        };
        let motion = Affine::translate((position.x, position.y)) * Affine::rotate(angle);
        sampled = true;
        if by_only ||
            attributes
                .svg_attribute(&local_name!("additive"))
                .as_deref() ==
                Some("sum")
        {
            result *= motion;
        } else {
            result = motion;
        }
    }
    sampled.then_some(result)
}

/// The transform that the `transform` attribute and motion animations give `element`.
pub(crate) fn element_transform(cx: &mut JSContext, element: &SVGElement) -> Affine {
    let base = animated_transform_values(cx, element, &local_name!("transform"))
        .iter()
        .fold(Affine::IDENTITY, |matrix, item| matrix * item.matrix);
    base * motion_transform(cx, element).unwrap_or(Affine::IDENTITY)
}

/// Whether `element` does not take part in rendering because of its `display`.
///
/// Check CSS display when style data is available, including stylesheet rules. Some SVG
/// descendants have no style data, so also check their presentation and style attributes.
fn is_display_none(element: &SVGElement) -> bool {
    let element = element.upcast::<Element>();
    if element
        .style()
        .is_some_and(|style| style.get_box().display == Display::None)
    {
        return true;
    }
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
    bounding_box_with_options(cx, element, true, false, false, false)
}

pub(crate) fn bounding_box_with_options(
    cx: &mut JSContext,
    element: &SVGElement,
    fill: bool,
    stroke: bool,
    markers: bool,
    clipped: bool,
) -> Option<Rect> {
    bounding_box_impl(
        cx,
        element,
        fill,
        stroke,
        markers,
        clipped,
        &mut HashSet::new(),
    )
}

fn bounding_box_impl(
    cx: &mut JSContext,
    element: &SVGElement,
    fill: bool,
    stroke: bool,
    markers: bool,
    clipped: bool,
    active_clips: &mut HashSet<String>,
) -> Option<Rect> {
    if let Some(geometry) = shape_geometry(cx, element) {
        if matches!(&geometry, SVGShapeGeometry::Path(path) if path.elements().is_empty()) {
            return None;
        }
        let mut bbox = match (fill, stroke) {
            (false, false) => None,
            (true, false) => Some(geometry.bounding_box()),
            (false, true) => geometry.stroke_bounding_box(element),
            (true, true) => Some(geometry.stroke_bounding_box(element).map_or_else(
                || geometry.bounding_box(),
                |stroke| stroke.union(geometry.bounding_box()),
            )),
        };
        if markers {
            if let Some(marker_bbox) = marker_bounding_box(cx, element, &geometry, active_clips) {
                let decorated = geometry.bounding_box().union(marker_bbox);
                bbox = Some(bbox.map_or(decorated, |bbox| bbox.union(decorated)));
            }
        }
        return bbox.map(|bbox| {
            if clipped {
                clip_bbox(cx, element, bbox, geometry.bounding_box(), active_clips)
            } else {
                bbox
            }
        });
    }
    let local_name = element.upcast::<Element>().local_name().clone();
    let is_use = local_name == local_name!("use");
    let has_css_clip = matches!(
        local_name,
        local_name!("image") | local_name!("foreignObject")
    );
    let bbox = match local_name {
        local_name!("image") => {
            let image = element.downcast::<SVGImageElement>()?;
            let has_source = image
                .upcast::<Element>()
                .get_attribute_string_value(&local_name!("href"))
                .or_else(|| {
                    image
                        .upcast::<Element>()
                        .get_attribute_string_value_with_namespace(
                            &ns!(xlink),
                            &local_name!("href"),
                        )
                })
                .is_some_and(|href| !href.is_empty());
            if !has_source {
                return None;
            }
            let x = length(
                cx,
                element,
                local_name!("x"),
                SVGLengthDirection::Horizontal,
            );
            let y = length(cx, element, local_name!("y"), SVGLengthDirection::Vertical);
            let (width, height) = image_size(cx, element);
            if width <= 0.0 || height <= 0.0 {
                return None;
            }
            Some(Rect::new(x, y, x + width, y + height))
        },
        local_name!("foreignObject") => {
            use SVGLengthDirection::{Horizontal, Vertical};
            let width = length(cx, element, local_name!("width"), Horizontal);
            let height = length(cx, element, local_name!("height"), Vertical);
            if width <= 0.0 || height <= 0.0 {
                return None;
            }
            let x = length(cx, element, local_name!("x"), Horizontal);
            let y = length(cx, element, local_name!("y"), Vertical);
            Some(Rect::new(x, y, x + width, y + height))
        },
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
            let mut bbox = bounding_box_impl(
                cx,
                &referenced,
                fill,
                stroke,
                markers,
                clipped,
                active_clips,
            )?;
            if clipped {
                // The use element's clipping path is in its pre-translation user space.
                let fill_bbox =
                    bounding_box_impl(cx, &referenced, true, false, false, false, active_clips)?;
                bbox = clip_bbox(cx, element, bbox, fill_bbox, active_clips);
            }
            if bbox == Rect::ZERO {
                return Some(Rect::ZERO);
            }
            Some(
                (Affine::translate((x, y)) * element_transform(cx, &referenced))
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
                if is_display_none(&child) || !child.passes_conditional_processing() {
                    continue;
                }
                let Some(bbox) =
                    bounding_box_impl(cx, &child, fill, stroke, markers, clipped, active_clips)
                else {
                    continue;
                };
                if bbox == Rect::ZERO {
                    continue;
                }
                let bbox = element_transform(cx, &child).transform_rect_bbox(bbox);
                union = Some(match union {
                    Some(union) => union.union(bbox),
                    None => bbox,
                });
            }
            Some(union.unwrap_or_default())
        },
        _ => None,
    };
    bbox.map(|bbox| {
        if clipped && !is_use {
            let fill_bbox = if fill && !stroke {
                bbox
            } else {
                bounding_box_impl(cx, element, true, false, false, false, active_clips)
                    .unwrap_or(bbox)
            };
            let bbox = clip_bbox(cx, element, bbox, fill_bbox, active_clips);
            if has_css_clip {
                css_clip_bbox(element, bbox, fill_bbox)
            } else {
                bbox
            }
        } else {
            bbox
        }
    })
}

/// The marker content at each marked vertex, expressed in the shape's user space.
fn marker_bounding_box(
    cx: &mut JSContext,
    element: &SVGElement,
    geometry: &SVGShapeGeometry,
    active_clips: &mut HashSet<String>,
) -> Option<Rect> {
    let subpaths = match geometry {
        SVGShapeGeometry::Path(path) => {
            let mut subpaths = Vec::new();
            let mut points = Vec::new();
            for segment in path.elements() {
                match segment {
                    PathEl::MoveTo(point) => {
                        if !points.is_empty() {
                            subpaths.push(std::mem::take(&mut points));
                        }
                        points.push(*point);
                    },
                    PathEl::LineTo(point) |
                    PathEl::QuadTo(_, point) |
                    PathEl::CurveTo(_, _, point) => points.push(*point),
                    PathEl::ClosePath => {
                        if let Some(first) = points.first().copied() {
                            points.push(first);
                        }
                    },
                }
            }
            if !points.is_empty() {
                subpaths.push(points);
            }
            subpaths
        },
        SVGShapeGeometry::Line(line) => vec![vec![line.p0, line.p1]],
        SVGShapeGeometry::Ellipse(_) => return None,
    };
    let mut union: Option<Rect> = None;
    for (property, kind) in [("marker-start", 0), ("marker-mid", 1), ("marker-end", 2)] {
        let value = computed_value(element, property);
        let value = if value.is_empty() || value == "none" {
            element
                .upcast::<Element>()
                .get_attribute_string_value(&LocalName::from(property))
                .map(|value| value.to_string())
                .unwrap_or(value)
        } else {
            value
        };
        let Some(url) = value
            .trim()
            .strip_prefix("url(")
            .and_then(|value| value.strip_suffix(')'))
        else {
            continue;
        };
        let url = url.trim().trim_matches(['\'', '"']);
        let Some((_, id)) = url.rsplit_once('#') else {
            continue;
        };
        let document = element.upcast::<Node>().owner_doc();
        let Some(referenced) = document.get_element_by_id(cx.no_gc(), &stylo_atoms::Atom::from(id))
        else {
            continue;
        };
        let Some(marker) = DomRoot::downcast::<SVGElement>(referenced) else {
            continue;
        };
        if marker.upcast::<Element>().local_name() != &local_name!("marker") {
            continue;
        }
        let children: Vec<DomRoot<SVGElement>> = marker
            .upcast::<Node>()
            .children()
            .filter_map(DomRoot::downcast::<SVGElement>)
            .collect();
        let mut content: Option<Rect> = None;
        for child in children {
            if is_display_none(&child) || !child.passes_conditional_processing() {
                continue;
            }
            let Some(bbox) = bounding_box_impl(cx, &child, true, false, false, false, active_clips)
            else {
                continue;
            };
            let bbox = element_transform(cx, &child).transform_rect_bbox(bbox);
            content = Some(content.map_or(bbox, |content| content.union(bbox)));
        }
        let Some(content) = content else {
            continue;
        };
        let marker_width = resolve_length(
            cx,
            &marker,
            &local_name!("markerWidth"),
            SVGLengthDirection::Horizontal,
            SVGLengthValue::number(3.0),
        ) as f64;
        let marker_height = resolve_length(
            cx,
            &marker,
            &local_name!("markerHeight"),
            SVGLengthDirection::Vertical,
            SVGLengthValue::number(3.0),
        ) as f64;
        if marker_width <= 0.0 || marker_height <= 0.0 {
            continue;
        }
        let view_transform = if let Some(view_box) = parse_view_box(cx, &marker) {
            let scale = (marker_width / view_box.width()).min(marker_height / view_box.height());
            let offset_x = (marker_width - view_box.width() * scale) / 2.0;
            let offset_y = (marker_height - view_box.height() * scale) / 2.0;
            Affine::translate((offset_x, offset_y)) *
                Affine::scale(scale) *
                Affine::translate((-view_box.x0, -view_box.y0))
        } else {
            Affine::IDENTITY
        };
        let ref_x = length(
            cx,
            &marker,
            local_name!("refX"),
            SVGLengthDirection::Horizontal,
        );
        let ref_y = length(
            cx,
            &marker,
            local_name!("refY"),
            SVGLengthDirection::Vertical,
        );
        let reference = view_transform * Point::new(ref_x, ref_y);
        let stroke_width = if marker.svg_attribute(&local_name!("markerUnits")).as_deref() ==
            Some("userSpaceOnUse")
        {
            1.0
        } else {
            computed_value(element, "stroke-width")
                .trim_end_matches("px")
                .parse::<f64>()
                .unwrap_or(1.0)
        };
        let orient = marker
            .svg_attribute(&local_name!("orient"))
            .unwrap_or_default();
        for points in &subpaths {
            for (index, point) in points.iter().enumerate() {
                if (kind == 0 && index != 0) ||
                    (kind == 1 && (index == 0 || index + 1 == points.len())) ||
                    (kind == 2 && index + 1 != points.len())
                {
                    continue;
                }
                let angle = if orient == "auto" || orient == "auto-start-reverse" {
                    let previous = points.get(index.saturating_sub(1)).unwrap_or(point);
                    let next = points
                        .get((index + 1).min(points.len() - 1))
                        .unwrap_or(point);
                    let angle = (next.y - previous.y).atan2(next.x - previous.x);
                    if orient == "auto-start-reverse" && kind == 0 {
                        angle + std::f64::consts::PI
                    } else {
                        angle
                    }
                } else {
                    orient
                        .trim_end_matches("deg")
                        .parse::<f64>()
                        .unwrap_or(0.0)
                        .to_radians()
                };
                let transform = Affine::translate((point.x, point.y)) *
                    Affine::rotate(angle) *
                    Affine::scale(stroke_width) *
                    Affine::translate((-reference.x, -reference.y)) *
                    view_transform;
                let bbox = transform.transform_rect_bbox(content);
                union = Some(union.map_or(bbox, |union| union.union(bbox)));
            }
        }
    }
    union
}

/// The bounding box of the clipping silhouette in the target's user space.
fn clip_path_bbox(
    cx: &mut JSContext,
    element: &SVGElement,
    fill_bbox: Rect,
    active_clips: &mut HashSet<String>,
) -> Option<Rect> {
    let value = computed_value(element, "clip-path");
    let value = if value.is_empty() {
        element
            .upcast::<Element>()
            .get_attribute_string_value(&local_name!("clip-path"))?
            .to_string()
    } else {
        value
    };
    if value.trim() == "none" {
        return None;
    }
    let url = value.trim().strip_prefix("url(")?.strip_suffix(')')?.trim();
    let url = url.trim_matches(['\'', '"']);
    let id = url.rsplit_once('#')?.1;
    if !active_clips.insert(id.to_string()) {
        return Some(Rect::ZERO);
    }
    let result = (|| {
        let document = element.upcast::<Node>().owner_doc();
        let referenced = document.get_element_by_id(cx.no_gc(), &stylo_atoms::Atom::from(id))?;
        let clip_path = DomRoot::downcast::<SVGElement>(referenced)?;
        if clip_path.upcast::<Element>().local_name() != &local_name!("clipPath") {
            return None;
        }
        let children: Vec<DomRoot<SVGElement>> = clip_path
            .upcast::<Node>()
            .children()
            .filter_map(DomRoot::downcast::<SVGElement>)
            .collect();
        let mut union: Option<Rect> = None;
        for child in children {
            if is_display_none(&child) || !child.passes_conditional_processing() {
                continue;
            }
            let Some(bbox) = bounding_box_impl(cx, &child, true, false, false, true, active_clips)
            else {
                continue;
            };
            if bbox == Rect::ZERO {
                continue;
            }
            let bbox = element_transform(cx, &child).transform_rect_bbox(bbox);
            union = Some(union.map_or(bbox, |union| union.union(bbox)));
        }
        let mut bbox = union.unwrap_or(Rect::ZERO);
        if bbox != Rect::ZERO {
            bbox = clip_bbox(cx, &clip_path, bbox, bbox, active_clips);
            bbox = element_transform(cx, &clip_path).transform_rect_bbox(bbox);
        }
        if bbox == Rect::ZERO {
            return Some(bbox);
        }
        if clip_path
            .svg_attribute(&local_name!("clipPathUnits"))
            .as_deref() ==
            Some("objectBoundingBox")
        {
            bbox = (Affine::translate((fill_bbox.x0, fill_bbox.y0)) *
                Affine::scale_non_uniform(fill_bbox.width(), fill_bbox.height()))
            .transform_rect_bbox(bbox);
        }
        Some(bbox)
    })();
    active_clips.remove(id);
    Some(result.unwrap_or(Rect::ZERO))
}

fn clip_bbox(
    cx: &mut JSContext,
    element: &SVGElement,
    bbox: Rect,
    fill_bbox: Rect,
    active_clips: &mut HashSet<String>,
) -> Rect {
    let Some(clip) = clip_path_bbox(cx, element, fill_bbox, active_clips) else {
        return bbox;
    };
    if clip == Rect::ZERO {
        return Rect::ZERO;
    }
    intersect_bbox(bbox, clip)
}

fn intersect_bbox(bbox: Rect, clip: Rect) -> Rect {
    if bbox.x0 >= clip.x1 || bbox.x1 <= clip.x0 || bbox.y0 >= clip.y1 || bbox.y1 <= clip.y0 {
        return Rect::ZERO;
    }
    bbox.intersect(clip)
}

/// The legacy CSS `clip` rectangle is positioned relative to the element's box.
fn css_clip_bbox(element: &SVGElement, bbox: Rect, fill_bbox: Rect) -> Rect {
    let value = computed_value(element, "clip");
    let value = if value.is_empty() || value == "auto" {
        element
            .upcast::<Element>()
            .get_attribute_string_value(&local_name!("clip"))
            .map(|value| value.to_string())
            .unwrap_or(value)
    } else {
        value
    };
    let Some(arguments) = value
        .strip_prefix("rect(")
        .and_then(|value| value.strip_suffix(')'))
    else {
        return bbox;
    };
    let numbers: Vec<f64> = arguments
        .split([',', ' '])
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.trim_end_matches("px").parse().ok())
        .collect();
    if numbers.len() != 4 {
        return bbox;
    }
    let clip = Rect::new(
        fill_bbox.x0 + numbers[3],
        fill_bbox.y0 + numbers[0],
        fill_bbox.x0 + numbers[1],
        fill_bbox.y0 + numbers[2],
    );
    intersect_bbox(bbox, clip)
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

/// The animated `viewBox`, if its width and height establish a view box.
pub(crate) fn parse_view_box(cx: &mut JSContext, element: &SVGElement) -> Option<Rect> {
    let [x, y, width, height] = element.view_box(cx).animated_rect(cx);
    (width > 0.0 && height > 0.0).then(|| Rect::new(x, y, x + width, y + height))
}

/// The transform from the user space inside the view box of `element` to its viewport of
/// size `width` by `height`.
///
/// <https://svgwg.org/svg2-draft/coords.html#ComputingAViewportsTransform>
pub(crate) fn view_box_transform(
    cx: &mut JSContext,
    element: &SVGElement,
    width: f64,
    height: f64,
) -> Affine {
    let Some(view_box) = parse_view_box(cx, element) else {
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
            .owner_window()
            .box_area_query(cx, svg.upcast(), BoxAreaType::Content, true)
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
    let view_box = view_box_transform(cx, svg, width, height);
    if is_outermost_svg(svg) {
        return view_box;
    }
    let x = length(cx, svg, local_name!("x"), SVGLengthDirection::Horizontal);
    let y = length(cx, svg, local_name!("y"), SVGLengthDirection::Vertical);
    nested_svg_css_transform(svg, width, height) * Affine::translate((x, y)) * view_box
}

/// CSS transforms of nested SVG viewports are not in layout's spatial tree: layout
/// rasterizes the whole outer SVG subtree as one box.
fn nested_svg_css_transform(svg: &SVGElement, width: f64, height: f64) -> Affine {
    let Some(style) = svg.upcast::<Element>().style() else {
        return Affine::IDENTITY;
    };
    let box_style = style.get_box();
    let padding = style.get_padding();
    let basis = Au::from_f64_px(width);
    let left = padding.padding_left.0.to_used_value(basis).to_f64_px();
    let right = padding.padding_right.0.to_used_value(basis).to_f64_px();
    let top = padding.padding_top.0.to_used_value(basis).to_f64_px();
    let bottom = padding.padding_bottom.0.to_used_value(basis).to_f64_px();
    let border = style.get_border();
    let border_width = |width: Au, style: style::values::specified::BorderStyle| {
        if style.none_or_hidden() {
            0.0
        } else {
            width.to_f64_px()
        }
    };
    let left = left + border_width(border.border_left_width.0, border.border_left_style);
    let right = right + border_width(border.border_right_width.0, border.border_right_style);
    let top = top + border_width(border.border_top_width.0, border.border_top_style);
    let bottom = bottom + border_width(border.border_bottom_width.0, border.border_bottom_style);

    let content_box = matches!(
        box_style.transform_box,
        TransformBox::ContentBox | TransformBox::FillBox
    );
    let reference_width = width + if content_box { 0.0 } else { left + right };
    let reference_height = height + if content_box { 0.0 } else { top + bottom };
    let reference_box = CSSRect::new(
        CSSPoint::new(CSSLength::new(0.0), CSSLength::new(0.0)),
        CSSSize::new(
            CSSLength::new(reference_width as f32),
            CSSLength::new(reference_height as f32),
        ),
    );
    let Ok((matrix, false)) = box_style
        .transform
        .to_transform_3d_matrix_f64(Some(&reference_box))
    else {
        return Affine::IDENTITY;
    };
    let origin = &box_style.transform_origin;
    let origin_x = origin
        .horizontal
        .to_used_value(Au::from_f64_px(reference_width))
        .to_f64_px();
    let origin_y = origin
        .vertical
        .to_used_value(Au::from_f64_px(reference_height))
        .to_f64_px();
    let transform = Affine::translate((origin_x, origin_y)) *
        Affine::new([
            matrix.m11, matrix.m12, matrix.m21, matrix.m22, matrix.m41, matrix.m42,
        ]) *
        Affine::translate((-origin_x, -origin_y));

    // The viewport starts after the border and padding. With a border-box
    // reference, that offset rotates along with the CSS transform, but does
    // not scale with it.
    let (offset_x, offset_y) = if content_box {
        (left, top)
    } else {
        let angle = transform.as_coeffs()[1].atan2(transform.as_coeffs()[0]);
        (
            left * angle.cos() - top * angle.sin(),
            top * angle.cos() + left * angle.sin(),
        )
    };
    Affine::translate((offset_x, offset_y)) * transform
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
        element_transform(cx, element)
    };
    if is_svg && is_outermost_svg(element) {
        if to_screen {
            transform = screen_transform(cx, element) * transform;
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
                return view_box_transform(cx, &ancestor, width, height) * transform;
            }
            transform = svg_element_transform(cx, &ancestor) * transform;
            if is_outermost_svg(&ancestor) {
                return screen_transform(cx, &ancestor) * transform;
            }
            continue;
        }
        transform = element_transform(cx, &ancestor) * transform;
    }
    transform
}

/// The transformation from the outermost `svg` viewport to the document viewport.
fn screen_transform(cx: &mut JSContext, svg: &SVGElement) -> Affine {
    let node = svg.upcast::<Node>();
    let Some(content_box) =
        node.owner_window()
            .box_area_query(cx, node, BoxAreaType::Content, true)
    else {
        return Affine::IDENTITY;
    };
    let css_transform = node
        .owner_window()
        .box_transform_query_without_reflow(node)
        .map(Affine::new)
        .unwrap_or(Affine::IDENTITY);
    css_transform *
        Affine::translate((
            content_box.origin.x.to_f64_px(),
            content_box.origin.y.to_f64_px(),
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
pub(crate) fn transform_to_user_space_of(
    cx: &mut JSContext,
    element: &SVGElement,
    ancestor: &SVGElement,
) -> Affine {
    let mut transform = element_transform(cx, element);
    for node in element.upcast::<Node>().ancestors() {
        if &*node == ancestor.upcast::<Node>() {
            break;
        }
        if let Some(svg_element) = node.downcast::<SVGElement>() {
            transform = element_transform(cx, svg_element) * transform;
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
    let pointer_events = computed_value(element, "pointer-events").to_ascii_lowercase();
    if pointer_events == "none" {
        return false;
    }
    if pointer_events == "bounding-box" {
        return bounding_box(cx, element).is_some_and(|bbox| {
            point.x >= bbox.x0 && point.x <= bbox.x1 && point.y >= bbox.y0 && point.y <= bbox.y1
        });
    }
    // For the other values, reject distant points before querying paint and visibility.
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
    if !in_box && winding == 0 && distance > MAXIMUM_HALF_STROKE_WIDTH {
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
        "all" => (false, true, true),
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
pub(crate) fn is_never_rendered(element: &SVGElement) -> bool {
    !element.passes_conditional_processing() ||
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
                    &local_name!("text") |
                        &local_name!("tspan") |
                        &local_name!("textPath") |
                        &local_name!("image") |
                        &local_name!("use") |
                        &local_name!("g") |
                        &local_name!("a") |
                        &local_name!("switch") |
                        &local_name!("svg")
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
            let transform =
                transform * Affine::translate((x, y)) * element_transform(cx, &referenced);
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
