// Copyright 2018 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

use strict_num::NonZeroPositiveF32;
use svgtypes::Length;
use tiny_skia_path::Point;

use super::converter;
use super::svgtree::{AId, EId, SvgNode};
use crate::{
    ApproxEqUlps, ApproxZeroUlps, ClipPath, Fill, Group, Node, NonZeroRect, Path, Size, Transform,
    ViewBox,
};

// Similar to `tiny_skia_path::PathSegment`, but without the `QuadTo`.
#[derive(Copy, Clone, Debug)]
enum Segment {
    MoveTo(Point),
    LineTo(Point),
    CubicTo(Point, Point, Point),
    Close,
}

pub(crate) fn is_valid(node: SvgNode) -> bool {
    // `marker-*` attributes cannot be set on shapes inside a `clipPath`.
    if node
        .ancestors()
        .any(|n| n.tag_name() == Some(EId::ClipPath))
    {
        return false;
    }

    let start = node.find_attribute::<SvgNode>(AId::MarkerStart);
    let mid = node.find_attribute::<SvgNode>(AId::MarkerMid);
    let end = node.find_attribute::<SvgNode>(AId::MarkerEnd);
    start.is_some() || mid.is_some() || end.is_some()
}

pub(crate) fn convert(
    node: SvgNode,
    path: &tiny_skia_path::Path,
    state: &converter::State,
    cache: &mut converter::Cache,
    parent: &mut Group,
) {
    let list = [
        (AId::MarkerStart, MarkerKind::Start),
        (AId::MarkerMid, MarkerKind::Middle),
        (AId::MarkerEnd, MarkerKind::End),
    ];

    for (aid, kind) in &list {
        let mut marker = None;
        if let Some(link) = node.find_attribute::<SvgNode>(*aid) {
            if link.tag_name() == Some(EId::Marker) {
                marker = Some(link);
            }
        }

        if let Some(marker) = marker {
            // TODO: move to svgtree
            // Check for recursive marker.
            if state.parent_markers.contains(&marker) {
                log::warn!("Recursive marker detected: {}", marker.element_id());
                continue;
            }

            resolve(node, path, marker, *kind, state, cache, parent);
        }
    }
}

#[derive(Clone, Copy)]
enum MarkerKind {
    Start,
    Middle,
    End,
}

enum MarkerOrientation {
    Auto,
    AutoStartReverse,
    Angle(f32),
}

