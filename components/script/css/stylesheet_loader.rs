/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

#![cfg_attr(crown, expect(crown::jscontext_first_arg))]

use std::io::{Read, Seek, Write};
use std::sync::atomic::{AtomicUsize, Ordering};

use bytes::{Bytes, BytesMut};
use cssparser::SourceLocation;
use encoding_rs::Encoding;
use html5ever::local_name;
use js::context::JSContext;
use net_traits::mime_classifier::MimeClassifier;
use net_traits::request::{CorsSettings, Destination, RequestId};
use net_traits::{
    FetchMetadata, LoadContext, Metadata, NetworkError, ReferrerPolicy, ResourceFetchTiming,
};
use pixels::CorsStatus;
use servo_arc::Arc;
use servo_config::pref;
use servo_url::ServoUrl;
use style::context::QuirksMode;
use style::global_style_data::STYLE_THREAD_POOL;
use style::media_queries::MediaList;
use style::shared_lock::{Locked, SharedRwLock};
use style::stylesheets::import_rule::{ImportLayer, ImportSheet, ImportSupportsCondition};
use style::stylesheets::{
    ImportRule, Origin, Stylesheet, StylesheetLoader as StyleStylesheetLoader, StylesheetReferrer,
    UrlExtraData,
};
use style::values::CssUrl;

use crate::css::css::stylesheet_referrer;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::refcounted::Trusted;
use crate::dom::bindings::reflector::DomGlobal;
use crate::dom::csp::{GlobalCspReporting, Violation};
use crate::dom::document::Document;
use crate::dom::element::Element;
use crate::dom::eventtarget::EventTarget;
use crate::dom::globalscope::GlobalScope;
use crate::dom::html::htmllinkelement::{HTMLLinkElement, RequestGenerationId};
use crate::dom::node::{Node, NodeTraits};
use crate::dom::performance::performanceresourcetiming::InitiatorType;
use crate::dom::shadowroot::shadowroot::ShadowRoot;
use crate::dom::types::ProcessingInstruction;
use crate::dom::window::CSSErrorReporter;
use crate::event_loop::document_loader::LoadType;
use crate::fetch::fetch::{RequestWithGlobalScope, create_a_potential_cors_request};
use crate::fetch::network_listener::{self, FetchResponseListener, ResourceTimingListener};
use crate::tasks::task_source::SendableTaskSource;
use crate::unminify::{
    BeautifyFileType, create_output_file, create_temp_files, execute_js_beautify,
};

/// An struct which is used to uniquely identify a [`StylesheetContext`] for
/// tracking the set of script-blocking stylesheets.
#[derive(Clone, Copy, Eq, Hash, JSTraceable, MallocSizeOf, PartialEq)]
pub(crate) struct StylesheetContextId(usize);

impl StylesheetContextId {
    fn next() -> Self {
        static NEXT_STYLESHEET_CONTEXT_INDEX: AtomicUsize = AtomicUsize::new(0);
        Self(NEXT_STYLESHEET_CONTEXT_INDEX.fetch_add(1, Ordering::Relaxed))
    }
}

pub(crate) trait StylesheetOwner {
    /// Returns whether this element was inserted by the parser (i.e., it should
    /// trigger a document-load-blocking load).
    fn parser_inserted(&self) -> bool;

    /// <https://html.spec.whatwg.org/multipage/#potentially-render-blocking>
    fn potentially_render_blocking(&self) -> bool;

    /// Which referrer policy should loads triggered by this owner follow
    fn referrer_policy(&self, cx: &mut JSContext) -> ReferrerPolicy;

    /// Notes that a new load is pending to finish.
    fn increment_pending_loads_count(&self);

    /// Returns None if there are still pending loads, or whether any load has
    /// failed since the loads started.
    fn load_finished(&self, successful: bool) -> Option<bool>;

    /// Sets origin_clean flag.
    fn set_origin_clean(&self, cx: &mut JSContext, origin_clean: bool);

    /// Whether the owner's media (the `media` attribute of an element, or the `media`
    /// pseudo-attribute of a processing instruction) matches the environment.
    fn media_matches_environment(&self) -> bool;
}

