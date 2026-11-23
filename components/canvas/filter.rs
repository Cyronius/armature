/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The CSS filter functions of the `filter` attribute of canvas rendering contexts.
//!
//! <https://drafts.fxtf.org/filter-effects/#filter-functions>

use euclid::default::Size2D;
use pixels::{Snapshot, SnapshotAlphaMode, SnapshotPixelFormat};
use servo_canvas_traits::canvas::FilterOperation;

use crate::blur::gaussian_blur;
use crate::shadow::blur_margin;

/// The most pixels a filter may spread an image by in any direction.
const MAX_MARGIN: u32 = 2048;

/// The number of pixels of space around an image that applying `operations` to it needs for
/// everything they spread out of the image to be kept.
pub(crate) fn margin(operations: &[FilterOperation]) -> u32 {
    operations
        .iter()
        .map(|operation| match operation {
            FilterOperation::Blur(x, y) => blur_margin(2.0 * x.max(*y)),
            FilterOperation::DropShadow {
                offset_x,
                offset_y,
                std_deviation,
                ..
            } => {
                offset_x
                    .abs()
                    .max(offset_y.abs())
                    .ceil()
                    .min(MAX_MARGIN as f64) as u32
                    + blur_margin(2.0 * std_deviation)
            },
            _ => 0,
        })
        .fold(0u32, |total, margin| total.saturating_add(margin))
        .min(MAX_MARGIN)
}

/// A premultiplied RGBA image with a float per channel.
struct Image {
    width: usize,
    height: usize,
    data: Vec<f32>,
}

impl Image {
    fn from_snapshot(snapshot: &Snapshot) -> Self {
        let size = snapshot.size();
        Self {
            width: size.width as usize,
            height: size.height as usize,
            data: snapshot
                .as_raw_bytes()
                .iter()
                .map(|byte| *byte as f32 / 255.0)
                .collect(),
        }
    }

    fn into_snapshot(self) -> Snapshot {
        Snapshot::from_vec(
            Size2D::new(self.width as u32, self.height as u32),
            SnapshotPixelFormat::RGBA,
            SnapshotAlphaMode::Transparent {
                premultiplied: true,
            },
            self.data
                .into_iter()
                .map(|value| (value.clamp(0.0, 1.0) * 255.0).round() as u8)
                .collect(),
        )
    }

    fn channel(&self, channel: usize) -> Vec<f32> {
        self.data.iter().skip(channel).step_by(4).copied().collect()
    }

    /// Blurs the image the way the blur of an SVG filter does, which works on 8 bits per channel.
    fn blur(&mut self, deviation_x: f64, deviation_y: f64) {
        if (deviation_x <= 0.0 && deviation_y <= 0.0) || self.width == 0 || self.height == 0 {
            return;
        }
        let mut bytes: Vec<u8> = self
            .data
            .iter()
            .map(|value| (value.clamp(0.0, 1.0) * 255.0).round() as u8)
            .collect();
        gaussian_blur(
            &mut bytes,
            self.width,
            self.height,
            deviation_x.clamp(0.0, 512.0),
            deviation_y.clamp(0.0, 512.0),
        );
        for (value, byte) in self.data.iter_mut().zip(bytes) {
            *value = byte as f32 / 255.0;
        }
    }

    /// Replaces every color by `transform(color)`, which gets and returns colors that are not
    /// premultiplied.
    fn map_colors(&mut self, transform: impl Fn([f32; 4]) -> [f32; 4]) {
        for pixel in self.data.chunks_exact_mut(4) {
            let alpha = pixel[3];
            if alpha <= 0.0 {
                continue;
            }
            let [red, green, blue, alpha] = transform([
                (pixel[0] / alpha).min(1.0),
                (pixel[1] / alpha).min(1.0),
                (pixel[2] / alpha).min(1.0),
                alpha,
            ]);
            let alpha = alpha.clamp(0.0, 1.0);
            pixel[0] = red.clamp(0.0, 1.0) * alpha;
            pixel[1] = green.clamp(0.0, 1.0) * alpha;
            pixel[2] = blue.clamp(0.0, 1.0) * alpha;
            pixel[3] = alpha;
        }
    }

    fn color_matrix(&mut self, matrix: [f32; 9]) {
        self.map_colors(|[red, green, blue, alpha]| {
            [
                matrix[0] * red + matrix[1] * green + matrix[2] * blue,
                matrix[3] * red + matrix[4] * green + matrix[5] * blue,
                matrix[6] * red + matrix[7] * green + matrix[8] * blue,
                alpha,
            ]
        });
    }

    fn drop_shadow(&mut self, offset_x: f64, offset_y: f64, deviation: f64, color: [f32; 4]) {
        let (width, height) = (self.width as isize, self.height as isize);
        let mut shadow = Image {
            width: self.width,
            height: self.height,
            data: self.data.clone(),
        };
        shadow.blur(deviation, deviation);
        let coverage = shadow.channel(3);
        let (offset_x, offset_y) = (offset_x.round() as isize, offset_y.round() as isize);
        let [red, green, blue, alpha] = color;
        for y in 0..height {
            for x in 0..width {
                let (source_x, source_y) = (x - offset_x, y - offset_y);
                if source_x < 0 || source_y < 0 || source_x >= width || source_y >= height {
                    continue;
                }
                let shadow = coverage[(source_y * width + source_x) as usize] * alpha;
                let pixel = &mut self.data[((y * width + x) * 4) as usize..][..4];
                let behind = 1.0 - pixel[3];
                pixel[0] += red * shadow * behind;
                pixel[1] += green * shadow * behind;
                pixel[2] += blue * shadow * behind;
                pixel[3] += shadow * behind;
            }
        }
    }
}

