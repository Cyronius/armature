/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Rendering of the shadows of canvas drawing operations.
//!
//! <https://html.spec.whatwg.org/multipage/#drawing-model>

use euclid::default::Size2D;
use pixels::{Snapshot, SnapshotAlphaMode, SnapshotPixelFormat};

/// The largest standard deviation we are willing to blur with. Anything bigger would allocate more
/// memory than any canvas could ever show.
const MAX_SIGMA: f64 = 512.0;

/// The number of pixels that blurring with `sigma` can move coverage away from a pixel.
pub(crate) fn blur_margin(blur: f64) -> u32 {
    let sigma = (blur / 2.0).clamp(0.0, MAX_SIGMA);
    if sigma <= 0.0 {
        0
    } else {
        (sigma * 3.0).ceil() as u32 + 1
    }
}

/// Builds the shadow of `source`, a premultiplied RGBA8 snapshot. The result is larger than `source`
/// by [`blur_margin`] pixels on every side, contains `color` (with its alpha multiplied by the
/// coverage of the blurred `source`) and is premultiplied.
///
/// The shadow of `source` is its alpha channel blurred with a Gaussian of standard deviation
/// `blur / 2`.
pub(crate) fn render_shadow(source: &Snapshot, blur: f64, color: [f32; 4]) -> Snapshot {
    let sigma = (blur / 2.0).clamp(0.0, MAX_SIGMA);
    let margin = blur_margin(blur) as usize;
    let source_size = source.size();
    let (source_width, source_height) = (source_size.width as usize, source_size.height as usize);
    let width = source_width + 2 * margin;
    let height = source_height + 2 * margin;

    let mut coverage = vec![0.0f32; width * height];
    let bytes = source.as_raw_bytes();
    for y in 0..source_height {
        let row = &mut coverage[(y + margin) * width + margin..][..source_width];
        for (x, value) in row.iter_mut().enumerate() {
            *value = bytes[(y * source_width + x) * 4 + 3] as f32 / 255.0;
        }
    }

    if sigma > 0.0 {
        blur_coverage(&mut coverage, width, height, sigma);
    }

    let [red, green, blue, alpha] = color;
    let mut data = Vec::with_capacity(width * height * 4);
    for value in coverage {
        let alpha = (value * alpha).clamp(0.0, 1.0);
        data.push((red * alpha * 255.0).round() as u8);
        data.push((green * alpha * 255.0).round() as u8);
        data.push((blue * alpha * 255.0).round() as u8);
        data.push((alpha * 255.0).round() as u8);
    }

    Snapshot::from_vec(
        Size2D::new(width as u32, height as u32),
        SnapshotPixelFormat::RGBA,
        SnapshotAlphaMode::Transparent {
            premultiplied: true,
        },
        data,
    )
}

/// Blurs `coverage` in place with a Gaussian of standard deviation `sigma`.
///
/// Small deviations use the exact kernel, larger ones three box blurs, as described by
/// <https://drafts.fxtf.org/filter-effects/#feGaussianBlurElement>.
fn blur_coverage(coverage: &mut [f32], width: usize, height: usize, sigma: f64) {
    let mut scratch = vec![0.0f32; coverage.len()];
    if sigma < 2.0 {
        let radius = (sigma * 3.0).ceil() as usize;
        let mut kernel: Vec<f32> = (0..=2 * radius)
            .map(|i| {
                let distance = i as f64 - radius as f64;
                (-distance * distance / (2.0 * sigma * sigma)).exp() as f32
            })
            .collect();
        let sum: f32 = kernel.iter().sum();
        kernel.iter_mut().for_each(|weight| *weight /= sum);
        convolve(coverage, &mut scratch, width, height, &kernel, radius, true);
        convolve(&scratch, coverage, width, height, &kernel, radius, false);
        return;
    }

    let size = (sigma * 3.0 * (2.0 * std::f64::consts::PI).sqrt() / 4.0 + 0.5).floor() as usize;
    let size = size.max(1);
    // Each pass is `(start offset, window length)` relative to the output pixel.
    let passes: [(isize, usize); 3] = if size % 2 == 1 {
        let start = -((size as isize - 1) / 2);
        [(start, size); 3]
    } else {
        let half = (size / 2) as isize;
        [(-half, size), (-half + 1, size), (-half, size + 1)]
    };
    for (start, length) in passes {
        box_pass(coverage, &mut scratch, width, height, start, length, true);
        box_pass(&scratch, coverage, width, height, start, length, false);
    }
}

fn convolve(
    source: &[f32],
    destination: &mut [f32],
    width: usize,
    height: usize,
    kernel: &[f32],
    radius: usize,
    horizontal: bool,
) {
    let (length, lines, stride, line_stride) = if horizontal {
        (width, height, 1, width)
    } else {
        (height, width, width, 1)
    };
    for line in 0..lines {
        let base = line * line_stride;
        for i in 0..length {
            let mut sum = 0.0;
            let first = i.saturating_sub(radius);
            let last = (i + radius).min(length - 1);
            for j in first..=last {
                sum += source[base + j * stride] * kernel[j + radius - i];
            }
            destination[base + i * stride] = sum;
        }
    }
}

/// Replaces every pixel by the average of the `length` pixels starting `start` pixels away from it
/// (pixels beyond the edges count as zero).
fn box_pass(
    source: &[f32],
    destination: &mut [f32],
    width: usize,
    height: usize,
    start: isize,
    length: usize,
    horizontal: bool,
) {
    let (count, lines, stride, line_stride) = if horizontal {
        (width as isize, height, 1, width)
    } else {
        (height as isize, width, width, 1)
    };
    let scale = 1.0 / length as f32;
    let end = start + length as isize;
    for line in 0..lines {
        let base = line * line_stride;
        let value = |index: isize| -> f32 {
            if index < 0 || index >= count {
                0.0
            } else {
                source[base + index as usize * stride]
            }
        };
        // Window of pixel `0` is `start..end`.
        let mut sum: f32 = (start..end).map(value).sum();
        for i in 0..count {
            destination[base + i as usize * stride] = sum * scale;
            sum += value(i + end) - value(i + start);
        }
    }
}
