/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use euclid::default::{Point2D, Rect, Size2D, Transform2D, Vector2D};
use malloc_size_of::MallocSizeOfOps;
use paint_api::CrossProcessPaintApi;
use pixels::Snapshot;
use profile_traits::mem::Report;
use servo_base::Epoch;
use servo_canvas_traits::canvas::*;
use webrender_api::ImageKey;

use crate::backend::GenericDrawTarget;
use crate::filter;
use crate::shadow::{blur_margin, render_shadow};

// Asserts on WR texture cache update for zero sized image with raw data.
// https://github.com/servo/webrender/blob/main/webrender/src/texture_cache.rs#L1475
const MIN_WR_IMAGE_SIZE: Size2D<u64> = Size2D::new(1, 1);

#[derive(Clone, Copy)]
pub(crate) enum Filter {
    Bilinear,
    Nearest,
}

/// <https://html.spec.whatwg.org/multipage/#dom-context-2d-beginlayer>
struct Layer<DrawTarget: GenericDrawTarget> {
    /// Where the layer is drawn. It is larger than the target below it by `margin` pixels on every
    /// side, so that its filters and shadow can bring in what is drawn outside of the canvas.
    draw_target: DrawTarget,
    margin: u32,
    options: LayerOptions,
}

pub(crate) struct CanvasData<DrawTarget: GenericDrawTarget> {
    draw_target: DrawTarget,
    /// The layers that are open, innermost last. While there are any, drawing goes to the innermost.
    layers: Vec<Layer<DrawTarget>>,
    /// The filter applied to every drawing operation.
    filter: Vec<FilterOperation>,
    paint_api: CrossProcessPaintApi,
    image_key: Option<ImageKey>,
}

impl<DrawTarget: GenericDrawTarget> CanvasData<DrawTarget> {
    pub(crate) fn new(
        size: Size2D<u64>,
        paint_api: CrossProcessPaintApi,
    ) -> CanvasData<DrawTarget> {
        CanvasData {
            draw_target: DrawTarget::new(size.max(MIN_WR_IMAGE_SIZE).cast()),
            layers: Vec::new(),
            filter: Vec::new(),
            paint_api,
            image_key: None,
        }
    }

    pub(crate) fn set_image_key(&mut self, image_key: ImageKey) {
        let (descriptor, data) = self.draw_target.image_descriptor_and_serializable_data();
        self.paint_api.add_image(image_key, descriptor, data, false);

        if let Some(old_image_key) = self.image_key.replace(image_key) {
            self.paint_api.delete_image(old_image_key);
        }
    }

    /// Returns memory-usage reports for a canvas.
    ///
    /// Called once for each canvas during memory reporting.
    pub(crate) fn collect_memory_report(
        &self,
        canvas_id: CanvasId,
        ops: &mut MallocSizeOfOps,
    ) -> Vec<Report> {
        let dimensions = self.draw_target.get_size();
        let canvas_info_string = format!(
            "canvas(id={}, {}x{})",
            canvas_id.0, dimensions.width, dimensions.height
        );
        self.draw_target
            .canvas_store_sizes(ops)
            .unwrap_or_default()
            .into_iter()
            .map(|canvas_store_type| Report {
                path: profile_traits::path!["canvas", canvas_info_string, canvas_store_type.name],
                kind: canvas_store_type.kind,
                size: canvas_store_type.size,
            })
            .collect()
    }

    #[expect(clippy::too_many_arguments)]
    pub(crate) fn draw_image(
        &mut self,
        snapshot: Snapshot,
        dest_rect: Rect<f64>,
        source_rect: Rect<f64>,
        smoothing_enabled: bool,
        shadow_options: ShadowOptions,
        composition_options: CompositionOptions,
        transform: Transform2D<f64>,
    ) {
        // We round up the floating pixel values to draw the pixels
        let source_rect = source_rect.ceil();
        // It discards the extra pixels (if any) that won't be painted
        let snapshot = if Rect::from_size(snapshot.size().to_f64()).contains_rect(&source_rect) {
            snapshot.get_rect(source_rect.to_u32())
        } else {
            snapshot
        };

        self.draw_with_effects(
            &shadow_options,
            composition_options,
            &dest_rect,
            transform,
            |draw_target, composition_options, transform| {
                write_image::<DrawTarget>(
                    draw_target,
                    snapshot.clone(),
                    dest_rect,
                    smoothing_enabled,
                    composition_options,
                    transform,
                );
            },
        );
    }

