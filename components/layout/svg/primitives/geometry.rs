/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use script::layout_dom::ServoLayoutElement;
use servo_svg::element::{Circle, Ellipse, Line, Path, Polygon, Polyline, Rectangle, Shape};
use servo_svg::geometry::{PathCommand, PathData, Point};
use servo_svg::units::Length;
use style::properties::ComputedValues;
use style::values::computed::box_::Zoom;
use style::values::computed::svg::PathLength;
use style::values::computed::{
    Length as CssLength, LengthPercentage, NonNegativeLengthPercentage, Size,
};
use style::values::generics::length::GenericLengthPercentageOrAuto;
use style::values::specified::DProperty;
use style_traits::CssWriter;
use svgtypes::{PointsParser, SimplePathSegment, SimplifyingPathParser};
use web_atoms::{LocalName, local_name};

use crate::svg::primitives::attrs::{get_attr, parse_length_resolved};

pub(crate) fn diagonal(width: f32, height: f32) -> f32 {
    width.hypot(height) * std::f32::consts::FRAC_1_SQRT_2
}

pub(crate) fn build_shape(
    element: &ServoLayoutElement,
    tag: &LocalName,
    computed: Option<&ComputedValues>,
    vw: f32,
    vh: f32,
) -> Option<Shape> {
    let style = computed.map(|style| style.get_svg());
    let zoom = computed.map_or(Zoom::ONE, |style| style.effective_zoom);
    let fs = computed.map_or(16.0, |style| {
        zoom.unzoom(style.get_font().font_size.computed_size().px())
    });
    let attribute_length =
        |name, reference| attr_length(element, name, fs, reference).unwrap_or_default();
    let length = |value: Option<&LengthPercentage>, name, reference| {
        value.map_or_else(
            || attribute_length(name, reference),
            |value| Length::new(resolve(value, reference, zoom)),
        )
    };
    let radius = |value: Option<&GenericLengthPercentageOrAuto<NonNegativeLengthPercentage>>,
                  name,
                  reference| match value {
        Some(GenericLengthPercentageOrAuto::LengthPercentage(value)) => {
            Some(Length::new(resolve(&value.0, reference, zoom)))
        },
        Some(GenericLengthPercentageOrAuto::Auto) => None,
        None => attr_length(element, name, fs, reference).filter(|length| length.get() >= 0.0),
    };
    let path_length = match style.map(|style| &style.path_length) {
        Some(PathLength::Length(value)) => Some(zoom.unzoom(value.0.px())),
        Some(PathLength::None) => None,
        None => get_attr(element, &local_name!("pathLength"))
            .and_then(|value| value.trim().parse::<f32>().ok())
            .filter(|value| value.is_finite() && *value >= 0.0),
    };

    Some(match *tag {
        local_name!("rect") => {
            let position = computed.map(|style| style.get_position());
            Shape::Rect(Rectangle {
                x: length(style.map(|style| &style.x), local_name!("x"), vw),
                y: length(style.map(|style| &style.y), local_name!("y"), vh),
                width: size(
                    position.map(|style| &style.width),
                    element,
                    local_name!("width"),
                    fs,
                    vw,
                    zoom,
                )
                .unwrap_or_default(),
                height: size(
                    position.map(|style| &style.height),
                    element,
                    local_name!("height"),
                    fs,
                    vh,
                    zoom,
                )
                .unwrap_or_default(),
                rx: radius(style.map(|style| &style.rx), local_name!("rx"), vw),
                ry: radius(style.map(|style| &style.ry), local_name!("ry"), vh),
                path_length,
            })
        },
        local_name!("circle") => {
            let diagonal = diagonal(vw, vh);
            Shape::Circle(Circle {
                cx: length(style.map(|style| &style.cx), local_name!("cx"), vw),
                cy: length(style.map(|style| &style.cy), local_name!("cy"), vh),
                r: length(style.map(|style| &style.r.0), local_name!("r"), diagonal),
                path_length,
            })
        },
        local_name!("ellipse") => Shape::Ellipse(Ellipse {
            cx: length(style.map(|style| &style.cx), local_name!("cx"), vw),
            cy: length(style.map(|style| &style.cy), local_name!("cy"), vh),
            rx: radius(style.map(|style| &style.rx), local_name!("rx"), vw),
            ry: radius(style.map(|style| &style.ry), local_name!("ry"), vh),
            path_length,
        }),
        local_name!("line") => Shape::Line(Line {
            x1: attribute_length(local_name!("x1"), vw),
            y1: attribute_length(local_name!("y1"), vh),
            x2: attribute_length(local_name!("x2"), vw),
            y2: attribute_length(local_name!("y2"), vh),
            path_length,
        }),
        local_name!("polygon") | local_name!("polyline") => {
            let points =
                PointsParser::from(get_attr(element, &local_name!("points")).unwrap_or_default())
                    .map(|(x, y)| Point::new(x as f32, y as f32))
                    .collect();
            if *tag == local_name!("polygon") {
                Shape::Polygon(Polygon {
                    points,
                    path_length,
                })
            } else {
                Shape::Polyline(Polyline {
                    points,
                    path_length,
                })
            }
        },
        local_name!("path") => {
            let attribute = get_attr(element, &local_name!("d")).unwrap_or_default();
            let mut serialized = String::new();
            let data = match style.map(|style| &style.d) {
                Some(DProperty::Path(path)) => {
                    path.to_css(&mut CssWriter::new(&mut serialized), false)
                        .ok()?;
                    &serialized
                },
                // A malformed attribute can still draw its valid prefix.
                Some(DProperty::None)
                    if !style::values::specified::SVGPathData::parse_bytes(
                        attribute.as_bytes(),
                    )
                    .1 =>
                {
                    attribute
                },
                Some(DProperty::None) => "",
                None => attribute,
            };
            Shape::Path(Path {
                path: parse_path(data),
                path_length,
            })
        },
        _ => return None,
    })
}

