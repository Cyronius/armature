/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Painting of boxes with non-round `corner-shape` values.
//!
//! WebRender can only draw elliptical corners, so the background, border and box
//! shadows of a box with shaped corners are rasterized into an image instead.
//!
//! <https://drafts.csswg.org/css-borders-4/#corner-shape-rendering>

use std::hash::{Hash, Hasher};

use resvg::tiny_skia::{self, FillRule, Mask, Paint, PathBuilder, Pixmap, Rect, Transform};
use style::color::{AbsoluteColor, ColorSpace};

#[derive(Clone, Copy, Debug)]
struct Point {
    x: f64,
    y: f64,
}

#[derive(Clone, Copy, Debug)]
struct Vector {
    x: f64,
    y: f64,
}

impl Vector {
    fn scale(self, s: f64) -> Self {
        Self {
            x: self.x * s,
            y: self.y * s,
        }
    }
    fn length(self) -> f64 {
        self.x.hypot(self.y)
    }
    fn normalized(self) -> Self {
        let length = self.length();
        if length != 0. {
            self.scale(1. / length)
        } else {
            self
        }
    }
    fn perpendicular(self) -> Self {
        Self {
            x: -self.y,
            y: self.x,
        }
    }
    fn from_points(p1: Point, p2: Point) -> Self {
        Self {
            x: p2.x - p1.x,
            y: p2.y - p1.y,
        }
    }
    fn add(self, other: Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }
    fn dot(self, other: Self) -> f64 {
        self.x * other.x + self.y * other.y
    }
    fn cross(self, other: Self) -> f64 {
        self.x * other.y - self.y * other.x
    }
}

fn extend(point: Point, vector: Vector) -> Point {
    Point {
        x: point.x + vector.x,
        y: point.y + vector.y,
    }
}

fn line_intersection(a: (Point, Point), b: (Point, Point)) -> Option<Point> {
    let a_length = Vector::from_points(a.0, a.1);
    let b_length = Vector::from_points(b.0, b.1);
    let denom = a_length.cross(b_length);
    if denom.abs() < 1e-6 {
        return None;
    }
    let a_scale = Vector::from_points(a.0, b.0).cross(b_length) / denom;
    Some(extend(a.0, a_length.scale(a_scale)))
}

fn segment_line_intersection(a: (Point, Point), b: (Point, Point)) -> Option<Point> {
    let a_length = Vector::from_points(a.0, a.1);
    let b_length = Vector::from_points(b.0, b.1);
    let denom = a_length.cross(b_length);
    if denom.abs() < 1e-6 {
        return None;
    }
    let offset = Vector::from_points(a.0, b.0);
    let a_scale = offset.cross(b_length) / denom;
    if (0. ..=1.).contains(&a_scale) {
        return Some(extend(a.0, a_length.scale(a_scale)));
    }
    let b_scale = offset.cross(a_length) / denom;
    if (0. ..=1.).contains(&b_scale) {
        return Some(extend(b.0, b_length.scale(b_scale)));
    }
    None
}

fn adjusted_radius_dimension(coverage: f64, radius: f64, outset: f64) -> f64 {
    if radius > outset || coverage > 1. {
        return radius + outset;
    }
    let ratio = radius / outset;
    radius + outset * (1. - (1. - ratio).powi(3) * (1. - coverage.powi(3)))
}

/// <https://drafts.csswg.org/css-borders-4/#corner-shape-outset>
fn outset_adjusted_border_radius(
    width: f64,
    height: f64,
    radius: [f64; 2],
    outset_x: f64,
    outset_y: f64,
) -> [f64; 2] {
    let coverage = 2. * (radius[0] / width).min(radius[1] / height);
    [
        adjusted_radius_dimension(coverage, radius[0], outset_x),
        adjusted_radius_dimension(coverage, radius[1], outset_y),
    ]
}

fn normalized_superellipse_half_corner(k: f64) -> f64 {
    let n = 2f64.powf(k.abs());
    let convex = 0.5f64.powf(1. / n);
    if k < 0. { 1. - convex } else { convex }
}