pub(crate) enum StylesheetContextSource {
    LinkElement,
    /// The style sheet of an `xml-stylesheet` processing instruction.
    ProcessingInstruction,
    /// An `@import` rule, and the referrer of the external style sheet that contains it, if any.
    Import(Arc<Locked<ImportRule>>, Option<Arc<StylesheetReferrer>>),
}

/// The context required for asynchronously loading an external stylesheet.
struct StylesheetContext {
    /// The id associated with this [`StylesheetContext`]. This is used to uniquely identify
    /// it in the `Document`'s script-blocking stylesheet set.
    id: StylesheetContextId,
    /// The element or processing instruction that initiated the request.
    element: Trusted<Node>,
    source: StylesheetContextSource,
    media: Arc<Locked<MediaList>>,
    url: ServoUrl,
    metadata: Option<Metadata>,
    /// The response body received to date.
    data: BytesMut,
    /// The node document for elem when the load was initiated.
    document: Trusted<Document>,
    shadow_root: Option<Trusted<ShadowRoot>>,
    origin_clean: bool,
    /// A token which must match the generation id of the `HTMLLinkElement` or `ProcessingInstruction`
    /// for it to load the stylesheet. This is ignored for `HTMLStyleElement` and imports.
    request_generation_id: Option<RequestGenerationId>,
    /// <https://html.spec.whatwg.org/multipage/#contributes-a-script-blocking-style-sheet>
    is_script_blocking: bool,
    /// <https://html.spec.whatwg.org/multipage/#render-blocking>
    is_render_blocking: bool,
}

impl StylesheetContext {
    fn unminify_css(&mut self, document: &Document, file_url: ServoUrl) {
        let Some(unminified_dir) = document.window().unminified_css_dir() else {
            return;
        };

        let mut style_content = std::mem::take(&mut self.data).to_vec();
        if let Some((input, mut output)) = create_temp_files() &&
            execute_js_beautify(
                input.path(),
                output.try_clone().unwrap(),
                BeautifyFileType::Css,
            )
        {
            output.seek(std::io::SeekFrom::Start(0)).unwrap();
            output.read_to_end(&mut style_content).unwrap();
        }
        match create_output_file(unminified_dir, &file_url, None) {
            Ok(mut file) => {
                file.write_all(&style_content).unwrap();
            },
            Err(why) => {
                log::warn!("Could not store script {:?}", why);
            },
        }

        self.data = Bytes::copy_from_slice(&style_content)
            .try_into_mut()
            .unwrap();
    }

    fn empty_stylesheet(&self, document: &Document) -> Arc<Stylesheet> {
        let shared_lock = document.style_shared_author_lock().clone();
        let quirks_mode = document.quirks_mode();

        Arc::new(Stylesheet::from_bytes(
            &[],
            UrlExtraData(self.url.get_arc()),
            None,
            None,
            Origin::Author,
            self.media.clone(),
            shared_lock,
            None,
            None,
            quirks_mode,
        ))
    }

    fn parse(
        &self,
        environment_encoding: &'static Encoding,
        quirks_mode: QuirksMode,
        shared_lock: SharedRwLock,
        css_error_reporter: &CSSErrorReporter,
        loader: ElementStylesheetLoader<'_>,
    ) -> Arc<Stylesheet> {
        let metadata = self
            .metadata
            .as_ref()
            .expect("Should never call parse without metadata.");

        // <https://w3c.github.io/webappsec-referrer-policy/#integration-with-css>
        // The resources this style sheet refers to are fetched with its URL as the referrer, and
        // with the referrer policy of its response.
        let url_data = UrlExtraData {
            base: metadata.final_url.get_arc(),
            referrer: Some(Arc::new(StylesheetReferrer {
                url: metadata.final_url.get_arc(),
                policy: ReferrerPolicy::parse_header_for_response(&metadata.headers).to_string(),
            })),
        };

        let _span = profile_traits::trace_span!("ParseStylesheet").entered();
        Arc::new(Stylesheet::from_bytes(
            &self.data,
            url_data,
            metadata.charset.as_deref(),
            Some(environment_encoding),
            Origin::Author,
            self.media.clone(),
            shared_lock,
            Some(&loader),
            Some(css_error_reporter),
            quirks_mode,
        ))
    }