pub(crate) fn size(
    value: Option<&Size>,
    element: &ServoLayoutElement,
    name: LocalName,
    fs: f32,
    reference: f32,
    zoom: Zoom,
) -> Option<Length> {
    match value {
        Some(Size::LengthPercentage(value)) => {
            Some(Length::new(resolve(&value.0, reference, zoom)))
        },
        Some(_) => None,
        None => attr_length(element, name, fs, reference).filter(|length| length.get() >= 0.0),
    }
}

fn attr_length(
    element: &ServoLayoutElement,
    name: LocalName,
    fs: f32,
    reference: f32,
) -> Option<Length> {
    parse_length_resolved(&name, element, fs, reference)
        .ok()
        .filter(|value| value.is_finite())
        .map(Length::new)
}

pub(crate) fn resolve(value: &LengthPercentage, reference: f32, zoom: Zoom) -> f32 {
    zoom.unzoom(value.resolve(CssLength::new(zoom.zoom(reference))).px())
}

fn parse_path(input: &str) -> PathData {
    let point = |x: f64, y: f64| Point::new(x as f32, y as f32);
    let commands = SimplifyingPathParser::from(input)
        .map_while(Result::ok)
        .map(|command| match command {
            SimplePathSegment::MoveTo { x, y } => PathCommand::MoveTo(point(x, y)),
            SimplePathSegment::LineTo { x, y } => PathCommand::LineTo(point(x, y)),
            SimplePathSegment::Quadratic { x1, y1, x, y } => {
                PathCommand::QuadTo(point(x1, y1), point(x, y))
            },
            SimplePathSegment::CurveTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => PathCommand::CurveTo(point(x1, y1), point(x2, y2), point(x, y)),
            SimplePathSegment::ClosePath => PathCommand::Close,
        })
        .collect();
    PathData { commands }
}
