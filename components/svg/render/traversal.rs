/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use kurbo::ParamCurveArclen;
use tiny_skia::{
    FillRule, LineCap, LineJoin, Mask, Paint, Path, PathBuilder, PathSegment, Pixmap, PixmapPaint,
    Rect, Stroke, StrokeDash, Transform,
};

use crate::model::document::{AspectAlign, MeetOrSlice, SvgTree, ViewportInfo};
use crate::model::element::{Shape, SvgNode, SvgTag};
use crate::model::geometry::PathCommand;
use crate::model::style::paint;
use crate::model::style::paint_servers::PaintServer;
use crate::model::style::{Display, Visibility};
use crate::model::transform::TransformOp;

// Bound the memory used by a single image, including temporary compositing layers.
const MAX_RASTER_BYTES: usize = 64 * 1024 * 1024;

pub fn render_svg_tree(tree: &SvgTree, width: f32, height: f32, scale: f32) -> Option<Pixmap> {
    if ![width, height, scale]
        .into_iter()
        .all(|value| value.is_finite() && value > 0.0)
    {
        return None;
    }

    let layer_cost = tree
        .root
        .children
        .iter()
        .map(|child| compositing_bytes_per_pixel(child))
        .max()
        .unwrap_or(0);
    let pixel_limit = MAX_RASTER_BYTES / 4usize.saturating_add(layer_cost);
    if pixel_limit == 0 {
        return None;
    }
    let requested_width = (f64::from(width) * f64::from(scale)).ceil();
    let requested_height = (f64::from(height) * f64::from(scale)).ceil();
    // Keep large viewports paintable at a lower resolution if the compositing stack
    // would otherwise require more than the raster budget.
    let reduction = (pixel_limit as f64 / (requested_width * requested_height))
        .sqrt()
        .min(1.0);
    let pixel_width = (requested_width * reduction)
        .max(1.0)
        .min(pixel_limit as f64) as u32;
    let pixel_height = (requested_height * reduction)
        .max(1.0)
        .min((pixel_limit / pixel_width as usize) as f64) as u32;
    let bytes = pixel_width as usize * pixel_height as usize * 4;

    let transform = if tree.viewport.view_box.is_some() {
        Transform::from_scale(pixel_width as f32 / width, pixel_height as f32 / height)
            .pre_concat(view_box_transform(&tree.viewport, width, height)?)
    } else {
        Transform::from_scale(
            pixel_width as f32 / tree.viewport.width.get(),
            pixel_height as f32 / tree.viewport.height.get(),
        )
    };
    let mut pixmap = Pixmap::new(pixel_width, pixel_height)?;
    // The outer SVG's CSS opacity and transform apply to its layout box.
    for child in &tree.root.children {
        render_node(&mut pixmap, child, transform, MAX_RASTER_BYTES - bytes);
    }
    Some(pixmap)
}

fn compositing_bytes_per_pixel(node: &SvgNode) -> usize {
    let opacity = node.style.opacity.get();
    if matches!(node.style.display, Display::None) || opacity == 0.0 {
        return 0;
    }
    let clip = node
        .viewport
        .as_ref()
        .is_some_and(|viewport| !viewport.viewport.overflow_visible);
    let own = if opacity != 1.0 || clip {
        4 + usize::from(clip)
    } else {
        0
    };
    own.saturating_add(
        node.children
            .iter()
            .map(|child| compositing_bytes_per_pixel(child))
            .max()
            .unwrap_or(0),
    )
}