fn resolve(
    shape_node: SvgNode,
    path: &tiny_skia_path::Path,
    marker_node: SvgNode,
    marker_kind: MarkerKind,
    state: &converter::State,
    cache: &mut converter::Cache,
    parent: &mut Group,
) -> Option<()> {
    let stroke_scale = stroke_scale(shape_node, marker_node, state)?.get();

    let r = convert_rect(marker_node, state)?;

    let view_box = marker_node.parse_viewbox().map(|vb| ViewBox {
        rect: vb,
        aspect: marker_node
            .attribute(AId::PreserveAspectRatio)
            .unwrap_or_default(),
    });

    let has_overflow = {
        let overflow = marker_node.attribute(AId::Overflow);
        // `overflow` is `hidden` by default.
        overflow.is_none() || overflow == Some("hidden") || overflow == Some("scroll")
    };

    let clip_path = if has_overflow {
        let clip_rect = if let Some(vbox) = view_box {
            vbox.rect
        } else {
            r.size().to_non_zero_rect(0.0, 0.0)
        };

        let mut clip_path = ClipPath::empty(cache.gen_clip_path_id());

        let mut path = Path::new_simple(Arc::new(tiny_skia_path::PathBuilder::from_rect(
            clip_rect.to_rect(),
        )))?;
        path.fill = Some(Fill::default());

        clip_path.root.children.push(Node::Path(Box::new(path)));

        Some(Arc::new(clip_path))
    } else {
        None
    };

    // TODO: avoid allocation
    let mut segments: Vec<Segment> = Vec::with_capacity(path.len());
    let mut prev = Point::zero();
    let mut prev_move = Point::zero();
    for seg in path.segments() {
        match seg {
            tiny_skia_path::PathSegment::MoveTo(p) => {
                segments.push(Segment::MoveTo(p));
                prev = p;
                prev_move = p;
            }
            tiny_skia_path::PathSegment::LineTo(p) => {
                segments.push(Segment::LineTo(p));
                prev = p;
            }
            tiny_skia_path::PathSegment::QuadTo(p1, p) => {
                let (p1, p2, p) = quad_to_curve(prev, p1, p);
                segments.push(Segment::CubicTo(p1, p2, p));
                prev = p;
            }
            tiny_skia_path::PathSegment::CubicTo(p1, p2, p) => {
                segments.push(Segment::CubicTo(p1, p2, p));
                prev = p;
            }
            tiny_skia_path::PathSegment::Close => {
                segments.push(Segment::Close);
                prev = prev_move;
            }
        }
    }

    // Armature: Arc commands are approximated by multiple cubic segments for painting,
    // but only the end of the original arc is a marker vertex.
    let arc_vertices = if matches!(marker_kind, MarkerKind::Middle) &&
        shape_node.tag_name() == Some(EId::Path)
    {
        shape_node
            .attribute::<&str>(AId::D)
            .and_then(|data| arc_vertex_mask(data, segments.len()))
    } else {
        None
    };

    let draw_marker = |p: tiny_skia_path::Point, idx: usize| {
        if arc_vertices.as_ref().is_some_and(|vertices| !vertices[idx]) {
            return;
        }
        let mut ts = Transform::from_translate(p.x, p.y);

        let angle = match convert_orientation(marker_node) {
            MarkerOrientation::AutoStartReverse if idx == 0 => {
                (calc_vertex_angle(&segments, idx) + 180.0) % 360.0
            }
            MarkerOrientation::Auto | MarkerOrientation::AutoStartReverse => {
                calc_vertex_angle(&segments, idx)
            }
            MarkerOrientation::Angle(angle) => angle,
        };

        if !angle.approx_zero_ulps(4) {
            ts = ts.pre_rotate(angle);
        }

        if let Some(vbox) = view_box {
            let size = Size::from_wh(r.width() * stroke_scale, r.height() * stroke_scale).unwrap();
            let vbox_ts = vbox.to_transform(size);
            let (sx, sy) = vbox_ts.get_scale();
            ts = ts.pre_scale(sx, sy);
        } else {
            ts = ts.pre_scale(stroke_scale, stroke_scale);
        }

        ts = ts.pre_translate(-r.x(), -r.y());

        // TODO: do not create a group when no clipPath
        let mut g = Group {
            transform: ts,
            abs_transform: parent.abs_transform.pre_concat(ts),
            clip_path: clip_path.clone(),
            ..Group::empty()
        };

        let mut marker_state = state.clone();
        marker_state.parent_markers.push(marker_node);
        converter::convert_children(marker_node, &marker_state, cache, &mut g);
        g.calculate_bounding_boxes();

        if g.has_children() {
            parent.children.push(Node::Group(Box::new(g)));
        }
    };

    draw_markers(&segments, marker_kind, draw_marker);

    Some(())
}