/// Applies `operations`, in order, to the premultiplied RGBA `snapshot`.
pub(crate) fn apply(snapshot: &Snapshot, operations: &[FilterOperation]) -> Snapshot {
    let mut image = Image::from_snapshot(snapshot);
    for operation in operations {
        match operation {
            FilterOperation::Blur(x, y) => image.blur(*x, *y),
            FilterOperation::Brightness(amount) => {
                let amount = amount.max(0.0) as f32;
                image.map_colors(|[red, green, blue, alpha]| {
                    [red * amount, green * amount, blue * amount, alpha]
                });
            },
            FilterOperation::Contrast(amount) => {
                let amount = amount.max(0.0) as f32;
                let intercept = 0.5 - 0.5 * amount;
                image.map_colors(|[red, green, blue, alpha]| {
                    [
                        red * amount + intercept,
                        green * amount + intercept,
                        blue * amount + intercept,
                        alpha,
                    ]
                });
            },
            FilterOperation::Grayscale(amount) => {
                let remaining = 1.0 - amount.clamp(0.0, 1.0) as f32;
                image.color_matrix([
                    0.2126 + 0.7874 * remaining,
                    0.7152 - 0.7152 * remaining,
                    0.0722 - 0.0722 * remaining,
                    0.2126 - 0.2126 * remaining,
                    0.7152 + 0.2848 * remaining,
                    0.0722 - 0.0722 * remaining,
                    0.2126 - 0.2126 * remaining,
                    0.7152 - 0.7152 * remaining,
                    0.0722 + 0.9278 * remaining,
                ]);
            },
            FilterOperation::HueRotate(degrees) => {
                let (sin, cos) = (degrees.to_radians() as f32).sin_cos();
                image.color_matrix([
                    0.213 + cos * 0.787 - sin * 0.213,
                    0.715 - cos * 0.715 - sin * 0.715,
                    0.072 - cos * 0.072 + sin * 0.928,
                    0.213 - cos * 0.213 + sin * 0.143,
                    0.715 + cos * 0.285 + sin * 0.140,
                    0.072 - cos * 0.072 - sin * 0.283,
                    0.213 - cos * 0.213 - sin * 0.787,
                    0.715 - cos * 0.715 + sin * 0.715,
                    0.072 + cos * 0.928 + sin * 0.072,
                ]);
            },
            FilterOperation::Invert(amount) => {
                let amount = amount.clamp(0.0, 1.0) as f32;
                let slope = 1.0 - 2.0 * amount;
                image.map_colors(|[red, green, blue, alpha]| {
                    [
                        red * slope + amount,
                        green * slope + amount,
                        blue * slope + amount,
                        alpha,
                    ]
                });
            },
            FilterOperation::Opacity(amount) => {
                let amount = amount.clamp(0.0, 1.0) as f32;
                image.data.iter_mut().for_each(|value| *value *= amount);
            },
            FilterOperation::Saturate(amount) => {
                let amount = amount.max(0.0) as f32;
                image.color_matrix([
                    0.213 + 0.787 * amount,
                    0.715 - 0.715 * amount,
                    0.072 - 0.072 * amount,
                    0.213 - 0.213 * amount,
                    0.715 + 0.285 * amount,
                    0.072 - 0.072 * amount,
                    0.213 - 0.213 * amount,
                    0.715 - 0.715 * amount,
                    0.072 + 0.928 * amount,
                ]);
            },
            FilterOperation::Sepia(amount) => {
                let remaining = 1.0 - amount.clamp(0.0, 1.0) as f32;
                image.color_matrix([
                    0.393 + 0.607 * remaining,
                    0.769 - 0.769 * remaining,
                    0.189 - 0.189 * remaining,
                    0.349 - 0.349 * remaining,
                    0.686 + 0.314 * remaining,
                    0.168 - 0.168 * remaining,
                    0.272 - 0.272 * remaining,
                    0.534 - 0.534 * remaining,
                    0.131 + 0.869 * remaining,
                ]);
            },
            FilterOperation::DropShadow {
                offset_x,
                offset_y,
                std_deviation,
                color,
            } => {
                let color = color.into_srgb_legacy();
                let channel = |value: f32| {
                    if value.is_nan() {
                        0.0
                    } else {
                        value.clamp(0.0, 1.0)
                    }
                };
                image.drop_shadow(
                    *offset_x,
                    *offset_y,
                    *std_deviation,
                    [
                        channel(color.components.0),
                        channel(color.components.1),
                        channel(color.components.2),
                        channel(color.alpha),
                    ],
                );
            },
        }
    }
    image.into_snapshot()
}
