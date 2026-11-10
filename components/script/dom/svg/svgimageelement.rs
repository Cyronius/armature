/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::Cell;

use base64::Engine as _;
use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix, local_name, ns};
use js::context::JSContext;
use js::rust::HandleObject;
use net_traits::image_cache::{
    Image, ImageCacheResult, ImageLoadListener, ImageOrMetadataAvailable, ImageResponse,
    VectorImageNaturalDimensions,
};
use net_traits::request::Destination;
use pixels::{CorsStatus, EncodedImageType, Snapshot};
use script_bindings::cell::DomRefCell;
use style::attr::AttrValue;

use crate::dom::bindings::codegen::Bindings::SVGImageElementBinding::SVGImageElementMethods;
use crate::dom::bindings::error::{Error, Fallible};
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::refcounted::Trusted;
use crate::dom::bindings::root::DomRoot;
use crate::dom::bindings::str::DOMString;
use crate::dom::document::Document;
use crate::dom::element::attributes::storage::AttrRef;
use crate::dom::element::{
    AttributeMutation, Element, cors_setting_for_element, referrer_policy_for_element,
    reflect_cross_origin_attribute, reflect_fetch_priority_attribute, set_cross_origin_attribute,
};
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
use crate::fetch::fetch::{RequestWithGlobalScope, create_a_potential_cors_request};

/// <https://svgwg.org/svg2-draft/embedded.html#Placement>
const DEFAULT_WIDTH: u32 = 300;
const DEFAULT_HEIGHT: u32 = 150;

thread_local! {
    static SUPPRESS_IMAGE_REQUESTS: Cell<bool> = const { Cell::new(false) };
}

/// SVG rendering clones the live subtree into its document. Those private copies must not
/// start image requests or fire image events.
pub(crate) struct SVGImageRequestSuppression(bool);

impl SVGImageRequestSuppression {
    pub(crate) fn new() -> Self {
        Self(SUPPRESS_IMAGE_REQUESTS.with(|suppressed| suppressed.replace(true)))
    }
}