fn arc_vertex_mask(data: &str, segment_count: usize) -> Option<Vec<bool>> {
    let mut vertices = vec![true; segment_count];
    let mut current = (0.0, 0.0);
    let mut subpath_start = current;
    let mut index = 0usize;
    let mut closed = false;

    for segment in svgtypes::PathParser::from(data) {
        let Ok(segment) = segment else {
            break;
        };
        if closed &&
            !matches!(
                segment,
                svgtypes::PathSegment::MoveTo { .. } | svgtypes::PathSegment::ClosePath { .. }
            )
        {
            // SimplifyingPathParser inserts a moveto after a closepath.
            index += 1;
        }

        match segment {
            svgtypes::PathSegment::MoveTo { abs, x, y } => {
                current = absolute_endpoint(current, abs, x, y);
                subpath_start = current;
                index += 1;
                closed = false;
            },
            svgtypes::PathSegment::LineTo { abs, x, y } |
            svgtypes::PathSegment::CurveTo { abs, x, y, .. } |
            svgtypes::PathSegment::SmoothCurveTo { abs, x, y, .. } |
            svgtypes::PathSegment::Quadratic { abs, x, y, .. } |
            svgtypes::PathSegment::SmoothQuadratic { abs, x, y } => {
                current = absolute_endpoint(current, abs, x, y);
                index += 1;
                closed = false;
            },
            svgtypes::PathSegment::HorizontalLineTo { abs, x } => {
                current.0 = if abs { x } else { current.0 + x };
                index += 1;
                closed = false;
            },
            svgtypes::PathSegment::VerticalLineTo { abs, y } => {
                current.1 = if abs { y } else { current.1 + y };
                index += 1;
                closed = false;
            },
            svgtypes::PathSegment::EllipticalArc {
                abs,
                rx,
                ry,
                x_axis_rotation,
                large_arc,
                sweep,
                x,
                y,
            } => {
                let end = absolute_endpoint(current, abs, x, y);
                let svg_arc = kurbo::SvgArc {
                    from: kurbo::Point::new(current.0, current.1),
                    to: kurbo::Point::new(end.0, end.1),
                    radii: kurbo::Vec2::new(rx, ry),
                    x_rotation: x_axis_rotation.to_radians(),
                    large_arc,
                    sweep,
                };
                let count = match kurbo::Arc::from_svg_arc(&svg_arc) {
                    Some(arc) => {
                        let mut count = 0;
                        arc.to_cubic_beziers(0.1, |_, _, _| count += 1);
                        count
                    },
                    None => 1,
                };
                if index + count > segment_count {
                    return None;
                }
                for vertex in &mut vertices[index..index + count.saturating_sub(1)] {
                    *vertex = false;
                }
                index += count;
                current = end;
                closed = false;
            },
            svgtypes::PathSegment::ClosePath { .. } => {
                if !closed {
                    index += 1;
                }
                current = subpath_start;
                closed = true;
            },
        }
        if index > segment_count {
            return None;
        }
    }

    (index == segment_count).then_some(vertices)
}

fn absolute_endpoint(current: (f64, f64), abs: bool, x: f64, y: f64) -> (f64, f64) {
    if abs { (x, y) } else { (current.0 + x, current.1 + y) }
}

fn stroke_scale(
    path_node: SvgNode,
    marker_node: SvgNode,
    state: &converter::State,
) -> Option<NonZeroPositiveF32> {
    match marker_node.attribute(AId::MarkerUnits) {
        Some("userSpaceOnUse") => NonZeroPositiveF32::new(1.0),
        _ => path_node.resolve_valid_length(AId::StrokeWidth, state, 1.0),
    }
}

fn draw_markers<P>(path: &[Segment], kind: MarkerKind, mut draw_marker: P)
where
    P: FnMut(tiny_skia_path::Point, usize),
{
    match kind {
        MarkerKind::Start => {
            if let Some(Segment::MoveTo(p)) = path.first().cloned() {
                draw_marker(p, 0);
            }
        }
        MarkerKind::Middle => {
            let total = path.len() - 1;
            let mut i = 1;
            while i < total {
                // Armature: When a drawn segment already returns to the subpath start,
                // closepath does not introduce another vertex at that position.
                if matches!(path[i], Segment::LineTo(_) | Segment::CubicTo(_, _, _)) &&
                    matches!(path[i + 1], Segment::Close)
                {
                    let end = match path[i] {
                        Segment::LineTo(point) | Segment::CubicTo(_, _, point) => point,
                        _ => unreachable!(),
                    };
                    if end == get_subpath_start(path, i) {
                        i += 1;
                        continue;
                    }
                }
                let p = match path[i] {
                    Segment::MoveTo(p) => p,
                    Segment::LineTo(p) => p,
                    Segment::CubicTo(_, _, p) => p,
                    Segment::Close => get_subpath_start(path, i),
                };

                draw_marker(p, i);

                i += 1;
            }
        }
        MarkerKind::End => {
            let idx = path.len() - 1;
            match path.last().cloned() {
                Some(Segment::LineTo(p)) => {
                    draw_marker(p, idx);
                }
                Some(Segment::CubicTo(_, _, p)) => {
                    draw_marker(p, idx);
                }
                Some(Segment::Close) => {
                    let p = get_subpath_start(path, idx);
                    draw_marker(p, idx);
                }
                _ => {}
            }
        }
    }
}