/// The region a shaped corner removes from its box, as a polygon, following the
/// algorithm of the web-platform-tests reference renderer.
#[allow(clippy::too_many_arguments)]
fn corner_clip_out_path(
    start_radius: f64,
    end_radius: f64,
    start_inset: f64,
    end_inset: f64,
    target_outer: Point,
    v3: Vector,
    v2: Vector,
    k: f64,
) -> Option<Vec<Point>> {
    if start_radius == 0. || end_radius == 0. || k == f64::INFINITY {
        return None;
    }

    let original_outer = extend(
        extend(target_outer, v3.scale(-end_inset)),
        v2.scale(-start_inset),
    );
    let original_start = extend(original_outer, v3.scale(end_radius));
    let original_end = extend(original_outer, v2.scale(start_radius));

    let clamp = |l: f64, v: f64, u: f64| v.min(u).max(l);
    let half_corner_x = normalized_superellipse_half_corner(k);
    let sqrt2 = std::f64::consts::SQRT_2;
    let control_point_x = clamp(0., half_corner_x / (sqrt2 - 1.) - 1. / sqrt2, 1.);
    let inset_diff = clamp(-start_radius, end_inset - start_inset, end_radius);

    if k <= 0. && (inset_diff == -start_radius || inset_diff == end_radius) {
        return None;
    }

    let mut start_control_point_x = control_point_x;
    let mut end_control_point_x = control_point_x;
    if inset_diff != 0. {
        let bevel_normal_delta =
            (start_radius.powi(2) + end_radius.powi(2) - inset_diff.powi(2)).sqrt();
        let bevel_normal_x = end_radius * inset_diff + start_radius * bevel_normal_delta;
        let bevel_normal_y = -start_radius * inset_diff + end_radius * bevel_normal_delta;
        let bevel_control_point_x = start_radius * bevel_normal_y /
            (start_radius * bevel_normal_y + end_radius * bevel_normal_x);
        if k < 0. {
            start_control_point_x = bevel_control_point_x * (2. * control_point_x);
        } else {
            start_control_point_x =
                1. - (1. - bevel_control_point_x) * (2. * (1. - control_point_x));
        }
        end_control_point_x = 2. * control_point_x - start_control_point_x;
    }

    let unmapped_start_normal = Vector {
        x: (1. - start_control_point_x) * start_radius,
        y: start_control_point_x * end_radius,
    }
    .normalized();
    let unmapped_end_normal = Vector {
        x: end_control_point_x * start_radius,
        y: (1. - end_control_point_x) * end_radius,
    }
    .normalized();

    let start_normal = v3
        .scale(unmapped_start_normal.x)
        .add(v2.scale(unmapped_start_normal.y));
    let end_normal = v3
        .scale(unmapped_end_normal.x)
        .add(v2.scale(unmapped_end_normal.y));

    let mut adjusted_start = extend(original_start, start_normal.scale(start_inset));
    let mut adjusted_end = extend(original_end, end_normal.scale(end_inset));

    let start_tangent = start_normal.perpendicular().scale(-1.);
    let end_tangent = end_normal.perpendicular();

    let mut miter_start = adjusted_start;
    let mut miter_end = adjusted_end;
    if start_inset < 0. {
        let clip_start = extend(target_outer, v3);
        miter_start = line_intersection(
            (adjusted_start, extend(adjusted_start, start_tangent)),
            (clip_start, target_outer),
        )
        .unwrap_or(adjusted_start);
        if k >= 0. {
            adjusted_start = miter_start;
        }
    }
    if end_inset < 0. {
        let clip_end = extend(target_outer, v2);
        miter_end = line_intersection(
            (adjusted_end, extend(adjusted_end, end_tangent)),
            (clip_end, target_outer),
        )
        .unwrap_or(adjusted_end);
        if k >= 0. {
            adjusted_end = miter_end;
        }
    }

    let adjusted_height = Vector::from_points(adjusted_start, adjusted_end).dot(v2);
    let adjusted_outer = extend(adjusted_end, v2.scale(-adjusted_height));
    let adjusted_center = extend(adjusted_start, v2.scale(adjusted_height));

    let map_point_to_corner = |x: f64, y: f64, start: Point, end: Point, center: Point| {
        extend(
            extend(center, Vector::from_points(center, end).scale(x)),
            Vector::from_points(center, start).scale(y),
        )
    };

    // Sample the curve where it crosses whole pixel coordinates.
    let t_values = |k: f64| {
        let n = 2f64.powf(k.abs());
        let mut values = vec![0., 1.];
        let denom = (1. / n).ln();
        let mut x = adjusted_start.x.min(adjusted_end.x);
        while x < adjusted_start.x.max(adjusted_end.x) {
            let t = ((x - adjusted_start.x) / (adjusted_end.x - adjusted_start.x)).ln() / denom;
            if t > 0. && t < 1. {
                values.push(t);
            }
            x += 1.;
        }
        let mut y = adjusted_start.y.min(adjusted_end.y);
        while y < adjusted_start.y.max(adjusted_end.y) {
            let t =
                (1. - (y - adjusted_start.y) / (adjusted_end.y - adjusted_start.y)).ln() / denom;
            if t > 0. && t < 1. {
                values.push(t);
            }
            y += 1.;
        }
        values.sort_by(f64::total_cmp);
        values.dedup();
        values
    };

    let mut self_intersection = None;
    if k < 0. &&
        start_inset < 0. &&
        end_inset < 0. &&
        (-end_inset >= start_radius || -start_inset >= end_radius)
    {
        self_intersection =
            segment_line_intersection((miter_start, adjusted_start), (miter_end, adjusted_end));
    }

    let mut points = vec![miter_start];
    if let Some(point) = self_intersection {
        points.push(point);
    } else if k == f64::NEG_INFINITY {
        points.push(adjusted_center);
    } else if k > 0. || k <= -1. {
        let n = 2f64.powf(k.abs());
        let curve_center = if k < 0. {
            adjusted_outer
        } else {
            adjusted_center
        };
        for t in t_values(k) {
            let x = t.powf(1. / n);
            let y = (1. - t).powf(1. / n);
            points.push(map_point_to_corner(
                x,
                y,
                adjusted_start,
                adjusted_end,
                curve_center,
            ));
        }
    } else if k > -1. && k < 0. {
        let tangent_intersection = line_intersection(
            (adjusted_start, extend(adjusted_start, start_tangent)),
            (adjusted_end, extend(adjusted_end, end_tangent)),
        )
        .unwrap_or(adjusted_start);
        for t in t_values(1.) {
            let x = 1. - (1. - t).sqrt();
            let y = 1. - t.sqrt();
            points.push(map_point_to_corner(
                x,
                y,
                adjusted_start,
                adjusted_end,
                tangent_intersection,
            ));
        }
    }
    points.push(miter_end);
    points.push(target_outer);
    Some(points)
}

