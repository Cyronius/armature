/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::Cell;

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix, local_name, ns};
use js::context::JSContext;
use js::rust::HandleObject;
use net_traits::image_cache::{
    Image, ImageCacheResult, ImageLoadListener, ImageOrMetadataAvailable, ImageResponse,
    VectorImageNaturalDimensions,
};
use net_traits::request::InternalRequest;
use script_bindings::cell::DomRefCell;
use style::attr::AttrValue;

use crate::dom::bindings::codegen::Bindings::SVGImageElementBinding::SVGImageElementMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::refcounted::Trusted;
use crate::dom::bindings::root::DomRoot;
use crate::dom::bindings::str::DOMString;
use crate::dom::document::Document;
use crate::dom::element::attributes::storage::AttrRef;
use crate::dom::element::{AttributeMutation, Element};
use crate::dom::event::eventtarget::EventTarget;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::node::{BindContext, Node, NodeTraits};
use crate::dom::performance::performanceresourcetiming::InitiatorType;
use crate::dom::svg::svggraphicselement::SVGGraphicsElement;
use crate::dom::svg::svgtypes::{
    SVGLengthValue, svg_animated_length, svg_preserve_aspect_ratio, svg_uri_reference,
};
use crate::dom::window::layout_image::fetch_image_into_cache;
use crate::event_loop::document_loader::{LoadBlocker, LoadType};

/// <https://svgwg.org/svg2-draft/embedded.html#Placement>
const DEFAULT_WIDTH: u32 = 300;
const DEFAULT_HEIGHT: u32 = 150;

#[dom_struct]
pub(crate) struct SVGImageElement {
    svggraphicselement: SVGGraphicsElement,
    load_blocker: DomRefCell<Option<LoadBlocker>>,
    /// Incremented whenever the image resource is processed again, so that the responses of
    /// earlier requests no longer fire events at the element.
    request_generation: Cell<u32>,
}

/// The outcome of fetching the image resource of an `image` element.
#[derive(Clone, Copy)]
enum ImageLoadOutcome {
    Loaded,
    Failed,
}

impl SVGImageElement {
    fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGImageElement {
        SVGImageElement {
            svggraphicselement: SVGGraphicsElement::new_inherited(local_name, prefix, document),
            load_blocker: DomRefCell::new(None),
            request_generation: Cell::new(0),
        }
    }