    fn contributes_to_the_styling_processing_model(&self, element: &Node) -> bool {
        if !element.is_connected() {
            return false;
        }

        if matches!(&self.source, StylesheetContextSource::ProcessingInstruction) {
            let processing_instruction = element.downcast::<ProcessingInstruction>().unwrap();
            return self.request_generation_id.is_none_or(|generation| {
                generation == processing_instruction.get_request_generation_id()
            });
        }

        // Whether or not this `StylesheetContext` is for a `<link>` element that comes
        // from a previous generation. This prevents processing of earlier stylsheet URLs
        // when the URL has changed.
        //
        // TODO(mrobinson): Shouldn't we also exit early if this is an import that was originally
        // imported from a `<link>` element that has advanced a generation as well?
        if !matches!(&self.source, StylesheetContextSource::LinkElement) {
            return true;
        }
        let link = element.downcast::<HTMLLinkElement>().unwrap();
        self.request_generation_id
            .is_none_or(|generation| generation == link.get_request_generation_id())
    }

    /// <https://html.spec.whatwg.org/multipage/#contributes-a-script-blocking-style-sheet>
    fn contributes_a_script_blocking_style_sheet(
        &self,
        element: &Node,
        owner: &dyn StylesheetOwner,
        document: &Document,
    ) -> bool {
        // el was created by that Document's parser.
        owner.parser_inserted()
        // el is either a style element or a link element that was an external resource link that
        // contributes to the styling processing model when the el was created by the parser.
        && element.downcast::<HTMLLinkElement>().is_none_or(|link|
            self.contributes_to_the_styling_processing_model(element)
            // el's style sheet was enabled when the element was created by the parser.
            && !link.is_effectively_disabled()
        )
        // el's media attribute's value matches the environment.
        && owner.media_matches_environment()
        // The last time the event loop reached step 1, el's root was that Document.
        && *element.owner_document() == *document
        // The user agent hasn't given up on loading that particular style sheet yet.
        // A user agent may give up on loading a style sheet at any time.
        //
        // This might happen when we time out a resource, but that happens in `fetch` instead
    }

    fn decrement_blockers_and_finish_load(
        self,
        document: &Document,
        cx: &mut js::context::JSContext,
    ) {
        if self.is_script_blocking {
            document.remove_script_blocking_stylesheet(self.id);
        }

        if self.is_render_blocking {
            document.decrement_render_blocking_element_count();
        }

        document.finish_load(LoadType::Stylesheet(self.url), cx);
    }