fn calc_vertex_angle(path: &[Segment], idx: usize) -> f32 {
    // Armature: The first and last vertices of a closed subpath share the same
    // position and bisect its incoming and outgoing directions.
    if let Segment::MoveTo(_) = path[idx] {
        if let Some(offset) = path[idx + 1..]
            .iter()
            .position(|segment| matches!(segment, Segment::MoveTo(_) | Segment::Close))
        {
            let end = idx + offset + 1;
            if matches!(path[end], Segment::Close) {
                return closed_subpath_angle(path, idx, end);
            }
        }
    } else if matches!(path[idx], Segment::Close) {
        if let Some(start) = path[..idx]
            .iter()
            .rposition(|segment| matches!(segment, Segment::MoveTo(_)))
        {
            return closed_subpath_angle(path, start, idx);
        }
    }

    if idx == 0 {
        // First segment.

        debug_assert!(path.len() > 1);

        let seg1 = path[0];
        let seg2 = path[1];

        match (seg1, seg2) {
            (Segment::MoveTo(pm), Segment::LineTo(p)) => calc_line_angle(pm.x, pm.y, p.x, p.y),
            (Segment::MoveTo(pm), Segment::CubicTo(p1, _, p)) => {
                if pm.x.approx_eq_ulps(&p1.x, 4) && pm.y.approx_eq_ulps(&p1.y, 4) {
                    calc_line_angle(pm.x, pm.y, p.x, p.y)
                } else {
                    calc_line_angle(pm.x, pm.y, p1.x, p1.y)
                }
            }
            _ => 0.0,
        }
    } else if idx == path.len() - 1 {
        // Last segment.

        let seg1 = path[idx - 1];
        let seg2 = path[idx];

        match (seg1, seg2) {
            (_, Segment::MoveTo(_)) => 0.0, // unreachable
            (_, Segment::LineTo(p)) => {
                let prev = get_prev_vertex(path, idx);
                calc_line_angle(prev.x, prev.y, p.x, p.y)
            }
            (_, Segment::CubicTo(p1, p2, p)) => {
                if p2.x.approx_eq_ulps(&p.x, 4) && p2.y.approx_eq_ulps(&p.y, 4) {
                    calc_line_angle(p1.x, p1.y, p.x, p.y)
                } else {
                    calc_line_angle(p2.x, p2.y, p.x, p.y)
                }
            }
            (Segment::LineTo(p), Segment::Close) => {
                let next = get_subpath_start(path, idx);
                calc_line_angle(p.x, p.y, next.x, next.y)
            }
            (Segment::CubicTo(_, p2, p), Segment::Close) => {
                let prev = get_prev_vertex(path, idx);
                let next = get_subpath_start(path, idx);
                calc_curves_angle(
                    prev.x, prev.y, p2.x, p2.y, p.x, p.y, next.x, next.y, next.x, next.y,
                )
            }
            (_, Segment::Close) => 0.0,
        }
    } else {
        // Middle segments.

        let seg1 = path[idx];
        let seg2 = path[idx + 1];

        // TODO: Not sure if there is a better way.
        match (seg1, seg2) {
            (Segment::MoveTo(pm), Segment::LineTo(p)) => calc_line_angle(pm.x, pm.y, p.x, p.y),
            (Segment::MoveTo(pm), Segment::CubicTo(p1, _, _)) => {
                calc_line_angle(pm.x, pm.y, p1.x, p1.y)
            }
            (Segment::LineTo(p1), Segment::LineTo(p2)) => {
                let prev = get_prev_vertex(path, idx);
                calc_angle(prev.x, prev.y, p1.x, p1.y, p1.x, p1.y, p2.x, p2.y)
            }
            (Segment::CubicTo(_, c1_p2, c1_p), Segment::CubicTo(c2_p1, _, c2_p)) => {
                let prev = get_prev_vertex(path, idx);
                calc_curves_angle(
                    prev.x, prev.y, c1_p2.x, c1_p2.y, c1_p.x, c1_p.y, c2_p1.x, c2_p1.y, c2_p.x,
                    c2_p.y,
                )
            }
            (Segment::LineTo(pl), Segment::CubicTo(p1, _, p)) => {
                let prev = get_prev_vertex(path, idx);
                calc_curves_angle(
                    prev.x, prev.y, prev.x, prev.y, pl.x, pl.y, p1.x, p1.y, p.x, p.y,
                )
            }
            (Segment::CubicTo(_, p2, p), Segment::LineTo(pl)) => {
                let prev = get_prev_vertex(path, idx);
                calc_curves_angle(prev.x, prev.y, p2.x, p2.y, p.x, p.y, pl.x, pl.y, pl.x, pl.y)
            }
            (Segment::LineTo(p), Segment::MoveTo(_)) => {
                let prev = get_prev_vertex(path, idx);
                calc_line_angle(prev.x, prev.y, p.x, p.y)
            }
            (Segment::CubicTo(_, p2, p), Segment::MoveTo(_)) => {
                if p.x.approx_eq_ulps(&p2.x, 4) && p.y.approx_eq_ulps(&p2.y, 4) {
                    let prev = get_prev_vertex(path, idx);
                    calc_line_angle(prev.x, prev.y, p.x, p.y)
                } else {
                    calc_line_angle(p2.x, p2.y, p.x, p.y)
                }
            }
            (Segment::LineTo(p), Segment::Close) => {
                let prev = get_prev_vertex(path, idx);
                let next = get_subpath_start(path, idx);
                calc_angle(prev.x, prev.y, p.x, p.y, p.x, p.y, next.x, next.y)
            }
            (_, Segment::Close) => {
                let prev = get_prev_vertex(path, idx);
                let next = get_subpath_start(path, idx);
                calc_line_angle(prev.x, prev.y, next.x, next.y)
            }
            (_, Segment::MoveTo(_)) | (Segment::Close, _) => 0.0,
        }
    }
}