impl Drop for SVGImageRequestSuppression {
    fn drop(&mut self) {
        SUPPRESS_IMAGE_REQUESTS.with(|suppressed| suppressed.set(self.0));
    }
}

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

    fn cached_image(&self) -> Option<Image> {
        let href = self.image_href()?;
        let node = self.upcast::<Node>();
        let url = node.owner_doc().encoding_parse_a_url(&href).ok()?;
        let window = self.owner_window();
        window.image_cache().get_image(
            url,
            window.origin().immutable().clone(),
            cors_setting_for_element(self.upcast()),
        )
    }

    pub(crate) fn get_raster_image_data(&self) -> Option<Snapshot> {
        let image = self.cached_image()?;
        let raster_image = match image {
            Image::Raster(image) => image.as_ref().clone(),
            Image::Vector(image) => {
                let size = webrender_api::units::DeviceIntSize::new(
                    i32::try_from(image.metadata.width).unwrap_or(i32::MAX),
                    i32::try_from(image.metadata.height).unwrap_or(i32::MAX),
                );
                self.owner_window()
                    .image_cache()
                    .rasterize_vector_image_for_canvas(image.id, size)?
            },
        };
        Some(raster_image.as_snapshot())
    }

    /// The SVG rasterizer cannot fetch URLs from the serialized data URL used for an inline SVG.
    /// Give its rendering copy the image already fetched by the document instead.
    pub(crate) fn external_image_data_url(&self) -> Option<String> {
        let href = self.image_href()?;
        let url = self
            .upcast::<Node>()
            .owner_doc()
            .encoding_parse_a_url(&href)
            .ok()?;
        if url.scheme() == "data" {
            return None;
        }
        let mut image = self.get_raster_image_data()?;
        let mut encoded = Vec::new();
        image
            .encode_for_mime_type(&EncodedImageType::Png, None, &mut encoded)
            .ok()?;
        Some(format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(encoded)
        ))
    }

    pub(crate) fn origin_is_clean(&self) -> bool {
        self.cached_image()
            .is_some_and(|image| image.cors_status() == CorsStatus::Safe)
    }

    pub(crate) fn is_usable(&self) -> Fallible<bool> {
        let Some(href) = self.image_href().filter(|href| !href.is_empty()) else {
            return Ok(false);
        };
        let node = self.upcast::<Node>();
        let Ok(url) = node.owner_doc().encoding_parse_a_url(&href) else {
            return Err(Error::InvalidState(Some("SVG image URL is invalid".into())));
        };
        let window = self.owner_window();
        let status = window.image_cache().get_cached_image_status(
            url,
            window.origin().immutable(),
            cors_setting_for_element(self.upcast()),
        );
        match status {
            ImageCacheResult::FailedToLoadOrDecode => Err(Error::InvalidState(Some(
                "SVG image failed to load or decode".into(),
            ))),
            ImageCacheResult::Available(ImageOrMetadataAvailable::ImageAvailable {
                image, ..
            }) => {
                let metadata = image.metadata();
                Ok(metadata.width > 0 && metadata.height > 0)
            },
            _ => Ok(false),
        }
    }

    /// The natural dimensions of the current resource, including available raster metadata.
    pub(crate) fn natural_dimensions(&self) -> Option<VectorImageNaturalDimensions> {
        let href = self.image_href()?;
        let node = self.upcast::<Node>();
        let url = node.owner_doc().encoding_parse_a_url(&href).ok()?;
        let window = self.owner_window();
        let cache_result = window.image_cache().get_cached_image_status(
            url,
            window.origin().immutable(),
            cors_setting_for_element(self.upcast()),
        );
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
        if SUPPRESS_IMAGE_REQUESTS.with(Cell::get) {
            return;
        }
        // Responses to any earlier request are no longer relevant.
        let generation = self.request_generation.get().wrapping_add(1);
        self.request_generation.set(generation);
        LoadBlocker::terminate(cx, &self.load_blocker);

        // Detached elements also fetch, as HTML images do. Rendering copies are suppressed above.
        let node = self.upcast::<Node>();
        if !node.owner_doc().has_browsing_context() {
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
        let cors_setting = cors_setting_for_element(self.upcast());
        *self.load_blocker.safe_borrow_mut(cx.no_gc()) = Some(LoadBlocker::new(
            &node.owner_doc(),
            LoadType::Image(url.clone()),
        ));
        let image_cache = window.image_cache();
        let cache_result = image_cache.get_cached_image_status(
            url.clone(),
            window.origin().immutable(),
            cors_setting,
        );
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
                let global = node.owner_global();
                let request = create_a_potential_cors_request(
                    Some(window.webview_id()),
                    url.clone(),
                    Destination::Image,
                    cors_setting,
                    None,
                    global.get_referrer(),
                )
                .with_global_scope(&global)
                .referrer_policy(referrer_policy_for_element(self.upcast()));
                fetch_image_into_cache(
                    url,
                    node,
                    id,
                    image_cache.clone(),
                    InitiatorType::LocalName(local_name!("image")),
                    request,
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
                if matches!(outcome, ImageLoadOutcome::Loaded) {
                    element
                        .upcast::<crate::dom::svg::svgelement::SVGElement>()
                        .invalidate_ancestor_svg_cache(cx.no_gc());
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
            matches!(attr.namespace(), &ns!() | &ns!(xlink)) ||
            attr.local_name() == &local_name!("crossorigin") && *attr.namespace() == ns!()
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

    /// <https://svgwg.org/svg2-draft/embedded.html#ImageElementCrossoriginAttribute>
    fn GetCrossOrigin(&self) -> Option<DOMString> {
        reflect_cross_origin_attribute(self.upcast::<Element>())
    }

    /// <https://svgwg.org/svg2-draft/embedded.html#ImageElementCrossoriginAttribute>
    fn SetCrossOrigin(&self, cx: &mut JSContext, value: Option<DOMString>) {
        set_cross_origin_attribute(cx, self.upcast::<Element>(), value);
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-img-fetchpriority>
    fn FetchPriority(&self) -> DOMString {
        reflect_fetch_priority_attribute(self.upcast::<Element>())
    }

    // <https://html.spec.whatwg.org/multipage/#dom-img-fetchpriority>
    make_setter!(SetFetchPriority, "fetchpriority");
}