    fn do_post_parse_tasks(
        self,
        success: bool,
        stylesheet: Arc<Stylesheet>,
        cx: &mut js::context::JSContext,
    ) {
        let element = self.element.root();
        let document = self.document.root();
        let owner = element
            .as_stylesheet_owner()
            .expect("Stylesheet not loaded by <style>, <link> or xml-stylesheet!");

        match &self.source {
            // https://html.spec.whatwg.org/multipage/#link-type-stylesheet%3Aprocess-the-linked-resource
            StylesheetContextSource::LinkElement => {
                let link = element
                    .downcast::<HTMLLinkElement>()
                    .expect("Should be HTMLinkElement due to StylesheetContextSource");
                // For failed requests, we should bail out if it is from a previous generation.
                // Since we can reissue another failed request, which resets the pending load counter
                // in a link element.
                if self
                    .request_generation_id
                    .is_some_and(|generation| generation != link.get_request_generation_id())
                {
                    self.decrement_blockers_and_finish_load(&document, cx);
                    return;
                }
                // https://html.spec.whatwg.org/multipage/#link-type-stylesheet
                // > When the disabled attribute of a link element with a stylesheet keyword is set,
                // > disable the associated CSS style sheet.
                if link.is_effectively_disabled() {
                    stylesheet.set_disabled(true);
                }
                // Step 3. If el has an associated CSS style sheet, remove the CSS style sheet.
                // Step 4. If success is true, then:
                // Step 4.1. Create a CSS style sheet with the following properties:
                //
                // Note that even in the failure case, we should create an empty stylesheet.
                // That's why `set_stylesheet` also removes the previous stylesheet
                link.set_stylesheet(cx.no_gc(), stylesheet);
            },
            // https://drafts.csswg.org/cssom/#requirements-on-user-agents-implementing-the-xml-stylesheet-processing-instruction
            StylesheetContextSource::ProcessingInstruction => {
                let processing_instruction = element
                    .downcast::<ProcessingInstruction>()
                    .expect("Should be ProcessingInstruction due to StylesheetContextSource");
                // Ignore the result of a fetch that the processing instruction has since
                // aborted, by changing or leaving the prolog.
                if self.request_generation_id.is_some_and(|generation| {
                    generation != processing_instruction.get_request_generation_id()
                }) {
                    self.decrement_blockers_and_finish_load(&document, cx);
                    return;
                }
                processing_instruction.set_stylesheet(cx.no_gc(), stylesheet);
            },
            StylesheetContextSource::Import(import_rule, _) => {
                let mut guard = document.style_shared_author_lock().write();
                import_rule.write_with(&mut guard).stylesheet = ImportSheet::Sheet(stylesheet);
            },
        }

        if let Some(ref shadow_root) = self.shadow_root {
            shadow_root.root().invalidate_stylesheets(cx.no_gc());
        } else {
            document.invalidate_stylesheets(cx.no_gc());
        }
        owner.set_origin_clean(cx, self.origin_clean);

        // Remaining steps are a combination of
        // https://html.spec.whatwg.org/multipage/#link-type-stylesheet%3Aprocess-the-linked-resource
        // and https://html.spec.whatwg.org/multipage/#the-style-element%3Acritical-subresources

        // Step 4.2. Fire an event named load at el.
        // Step 5. Otherwise, fire an event named error at el.
        //
        // No events are fired at processing instructions.
        if let Some(any_failed) = owner.load_finished(success) &&
            element.is::<Element>()
        {
            // Only fire an event if we have no more pending events
            // (in which case `owner.load_finished` would return None)
            let event = match any_failed {
                true => atom!("error"),
                false => atom!("load"),
            };
            element.upcast::<EventTarget>().fire_event(cx, event);
        }
        // Regardless if there are other pending events, we need to unblock
        // rendering for this particular request and signal that the load has finished

        // Step 6. If el contributes a script-blocking style sheet, then:
        // Step 7. Unblock rendering on el.
        self.decrement_blockers_and_finish_load(&document, cx);
    }
}

impl FetchResponseListener for StylesheetContext {
    fn process_request_body(&mut self, _: &GlobalScope, _: RequestId) {}

    fn process_response(
        &mut self,
        _: &mut js::context::JSContext,
        _: &GlobalScope,
        _: RequestId,
        metadata: Result<FetchMetadata, NetworkError>,
    ) {
        if let Ok(metadata) = metadata.as_ref() &&
            metadata.cors_status() == CorsStatus::Unsafe
        {
            self.origin_clean = false;
        }

        self.metadata = metadata.ok().map(Into::into);
    }

    fn process_response_chunk(
        &mut self,
        _: &mut js::context::JSContext,
        _: &GlobalScope,
        _: RequestId,
        payload: Bytes,
    ) {
        self.data.extend_from_slice(&payload);
    }