    pub(crate) fn new(
        cx: &mut js::context::JSContext,
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
        proto: Option<HandleObject>,
    ) -> DomRoot<SVGImageElement> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(SVGImageElement::new_inherited(local_name, prefix, document)),
            document,
            proto,
        )
    }

    /// The URL of the image resource: the `href` attribute, or the deprecated `xlink:href`
    /// attribute when there is no `href` attribute.
    /// <https://svgwg.org/svg2-draft/linking.html#XLinkHrefAttribute>
    fn image_href(&self) -> Option<String> {
        let element = self.upcast::<Element>();
        element
            .get_attribute_string_value(&local_name!("href"))
            .or_else(|| {
                element.get_attribute_string_value_with_namespace(&ns!(xlink), &local_name!("href"))
            })
    }

    /// The natural dimensions of the current resource, including available raster metadata.
    pub(crate) fn natural_dimensions(&self) -> Option<VectorImageNaturalDimensions> {
        let href = self.image_href()?;
        let node = self.upcast::<Node>();
        let url = node.owner_doc().encoding_parse_a_url(&href).ok()?;
        let window = self.owner_window();
        let cache_result =
            window
                .image_cache()
                .get_cached_image_status(url, window.origin().immutable(), None);
        let dimensions = |width: u32, height: u32| {
            let width = width as f32;
            let height = height as f32;
            VectorImageNaturalDimensions {
                width: Some(width),
                height: Some(height),
                ratio: if width > 0.0 && height > 0.0 {
                    Some(width / height)
                } else {
                    None
                },
            }
        };
        Some(match cache_result {
            ImageCacheResult::Available(ImageOrMetadataAvailable::ImageAvailable {
                image, ..
            }) => match image {
                Image::Vector(image) => image.natural_dimensions,
                Image::Raster(image) => dimensions(image.metadata.width, image.metadata.height),
            },
            ImageCacheResult::Available(ImageOrMetadataAvailable::MetadataAvailable(
                metadata,
                _,
            )) => dimensions(metadata.width, metadata.height),
            _ => return None,
        })
    }

    /// Fetch the image resource and fire a `load` or `error` event at the element once it has
    /// been fetched and decoded, as browsers do for the SVG `image` element.
    ///
    /// <https://svgwg.org/svg2-draft/linking.html#processingURL>
    /// <https://svgwg.org/svg2-draft/embedded.html#ImageElement>
    ///
    /// Only the events come from this request: the image is still drawn by the renderer of the
    /// enclosing `svg` element.
    fn process_image_resource(&self, cx: &mut JSContext) {
        // Responses to any earlier request are no longer relevant.
        let generation = self.request_generation.get().wrapping_add(1);
        self.request_generation.set(generation);
        LoadBlocker::terminate(cx, &self.load_blocker);

        // Only connected elements in a document with a browsing context start a fetch (a request
        // that has started still fires its event if the element is removed). The
        // copies of the subtree that the enclosing `svg` element makes to render it are not
        // connected, so they do not fetch or fire events (their `onload` attributes would run
        // again). Browsers also fetch for elements that are not connected, which this does not.
        let node = self.upcast::<Node>();
        if !node.is_connected() || !node.owner_doc().has_browsing_context() {
            return;
        }

        // A missing or empty URL refers to no resource, and fires no event.
        let Some(href) = self.image_href().filter(|href| !href.is_empty()) else {
            return;
        };

        let Ok(url) = node.owner_doc().encoding_parse_a_url(&href) else {
            self.queue_image_load_event(generation, ImageLoadOutcome::Failed);
            return;
        };

        let window = self.owner_window();
        *self.load_blocker.safe_borrow_mut(cx.no_gc()) = Some(LoadBlocker::new(
            &node.owner_doc(),
            LoadType::Image(url.clone()),
        ));
        let image_cache = window.image_cache();
        let cache_result =
            image_cache.get_cached_image_status(url.clone(), window.origin().immutable(), None);
        let id = match cache_result {
            ImageCacheResult::Available(ImageOrMetadataAvailable::ImageAvailable { .. }) => {
                self.queue_image_load_event(generation, ImageLoadOutcome::Loaded);
                return;
            },
            ImageCacheResult::FailedToLoadOrDecode => {
                self.queue_image_load_event(generation, ImageLoadOutcome::Failed);
                return;
            },
            ImageCacheResult::Available(ImageOrMetadataAvailable::MetadataAvailable(_, id)) |
            ImageCacheResult::Pending(id) => id,
            ImageCacheResult::ReadyForRequest(id) => {
                fetch_image_into_cache(
                    url,
                    node,
                    id,
                    InternalRequest::No,
                    image_cache.clone(),
                    InitiatorType::LocalName(local_name!("image")),
                );
                id
            },
        };

        let trusted_element = Trusted::new(self);
        let callback = window.register_image_cache_listener(id, move |response, _| {
            let outcome = match response.response {
                ImageResponse::Loaded(..) => ImageLoadOutcome::Loaded,
                ImageResponse::FailedToLoadOrDecode => ImageLoadOutcome::Failed,
                ImageResponse::MetadataLoaded(_) => return,
            };
            trusted_element
                .root()
                .queue_image_load_event(generation, outcome);
        });
        image_cache.add_listener(ImageLoadListener::new(callback, window.pipeline_id(), id));
    }

    /// Queue a task to fire `load` or `error` at the element, unless the element has started
    /// another request by the time the task runs.
    fn queue_image_load_event(&self, generation: u32, outcome: ImageLoadOutcome) {
        let trusted_element = Trusted::new(self);
        self.owner_global()
            .task_manager()
            .dom_manipulation_task_source()
            .queue(task!(fire_svg_image_load_event: move |cx| {
                let element = trusted_element.root();
                if element.request_generation.get() != generation {
                    return;
                }
                LoadBlocker::terminate(cx, &element.load_blocker);
                let name = match outcome {
                    ImageLoadOutcome::Loaded => atom!("load"),
                    ImageLoadOutcome::Failed => atom!("error"),
                };
                element.upcast::<EventTarget>().fire_event(cx, name);
            }));
    }
}

impl VirtualMethods for SVGImageElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<SVGGraphicsElement>() as &dyn VirtualMethods)
    }

    fn attribute_mutated(
        &self,
        cx: &mut js::context::JSContext,
        attr: AttrRef<'_>,
        mutation: AttributeMutation,
    ) {
        self.super_type()
            .unwrap()
            .attribute_mutated(cx, attr, mutation);
        if attr.local_name() == &local_name!("href") &&
            matches!(attr.namespace(), &ns!() | &ns!(xlink))
        {
            self.process_image_resource(cx);
        }
    }

    fn bind_to_tree(&self, cx: &mut JSContext, context: &BindContext) {
        if let Some(super_type) = self.super_type() {
            super_type.bind_to_tree(cx, context);
        }
        if context.tree_connected {
            self.process_image_resource(cx);
        }
    }

    fn attribute_affects_presentational_hints(&self, attr: AttrRef<'_>) -> bool {
        match attr.local_name() {
            &local_name!("width") | &local_name!("height") => true,
            _ => self
                .super_type()
                .unwrap()
                .attribute_affects_presentational_hints(attr),
        }
    }

    fn parse_plain_attribute(&self, name: &LocalName, value: DOMString) -> AttrValue {
        match *name {
            local_name!("width") => AttrValue::from_u32(value.into(), DEFAULT_WIDTH),
            local_name!("height") => AttrValue::from_u32(value.into(), DEFAULT_HEIGHT),
            _ => self
                .super_type()
                .unwrap()
                .parse_plain_attribute(name, value),
        }
    }
}

impl SVGImageElementMethods<crate::DomTypeHolder> for SVGImageElement {
    svg_preserve_aspect_ratio!();

    svg_animated_length!(X, "x", Horizontal, SVGLengthValue::number(0.0));
    svg_animated_length!(Y, "y", Vertical, SVGLengthValue::number(0.0));
    svg_animated_length!(Width, "width", Horizontal, SVGLengthValue::number(0.0));
    svg_animated_length!(Height, "height", Vertical, SVGLengthValue::number(0.0));

    // <https://svgwg.org/svg2-draft/types.html#__svg__SVGURIReference__href>
    svg_uri_reference!();
}
