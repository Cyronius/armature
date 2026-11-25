/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use servo_svg::style::paint::{FillRule, LineCap, LineJoin};
use servo_svg::style::paint_servers::PaintServer;
use servo_svg::style::{Display, FillParams, NodeStyle, StrokeParams, Visibility};
use servo_svg::units::{Length, Opacity};
use style::color::ColorSpace;
use style::properties::ComputedValues;
use style::values::computed::svg::{
    SVGLength, SVGOpacity, SVGPaint, SVGPaintKind, SVGStrokeDashArray, SVGWidth, StrokeLinecap,
    StrokeLinejoin,
};
use style::values::computed::{FillRule as CssFillRule, LengthPercentage};
use style::values::generics::svg::SVGPaintFallback;

use crate::svg::primitives::geometry::resolve;

pub(super) fn node_style(style: Option<&ComputedValues>, diagonal: f32) -> NodeStyle {
    let Some(style) = style else {
        return NodeStyle {
            fill: Some(FillParams {
                paint_server: Some(PaintServer::Solid(svgtypes::Color::black())),
                opacity: Opacity::ONE,
                fill_rule: FillRule::NonZero,
            }),
            ..NodeStyle::default()
        };
    };
    let svg = style.get_inherited_svg();
    let resolve_stroke =
        |length: &LengthPercentage| resolve(length, diagonal, style.effective_zoom);
    let fill = solid_paint(&svg.fill, style).map(|server| FillParams {
        paint_server: Some(server),
        opacity: opacity(svg.fill_opacity),
        fill_rule: match svg.fill_rule {
            CssFillRule::Nonzero => FillRule::NonZero,
            CssFillRule::Evenodd => FillRule::EvenOdd,
        },
    });
    let stroke = solid_paint(&svg.stroke, style).map(|server| StrokeParams {
        paint_server: Some(server),
        opacity: opacity(svg.stroke_opacity),
        width: Length::new(match &svg.stroke_width {
            SVGWidth::LengthPercentage(value) => resolve_stroke(&value.0),
            _ => 1.0,
        }),
        line_cap: match svg.stroke_linecap {
            StrokeLinecap::Butt => LineCap::Butt,
            StrokeLinecap::Round => LineCap::Round,
            StrokeLinecap::Square => LineCap::Square,
        },
        line_join: match svg.stroke_linejoin {
            StrokeLinejoin::Miter => LineJoin::Miter,
            StrokeLinejoin::Round => LineJoin::Round,
            StrokeLinejoin::Bevel => LineJoin::Bevel,
        },
        miter_limit: svg.stroke_miterlimit.0,
        dash_array: match &svg.stroke_dasharray {
            SVGStrokeDashArray::Values(values) if !values.is_empty() => Some(
                values
                    .iter()
                    .map(|value| resolve_stroke(&value.0))
                    .collect(),
            ),
            _ => None,
        },
        dash_offset: match &svg.stroke_dashoffset {
            SVGLength::LengthPercentage(value) => resolve_stroke(value),
            _ => 0.0,
        },
    });
    NodeStyle {
        display: if style.get_box().display.is_none() {
            Display::None
        } else {
            Display::Inline
        },
        visibility: if style.get_inherited_box().visibility
            == style::values::computed::Visibility::Visible
        {
            Visibility::Visible
        } else {
            Visibility::Hidden
        },
        fill,
        stroke,
        opacity: Opacity::new(*style.get_opacity()),
        ..NodeStyle::default()
    }
}

fn opacity(opacity: SVGOpacity) -> Opacity {
    match opacity {
        SVGOpacity::Opacity(value) => Opacity::new(value),
        _ => Opacity::ONE,
    }
}

fn solid_paint(paint: &SVGPaint, style: &ComputedValues) -> Option<PaintServer> {
    let color = match (&paint.kind, &paint.fallback) {
        (SVGPaintKind::Color(color), _) => color,
        (SVGPaintKind::PaintServer(_), SVGPaintFallback::Color(color)) => color,
        _ => return None,
    };
    let color = style.resolve_color(color).to_color_space(ColorSpace::Srgb);
    let to_byte = |component: f32| (component.clamp(0.0, 1.0) * 255.0).round() as u8;
    Some(PaintServer::Solid(svgtypes::Color::new_rgba(
        to_byte(color.components.0),
        to_byte(color.components.1),
        to_byte(color.components.2),
        to_byte(color.alpha),
    )))
}