    fn process_response_eof(
        mut self,
        cx: &mut js::context::JSContext,
        global: &GlobalScope,
        _: RequestId,
        status: Result<(), NetworkError>,
        timing: ResourceFetchTiming,
    ) {
        network_listener::submit_timing(cx, global, &self, &status, &timing);

        let document = self.document.root();
        let Some(metadata) = self.metadata.as_ref() else {
            let empty_stylesheet = self.empty_stylesheet(&document);
            self.do_post_parse_tasks(false, empty_stylesheet, cx);
            return;
        };

        let element = self.element.root();

        // https://html.spec.whatwg.org/multipage/#link-type-stylesheet:process-the-linked-resource
        //
        // Style sheets of `xml-stylesheet` processing instructions are handled in the same way.
        if element.is::<HTMLLinkElement>() || element.is::<ProcessingInstruction>() {
            // Step 1. If the resource's Content-Type metadata is not text/css, then set success to false.
            let is_css = MimeClassifier::is_css(
                &metadata.resource_content_type_metadata(LoadContext::Style, &self.data),
            ) || (
                // From <https://html.spec.whatwg.org/multipage/#link-type-stylesheet>:
                // > Quirk: If the document has been set to quirks mode, has the same origin as
                // > the URL of the external resource, and the Content-Type metadata of the
                // > external resource is not a supported style sheet type, the user agent must
                // > instead assume it to be text/css.
                document.quirks_mode() == QuirksMode::Quirks &&
                    document.origin().immutable().clone() == metadata.final_url.origin()
            );

            if !is_css {
                let empty_stylesheet = self.empty_stylesheet(&document);
                self.do_post_parse_tasks(false, empty_stylesheet, cx);
                return;
            }

            // Step 2. If el no longer creates an external resource link that contributes to the styling processing model,
            // or if, since the resource in question was fetched, it has become appropriate to fetch it again, then:
            if !self.contributes_to_the_styling_processing_model(&element) {
                // Step 2.1. Remove el from el's node document's script-blocking style sheet set.
                self.decrement_blockers_and_finish_load(&document, cx);
                // Step 2.2. Return.
                return;
            }
        }

        if metadata.status != http::StatusCode::OK {
            let empty_stylesheet = self.empty_stylesheet(&document);
            self.do_post_parse_tasks(false, empty_stylesheet, cx);
            return;
        }

        self.unminify_css(&document, metadata.final_url.clone());

        let loader = if pref!(dom_parallel_css_parsing_enabled) {
            ElementStylesheetLoader::Asynchronous(AsynchronousStylesheetLoader::new(&element))
        } else {
            ElementStylesheetLoader::Synchronous { element: &element }
        };
        loader.parse(self, &element, &document, cx);
    }

    fn process_csp_violations(
        &mut self,
        cx: &mut js::context::JSContext,
        global: &GlobalScope,
        _request_id: RequestId,
        violations: Vec<Violation>,
    ) {
        global.report_csp_violations(cx, violations, None, None);
    }

    fn process_content_length(&mut self, _request_id: RequestId, size: usize) {
        self.data.reserve(size.saturating_sub(self.data.len()));
    }
}

impl ResourceTimingListener for StylesheetContext {
    fn resource_timing_information(&self) -> (InitiatorType, ServoUrl) {
        let initiator_type = match self.element.root().downcast::<Element>() {
            Some(element) => InitiatorType::LocalName(element.local_name().clone()),
            None => InitiatorType::Other,
        };
        (initiator_type, self.url.clone())
    }
}

pub(crate) enum ElementStylesheetLoader<'a> {
    Synchronous { element: &'a Node },
    Asynchronous(AsynchronousStylesheetLoader),
}

impl<'a> ElementStylesheetLoader<'a> {
    pub(crate) fn new(element: &'a Node) -> Self {
        ElementStylesheetLoader::Synchronous { element }
    }
}