fn closed_subpath_angle(path: &[Segment], start_idx: usize, close_idx: usize) -> f32 {
    let Segment::MoveTo(start) = path[start_idx] else {
        return 0.0;
    };

    let outgoing = match path.get(start_idx + 1) {
        Some(Segment::LineTo(point)) => *point,
        Some(Segment::CubicTo(first, second, end)) => {
            if *first != start {
                *first
            } else if *second != start {
                *second
            } else {
                *end
            }
        },
        _ => return 0.0,
    };

    let incoming = match path[close_idx - 1] {
        Segment::LineTo(end) if end != start => end,
        Segment::CubicTo(_, _, end) if end != start => end,
        Segment::LineTo(_) => get_prev_vertex(path, close_idx - 1),
        Segment::CubicTo(first, second, end) => {
            if second != end {
                second
            } else if first != end {
                first
            } else {
                get_prev_vertex(path, close_idx - 1)
            }
        },
        Segment::MoveTo(_) | Segment::Close => return 0.0,
    };

    calc_angle(
        incoming.x, incoming.y, start.x, start.y, start.x, start.y, outgoing.x, outgoing.y,
    )
}

fn calc_line_angle(x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    calc_angle(x1, y1, x2, y2, x1, y1, x2, y2)
}

fn calc_curves_angle(
    px: f32,
    py: f32, // previous vertex
    cx1: f32,
    cy1: f32, // previous control point
    x: f32,
    y: f32, // current vertex
    cx2: f32,
    cy2: f32, // next control point
    nx: f32,
    ny: f32, // next vertex
) -> f32 {
    if cx1.approx_eq_ulps(&x, 4) && cy1.approx_eq_ulps(&y, 4) {
        calc_angle(px, py, x, y, x, y, cx2, cy2)
    } else if x.approx_eq_ulps(&cx2, 4) && y.approx_eq_ulps(&cy2, 4) {
        calc_angle(cx1, cy1, x, y, x, y, nx, ny)
    } else {
        calc_angle(cx1, cy1, x, y, x, y, cx2, cy2)
    }
}

