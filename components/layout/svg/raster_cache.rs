/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashMap;
use std::sync::{Arc, mpsc};

use paint_api::{CrossProcessPaintApi, SerializableImageData};
use servo_base::generic_channel::GenericSharedMemory;
use servo_base::id::WebViewId;
use servo_base::threadpool::ThreadPool;
use servo_svg::document::SvgTree;
use webrender_api::units::DeviceIntSize;
use webrender_api::{ImageDescriptor, ImageDescriptorFlags, ImageFormat, ImageKey};

type RasterKey = (usize, u32, u32, u32);

struct Raster {
    key: ImageKey,
    // Keep the identity used by the cache key alive until the raster is discarded.
    _tree: Arc<SvgTree>,
}

pub(crate) struct SvgRasterCache {
    paint_api: CrossProcessPaintApi,
    webview_id: WebViewId,
    previous: HashMap<RasterKey, Raster>,
    current: HashMap<RasterKey, Raster>,
}

impl SvgRasterCache {
    pub(crate) fn new(paint_api: CrossProcessPaintApi, webview_id: WebViewId) -> Self {
        Self {
            paint_api,
            webview_id,
            previous: HashMap::new(),
            current: HashMap::new(),
        }
    }

    pub(crate) fn image_key(
        &mut self,
        tree: &Arc<SvgTree>,
        width: f32,
        height: f32,
        scale: f32,
    ) -> Option<ImageKey> {
        let id = (
            Arc::as_ptr(tree) as usize,
            width.to_bits(),
            height.to_bits(),
            scale.to_bits(),
        );
        if let Some(raster) = self.current.get(&id) {
            return Some(raster.key);
        }
        let raster = match self.previous.remove(&id) {
            Some(raster) => raster,
            None => self.rasterize(tree, width, height, scale)?,
        };
        let key = raster.key;
        self.current.insert(id, raster);
        Some(key)
    }

    fn rasterize(
        &self,
        tree: &Arc<SvgTree>,
        width: f32,
        height: f32,
        scale: f32,
    ) -> Option<Raster> {
        let source = Arc::clone(tree);
        let (sender, receiver) = mpsc::channel();
        ThreadPool::global().spawn(move || {
            let raster =
                servo_svg::render_svg_tree(&source, width, height, scale).and_then(|mut pixmap| {
                    let size = DeviceIntSize::new(pixmap.width() as i32, pixmap.height() as i32);
                    // tiny-skia produces premultiplied RGBA; the compositor expects BGRA.
                    let mut visible_pixels = false;
                    for pixel in pixmap.data_mut().chunks_exact_mut(4) {
                        visible_pixels |= pixel[3] != 0;
                        pixel.swap(0, 2);
                    }
                    visible_pixels.then(|| (size, GenericSharedMemory::from_vec(pixmap.take())))
                });
            let _ = sender.send(raster);
        });
        let (size, pixels) = receiver.recv().ok()??;
        let key = self
            .paint_api
            .generate_image_key_blocking(self.webview_id)?;
        self.paint_api.add_image(
            key,
            ImageDescriptor {
                size,
                stride: None,
                format: ImageFormat::BGRA8,
                offset: 0,
                flags: ImageDescriptorFlags::empty(),
            },
            SerializableImageData::Raw(pixels),
            false,
        );
        Some(Raster {
            key,
            _tree: Arc::clone(tree),
        })
    }

    /// Call after sending the new display list to release images only used by its predecessor.
    pub(crate) fn finish_display_list(&mut self) {
        for (_, raster) in self.previous.drain() {
            self.paint_api.delete_image(raster.key);
        }
        std::mem::swap(&mut self.current, &mut self.previous);
    }
}

impl Drop for SvgRasterCache {
    fn drop(&mut self) {
        for raster in self.previous.values().chain(self.current.values()) {
            self.paint_api.delete_image(raster.key);
        }
    }
}