impl ElementStylesheetLoader<'_> {
    pub(crate) fn load_with_element(
        cx: &mut JSContext,
        element: &Node,
        source: StylesheetContextSource,
        media: Arc<Locked<MediaList>>,
        url: ServoUrl,
        cors_setting: Option<CorsSettings>,
        integrity_metadata: String,
    ) {
        let document = element.owner_document();
        let shadow_root = element
            .containing_shadow_root()
            .map(|shadow_root| Trusted::new(&*shadow_root));
        let generation = element
            .downcast::<HTMLLinkElement>()
            .map(HTMLLinkElement::get_request_generation_id)
            .or_else(|| {
                element
                    .downcast::<ProcessingInstruction>()
                    .map(ProcessingInstruction::get_request_generation_id)
            });
        let mut context = StylesheetContext {
            id: StylesheetContextId::next(),
            element: Trusted::new(element),
            source,
            media,
            url: url.clone(),
            metadata: None,
            data: BytesMut::new(),
            document: Trusted::new(&*document),
            shadow_root,
            origin_clean: true,
            request_generation_id: generation,
            is_script_blocking: false,
            is_render_blocking: false,
        };

        let owner = element
            .as_stylesheet_owner()
            .expect("Stylesheet not loaded by <style>, <link> or xml-stylesheet!");
        let owner_referrer_policy = owner.referrer_policy(cx);
        owner.increment_pending_loads_count();

        // Final steps of https://html.spec.whatwg.org/multipage/#update-a-style-block
        // and part of https://html.spec.whatwg.org/multipage/#link-type-stylesheet:linked-resource-fetch-setup-steps

        // If element contributes a script-blocking style sheet, append element to its node document's script-blocking style sheet set.
        context.is_script_blocking =
            context.contributes_a_script_blocking_style_sheet(element, owner, &document);
        if context.is_script_blocking {
            document.add_script_blocking_stylesheet(context.id);
        }

        // If element's media attribute's value matches the environment and
        // element is potentially render-blocking, then block rendering on element.
        context.is_render_blocking = owner.media_matches_environment() &&
            owner.potentially_render_blocking() &&
            document.allows_adding_render_blocking_elements();
        if context.is_render_blocking {
            document.increment_render_blocking_element_count();
        }

        // https://html.spec.whatwg.org/multipage/#default-fetch-and-process-the-linked-resource
        let global = element.global();
        // An `@import` rule of an external style sheet uses the style sheet's referrer.
        let (referrer, referrer_policy) = match &context.source {
            StylesheetContextSource::Import(_, Some(referrer)) => {
                stylesheet_referrer(&global, Some(referrer))
            },
            _ => (global.get_referrer(), owner_referrer_policy),
        };
        let request = create_a_potential_cors_request(
            Some(document.webview_id()),
            url.clone(),
            Destination::Style,
            cors_setting,
            None,
            referrer,
        )
        .with_global_scope(&global)
        .referrer_policy(referrer_policy)
        .integrity_metadata(integrity_metadata);

        document.fetch_blocking(LoadType::Stylesheet(url), request, context);
    }

    fn parse(
        self,
        listener: StylesheetContext,
        element: &Node,
        document: &Document,
        cx: &mut js::context::JSContext,
    ) {
        let shared_lock = document.style_shared_author_lock().clone();
        let quirks_mode = document.quirks_mode();
        let window = element.owner_window();
        let environment_encoding = environment_encoding(&listener.source, element, document);

        match self {
            ElementStylesheetLoader::Synchronous { .. } => {
                let stylesheet = listener.parse(
                    environment_encoding,
                    quirks_mode,
                    shared_lock,
                    window.css_error_reporter(),
                    self,
                );
                listener.do_post_parse_tasks(true, stylesheet, cx);
            },
            ElementStylesheetLoader::Asynchronous(asynchronous_loader) => {
                let css_error_reporter = window.css_error_reporter().clone();
                let task_source = window
                    .as_global_scope()
                    .task_manager()
                    .networking_task_source()
                    .to_sendable();

                let parse_stylesheet = move || {
                    let loader = ElementStylesheetLoader::Asynchronous(asynchronous_loader);
                    let stylesheet = listener.parse(
                        environment_encoding,
                        quirks_mode,
                        shared_lock,
                        &css_error_reporter,
                        loader,
                    );

                    task_source.queue(
                        task!(finish_parsing_of_stylesheet_on_main_thread: move |cx| {
                            listener.do_post_parse_tasks(true, stylesheet, cx);
                        }),
                    );
                };

                let thread_pool = STYLE_THREAD_POOL.pool();
                if let Some(thread_pool) = thread_pool.as_ref() {
                    thread_pool.spawn(parse_stylesheet);
                } else {
                    parse_stylesheet();
                }
            },
        };
    }
}