/// The shape of a box: its border box size, corner radii (horizontal and vertical,
/// in top-left, top-right, bottom-right, bottom-left order) and corner shapes.
#[derive(Clone, Debug)]
pub(crate) struct CornerShapedBox {
    pub width: f64,
    pub height: f64,
    pub radii: [[f64; 2]; 4],
    pub shapes: [f64; 4],
}

/// Insets from the border box, as left, top, right, bottom.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Insets {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl Insets {
    pub(crate) fn uniform(value: f64) -> Self {
        Self {
            left: value,
            top: value,
            right: value,
            bottom: value,
        }
    }
}

impl CornerShapedBox {
    /// A mask of the border box inset by `insets` with shaped corners, drawn into a
    /// pixmap of `size` whose origin is at `origin` in border box coordinates.
    fn contour_mask(&self, insets: Insets, offset: (f64, f64), origin: (f64, f64), size: (u32, u32)) -> Option<Mask> {
        let target_left = insets.left;
        let target_top = insets.top;
        let target_right = self.width - insets.right;
        let target_bottom = self.height - insets.bottom;
        if target_right <= target_left || target_bottom <= target_top {
            return None;
        }

        let adjusted_inset = |radius: [f64; 2], inset_x: f64, inset_y: f64| {
            let adjusted =
                outset_adjusted_border_radius(self.width, self.height, radius, -inset_x, -inset_y);
            [radius[0] - adjusted[0], radius[1] - adjusted[1]]
        };
        let [top_left, top_right, bottom_right, bottom_left] = self.radii;
        let top_right_inset = adjusted_inset(top_right, insets.right, insets.top);
        let bottom_right_inset = adjusted_inset(bottom_right, insets.right, insets.bottom);
        let bottom_left_inset = adjusted_inset(bottom_left, insets.left, insets.bottom);
        let top_left_inset = adjusted_inset(top_left, insets.left, insets.top);

        let transform = Transform::from_translate(
            (offset.0 - origin.0) as f32,
            (offset.1 - origin.1) as f32,
        );
        let rect = Rect::from_ltrb(
            target_left as f32,
            target_top as f32,
            target_right as f32,
            target_bottom as f32,
        )?;
        let mut mask = Mask::new(size.0, size.1)?;
        mask.fill_path(
            &PathBuilder::from_rect(rect),
            FillRule::Winding,
            true,
            transform,
        );

        let corners = [
            corner_clip_out_path(
                top_right[1],
                top_right[0],
                top_right_inset[1],
                top_right_inset[0],
                Point {
                    x: target_right,
                    y: target_top,
                },
                Vector { x: -1., y: 0. },
                Vector { x: 0., y: 1. },
                self.shapes[1],
            ),
            corner_clip_out_path(
                bottom_right[0],
                bottom_right[1],
                bottom_right_inset[0],
                bottom_right_inset[1],
                Point {
                    x: target_right,
                    y: target_bottom,
                },
                Vector { x: 0., y: -1. },
                Vector { x: -1., y: 0. },
                self.shapes[2],
            ),
            corner_clip_out_path(
                bottom_left[1],
                bottom_left[0],
                bottom_left_inset[1],
                bottom_left_inset[0],
                Point {
                    x: target_left,
                    y: target_bottom,
                },
                Vector { x: 1., y: 0. },
                Vector { x: 0., y: -1. },
                self.shapes[3],
            ),
            corner_clip_out_path(
                top_left[0],
                top_left[1],
                top_left_inset[0],
                top_left_inset[1],
                Point {
                    x: target_left,
                    y: target_top,
                },
                Vector { x: 0., y: 1. },
                Vector { x: 1., y: 0. },
                self.shapes[0],
            ),
        ];
        for corner in corners.into_iter().flatten() {
            // Remove the corner region: intersect with the box XOR the corner.
            let mut builder = PathBuilder::new();
            builder.push_rect(rect);
            builder.move_to(corner[0].x as f32, corner[0].y as f32);
            for point in &corner[1..] {
                builder.line_to(point.x as f32, point.y as f32);
            }
            builder.close();
            if let Some(path) = builder.finish() {
                mask.intersect_path(&path, FillRule::EvenOdd, true, transform);
            }
        }
        Some(mask)
    }
}

