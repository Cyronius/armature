/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix, local_name, ns};
use js::rust::HandleObject;
use kurbo::{BezPath, PathEl, Point, Shape};
use svgtypes::{PathParser, PathSegment};

use crate::dom::bindings::codegen::Bindings::SVGPathElementBinding::{
    SVGPathDataOptions, SVGPathElementMethods, SVGPathSegment,
};
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::num::Finite;
use crate::dom::bindings::root::DomRoot;
use crate::dom::bindings::str::DOMString;
use crate::dom::document::Document;
use crate::dom::element::Element;
use crate::dom::node::Node;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::svg::svggeometryelement::SVGGeometryElement;

#[dom_struct]
pub(crate) struct SVGPathElement {
    svggeometryelement: SVGGeometryElement,
}

impl SVGPathElement {
    fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGPathElement {
        SVGPathElement {
            svggeometryelement: SVGGeometryElement::new_inherited(local_name, prefix, document),
        }
    }

    pub(crate) fn new(
        cx: &mut js::context::JSContext,
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
        proto: Option<HandleObject>,
    ) -> DomRoot<SVGPathElement> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(SVGPathElement::new_inherited(local_name, prefix, document)),
            document,
            proto,
        )
    }
}

impl VirtualMethods for SVGPathElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<SVGGeometryElement>() as &dyn VirtualMethods)
    }
}

fn segment_data(segment: PathSegment) -> SVGPathSegment {
    let values = match segment {
        PathSegment::MoveTo { x, y, .. } | PathSegment::LineTo { x, y, .. } => vec![x, y],
        PathSegment::HorizontalLineTo { x, .. } => vec![x],
        PathSegment::VerticalLineTo { y, .. } => vec![y],
        PathSegment::CurveTo {
            x1,
            y1,
            x2,
            y2,
            x,
            y,
            ..
        } => vec![x1, y1, x2, y2, x, y],
        PathSegment::SmoothCurveTo { x2, y2, x, y, .. } |
        PathSegment::Quadratic {
            x1: x2,
            y1: y2,
            x,
            y,
            ..
        } => vec![x2, y2, x, y],
        PathSegment::SmoothQuadratic { x, y, .. } => vec![x, y],
        PathSegment::EllipticalArc {
            rx,
            ry,
            x_axis_rotation,
            large_arc,
            sweep,
            x,
            y,
            ..
        } => vec![
            rx,
            ry,
            x_axis_rotation,
            f64::from(large_arc),
            f64::from(sweep),
            x,
            y,
        ],
        PathSegment::ClosePath { .. } => vec![],
    };
    SVGPathSegment {
        type_: DOMString::from((segment.command() as char).to_string()),
        values,
    }
}

fn parse_segments(data: &str) -> Vec<SVGPathSegment> {
    PathParser::from(data)
        .map_while(Result::ok)
        .map(segment_data)
        .collect()
}

fn arity(command: char) -> Option<usize> {
    Some(match command.to_ascii_uppercase() {
        'M' | 'L' | 'T' => 2,
        'H' | 'V' => 1,
        'C' => 6,
        'S' | 'Q' => 4,
        'A' => 7,
        'Z' => 0,
        _ => return None,
    })
}

fn serialize_segment(segment: &SVGPathSegment) -> Option<String> {
    let segment_type = segment.type_.str();
    let mut chars = segment_type.chars();
    let command = chars.next()?;
    if chars.next().is_some() ||
        segment.values.len() != arity(command)? ||
        segment.values.iter().any(|value| !value.is_finite())
    {
        return None;
    }
    let mut result = command.to_string();
    if command == 'Z' || command == 'z' {
        result = "Z".into();
    }
    for (index, value) in segment.values.iter().enumerate() {
        let value = if command.eq_ignore_ascii_case(&'a') && (index == 3 || index == 4) {
            f64::from(*value != 0.0)
        } else {
            *value
        };
        result.push(' ');
        result.push_str(&value.to_string());
    }
    Some(result)
}

fn normalized_segments(data: &str) -> Vec<SVGPathSegment> {
    let parsed = parse_segments(data);
    let canonical = parsed
        .iter()
        .filter_map(serialize_segment)
        .collect::<Vec<_>>()
        .join(" ");
    let Ok(path) = BezPath::from_svg(&canonical) else {
        return vec![];
    };
    let mut current = Point::ZERO;
    let mut subpath_start = Point::ZERO;
    path.elements()
        .iter()
        .map(|element| {
            let (command, values) = match element {
                PathEl::MoveTo(point) => {
                    current = *point;
                    subpath_start = *point;
                    ("M", vec![point.x, point.y])
                },
                PathEl::LineTo(point) => {
                    current = *point;
                    ("L", vec![point.x, point.y])
                },
                PathEl::CurveTo(a, b, point) => {
                    current = *point;
                    ("C", vec![a.x, a.y, b.x, b.y, point.x, point.y])
                },
                PathEl::QuadTo(control, point) => {
                    // SVG Paths normalization uses cubic curves for quadratic commands.
                    let a = current + (*control - current) * (2.0 / 3.0);
                    let b = *point + (*control - *point) * (2.0 / 3.0);
                    current = *point;
                    ("C", vec![a.x, a.y, b.x, b.y, point.x, point.y])
                },
                PathEl::ClosePath => {
                    current = subpath_start;
                    ("Z", vec![])
                },
            };
            SVGPathSegment {
                type_: DOMString::from(command),
                values,
            }
        })
        .collect()
}

impl SVGPathElementMethods<crate::DomTypeHolder> for SVGPathElement {
    fn GetPathData(&self, options: &SVGPathDataOptions) -> Vec<SVGPathSegment> {
        let data = self
            .upcast::<crate::dom::svg::svgelement::SVGElement>()
            .svg_attribute(&local_name!("d"))
            .unwrap_or_default();
        if options.normalize {
            normalized_segments(&data)
        } else {
            parse_segments(&data)
        }
    }

    fn SetPathData(&self, cx: &mut js::context::JSContext, path_data: Vec<SVGPathSegment>) {
        let mut segments = Vec::new();
        for segment in &path_data {
            let Some(serialized) = serialize_segment(segment) else {
                break;
            };
            if segments.is_empty() && !matches!(&*segment.type_.str(), "M" | "m") {
                break;
            }
            segments.push(serialized);
        }
        let element = self.upcast::<Element>();
        if segments.is_empty() {
            element.remove_attribute(cx, &ns!(), &local_name!("d"));
        } else {
            element.set_string_attribute(
                cx,
                &local_name!("d"),
                DOMString::from(segments.join(" ")),
            );
        }
    }

    fn GetPathSegmentAtLength(&self, distance: Finite<f64>) -> Option<SVGPathSegment> {
        let mut segments = self.GetPathData(&SVGPathDataOptions { normalize: false });
        if segments.is_empty() {
            return None;
        }
        if *distance <= 0.0 {
            return Some(segments.remove(0));
        }
        let mut data = String::new();
        let mut previous_length = 0.0;
        let mut last = None;
        for segment in segments {
            let serialized = serialize_segment(&segment)?;
            if !data.is_empty() {
                data.push(' ');
            }
            data.push_str(&serialized);
            let length = BezPath::from_svg(&data).ok()?.perimeter(1e-4);
            if length > previous_length && *distance <= length {
                return Some(segment);
            }
            previous_length = length;
            last = Some(segment);
        }
        last
    }
}