fn render_node(pixmap: &mut Pixmap, node: &SvgNode, transform: Transform, layer_budget: usize) {
    let opacity = node.style.opacity.get();
    if matches!(node.style.display, Display::None) || opacity == 0.0 {
        return;
    }

    let mut transform = node.transforms.iter().fold(transform, |parent, op| {
        parent.pre_concat(match *op {
            TransformOp::Translate(x, y) => Transform::from_translate(x, y),
            TransformOp::Scale(x, y) => Transform::from_scale(x, y),
            TransformOp::Rotate(angle, x, y) => Transform::from_rotate_at(angle, x, y),
            TransformOp::SkewX(angle) => Transform::from_skew(angle.to_radians().tan(), 0.0),
            TransformOp::SkewY(angle) => Transform::from_skew(0.0, angle.to_radians().tan()),
            TransformOp::Matrix([a, b, c, d, e, f]) => Transform::from_row(a, b, c, d, e, f),
        })
    });
    let mut clip = None;
    if let Some(viewport) = &node.viewport {
        let width = viewport.viewport.width.get();
        let height = viewport.viewport.height.get();
        let Some(rect) = Rect::from_xywh(viewport.x.get(), viewport.y.get(), width, height) else {
            return;
        };
        if !viewport.viewport.overflow_visible {
            clip = PathBuilder::from_rect(rect).transform(transform);
        }
        let Some(view_box) = view_box_transform(&viewport.viewport, width, height) else {
            return;
        };
        transform = transform
            .pre_translate(viewport.x.get(), viewport.y.get())
            .pre_concat(view_box);
    }

    if opacity == 1.0 && clip.is_none() {
        render_contents(pixmap, node, transform, layer_budget);
        return;
    }

    let layer_bytes = pixmap.data().len();
    let clip_bytes = if clip.is_some() { layer_bytes / 4 } else { 0 };
    let Some(remaining) = layer_budget.checked_sub(layer_bytes + clip_bytes) else {
        return;
    };
    let Some(mut layer) = Pixmap::new(pixmap.width(), pixmap.height()) else {
        return;
    };
    render_contents(&mut layer, node, transform, remaining);
    let mask = clip.and_then(|path| {
        let mut mask = Mask::new(pixmap.width(), pixmap.height())?;
        mask.fill_path(&path, FillRule::Winding, true, Transform::identity());
        Some(mask)
    });
    pixmap.draw_pixmap(
        0,
        0,
        layer.as_ref(),
        &PixmapPaint {
            opacity,
            ..PixmapPaint::default()
        },
        Transform::identity(),
        mask.as_ref(),
    );
}

fn render_contents(pixmap: &mut Pixmap, node: &SvgNode, transform: Transform, layer_budget: usize) {
    if matches!(node.style.visibility, Visibility::Visible)
        && let SvgTag::Shape(shape) = &node.tag
        && let Some(path) = shape_path(shape)
    {
        if let Some(fill) = &node.style.fill
            && let Some(paint) = solid_paint(fill.paint_server.as_ref(), fill.opacity.get())
        {
            let rule = match fill.fill_rule {
                paint::FillRule::NonZero => FillRule::Winding,
                paint::FillRule::EvenOdd => FillRule::EvenOdd,
            };
            pixmap.fill_path(&path, &paint, rule, transform, None);
        }
        if let Some(stroke) = &node.style.stroke
            && stroke.width.get() > 0.0
            && let Some(paint) = solid_paint(stroke.paint_server.as_ref(), stroke.opacity.get())
        {
            let stroke = Stroke {
                width: stroke.width.get(),
                miter_limit: stroke.miter_limit,
                line_cap: match stroke.line_cap {
                    paint::LineCap::Butt => LineCap::Butt,
                    paint::LineCap::Round => LineCap::Round,
                    paint::LineCap::Square => LineCap::Square,
                },
                line_join: match stroke.line_join {
                    paint::LineJoin::Miter => LineJoin::Miter,
                    paint::LineJoin::MiterClip => LineJoin::MiterClip,
                    paint::LineJoin::Bevel => LineJoin::Bevel,
                    paint::LineJoin::Round | paint::LineJoin::Arcs => LineJoin::Round,
                },
                dash: stroke_dash(stroke, shape.path_length(), &path),
            };
            pixmap.stroke_path(&path, &paint, &stroke, transform, None);
        }
    }

    for child in &node.children {
        render_node(pixmap, child, transform, layer_budget);
    }
}