/// A box shadow without blur.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Shadow {
    pub inset: bool,
    pub offset: (f64, f64),
    pub spread: f64,
    pub color: AbsoluteColor,
}

/// Everything painted for a box with shaped corners.
pub(crate) struct CornerShapedDecorations {
    pub shape: CornerShapedBox,
    pub border_widths: Insets,
    pub border_color: Option<AbsoluteColor>,
    pub background_color: Option<AbsoluteColor>,
    pub shadows: Vec<Shadow>,
}

/// A rasterized image of the decorations and its placement relative to the border
/// box origin, in CSS pixels.
pub(crate) struct RasterizedDecorations {
    pub origin: (f64, f64),
    pub size: (u32, u32),
    pub scale: f32,
    pub bgra_pixels: Vec<u8>,
    pub content_hash: u64,
}

fn paint_for(color: AbsoluteColor) -> Paint<'static> {
    let color = color.to_color_space(ColorSpace::Srgb);
    let mut paint = Paint::default();
    paint.set_color(
        tiny_skia::Color::from_rgba(
            color.components.0.clamp(0., 1.),
            color.components.1.clamp(0., 1.),
            color.components.2.clamp(0., 1.),
            color.alpha.clamp(0., 1.),
        )
        .unwrap_or(tiny_skia::Color::BLACK),
    );
    paint.anti_alias = true;
    paint
}