fn calc_angle(x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32, x4: f32, y4: f32) -> f32 {
    use std::f32::consts::*;

    fn normalize(rad: f32) -> f32 {
        let v = rad % (PI * 2.0);
        if v < 0.0 { v + PI * 2.0 } else { v }
    }

    fn vector_angle(vx: f32, vy: f32) -> f32 {
        let rad = vy.atan2(vx);
        if rad.is_nan() { 0.0 } else { normalize(rad) }
    }

    let in_a = vector_angle(x2 - x1, y2 - y1);
    let out_a = vector_angle(x4 - x3, y4 - y3);
    let d = (out_a - in_a) * 0.5;

    let mut angle = in_a + d;
    if FRAC_PI_2 < d.abs() {
        angle -= PI;
    }

    normalize(angle).to_degrees()
}

fn get_subpath_start(segments: &[Segment], idx: usize) -> tiny_skia_path::Point {
    let offset = segments.len() - idx;
    for seg in segments.iter().rev().skip(offset) {
        if let Segment::MoveTo(p) = *seg {
            return p;
        }
    }

    tiny_skia_path::Point::zero()
}

fn get_prev_vertex(segments: &[Segment], idx: usize) -> tiny_skia_path::Point {
    match segments[idx - 1] {
        Segment::MoveTo(p) => p,
        Segment::LineTo(p) => p,
        Segment::CubicTo(_, _, p) => p,
        Segment::Close => get_subpath_start(segments, idx),
    }
}

fn convert_rect(node: SvgNode, state: &converter::State) -> Option<NonZeroRect> {
    NonZeroRect::from_xywh(
        node.convert_user_length(AId::RefX, state, Length::zero()),
        node.convert_user_length(AId::RefY, state, Length::zero()),
        node.convert_user_length(AId::MarkerWidth, state, Length::new_number(3.0)),
        node.convert_user_length(AId::MarkerHeight, state, Length::new_number(3.0)),
    )
}

fn convert_orientation(node: SvgNode) -> MarkerOrientation {
    match node.attribute(AId::Orient) {
        Some("auto") => MarkerOrientation::Auto,
        Some("auto-start-reverse") => MarkerOrientation::AutoStartReverse,
        _ => match node.attribute::<svgtypes::Angle>(AId::Orient) {
            Some(angle) => MarkerOrientation::Angle(angle.to_degrees() as f32),
            None => MarkerOrientation::Angle(0.0),
        },
    }
}

fn quad_to_curve(prev: Point, p1: Point, p: Point) -> (Point, Point, Point) {
    #[inline]
    fn calc(n1: f32, n2: f32) -> f32 {
        (n1 + n2 * 2.0) / 3.0
    }

    (
        Point::from_xy(calc(prev.x, p1.x), calc(prev.y, p1.y)),
        Point::from_xy(calc(p.x, p1.x), calc(p.y, p1.y)),
        p,
    )
}