fn stroke_dash(
    stroke: &paint::StrokeParams,
    declared_length: Option<f32>,
    path: &Path,
) -> Option<StrokeDash> {
    let array = stroke.dash_array.as_ref()?;
    let scale = match declared_length {
        Some(0.0) => return None,
        Some(length) => path_length(path) / length,
        None => 1.0,
    };
    let mut intervals: Vec<_> = array.iter().map(|length| length * scale).collect();
    if intervals.len() % 2 != 0 {
        intervals.extend_from_within(..);
    }
    StrokeDash::new(intervals, stroke.dash_offset * scale)
}

fn path_length(path: &Path) -> f32 {
    let mut geometry = kurbo::BezPath::new();
    let point = |point: tiny_skia::Point| kurbo::Point::new(f64::from(point.x), f64::from(point.y));
    for segment in path.segments() {
        match segment {
            PathSegment::MoveTo(p) => geometry.move_to(point(p)),
            PathSegment::LineTo(p) => geometry.line_to(point(p)),
            PathSegment::QuadTo(control, p) => geometry.quad_to(point(control), point(p)),
            PathSegment::CubicTo(a, b, p) => geometry.curve_to(point(a), point(b), point(p)),
            PathSegment::Close => geometry.close_path(),
        }
    }
    geometry
        .segments()
        .map(|segment| segment.arclen(0.01))
        .sum::<f64>() as f32
}

fn solid_paint(server: Option<&PaintServer>, opacity: f32) -> Option<Paint<'static>> {
    let color = match server? {
        PaintServer::Solid(color) => color,
        PaintServer::Ref {
            fallback: Some(color),
            ..
        } => color,
        _ => return None,
    };
    let mut paint = Paint::default();
    paint.set_color_rgba8(
        color.red,
        color.green,
        color.blue,
        (color.alpha as f32 * opacity).round() as u8,
    );
    Some(paint)
}

fn shape_path(shape: &Shape) -> Option<Path> {
    let mut builder = PathBuilder::new();
    match shape {
        Shape::Rect(rect) => {
            let (x, y) = (rect.x.get(), rect.y.get());
            let (width, height) = (rect.width.get(), rect.height.get());
            if width <= 0.0 || height <= 0.0 {
                return None;
            }
            let bounds = Rect::from_xywh(x, y, width, height)?;
            let rx = rect
                .rx
                .or(rect.ry)
                .unwrap_or_default()
                .get()
                .min(width / 2.0);
            let ry = rect
                .ry
                .or(rect.rx)
                .unwrap_or_default()
                .get()
                .min(height / 2.0);
            if rx == 0.0 || ry == 0.0 {
                return Some(PathBuilder::from_rect(bounds));
            }
            let right = bounds.right();
            let bottom = bounds.bottom();
            // Cubic approximation of a quarter ellipse.
            let kx = rx * 0.552_284_8;
            let ky = ry * 0.552_284_8;
            builder.move_to(x + rx, y);
            builder.line_to(right - rx, y);
            builder.cubic_to(right - rx + kx, y, right, y + ry - ky, right, y + ry);
            builder.line_to(right, bottom - ry);
            builder.cubic_to(
                right,
                bottom - ry + ky,
                right - rx + kx,
                bottom,
                right - rx,
                bottom,
            );
            builder.line_to(x + rx, bottom);
            builder.cubic_to(x + rx - kx, bottom, x, bottom - ry + ky, x, bottom - ry);
            builder.line_to(x, y + ry);
            builder.cubic_to(x, y + ry - ky, x + rx - kx, y, x + rx, y);
            builder.close();
        },
        Shape::Circle(circle) => {
            if circle.r.get() <= 0.0 {
                return None;
            }
            return ellipse_path(
                circle.cx.get(),
                circle.cy.get(),
                circle.r.get(),
                circle.r.get(),
            );
        },
        Shape::Ellipse(ellipse) => {
            let rx = ellipse.rx.or(ellipse.ry).unwrap_or_default().get();
            let ry = ellipse.ry.or(ellipse.rx).unwrap_or_default().get();
            if rx <= 0.0 || ry <= 0.0 {
                return None;
            }
            return ellipse_path(ellipse.cx.get(), ellipse.cy.get(), rx, ry);
        },
        Shape::Line(line) => {
            builder.move_to(line.x1.get(), line.y1.get());
            builder.line_to(line.x2.get(), line.y2.get());
        },
        Shape::Polyline(crate::element::Polyline { points, .. })
        | Shape::Polygon(crate::element::Polygon { points, .. }) => {
            let (first, rest) = points.split_first()?;
            builder.move_to(first.x, first.y);
            for point in rest {
                builder.line_to(point.x, point.y);
            }
            if matches!(shape, Shape::Polygon(_)) {
                builder.close();
            }
        },
        Shape::Path(path) => {
            for command in &path.path.commands {
                match command {
                    PathCommand::MoveTo(p) => builder.move_to(p.x, p.y),
                    PathCommand::LineTo(p) => builder.line_to(p.x, p.y),
                    PathCommand::QuadTo(p1, p) => builder.quad_to(p1.x, p1.y, p.x, p.y),
                    PathCommand::CurveTo(p1, p2, p) => {
                        builder.cubic_to(p1.x, p1.y, p2.x, p2.y, p.x, p.y)
                    },
                    PathCommand::Close => builder.close(),
                }
            }
        },
    }
    builder.finish()
}