impl CornerShapedDecorations {
    pub(crate) fn rasterize(&self, scale: f32) -> Option<RasterizedDecorations> {
        let mut extent = 1f64;
        for shadow in self.shadows.iter().filter(|shadow| !shadow.inset) {
            extent = extent.max(shadow.spread + shadow.offset.0.abs().max(shadow.offset.1.abs()) + 1.);
        }
        let origin = (-extent, -extent);
        let width = ((self.shape.width + 2. * extent) * scale as f64).ceil() as u32;
        let height = ((self.shape.height + 2. * extent) * scale as f64).ceil() as u32;
        if width == 0 || height == 0 || width > 8192 || height > 8192 {
            return None;
        }
        let mut pixmap = Pixmap::new(width, height)?;
        let full = Rect::from_xywh(0., 0., width as f32, height as f32)?;
        let size = (width, height);
        let scaled_origin = (origin.0 * scale as f64, origin.1 * scale as f64);
        let scaled = |insets: Insets| Insets {
            left: insets.left * scale as f64,
            top: insets.top * scale as f64,
            right: insets.right * scale as f64,
            bottom: insets.bottom * scale as f64,
        };
        let shape = CornerShapedBox {
            width: self.shape.width * scale as f64,
            height: self.shape.height * scale as f64,
            radii: self.shape.radii.map(|r| [r[0] * scale as f64, r[1] * scale as f64]),
            shapes: self.shape.shapes,
        };
        let mask = |insets: Insets, offset: (f64, f64)| {
            shape.contour_mask(
                scaled(insets),
                (offset.0 * scale as f64, offset.1 * scale as f64),
                scaled_origin,
                size,
            )
        };

        // Outer shadows, in reverse order so that the first is on top.
        for shadow in self.shadows.iter().rev().filter(|shadow| !shadow.inset) {
            if let Some(shadow_mask) = mask(Insets::uniform(-shadow.spread), shadow.offset) {
                // The shadow is not painted under the box.
                let mut shadow_mask = shadow_mask;
                if let Some(box_mask) = mask(Insets::uniform(0.), (0., 0.)) {
                    let mut inverted = box_mask;
                    inverted.invert();
                    combine_masks(&mut shadow_mask, &inverted);
                }
                pixmap.fill_rect(full, &paint_for(shadow.color), Transform::identity(), Some(&shadow_mask));
            }
        }

        if let Some(background_color) = self.background_color {
            if let Some(background_mask) = mask(Insets::uniform(0.), (0., 0.)) {
                pixmap.fill_rect(
                    full,
                    &paint_for(background_color),
                    Transform::identity(),
                    Some(&background_mask),
                );
            }
        }

        // Inset shadows are painted inside the padding box, over the background.
        for shadow in self.shadows.iter().rev().filter(|shadow| shadow.inset) {
            let Some(mut padding_mask) = mask(self.border_widths, (0., 0.)) else {
                continue;
            };
            let inner = Insets {
                left: self.border_widths.left + shadow.spread,
                top: self.border_widths.top + shadow.spread,
                right: self.border_widths.right + shadow.spread,
                bottom: self.border_widths.bottom + shadow.spread,
            };
            if let Some(mut hole) = mask(inner, shadow.offset) {
                hole.invert();
                combine_masks(&mut padding_mask, &hole);
            }
            pixmap.fill_rect(full, &paint_for(shadow.color), Transform::identity(), Some(&padding_mask));
        }

        if let Some(border_color) = self.border_color {
            if let Some(mut border_mask) = mask(Insets::uniform(0.), (0., 0.)) {
                if let Some(mut inner) = mask(self.border_widths, (0., 0.)) {
                    inner.invert();
                    combine_masks(&mut border_mask, &inner);
                }
                pixmap.fill_rect(full, &paint_for(border_color), Transform::identity(), Some(&border_mask));
            }
        }

        // tiny-skia stores premultiplied RGBA; WebRender wants BGRA.
        let mut bgra_pixels = pixmap.take();
        for pixel in bgra_pixels.chunks_mut(4) {
            pixel.swap(0, 2);
        }
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (width, height).hash(&mut hasher);
        bgra_pixels.hash(&mut hasher);
        Some(RasterizedDecorations {
            origin,
            size,
            scale,
            bgra_pixels,
            content_hash: hasher.finish(),
        })
    }
}

/// Multiply `mask` by `other`.
fn combine_masks(mask: &mut Mask, other: &Mask) {
    for (a, b) in mask.data_mut().iter_mut().zip(other.data()) {
        *a = ((*a as u16 * *b as u16 + 127) / 255) as u8;
    }
}