    pub(crate) fn fill_text(
        &mut self,
        text_bounds: Rect<f64>,
        text_runs: Vec<TextRun>,
        fill_or_stroke_style: FillOrStrokeStyle,
        shadow_options: ShadowOptions,
        composition_options: CompositionOptions,
        transform: Transform2D<f64>,
    ) {
        self.draw_shape(
            fill_or_stroke_style,
            &shadow_options,
            composition_options,
            &pad_text_bounds(text_bounds),
            transform,
            |draw_target, style, composition_options, transform| {
                draw_target.fill_text(&text_runs, style, composition_options, transform);
            },
        );
    }

    pub(crate) fn stroke_text(
        &mut self,
        text_bounds: Rect<f64>,
        text_runs: Vec<TextRun>,
        fill_or_stroke_style: FillOrStrokeStyle,
        line_options: LineOptions,
        shadow_options: ShadowOptions,
        composition_options: CompositionOptions,
        transform: Transform2D<f64>,
    ) {
        let bounds = stroked_bounds(&pad_text_bounds(text_bounds), &line_options);
        self.draw_shape(
            fill_or_stroke_style,
            &shadow_options,
            composition_options,
            &bounds,
            transform,
            |draw_target, style, composition_options, transform| {
                draw_target.stroke_text(
                    &text_runs,
                    style,
                    &line_options,
                    composition_options,
                    transform,
                );
            },
        );
    }

    pub(crate) fn fill_rect(
        &mut self,
        rect: &Rect<f32>,
        style: FillOrStrokeStyle,
        shadow_options: ShadowOptions,
        composition_options: CompositionOptions,
        transform: Transform2D<f64>,
    ) {
        if style.is_zero_size_gradient() {
            return; // Paint nothing if gradient size is zero.
        }

        self.draw_shape(
            style,
            &shadow_options,
            composition_options,
            &rect.cast(),
            transform,
            |draw_target, style, composition_options, transform| {
                draw_target.fill_rect(rect, style, composition_options, transform);
            },
        );
    }

    pub(crate) fn clear_rect(&mut self, rect: &Rect<f32>, transform: Transform2D<f64>) {
        let transform = self.in_target_space(transform);
        self.target().clear_rect(rect, transform);
    }

    pub(crate) fn stroke_rect(
        &mut self,
        rect: &Rect<f32>,
        style: FillOrStrokeStyle,
        line_options: LineOptions,
        shadow_options: ShadowOptions,
        composition_options: CompositionOptions,
        transform: Transform2D<f64>,
    ) {
        if style.is_zero_size_gradient() {
            return; // Paint nothing if gradient size is zero.
        }

        let bounds = stroked_bounds(&rect.cast(), &line_options);
        self.draw_shape(
            style,
            &shadow_options,
            composition_options,
            &bounds,
            transform,
            |draw_target, style, composition_options, transform| {
                draw_target.stroke_rect(rect, style, &line_options, composition_options, transform);
            },
        );
    }

    pub(crate) fn fill_path(
        &mut self,
        path: &Path,
        fill_rule: FillRule,
        style: FillOrStrokeStyle,
        shadow_options: ShadowOptions,
        composition_options: CompositionOptions,
        transform: Transform2D<f64>,
    ) {
        if style.is_zero_size_gradient() {
            return; // Paint nothing if gradient size is zero.
        }

        self.draw_shape(
            style,
            &shadow_options,
            composition_options,
            &path.bounding_box(),
            transform,
            |draw_target, style, composition_options, transform| {
                draw_target.fill(
                    path,
                    fill_rule.clone(),
                    style,
                    composition_options,
                    transform,
                );
            },
        );
    }

    pub(crate) fn stroke_path(
        &mut self,
        path: &Path,
        style: FillOrStrokeStyle,
        line_options: LineOptions,
        shadow_options: ShadowOptions,
        composition_options: CompositionOptions,
        transform: Transform2D<f64>,
    ) {
        if style.is_zero_size_gradient() {
            return; // Paint nothing if gradient size is zero.
        }

        let bounds = stroked_bounds(&path.bounding_box(), &line_options);
        self.draw_shape(
            style,
            &shadow_options,
            composition_options,
            &bounds,
            transform,
            |draw_target, style, composition_options, transform| {
                draw_target.stroke(path, style, &line_options, composition_options, transform);
            },
        );
    }

    pub(crate) fn clip_path(
        &mut self,
        path: &Path,
        fill_rule: FillRule,
        transform: Transform2D<f64>,
    ) {
        let transform = self.in_target_space(transform);
        self.target().push_clip(path, fill_rule, transform);
    }