fn ellipse_path(cx: f32, cy: f32, rx: f32, ry: f32) -> Option<Path> {
    // SVG defines the equivalent path starting at the rightmost point, clockwise.
    let right = kurbo::Point::new(f64::from(cx + rx), f64::from(cy));
    let left = kurbo::Point::new(f64::from(cx - rx), f64::from(cy));
    let mut path = PathBuilder::new();
    path.move_to(right.x as f32, right.y as f32);
    for (from, to) in [(right, left), (left, right)] {
        let arc = kurbo::Arc::from_svg_arc(&kurbo::SvgArc {
            from,
            to,
            radii: kurbo::Vec2::new(f64::from(rx), f64::from(ry)),
            x_rotation: 0.0,
            large_arc: false,
            sweep: true,
        })?;
        // Use the same approximation as svgtypes uses for authored path arcs.
        arc.to_cubic_beziers(0.1, |a, b, p| {
            path.cubic_to(
                a.x as f32, a.y as f32, b.x as f32, b.y as f32, p.x as f32, p.y as f32,
            );
        });
    }
    path.close();
    path.finish()
}

fn view_box_transform(viewport: &ViewportInfo, width: f32, height: f32) -> Option<Transform> {
    let Some(view_box) = &viewport.view_box else {
        return Some(Transform::identity());
    };
    let (view_width, view_height) = (view_box.width.get(), view_box.height.get());
    if view_width == 0.0 || view_height == 0.0 {
        return None;
    }
    let (x, y) = (view_box.min_x.get(), view_box.min_y.get());
    let (sx, sy) = (width / view_width, height / view_height);
    let default_aspect = Default::default();
    let aspect = viewport.aspect_ratio.as_ref().unwrap_or(&default_aspect);
    let (ax, ay) = match aspect.align {
        AspectAlign::None => return Some(Transform::from_row(sx, 0.0, 0.0, sy, -x * sx, -y * sy)),
        AspectAlign::XMinYMin => (0.0, 0.0),
        AspectAlign::XMidYMin => (0.5, 0.0),
        AspectAlign::XMaxYMin => (1.0, 0.0),
        AspectAlign::XMinYMid => (0.0, 0.5),
        AspectAlign::XMidYMid => (0.5, 0.5),
        AspectAlign::XMaxYMid => (1.0, 0.5),
        AspectAlign::XMinYMax => (0.0, 1.0),
        AspectAlign::XMidYMax => (0.5, 1.0),
        AspectAlign::XMaxYMax => (1.0, 1.0),
    };
    let scale = match aspect.meet_or_slice {
        MeetOrSlice::Meet => sx.min(sy),
        MeetOrSlice::Slice => sx.max(sy),
    };
    let tx = (width - scale * view_width) * ax - scale * x;
    let ty = (height - scale * view_height) * ay - scale * y;
    Some(Transform::from_row(scale, 0.0, 0.0, scale, tx, ty))
}