/// The environment encoding of an external style sheet, which decodes it when neither its
/// response, nor a byte order mark, nor an `@charset` rule gives its encoding:
/// <https://drafts.csswg.org/css-syntax/#environment-encoding>.
fn environment_encoding(
    source: &StylesheetContextSource,
    element: &Node,
    document: &Document,
) -> &'static Encoding {
    // > If el has a charset attribute, get an encoding from that attribute's value. If that
    // > succeeds, return the resulting encoding.
    //
    // The `charset` pseudo-attribute of an `xml-stylesheet` processing instruction works
    // the same way.
    let charset = match source {
        StylesheetContextSource::LinkElement => element.downcast::<Element>().map(|element| {
            element
                .get_string_attribute(&local_name!("charset"))
                .to_string()
        }),
        StylesheetContextSource::ProcessingInstruction => element
            .downcast::<ProcessingInstruction>()
            .and_then(ProcessingInstruction::stylesheet_charset),
        // TODO: The environment encoding of an imported style sheet is the encoding of the
        // style sheet that imports it, which is only known once that is decoded. Until it
        // is recorded, use the document's encoding, which is what that style sheet falls
        // back to.
        StylesheetContextSource::Import(..) => None,
    };
    if let Some(encoding) =
        charset.and_then(|charset| Encoding::for_label(charset.trim().as_bytes()))
    {
        return encoding;
    }

    // > Otherwise, return the document's character encoding.
    document.encoding()
}

impl StyleStylesheetLoader for ElementStylesheetLoader<'_> {
    /// Request a stylesheet after parsing a given `@import` rule, and return
    /// the constructed `@import` rule.
    fn request_stylesheet(
        &self,
        url: CssUrl,
        source_location: SourceLocation,
        lock: &SharedRwLock,
        media: Arc<Locked<MediaList>>,
        supports: Option<ImportSupportsCondition>,
        layer: ImportLayer,
    ) -> Arc<Locked<ImportRule>> {
        // Ensure the supports conditions for this @import are true, if not, refuse to load
        if supports.as_ref().is_some_and(|s| !s.enabled) {
            return Arc::new(lock.wrap(ImportRule {
                url,
                stylesheet: ImportSheet::new_refused(),
                supports,
                layer,
                source_location,
            }));
        }

        let referrer = url.referrer().cloned();
        let resolved_url = match url.url().cloned() {
            Some(url) => url,
            None => {
                return Arc::new(lock.wrap(ImportRule {
                    url,
                    stylesheet: ImportSheet::new_refused(),
                    supports,
                    layer,
                    source_location,
                }));
            },
        };

        let import_rule = Arc::new(lock.wrap(ImportRule {
            url,
            stylesheet: ImportSheet::new_pending(),
            supports,
            layer,
            source_location,
        }));

        // TODO (mrnayak) : Whether we should use the original loader's CORS
        // setting? Fix this when spec has more details.
        let source = StylesheetContextSource::Import(import_rule.clone(), referrer);

        match self {
            ElementStylesheetLoader::Synchronous { element } => {
                // TODO: https://github.com/servo/servo/issues/44685
                #[expect(unsafe_code)]
                let mut cx = unsafe { script_bindings::script_runtime::temp_cx() };
                Self::load_with_element(
                    &mut cx,
                    element,
                    source,
                    media,
                    resolved_url.into(),
                    None,
                    String::new(),
                );
            },
            ElementStylesheetLoader::Asynchronous(AsynchronousStylesheetLoader {
                element,
                task_source,
            }) => {
                let element = element.clone();
                task_source.queue(task!(load_import_stylesheet_on_main_thread: move |cx| {
                    Self::load_with_element(
                        cx,
                        &element.root(),
                        source,
                        media,
                        resolved_url.into(),
                        None,
                        String::new(),
                    );
                }));
            },
        }

        import_rule
    }
}

pub(crate) struct AsynchronousStylesheetLoader {
    element: Trusted<Node>,
    task_source: SendableTaskSource,
}

impl AsynchronousStylesheetLoader {
    pub(crate) fn new(element: &Node) -> Self {
        let window = element.owner_window();
        Self {
            element: Trusted::new(element),
            task_source: window
                .as_global_scope()
                .task_manager()
                .networking_task_source()
                .into(),
        }
    }
}
