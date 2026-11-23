/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The Gaussian blur of the filters of SVG, which is also the one of the filters of canvases, so
//! that they render the same.
//!
//! The blurs are those of resvg, which ours is not shared with because it is private.

mod box_blur;
mod iir_blur;

/// A pixel of an image whose channels are red, green, blue and alpha, which is premultiplied.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub(super) struct RGBA8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

pub(super) struct ImageRefMut<'a> {
    pub width: u32,
    pub height: u32,
    pub data: &'a mut [RGBA8],
}

impl<'a> ImageRefMut<'a> {
    pub fn new(width: u32, height: u32, data: &'a mut [RGBA8]) -> Self {
        Self {
            width,
            height,
            data,
        }
    }
}

/// Blurs the premultiplied RGBA8 `data`, which is `width` by `height` pixels, with a Gaussian
/// of standard deviation `sigma_x` along the x axis and `sigma_y` along the y axis.
pub(crate) fn gaussian_blur(
    data: &mut [u8],
    width: usize,
    height: usize,
    sigma_x: f64,
    sigma_y: f64,
) {
    let mut sigma_x = sigma_x as f32;
    let mut sigma_y = sigma_y as f32;

    // 'A negative value or a value of zero disables the effect of the given filter primitive
    // (i.e., the result is the filter input image).'
    if sigma_x <= 0.0 && sigma_y <= 0.0 {
        return;
    }

    // Ignore tiny deviations. In case of IIR blur it can lead to a transparent image.
    if sigma_x < 0.05 {
        sigma_x = 0.0;
    }
    if sigma_y < 0.05 {
        sigma_y = 0.0;
    }

    const BLUR_SIGMA_THRESHOLD: f32 = 2.0;
    let use_box_blur = sigma_x >= BLUR_SIGMA_THRESHOLD || sigma_y >= BLUR_SIGMA_THRESHOLD;

    let mut pixels: Vec<RGBA8> = data
        .chunks_exact(4)
        .map(|pixel| RGBA8 {
            r: pixel[0],
            g: pixel[1],
            b: pixel[2],
            a: pixel[3],
        })
        .collect();
    let image = ImageRefMut::new(width as u32, height as u32, &mut pixels);
    if use_box_blur {
        box_blur::apply(sigma_x as f64, sigma_y as f64, image);
    } else {
        iir_blur::apply(sigma_x as f64, sigma_y as f64, image);
    }
    for (pixel, blurred) in data.chunks_exact_mut(4).zip(&pixels) {
        pixel.copy_from_slice(&[blurred.r, blurred.g, blurred.b, blurred.a]);
    }
}