    /// <https://html.spec.whatwg.org/multipage/#reset-the-rendering-context-to-its-default-state>
    pub(crate) fn recreate(&mut self, size: Option<Size2D<u64>>) {
        let size = size
            .unwrap_or_else(|| self.draw_target.get_size().to_u64())
            .max(MIN_WR_IMAGE_SIZE);

        // Step 1. Clear canvas's bitmap to transparent black.
        self.layers.clear();
        self.filter.clear();
        self.draw_target = self
            .draw_target
            .create_similar_draw_target(&Size2D::new(size.width, size.height).cast());

        self.update_image_rendering(None);
    }

    /// Update image in WebRender
    pub(crate) fn update_image_rendering(&mut self, canvas_epoch: Option<Epoch>) {
        let Some(image_key) = self.image_key else {
            return;
        };

        let (descriptor, data) = {
            let _span =
                profile_traits::trace_span!("image_descriptor_and_serializable_data").entered();
            self.draw_target.image_descriptor_and_serializable_data()
        };

        self.paint_api
            .update_image(image_key, descriptor, data, canvas_epoch);
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-context-2d-putimagedata>
    pub(crate) fn put_image_data(&mut self, snapshot: Snapshot, rect: Rect<u32>) {
        assert_eq!(rect.size, snapshot.size());
        let source_surface = self
            .draw_target
            .create_source_surface_from_data(snapshot)
            .unwrap();
        self.draw_target.copy_surface(
            source_surface,
            Rect::from_size(rect.size.to_i32()),
            rect.origin.to_i32(),
        );
    }

    /// The draw target that drawing operations currently go to.
    fn target(&mut self) -> &mut DrawTarget {
        match self.layers.last_mut() {
            Some(layer) => &mut layer.draw_target,
            None => &mut self.draw_target,
        }
    }

    /// Moves a transformation from canvas space to the space of [`Self::target`].
    fn in_target_space(&self, transform: Transform2D<f64>) -> Transform2D<f64> {
        let offset: u32 = self.layers.iter().map(|layer| layer.margin).sum();
        transform.then_translate(Vector2D::new(offset as f64, offset as f64))
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-context-2d-beginlayer>
    pub(crate) fn begin_layer(&mut self, options: LayerOptions) {
        let size = self.target().get_size();
        let mut margin = filter::margin(&options.filter);
        if options.shadow_options.need_to_draw_shadow() {
            let shadow = &options.shadow_options;
            margin += shadow
                .offset_x
                .abs()
                .max(shadow.offset_y.abs())
                .ceil()
                .min(2048.0) as u32
                + blur_margin(shadow.blur);
        }
        // Keep the layer to a size we can allocate.
        while margin > 0
            && (size.width as u64 + 2 * margin as u64) * (size.height as u64 + 2 * margin as u64)
                > MAX_LAYER_AREA
        {
            margin /= 2;
        }
        let draw_target = self.target().create_similar_draw_target(&Size2D::new(
            size.width + 2 * margin as i32,
            size.height + 2 * margin as i32,
        ));
        self.layers.push(Layer {
            draw_target,
            margin,
            options,
        });
        self.filter.clear();
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-context-2d-endlayer>
    pub(crate) fn end_layer(&mut self) {
        let Some(mut layer) = self.layers.pop() else {
            return;
        };
        if !layer.options.rasterizable {
            return;
        }
        let snapshot = layer.draw_target.snapshot();
        let snapshot = filter::apply(&snapshot, &layer.options.filter);
        let origin = Point2D::new(-(layer.margin as f64), -(layer.margin as f64));
        self.composite(
            snapshot,
            origin,
            &layer.options.shadow_options,
            layer.options.composition_options,
        );
    }

    pub(crate) fn set_filter(&mut self, filter: Vec<FilterOperation>) {
        self.filter = filter;
    }

    /// Draws a shape, preceded by its shadow if it casts one.
    ///
    /// `user_bounds` must contain everything `draw` paints, in the coordinate space `transform`
    /// maps to the canvas.
    fn draw_shape<F>(
        &mut self,
        style: FillOrStrokeStyle,
        shadow_options: &ShadowOptions,
        composition_options: CompositionOptions,
        user_bounds: &Rect<f64>,
        transform: Transform2D<f64>,
        draw: F,
    ) where
        F: Fn(&mut DrawTarget, FillOrStrokeStyle, CompositionOptions, Transform2D<f64>),
    {
        self.draw_with_effects(
            shadow_options,
            composition_options,
            user_bounds,
            transform,
            |draw_target, composition_options, transform| {
                bound_shape_with_pattern(
                    draw_target,
                    style.clone(),
                    composition_options,
                    user_bounds,
                    transform,
                    |draw_target, style| draw(draw_target, style, composition_options, transform),
                );
            },
        );
    }

    /// Draws something with the filter and the shadow that apply to it.
    ///
    /// `draw` draws the thing on the draw target it is given, with the composition options and the
    /// transform it is given. It is called with the transform set up so that `user_bounds` ends up
    /// where the thing is to be drawn. See [`Self::draw_shape`] for `user_bounds`.
    fn draw_with_effects<F>(
        &mut self,
        shadow_options: &ShadowOptions,
        composition_options: CompositionOptions,
        user_bounds: &Rect<f64>,
        transform: Transform2D<f64>,
        draw: F,
    ) where
        F: Fn(&mut DrawTarget, CompositionOptions, Transform2D<f64>),
    {
        let transform = self.in_target_space(transform);

        if !self.filter.is_empty() {
            // The filter applies to the thing alone, and the shadow to what the filter makes of it.
            let margin = filter::margin(&self.filter) as i32;
            let size = self.target().get_size();
            let mut source_target = self.target().create_similar_draw_target(&Size2D::new(
                size.width + 2 * margin,
                size.height + 2 * margin,
            ));
            draw(
                &mut source_target,
                UNSHADOWED_COMPOSITION,
                transform.then_translate(Vector2D::new(margin as f64, margin as f64)),
            );
            let snapshot = filter::apply(&source_target.snapshot(), &self.filter);
            self.composite(
                snapshot,
                Point2D::new(-margin as f64, -margin as f64),
                shadow_options,
                composition_options,
            );
            return;
        }

        if shadow_options.need_to_draw_shadow() {
            self.draw_shadow(
                shadow_options,
                composition_options,
                user_bounds,
                transform,
                |draw_target, transform| draw(draw_target, UNSHADOWED_COMPOSITION, transform),
            );
        }
        draw(self.target(), composition_options, transform);
    }

    /// Draws the shadow of a shape on the canvas.
    ///
    /// `draw_source` draws the shape on the draw target it is given, using the transform it is
    /// given. See [`Self::draw_shape`] for `user_bounds`.
    ///
    /// <https://html.spec.whatwg.org/multipage/#when-shadows-are-drawn>
    fn draw_shadow<F>(
        &mut self,
        shadow_options: &ShadowOptions,
        composition_options: CompositionOptions,
        user_bounds: &Rect<f64>,
        transform: Transform2D<f64>,
        draw_source: F,
    ) where
        F: FnOnce(&mut DrawTarget, Transform2D<f64>),
    {
        let canvas_size = self.target().get_size().to_f64();
        let margin = blur_margin(shadow_options.blur) as f64;
        let offset = Vector2D::new(shadow_options.offset_x, shadow_options.offset_y);

        // The only part of the shape that can reach the canvas is the one that is, once moved by the
        // offset and spread by the blur, on top of the canvas.
        let reachable = Rect::new(
            Point2D::new(-margin, -margin) - offset,
            Size2D::new(
                canvas_size.width + 2.0 * margin,
                canvas_size.height + 2.0 * margin,
            ),
        );
        let device_bounds = transform.outer_transformed_rect(user_bounds);
        if !(device_bounds.origin.is_finite() && device_bounds.size.to_vector().is_finite()) {
            return;
        }
        let Some(visible) = reachable.intersection(&device_bounds) else {
            return;
        };
        let origin = Point2D::new(visible.min_x().floor(), visible.min_y().floor());
        let size = Size2D::new(
            (visible.max_x().ceil() - origin.x).max(1.0),
            (visible.max_y().ceil() - origin.y).max(1.0),
        );

        let mut source_target = self.target().create_similar_draw_target(&size.to_i32());
        draw_source(
            &mut source_target,
            transform.then_translate(-origin.to_vector()),
        );
        let source = source_target.snapshot();
        self.draw_shadow_of(&source, origin, shadow_options, composition_options);
    }

    /// Draws the shadow of `source`, an image whose top left corner is at `origin` in the space of
    /// [`Self::target`].
    fn draw_shadow_of(
        &mut self,
        source: &Snapshot,
        origin: Point2D<f64>,
        shadow_options: &ShadowOptions,
        composition_options: CompositionOptions,
    ) {
        let color = shadow_options.color.into_srgb_legacy();
        let channel = |value: f32| {
            if value.is_nan() {
                0.0
            } else {
                value.clamp(0.0, 1.0)
            }
        };
        let shadow = render_shadow(
            source,
            shadow_options.blur,
            [
                channel(color.components.0),
                channel(color.components.1),
                channel(color.components.2),
                channel(color.alpha),
            ],
        );

        // Snap the shadow to the pixel grid, as it is not worth resampling.
        let margin = blur_margin(shadow_options.blur) as f64;
        let destination = Point2D::new(
            (origin.x + shadow_options.offset_x - margin).round(),
            (origin.y + shadow_options.offset_y - margin).round(),
        );
        let destination = Rect::new(destination, shadow.size().to_f64());
        write_image::<DrawTarget>(
            self.target(),
            shadow,
            destination,
            false,
            composition_options,
            Transform2D::identity(),
        );
    }

    /// Composites an image that has been drawn, and its shadow, onto the canvas. `origin` is where the
    /// top left corner of the image is, in the space of [`Self::target`].
    fn composite(
        &mut self,
        snapshot: Snapshot,
        origin: Point2D<f64>,
        shadow_options: &ShadowOptions,
        composition_options: CompositionOptions,
    ) {
        if shadow_options.need_to_draw_shadow() {
            self.draw_shadow_of(&snapshot, origin, shadow_options, composition_options);
        }
        let destination = Rect::new(origin, snapshot.size().to_f64());
        write_image::<DrawTarget>(
            self.target(),
            snapshot,
            destination,
            false,
            composition_options,
            Transform2D::identity(),
        );
    }

    /// It reads image data from the canvas
    /// canvas_size: The size of the canvas we're reading from
    /// read_rect: The area of the canvas we want to read from
    #[servo_tracing::instrument(skip_all)]
    pub(crate) fn read_pixels(&mut self, read_rect: Option<Rect<u32>>) -> Snapshot {
        let canvas_size = self.draw_target.get_size().cast();

        if let Some(read_rect) = read_rect {
            let canvas_rect = Rect::from_size(canvas_size);
            if canvas_rect
                .intersection(&read_rect)
                .is_none_or(|rect| rect.is_empty())
            {
                Snapshot::empty()
            } else {
                self.draw_target.snapshot().get_rect(read_rect)
            }
        } else {
            self.draw_target.snapshot()
        }
    }

    pub(crate) fn pop_clips(&mut self, clips: usize) {
        for _ in 0..clips {
            self.target().pop_clip();
        }
    }
}

impl<D: GenericDrawTarget> Drop for CanvasData<D> {
    fn drop(&mut self) {
        if let Some(image_key) = self.image_key {
            self.paint_api.delete_image(image_key);
        }
    }
}

/// The largest number of pixels a layer may have.
const MAX_LAYER_AREA: u64 = 64 * 1024 * 1024;

/// The composition options used to draw the source of a shadow: the shadow is what is composited
/// with the canvas, so the source is drawn plainly.
const UNSHADOWED_COMPOSITION: CompositionOptions = CompositionOptions {
    alpha: 1.0,
    composition_operation: CompositionOrBlending::Composition(CompositionStyle::SourceOver),
};

/// Grows `bounds` to contain a stroke of the shape it bounds.
fn stroked_bounds(bounds: &Rect<f64>, line_options: &LineOptions) -> Rect<f64> {
    // Miter joins reach `miter_limit` times the line width away from the corner, square caps
    // `sqrt(2)` half widths.
    let reach = line_options.width.max(0.0) / 2.0
        * line_options.miter_limit.max(std::f64::consts::SQRT_2)
        + 1.0;
    bounds.inflate(reach, reach)
}

/// Grows the bounds of some text to contain glyphs that extend beyond the advance box.
fn pad_text_bounds(bounds: Rect<f64>) -> Rect<f64> {
    let padding = (bounds.size.height * 0.25).max(2.0);
    bounds.inflate(padding, padding)
}

/// Draws a shape with `draw_shape`, clipped to the non-repeating bound (either x, y, or both) of
/// the given pattern.
fn bound_shape_with_pattern<DrawTarget: GenericDrawTarget, F>(
    draw_target: &mut DrawTarget,
    style: FillOrStrokeStyle,
    composition_options: CompositionOptions,
    path_bound_box: &Rect<f64>,
    transform: Transform2D<f64>,
    draw_shape: F,
) where
    F: FnOnce(&mut DrawTarget, FillOrStrokeStyle),
{
    let x_bound = style.x_bound();
    let y_bound = style.y_bound();
    // Clear operations are also unbounded.
    if matches!(
        composition_options.composition_operation,
        CompositionOrBlending::Composition(CompositionStyle::Clear)
    ) || (x_bound.is_none() && y_bound.is_none())
    {
        draw_shape(draw_target, style);
        return;
    }
    let rect = Rect::from_size(Size2D::new(
        x_bound.unwrap_or(path_bound_box.size.width.ceil() as u32),
        y_bound.unwrap_or(path_bound_box.size.height.ceil() as u32),
    ))
    .cast();
    let rect = transform.outer_transformed_rect(&rect);

    // The pattern is transparent outside its image. The operations that are not bounded by the
    // source make the destination transparent wherever the source is transparent, so for them
    // draw without a clip, which the backend does not blend correctly under, and clear what
    // lies outside the image afterwards.
    // <https://html.spec.whatwg.org/multipage/#drawing-model>
    let is_unbounded = matches!(
        composition_options.composition_operation,
        CompositionOrBlending::Composition(
            CompositionStyle::Copy |
                CompositionStyle::SourceIn |
                CompositionStyle::SourceOut |
                CompositionStyle::DestinationIn |
                CompositionStyle::DestinationAtop
        )
    );
    if !is_unbounded {
        draw_target.push_clip_rect(&rect.cast());
        draw_shape(draw_target, style);
        draw_target.pop_clip();
        return;
    }

    draw_shape(draw_target, style);
    let size = draw_target.get_size().to_f32();
    let rect = rect.cast::<f32>();
    let (left, top) = (
        rect.min_x().clamp(0., size.width),
        rect.min_y().clamp(0., size.height),
    );
    let (right, bottom) = (
        rect.max_x().clamp(left, size.width),
        rect.max_y().clamp(top, size.height),
    );
    let outside = [
        Rect::new(Point2D::zero(), Size2D::new(size.width, top)),
        Rect::new(
            Point2D::new(0., bottom),
            Size2D::new(size.width, size.height - bottom),
        ),
        Rect::new(Point2D::new(0., top), Size2D::new(left, bottom - top)),
        Rect::new(
            Point2D::new(right, top),
            Size2D::new(size.width - right, bottom - top),
        ),
    ];
    for rect in outside.iter().filter(|rect| !rect.is_empty()) {
        draw_target.clear_rect(rect, Transform2D::identity());
    }
}

/// It writes an image to the destination target
/// draw_target: the destination target where the image_data will be copied
/// image_data: Pixel information of the image to be written. It takes RGBA8
/// image_size: The size of the image to be written
/// dest_rect: Area of the destination target where the pixels will be copied
/// smoothing_enabled: It determines if smoothing is applied to the image result
/// premultiply: Determines whenever the image data should be premultiplied or not
fn write_image<DrawTarget: GenericDrawTarget>(
    draw_target: &mut DrawTarget,
    snapshot: Snapshot,
    dest_rect: Rect<f64>,
    smoothing_enabled: bool,
    composition_options: CompositionOptions,
    transform: Transform2D<f64>,
) {
    if snapshot.size().is_empty() {
        return;
    }

    let image_rect = Rect::new(Point2D::zero(), snapshot.size().cast());

    // From spec https://html.spec.whatwg.org/multipage/#dom-context-2d-drawimage
    // When scaling up, if the imageSmoothingEnabled attribute is set to true, the user agent should attempt
    // to apply a smoothing algorithm to the image data when it is scaled.
    // Otherwise, the image must be rendered using nearest-neighbor interpolation.
    let filter = if smoothing_enabled {
        Filter::Bilinear
    } else {
        Filter::Nearest
    };

    let source_surface = draw_target
        .create_source_surface_from_data(snapshot)
        .unwrap();

    draw_target.draw_surface(
        source_surface,
        dest_rect,
        image_rect,
        filter,
        composition_options,
        transform,
    );
}

pub(crate) trait RectToi32 {
    fn ceil(&self) -> Rect<f64>;
}

impl RectToi32 for Rect<f64> {
    fn ceil(&self) -> Rect<f64> {
        Rect::new(
            Point2D::new(self.origin.x.ceil(), self.origin.y.ceil()),
            Size2D::new(self.size.width.ceil(), self.size.height.ceil()),
        )
    }
}
