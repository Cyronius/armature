/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::borrow::Cow;
use std::cell::Cell;
use std::ffi::CStr;
use std::fs::read_to_string;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use bytes::{Bytes, BytesMut};
use dom_struct::dom_struct;
use encoding_rs::Encoding;
use html5ever::{LocalName, Prefix, local_name, ns};
use js::context::JSContext;
use js::offthread::compile_to_stencil_offthread;
use js::rust::HandleObject;
use net_traits::blob_url_store::UrlWithBlobClaim;
use net_traits::http_status::HttpStatus;
use net_traits::request::{
    CorsSettings, Destination, ParserMetadata, Referrer, RequestBuilder, RequestId,
};
use net_traits::{FetchMetadata, Metadata, NetworkError, ResourceFetchTiming};
use pixels::CorsStatus;
use script_bindings::cell::DomRefCell;
use script_bindings::domstring::{DOMString, TracedDOMString};
use script_bindings::trace::RootedTraceableBox;
use servo_base::id::WebViewId;
use servo_config::pref;
use servo_url::ServoUrl;
use style::attr::AttrValue;
use style::str::{HTML_SPACE_CHARACTERS, StaticStringVec};
use stylo_atoms::Atom;

use crate::dom::bindings::codegen::Bindings::DOMTokenListBinding::DOMTokenListMethods;
use crate::dom::bindings::codegen::Bindings::HTMLScriptElementBinding::HTMLScriptElementMethods;
use crate::dom::bindings::codegen::Bindings::NodeBinding::NodeMethods;
use crate::dom::bindings::codegen::UnionTypes::{
    TrustedScriptOrString, TrustedScriptURLOrUSVString,
};
use crate::dom::bindings::error::Fallible;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::refcounted::Trusted;
use crate::dom::bindings::reflector::DomGlobal;
use crate::dom::bindings::root::{Dom, DomRoot, MutNullableDom};
use crate::dom::csp::{CspReporting, GlobalCspReporting, InlineCheckType, Violation};
use crate::dom::document::Document;
use crate::dom::domtokenlist::DOMTokenList;
use crate::dom::element::attributes::storage::AttrRef;
use crate::dom::element::{
    AttributeMutation, Element, ElementCreator, cors_setting_for_element,
    cors_settings_attribute_credential_mode, referrer_policy_for_element,
    reflect_cross_origin_attribute, reflect_referrer_policy_attribute, set_cross_origin_attribute,
};
use crate::dom::event::eventtarget::EventTarget;
use crate::dom::globalscope::GlobalScope;
use crate::dom::html::htmlelement::HTMLElement;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::node::{ChildrenMutation, CloneChildrenFlag, Node, NodeTraits, UnbindContext};
use crate::dom::performance::performanceresourcetiming::InitiatorType;
use crate::dom::svg::svgscriptelement::SVGScriptElement;
use crate::dom::trustedtypes::trustedscript::TrustedScript;
use crate::dom::trustedtypes::trustedscripturl::TrustedScriptURL;
use crate::dom::window::Window;
use crate::event_loop::document_loader::{LoadBlocker, LoadType};
use crate::fetch::fetch::{RequestWithGlobalScope, create_a_potential_cors_request_with_claim};
use crate::fetch::network_listener::{self, FetchResponseListener, ResourceTimingListener};
use crate::runtime::script_runtime::IntroductionType;
use crate::scripting::fetching_options::ScriptFetchOptions;
use crate::scripting::import_map::{ImportMap, parse_an_import_map_string, register_import_map};
use crate::scripting::script_execution::{
    ClassicScript, RethrowErrors, ScriptOptions, fill_compile_options,
};
use crate::scripting::script_module::{
    ModuleTree, fetch_an_external_module_script, fetch_inline_module_script,
};
use crate::url::ensure_blob_referenced_by_url_is_kept_alive;

#[dom_struct]
pub(crate) struct HTMLScriptElement {
    htmlelement: HTMLElement,

    /// The state of the script processing model for this element.
    script_state: ScriptElementState,

    /// <https://html.spec.whatwg.org/multipage/#dom-script-blocking>
    blocking: MutNullableDom<DOMTokenList>,
}

impl HTMLScriptElement {
    fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
        creator: ElementCreator,
    ) -> HTMLScriptElement {
        HTMLScriptElement {
            htmlelement: HTMLElement::new_inherited(local_name, prefix, document),
            script_state: ScriptElementState::new(document, creator),
            blocking: Default::default(),
        }
    }

    pub(crate) fn new(
        cx: &mut JSContext,
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
        proto: Option<HandleObject>,
        creator: ElementCreator,
    ) -> DomRoot<HTMLScriptElement> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(HTMLScriptElement::new_inherited(
                local_name, prefix, document, creator,
            )),
            document,
            proto,
        )
    }
}

/// The state that the script processing model keeps for a `script` element. HTML and SVG
/// `script` elements both embed it.
#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
pub(crate) struct ScriptElementState {
    /// <https://html.spec.whatwg.org/multipage/#concept-script-delay-load>
    delaying_the_load_event: DomRefCell<Option<LoadBlocker>>,

    /// <https://html.spec.whatwg.org/multipage/#already-started>
    already_started: Cell<bool>,

    /// <https://html.spec.whatwg.org/multipage/#parser-inserted>
    parser_inserted: Cell<bool>,

    /// <https://html.spec.whatwg.org/multipage/#non-blocking>
    ///
    /// (currently unused)
    non_blocking: Cell<bool>,

    /// Document of the parser that created this element
    /// <https://html.spec.whatwg.org/multipage/#parser-document>
    parser_document: Dom<Document>,

    /// Prevents scripts that move between documents during preparation from executing.
    /// <https://html.spec.whatwg.org/multipage/#preparation-time-document>
    preparation_time_document: MutNullableDom<Document>,

    /// Track line line_number
    line_number: u64,

    /// <https://w3c.github.io/trusted-types/dist/spec/#htmlscriptelement-script-text>
    script_text: DomRefCell<TracedDOMString>,

    /// <https://html.spec.whatwg.org/multipage/#concept-script-external>
    from_an_external_file: Cell<bool>,

    /// Used to keep track whether we consider this script element render blocking during
    /// `prepare`
    marked_as_render_blocking: Cell<bool>,

    /// <https://html.spec.whatwg.org/multipage/#concept-script-result>
    result: DomRefCell<Option<TracedScriptResult>>,
}

impl ScriptElementState {
    pub(crate) fn new(document: &Document, creator: ElementCreator) -> ScriptElementState {
        ScriptElementState {
            delaying_the_load_event: Default::default(),
            already_started: Cell::new(false),
            parser_inserted: Cell::new(creator.is_parser_created()),
            non_blocking: Cell::new(!creator.is_parser_created()),
            parser_document: Dom::from_ref(document),
            preparation_time_document: MutNullableDom::new(None),
            line_number: creator.return_line_number(),
            script_text: Default::default(),
            from_an_external_file: Cell::new(false),
            marked_as_render_blocking: Default::default(),
            result: DomRefCell::new(None),
        }
    }
}

/// Supported script types as defined by
/// <https://html.spec.whatwg.org/multipage/#javascript-mime-type>.
pub(crate) static SCRIPT_JS_MIMES: StaticStringVec = &[
    "application/ecmascript",
    "application/javascript",
    "application/x-ecmascript",
    "application/x-javascript",
    "text/ecmascript",
    "text/javascript",
    "text/javascript1.0",
    "text/javascript1.1",
    "text/javascript1.2",
    "text/javascript1.3",
    "text/javascript1.4",
    "text/javascript1.5",
    "text/jscript",
    "text/livescript",
    "text/x-ecmascript",
    "text/x-javascript",
];

#[derive(Clone, Copy, JSTraceable, MallocSizeOf, PartialEq)]
pub(crate) enum ScriptType {
    Classic,
    Module,
    ImportMap,
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
pub(crate) enum TracedScript {
    Classic(Box<ClassicScript>),
    Module(#[conditional_malloc_size_of] Rc<ModuleTree>),
}

#[expect(clippy::large_enum_variant)]
pub(crate) enum Script {
    Classic(RootedTraceableBox<ClassicScript>),
    Module(RootedTraceableBox<Rc<ModuleTree>>),
    ImportMap(Fallible<ImportMap>),
}

pub(crate) type TracedScriptResult = Result<TracedScript, ()>;
pub(crate) type ScriptResult = Result<Script, ()>;

impl From<Script> for TracedScript {
    fn from(value: Script) -> Self {
        match value {
            Script::Classic(classic) => TracedScript::Classic(classic.into_box()),
            Script::Module(module) => TracedScript::Module(*module.into_box()),
            Script::ImportMap(_) => {
                unreachable!("Import maps are executed immediately, traced variant is not needed")
            },
        }
    }
}

impl From<TracedScript> for Script {
    #[cfg_attr(crown, expect(crown::unrooted_must_root))]
    fn from(value: TracedScript) -> Self {
        match value {
            TracedScript::Classic(classic) => {
                Script::Classic(RootedTraceableBox::from_box(classic))
            },
            TracedScript::Module(module) => Script::Module(RootedTraceableBox::new(module)),
        }
    }
}

/// The context required for asynchronously loading an external script source.
struct ClassicContext {
    /// The `script` element that initiated the request.
    elem: Trusted<Element>,
    /// The kind of external script.
    kind: ExternalScriptKind,
    /// The (fallback) character encoding argument to the "fetch a classic
    /// script" algorithm.
    character_encoding: &'static Encoding,
    /// The response body received to date.
    data: BytesMut,
    /// The response metadata received to date.
    metadata: Option<Metadata>,
    /// The initial URL requested.
    url: UrlWithBlobClaim,
    /// Indicates whether the request failed, and why
    status: Result<(), NetworkError>,
    /// The fetch options of the script
    fetch_options: ScriptFetchOptions,
    /// Used to set muted errors flag of classic scripts
    response_was_cors_cross_origin: bool,
}

impl FetchResponseListener for ClassicContext {
    // TODO(KiChjang): Perhaps add custom steps to perform fetch here?
    fn process_request_body(&mut self, _: &GlobalScope, _: RequestId) {}

    fn process_response(
        &mut self,
        _: &mut JSContext,
        _: &GlobalScope,
        _: RequestId,
        metadata: Result<FetchMetadata, NetworkError>,
    ) {
        self.metadata = metadata.ok().map(|metadata| {
            self.response_was_cors_cross_origin = metadata.cors_status() == CorsStatus::Unsafe;
            metadata.into()
        });

        let status = self
            .metadata
            .as_ref()
            .map(|m| m.status.clone())
            .unwrap_or_else(HttpStatus::new_error);

        self.status = {
            if status.is_error() {
                Err(NetworkError::ResourceLoadError(
                    "No http status code received".to_owned(),
                ))
            } else if status.is_success() {
                Ok(())
            } else {
                Err(NetworkError::ResourceLoadError(format!(
                    "HTTP error code {}",
                    status.code()
                )))
            }
        };
    }

    fn process_response_chunk(
        &mut self,
        _: &mut JSContext,
        _: &GlobalScope,
        _: RequestId,
        chunk: Bytes,
    ) {
        if self.status.is_ok() {
            self.data.extend_from_slice(&chunk);
        }
    }

    /// <https://html.spec.whatwg.org/multipage/#fetch-a-classic-script>
    /// step 4-9
    fn process_response_eof(
        mut self,
        cx: &mut JSContext,
        global: &GlobalScope,
        _: RequestId,
        response: Result<(), NetworkError>,
        timing: ResourceFetchTiming,
    ) {
        // Resource timing is expected to be available before "error" or "load" events are fired.
        network_listener::submit_timing(cx, global, &self, &response, &timing);

        let elem = self.elem.root();

        match (response.as_ref(), self.status.as_ref()) {
            (Err(error), _) | (_, Err(error)) => {
                error!(
                    "Fetching classic script failed {:?} ({:?})",
                    error, self.url
                );
                // Step 6, response is an error.
                let script = elem.as_script_element();
                *script.script_state().result.borrow_mut() = Some(Err(()));
                script.finish_fetching_a_script(cx, self.kind);
                return;
            },
            _ => {},
        };

        let metadata = self.metadata.take().unwrap();
        let final_url = metadata.final_url;

        // Step 5.3. Let potentialMIMETypeForEncoding be the result of extracting a MIME type given response's header list.
        // Step 5.4. Set encoding to the result of legacy extracting an encoding given potentialMIMETypeForEncoding and encoding.
        let encoding = metadata
            .charset
            .and_then(|encoding| Encoding::for_label(encoding.as_bytes()))
            .unwrap_or(self.character_encoding);

        // Step 5.5. Let sourceText be the result of decoding bodyBytes to Unicode, using encoding as the fallback encoding.
        let (mut source_text, _, _) = encoding.decode(&self.data);

        let global = elem.global();

        if let Some(window) = global.downcast::<Window>() &&
            let Some(script_source) = window.local_script_source()
        {
            substitute_with_local_script(script_source, &mut source_text, &final_url);
        }

        // Step 5.6. Let mutedErrors be true if response was CORS-cross-origin, and false otherwise.
        let mut script_options = ScriptOptions::External;
        script_options.set(
            ScriptOptions::MutedErrors,
            self.response_was_cors_cross_origin,
        );

        let fetch_options = self.fetch_options.clone();

        if pref!(dom_script_asynch) {
            let options = fill_compile_options(
                cx,
                final_url.as_str(),
                script_options,
                Some(IntroductionType::SRC_SCRIPT),
                1,
            );

            let src = Arc::new(source_text.to_string());

            let task_source = global
                .task_manager()
                .dom_manipulation_task_source()
                .to_sendable();

            let trusted = self.elem.clone();

            let _offthread_token = compile_to_stencil_offthread(options.ptr, src, move |result| {
                task_source.queue(task!(instantiate_stencil: move |cx| {
                    let script_element = trusted.root();
                    let script_element = script_element.as_script_element();

                    let script = ClassicScript::from_stencil(
                        cx,
                        result,
                        final_url,
                        script_options,
                        fetch_options,
                    );

                    let load = Script::Classic(script);
                    *script_element.script_state().result.borrow_mut() = Some(Ok(load.into()));
                    script_element.finish_fetching_a_script(cx, self.kind);
                }));

                None
            });

            return;
        }

        // Step 5.7. Let script be the result of creating a classic script given
        // sourceText, settingsObject, response's URL, options, mutedErrors, and url.
        let script = global.create_a_classic_script(
            cx,
            source_text,
            final_url,
            script_options,
            fetch_options,
            Some(IntroductionType::SRC_SCRIPT),
            1,
        );

        let load = Script::Classic(script);
        let script = elem.as_script_element();
        *script.script_state().result.borrow_mut() = Some(Ok(load.into()));
        script.finish_fetching_a_script(cx, self.kind);
    }

    fn process_csp_violations(
        &mut self,
        cx: &mut JSContext,
        global: &GlobalScope,
        _request_id: RequestId,
        violations: Vec<Violation>,
    ) {
        let elem = self.elem.root();
        global.report_csp_violations(cx, violations, Some(&*elem), None);
    }

    fn process_content_length(&mut self, _request_id: RequestId, size: usize) {
        self.data.reserve(size.saturating_sub(self.data.len()));
    }
}

impl ResourceTimingListener for ClassicContext {
    fn resource_timing_information(&self) -> (InitiatorType, ServoUrl) {
        let initiator_type = InitiatorType::LocalName(self.elem.root().local_name().clone());
        (initiator_type, self.url.url())
    }
}

/// Steps 1-2 of <https://html.spec.whatwg.org/multipage/#fetch-a-classic-script>
// This function is also used to prefetch a script in `script::dom::servoparser::prefetch`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn script_fetch_request(
    webview_id: WebViewId,
    url: UrlWithBlobClaim,
    cors_setting: Option<CorsSettings>,
    options: ScriptFetchOptions,
    referrer: Referrer,
) -> RequestBuilder {
    // We intentionally ignore options' credentials_mode member for classic scripts.
    // The mode is initialized by create_a_potential_cors_request.
    create_a_potential_cors_request_with_claim(
        Some(webview_id),
        url,
        Destination::Script,
        cors_setting,
        None,
        referrer,
    )
    .parser_metadata(options.parser_metadata)
    .integrity_metadata(options.integrity_metadata.clone())
    .referrer_policy(options.referrer_policy)
    .cryptographic_nonce_metadata(options.cryptographic_nonce)
}

/// <https://html.spec.whatwg.org/multipage/#fetch-a-classic-script>
fn fetch_a_classic_script(
    script: &Element,
    kind: ExternalScriptKind,
    url: UrlWithBlobClaim,
    cors_setting: Option<CorsSettings>,
    options: ScriptFetchOptions,
    character_encoding: &'static Encoding,
) {
    // Step 1, 2.
    let doc = script.owner_document();
    let global = script.global();
    let referrer = global.get_referrer();
    let request = script_fetch_request(
        doc.webview_id(),
        url.clone(),
        cors_setting,
        options.clone(),
        referrer,
    )
    .with_global_scope(&global);

    // TODO: Step 3, Add custom steps to perform fetch

    let context = ClassicContext {
        elem: Trusted::new(script),
        kind,
        character_encoding,
        data: BytesMut::new(),
        metadata: None,
        url,
        status: Ok(()),
        fetch_options: options,
        response_was_cors_cross_origin: false,
    };
    doc.fetch_background(request, context);
}

/// A `script` element in the HTML or the SVG namespace. Both are processed by HTML's script
/// processing model. SVG `script` elements take their URL from `href` (or the deprecated
/// `xlink:href`) instead of `src`, and do not have the attributes that only HTML defines for
/// `script` elements (`async`, `defer`, `nomodule`, `charset`, `integrity`, `blocking`, `event`,
/// `for` and `language`).
///
/// <https://svgwg.org/svg2-draft/interact.html#ScriptElement>
pub(crate) trait ScriptElement {
    fn element(&self) -> &Element;

    fn script_state(&self) -> &ScriptElementState;

    fn has_render_blocking_attribute(&self) -> bool;

    fn is_html(&self) -> bool {
        *self.element().namespace() == ns!(html)
    }

    /// Whether the element has `name`, one of the attributes that only HTML `script`
    /// elements have.
    fn has_html_only_attribute(&self, name: &LocalName) -> bool {
        self.is_html() && self.element().has_attribute(name)
    }

    /// The value of `name`, one of the attributes that only HTML `script` elements have.
    fn get_html_only_attribute(&self, name: &LocalName) -> Option<String> {
        if !self.is_html() {
            return None;
        }
        self.element().get_attribute_string_value(name)
    }

    /// The value of the `src` attribute of an HTML `script` element, or of the `href` attribute
    /// (or failing that, the `xlink:href` attribute) of an SVG `script` element.
    ///
    /// <https://svgwg.org/svg2-draft/linking.html#XLinkRefAttrs>
    fn source_attribute(&self) -> Option<String> {
        let element = self.element();
        if self.is_html() {
            return element.get_attribute_string_value(&local_name!("src"));
        }
        element
            .get_attribute_string_value(&local_name!("href"))
            .or_else(|| {
                element.get_attribute_string_value_with_namespace(&ns!(xlink), &local_name!("href"))
            })
    }

    /// Marks that element as delaying the load event or not.
    ///
    /// <https://html.spec.whatwg.org/multipage/#concept-script-delay-load>
    /// <https://html.spec.whatwg.org/multipage/#delaying-the-load-event-flag>
    fn delay_load_event(&self, document: &Document, url: ServoUrl) {
        let state = self.script_state();
        debug_assert!(state.delaying_the_load_event.borrow().is_none());

        *state.delaying_the_load_event.borrow_mut() =
            Some(LoadBlocker::new(document, LoadType::Script(url)));
    }

    /// Helper method to determine the script kind based on attributes and insertion context.
    ///
    /// This duplicates the script preparation logic from the HTML spec to determine the
    /// script's active document without full preparation.
    ///
    /// <https://html.spec.whatwg.org/multipage/#prepare-the-script-element>
    fn get_script_kind(&self, script_type: ScriptType) -> ExternalScriptKind {
        let state = self.script_state();

        if self.has_html_only_attribute(&local_name!("async")) || state.non_blocking.get() {
            ExternalScriptKind::Asap
        } else if !state.parser_inserted.get() {
            ExternalScriptKind::AsapInOrder
        } else if self.has_html_only_attribute(&local_name!("defer")) ||
            script_type == ScriptType::Module
        {
            ExternalScriptKind::Deferred
        } else {
            ExternalScriptKind::ParsingBlocking
        }
    }

    /// <https://html.spec.whatwg.org/multipage/#prepare-the-script-element>
    fn get_script_active_document(&self, script_kind: ExternalScriptKind) -> DomRoot<Document> {
        let state = self.script_state();
        match script_kind {
            ExternalScriptKind::Asap => state.preparation_time_document.get().unwrap(),
            ExternalScriptKind::AsapInOrder => state.preparation_time_document.get().unwrap(),
            ExternalScriptKind::Deferred => state.parser_document.as_rooted(),
            ExternalScriptKind::ParsingBlocking => state.parser_document.as_rooted(),
        }
    }

    /// <https://html.spec.whatwg.org/multipage/#steps-to-run-when-the-result-is-ready>
    fn finish_fetching_a_script(&self, cx: &mut JSContext, script_kind: ExternalScriptKind) {
        let state = self.script_state();
        let element = self.element();
        let load = state
            .result
            .take()
            .map(|result| result.map(Into::into))
            .expect("Result must be ready to proceed");

        // Step 2. If el's steps to run when the result is ready are not null, then run them.
        match script_kind {
            ExternalScriptKind::Asap => {
                let document = state.preparation_time_document.get().unwrap();
                document.asap_script_loaded(cx, element, load)
            },
            ExternalScriptKind::AsapInOrder => {
                let document = state.preparation_time_document.get().unwrap();
                document.asap_in_order_script_loaded(cx, element, load)
            },
            ExternalScriptKind::Deferred => {
                let document = state.parser_document.as_rooted();
                document.deferred_script_loaded(cx, element, load);
            },
            ExternalScriptKind::ParsingBlocking => {
                let document = state.parser_document.as_rooted();
                document.pending_parsing_blocking_script_loaded(element, load, cx);
            },
        }

        // Step 4. Set el's delaying the load event to false.
        LoadBlocker::terminate(cx, &state.delaying_the_load_event);
    }

    /// <https://w3c.github.io/trusted-types/dist/spec/#setting-slot-values-from-parser>
    fn set_initial_script_text(&self) {
        *self.script_state().script_text.borrow_mut() = self.text().traced();
    }

    /// <https://w3c.github.io/trusted-types/dist/spec/#prepare-script-text>
    fn prepare_the_script_text(&self, cx: &mut JSContext) -> Fallible<()> {
        // Step 1. If script’s script text value is not equal to its child text content,
        // set script’s script text to the result of executing
        // Get Trusted Type compliant string, with the following arguments:
        let state = self.script_state();
        let text = self.text();
        if *state.script_text.borrow() != *text {
            let sink = if self.is_html() {
                "HTMLScriptElement text"
            } else {
                "SVGScriptElement text"
            };
            let result = TrustedScript::get_trusted_type_compliant_string(
                cx,
                &self.element().owner_global(),
                TrustedScriptOrString::String(text),
                sink,
            )?;
            *state.script_text.safe_borrow_mut(cx.no_gc()) = result.traced();
        }

        Ok(())
    }

    /// <https://html.spec.whatwg.org/multipage/#potentially-render-blocking>
    fn potentially_render_blocking(&self, script_type: ScriptType) -> bool {
        // SVG `script` elements have no `blocking` attribute and are not implicitly potentially
        // render-blocking.
        if !self.is_html() {
            return false;
        }
        // An element is potentially render-blocking if its blocking tokens set contains "render",
        // or if it is implicitly potentially render-blocking, which will be defined at the individual elements.
        // By default, an element is not implicitly potentially render-blocking.
        if self.has_render_blocking_attribute() {
            return true;
        }
        let element = self.element();
        // https://html.spec.whatwg.org/multipage/#script-processing-model:implicitly-potentially-render-blocking
        // > A script element el is implicitly potentially render-blocking if el's type is "classic",
        // > el is parser-inserted, and el does not have an async or defer attribute.
        script_type == ScriptType::Classic &&
            self.script_state().parser_inserted.get() &&
            !element.has_attribute(&local_name!("async")) &&
            !element.has_attribute(&local_name!("defer"))
    }

    /// <https://html.spec.whatwg.org/multipage/#prepare-the-script-element>
    fn prepare(&self, cx: &mut JSContext, introduction_type_override: Option<&'static CStr>) {
        let introduction_type =
            introduction_type_override.or(Some(IntroductionType::INLINE_SCRIPT));
        let state = self.script_state();

        // Step 1. If el's already started is true, then return.
        if state.already_started.get() {
            return;
        }

        // Step 2. Let parser document be el's parser document.
        // TODO

        // Step 3. Set el's parser document to null.
        let was_parser_inserted = state.parser_inserted.get();
        state.parser_inserted.set(false);

        // Step 4.
        // If parser document is non-null and el does not have an async attribute, then set el's force async to true.
        let element = self.element();
        let asynch = self.has_html_only_attribute(&local_name!("async"));
        // Note: confusingly, this is done if the element does *not* have an "async" attribute.
        if was_parser_inserted && !asynch {
            state.non_blocking.set(true);
        }

        // Step 5. Execute the Prepare the script text algorithm on el.
        // If that algorithm threw an error, then return.
        if self.prepare_the_script_text(cx).is_err() {
            return;
        }
        // Step 5a. Let source text be el’s script text value.
        let text: Cow<'_, str> = Cow::Owned(String::from(state.script_text.borrow().str()));
        let source = self.source_attribute();
        // Step 6. If el has no src attribute, and source text is the empty string, then return.
        if text.is_empty() && source.is_none() {
            return;
        }

        // Step 7. If el is not connected, then return.
        if !element.upcast::<Node>().is_connected() {
            return;
        }

        let Some(script_type) = self.script_type() else {
            // Step 12. Otherwise, return. (No script is executed, and el's type is left as null.)
            return;
        };

        // Step 13.
        // If parser document is non-null, then set el's parser document back to parser document and set el's force
        // async to false.
        if was_parser_inserted {
            state.parser_inserted.set(true);
            state.non_blocking.set(false);
        }

        // Step 14. Set el's already started to true.
        state.already_started.set(true);

        // Step 15. Set el's preparation-time document to its node document.
        let doc = element.owner_document();
        state.preparation_time_document.set(Some(&doc));

        // Step 16.
        // If parser document is non-null, and parser document is not equal to el's preparation-time document, then
        // return.
        if state.parser_inserted.get() && *state.parser_document != *doc {
            return;
        }

        // Step 17. If scripting is disabled for el, then return.
        if !doc.scripting_enabled() {
            return;
        }

        // Step 18. If el has a nomodule content attribute and its type is "classic", then return.
        if self.has_html_only_attribute(&local_name!("nomodule")) &&
            script_type == ScriptType::Classic
        {
            return;
        }

        let global = &doc.global();

        // Step 19. CSP.
        if source.is_none() &&
            global
                .get_csp_list()
                .should_elements_inline_type_behavior_be_blocked(
                    cx,
                    global,
                    element,
                    InlineCheckType::Script,
                    &text,
                    state.line_number as u32,
                )
        {
            warn!("Blocking inline script due to CSP");
            return;
        }

        // Step 20. If el has an event attribute and a for attribute, and el's type is "classic", then:
        if script_type == ScriptType::Classic {
            let for_attribute = self.get_html_only_attribute(&local_name!("for"));
            let event_attribute = self.get_html_only_attribute(&local_name!("event"));
            if let (Some(for_attribute), Some(event_attribute)) = (for_attribute, event_attribute) {
                let for_value = for_attribute.trim_matches(HTML_SPACE_CHARACTERS);
                if !for_value.eq_ignore_ascii_case("window") {
                    return;
                }

                let event_value = event_attribute.trim_matches(HTML_SPACE_CHARACTERS);
                if !event_value.eq_ignore_ascii_case("onload") &&
                    !event_value.eq_ignore_ascii_case("onload()")
                {
                    return;
                }
            }
        }

        // Step 21. If el has a charset attribute, then let encoding be the result of getting
        // an encoding from the value of the charset attribute.
        // If el does not have a charset attribute, or if getting an encoding failed,
        // then let encoding be el's node document's the encoding.
        let encoding = self
            .get_html_only_attribute(&local_name!("charset"))
            .and_then(|charset| Encoding::for_label(charset.as_bytes()))
            .unwrap_or_else(|| doc.encoding());

        // Step 22. CORS setting.
        let cors_setting = cors_setting_for_element(element);

        // Step 23. Let module script credentials mode be the CORS settings attribute credentials mode for el's crossorigin content attribute.
        let module_credentials_mode = cors_settings_attribute_credential_mode(element);

        // Step 24. Let cryptographic nonce be el's [[CryptographicNonce]] internal slot's value.
        // If the element has a nonce content attribute but is not nonceable strip the nonce to prevent injection attacks.
        // Elements without a nonce content attribute (e.g. JS-created with .nonce = "abc")
        // use the internal slot directly — the nonceable check only applies to parser-created elements.
        let cryptographic_nonce =
            if element.is_nonceable() || !element.has_attribute(&local_name!("nonce")) {
                element.nonce_value().trim().to_owned()
            } else {
                String::new()
            };

        // Step 25. If el has an integrity attribute, then let integrity metadata be that attribute's value.
        // Otherwise, let integrity metadata be the empty string.
        let integrity_val = self.get_html_only_attribute(&local_name!("integrity"));
        let integrity_val_is_none = integrity_val.is_none();
        let integrity_metadata = integrity_val.unwrap_or_default();

        // Step 26. Let referrer policy be the current state of el's referrerpolicy content attribute.
        // SVG `script` elements have the same attribute.
        // <https://svgwg.org/svg2-draft/interact.html#ScriptElementReferrerpolicyAttribute>
        let referrer_policy = referrer_policy_for_element(element);

        // TODO: Step 27. Fetch priority.

        // Step 28. Let parser metadata be "parser-inserted" if el is parser-inserted,
        // and "not-parser-inserted" otherwise.
        let parser_metadata = if state.parser_inserted.get() {
            ParserMetadata::ParserInserted
        } else {
            ParserMetadata::NotParserInserted
        };

        // Step 29. Fetch options.
        let mut script_fetch_options = ScriptFetchOptions {
            cryptographic_nonce,
            integrity_metadata,
            parser_metadata,
            referrer_policy,
            credentials_mode: module_credentials_mode,
            render_blocking: false,
        };

        // Step 30. Let settings object be el's node document's relevant settings object.

        let base_url = doc.base_url();

        let kind = self.get_script_kind(script_type);
        let delayed_document = self.get_script_active_document(kind);

        let potentially_render_blocking = self.potentially_render_blocking(script_type);

        // Step 31. If el has a src content attribute, then:
        // Step 31.2. Let src be the value of el's src attribute.
        if let Some(src) = source {
            // Step 31.1. If el's type is "importmap".
            if script_type == ScriptType::ImportMap {
                // then queue an element task on the DOM manipulation task source
                // given el to fire an event named error at el, and return.
                self.queue_error_event();
                return;
            }

            // Step 31.3. If src is the empty string.
            if src.is_empty() {
                self.queue_error_event();
                return;
            }

            // Step 31.4. Set el's from an external file to true.
            state.from_an_external_file.set(true);

            // Step 31.5-31.6. Parse URL.
            let url = match base_url.join(&src) {
                Ok(url) => url,
                Err(_) => {
                    warn!("error parsing URL for script {}", src);
                    self.queue_error_event();
                    return;
                },
            };
            let url = ensure_blob_referenced_by_url_is_kept_alive(global, url);

            // Step 31.7. If el is potentially render-blocking, then block rendering on el.
            if potentially_render_blocking && doc.allows_adding_render_blocking_elements() {
                state.marked_as_render_blocking.set(true);
                doc.increment_render_blocking_element_count();
            }

            // Step 31.8. Set el's delaying the load event to true.
            self.delay_load_event(&delayed_document, url.url());

            // Step 31.9. If el is currently render-blocking, then set options's render-blocking to true.
            if state.marked_as_render_blocking.get() {
                script_fetch_options.render_blocking = true;
            }

            // Step 31.11. Switch on el's type:
            match script_type {
                ScriptType::Classic => {
                    // Step 31.11. Fetch a classic script.
                    fetch_a_classic_script(
                        element,
                        kind,
                        url,
                        cors_setting,
                        script_fetch_options,
                        encoding,
                    );
                },
                ScriptType::Module => {
                    // If el does not have an integrity attribute, then set options's integrity metadata to
                    // the result of resolving a module integrity metadata with url and settings object.
                    if integrity_val_is_none {
                        script_fetch_options.integrity_metadata = global
                            .import_map()
                            .resolve_a_module_integrity_metadata(&url.url());
                    }

                    let script = DomRoot::from_ref(element);

                    // Step 31.11. Fetch an external module script graph.
                    fetch_an_external_module_script(
                        cx,
                        url,
                        global,
                        script_fetch_options,
                        move |cx, module_tree| {
                            let script = script.as_script_element();
                            let load = module_tree
                                .map(|module| Script::Module(RootedTraceableBox::new(module)))
                                .ok_or(());
                            *script.script_state().result.borrow_mut() = Some(load.map(Into::into));

                            script.finish_fetching_a_script(cx, kind);
                        },
                    );
                },
                ScriptType::ImportMap => (),
            }
        } else {
            // Step 32. If el does not have a src content attribute:

            assert!(!text.is_empty());

            // Step 32.2: Switch on el's type:
            match script_type {
                ScriptType::Classic => {
                    // Step 32.2.1 Let script be the result of creating a classic script
                    // using source text, settings object, base URL, and options.
                    let script = element.global().create_a_classic_script(
                        cx,
                        text,
                        base_url,
                        ScriptOptions::empty(),
                        script_fetch_options,
                        introduction_type,
                        state.line_number as u32,
                    );
                    let result = Ok(Script::Classic(script));

                    if was_parser_inserted &&
                        doc.get_current_parser()
                            .is_some_and(|parser| parser.script_nesting_level() <= 1) &&
                        doc.has_a_stylesheet_that_is_blocking_scripts()
                    {
                        // Step 34.2: classic, has no src, was parser-inserted, is blocked on stylesheet.
                        doc.set_pending_parsing_blocking_script(element, Some(result));
                    } else {
                        // Step 34.3: otherwise.
                        self.execute(cx, result);
                    }
                    return;
                },
                ScriptType::Module => {
                    // Step 32.2.2.1 Set el's delaying the load event to true.
                    self.delay_load_event(&delayed_document, base_url.clone());

                    // Step 32.2.2.2 If el is potentially render-blocking, then:
                    if potentially_render_blocking && doc.allows_adding_render_blocking_elements() {
                        // Step 32.2.2.2.1 Block rendering on el.
                        state.marked_as_render_blocking.set(true);
                        doc.increment_render_blocking_element_count();

                        // Step 32.2.2.2.2 Set options's render-blocking to true.
                        script_fetch_options.render_blocking = true;
                    }

                    let script = DomRoot::from_ref(element);
                    // Step 32.2.2.3 Fetch an inline module script graph, given source text, base
                    // URL, settings object, options, and with the following steps given result:
                    let module = fetch_inline_module_script(
                        cx,
                        global,
                        text,
                        base_url,
                        script_fetch_options,
                        state.line_number as u32,
                        introduction_type,
                        move |_, module_tree| {
                            if module_tree.is_none() {
                                *script
                                    .as_script_element()
                                    .script_state()
                                    .result
                                    .borrow_mut() = Some(Err(()));
                            }
                            let trusted = Trusted::new(&*script);

                            // Queue an element task on the networking task source given el to perform the following steps:
                            script
                                .owner_global()
                                .task_manager()
                                .networking_task_source()
                                .queue(task!(terminate_module_fetch: move |cx| {
                                    // Mark as ready el given result.
                                    let element = trusted.root();
                                    element.as_script_element().finish_fetching_a_script(cx, kind);
                                }));
                        },
                    );

                    *state.result.borrow_mut() = Some(Ok(TracedScript::Module(*module.into_box())));
                },
                ScriptType::ImportMap => {
                    // Step 32.1 Let result be the result of creating an import map
                    // parse result given source text and base URL.
                    let import_map_result = parse_an_import_map_string(cx, global, &text, base_url);
                    let script = Script::ImportMap(import_map_result);

                    // Step 34.3
                    self.execute(cx, Ok(script));
                    return;
                },
            }
        }

        // Step 33.2/33.3/33.4/33.5, substeps 1-2. Add el to the corresponding script list.
        match kind {
            ExternalScriptKind::Deferred => delayed_document.add_deferred_script(element),
            ExternalScriptKind::ParsingBlocking => {
                delayed_document.set_pending_parsing_blocking_script(element, None);
            },
            ExternalScriptKind::AsapInOrder => delayed_document.push_asap_in_order_script(element),
            ExternalScriptKind::Asap => delayed_document.add_asap_script(element),
        }
    }

    /// <https://html.spec.whatwg.org/multipage/#execute-the-script-element>
    fn execute(&self, cx: &mut JSContext, result: ScriptResult) {
        let state = self.script_state();
        let element = self.element();

        // Step 1. Let document be el's node document.
        let doc = element.owner_document();

        // Step 2. If el's preparation-time document is not equal to document, then return.
        if *doc != *state.preparation_time_document.get().unwrap() {
            return;
        }

        // Step 3. Unblock rendering on el.
        if state.marked_as_render_blocking.replace(false) {
            state.marked_as_render_blocking.set(false);
            doc.decrement_render_blocking_element_count();
        }

        let script = match result {
            // Step 4. If el's result is null, then fire an event named error at el, and return.
            Err(_) => {
                element
                    .upcast::<EventTarget>()
                    .fire_event(cx, atom!("error"));
                return;
            },

            Ok(script) => script,
        };

        // Step 5.
        // If el's from an external file is true, or el's type is "module", then increment document's
        // ignore-destructive-writes counter.
        let neutralized_doc =
            if state.from_an_external_file.get() || matches!(script, Script::Module(_)) {
                let doc = element.owner_document();
                doc.incr_ignore_destructive_writes_counter();
                Some(doc)
            } else {
                None
            };

        let document = element.owner_document();

        match script {
            Script::Classic(script) => {
                // Step 6."classic".1. Let oldCurrentScript be the value to which document's currentScript object was most recently set.
                let old_script = document.current_script();

                // Step 6."classic".2. If el's root is not a shadow root,
                // then set document's currentScript attribute to el. Otherwise, set it to null.
                if element.upcast::<Node>().is_in_a_shadow_tree() {
                    document.set_current_script(None)
                } else {
                    document.set_current_script(Some(element))
                }

                // Step 6."classic".3. Run the classic script given by el's result.
                _ = element.owner_global().run_a_classic_script(
                    cx,
                    script,
                    RethrowErrors::No,
                    None, // return_value
                );

                // Step 6."classic".4. Set document's currentScript attribute to oldCurrentScript.
                document.set_current_script(old_script.as_deref());
            },
            Script::Module(module_tree) => {
                // TODO Step 6."module".1. Assert: document's currentScript attribute is null.
                document.set_current_script(None);

                // Step 6."module".2. Run the module script given by el's result.
                element
                    .owner_global()
                    .run_a_module_script(cx, module_tree, false);
            },
            Script::ImportMap(import_map) => {
                // Step 6."importmap".1. Register an import map given el's relevant global object and el's result.
                register_import_map(cx, &element.owner_global(), import_map);
            },
        }

        // Step 7.
        // Decrement the ignore-destructive-writes counter of document, if it was incremented in the earlier step.
        if let Some(doc) = neutralized_doc {
            doc.decr_ignore_destructive_writes_counter();
        }

        // Step 8. If el's from an external file is true, then fire an event named load at el.
        if state.from_an_external_file.get() {
            element
                .upcast::<EventTarget>()
                .fire_event(cx, atom!("load"));
        }
    }

    fn queue_error_event(&self) {
        let element = self.element();
        element
            .owner_global()
            .task_manager()
            .dom_manipulation_task_source()
            .queue_simple_event(element.upcast(), atom!("error"));
    }

    /// <https://html.spec.whatwg.org/multipage/#prepare-a-script> Steps 8-12.
    fn script_type(&self) -> Option<ScriptType> {
        let element = self.element();

        let type_attr = element.get_attribute_string_ref(&local_name!("type"));
        let language_attr = if self.is_html() {
            element.get_attribute_string_ref(&local_name!("language"))
        } else {
            None
        };

        match (type_attr, language_attr) {
            // Step 8. If any of the following are true:
            // el has a type attribute whose value is the empty string;
            // el has no type attribute but it has a language attribute and that attribute's value
            // is the empty string; or el has neither a type attribute nor a language attribute,
            // then let the script block's type string for this script element be "text/javascript".
            (Some(type_attribute), _) if type_attribute.is_empty() => Some(ScriptType::Classic),
            (None, Some(lang)) if lang.is_empty() => Some(ScriptType::Classic),
            (None, None) => Some(ScriptType::Classic),
            // Otherwise, el has a non-empty language attribute; let the script block's type string
            // be the concatenation of "text/" and the value of el's language attribute.
            (None, Some(lang)) => {
                let language_value = lang.as_attr_ref().value();
                let script_block_type_string = format!("text/{}", language_value.as_ref());

                // Step 9. If the script block's type string is a JavaScript MIME type essence match,
                // or el has a type attribute whose value with leading and trailing ASCII whitespace
                // stripped is a JavaScript MIME type essence match, then set el's type to "classic".
                let is_a_javascript_mime = SCRIPT_JS_MIMES
                    .iter()
                    .any(|mime| script_block_type_string.eq_ignore_ascii_case(mime));

                is_a_javascript_mime.then_some(ScriptType::Classic)
            },
            // Otherwise, if el has a type attribute, then let the script block's type string be the
            // value of that attribute.
            (Some(ty), _) => {
                // Step 10. Otherwise, if the script block's type string is an ASCII case-insensitive
                // match for the string "module", then set el's type to "module".
                if ty.eq_ignore_ascii_case("module") {
                    return Some(ScriptType::Module);
                }

                // Step 11. Otherwise, if the script block's type string is an ASCII case-insensitive
                // match for the string "importmap", then set el's type to "importmap".
                if ty.eq_ignore_ascii_case("importmap") {
                    return Some(ScriptType::ImportMap);
                }

                let type_attr_value = ty.as_attr_ref().value();

                // Step 9. If the script block's type string is a JavaScript MIME type essence match,
                // or el has a type attribute whose value with leading and trailing ASCII whitespace
                // stripped is a JavaScript MIME type essence match, then set el's type to "classic".
                let is_a_javascript_mime = SCRIPT_JS_MIMES.iter().any(|mime| {
                    type_attr_value
                        .trim_matches(HTML_SPACE_CHARACTERS)
                        .eq_ignore_ascii_case(mime)
                });

                is_a_javascript_mime.then_some(ScriptType::Classic)
            },
        }
    }

    fn set_parser_inserted(&self, parser_inserted: bool) {
        self.script_state().parser_inserted.set(parser_inserted);
    }

    fn set_already_started(&self, already_started: bool) {
        self.script_state().already_started.set(already_started);
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-script-text>
    fn text(&self) -> DOMString {
        self.element().upcast::<Node>().child_text_content()
    }

    /// The `script` element's steps for when an attribute is set, changed or removed: an element
    /// that is not parser-inserted and is connected is prepared when it gets a source URL.
    ///
    /// <https://html.spec.whatwg.org/multipage/#script-processing-model:the-script-element-21>
    fn source_attribute_mutated(&self, cx: &mut JSContext, mutation: AttributeMutation) {
        if let AttributeMutation::Set(_) = mutation &&
            !self.script_state().parser_inserted.get() &&
            self.element().upcast::<Node>().is_connected()
        {
            self.prepare(cx, Some(IntroductionType::INJECTED_SCRIPT));
        }
    }

    /// <https://html.spec.whatwg.org/multipage/#script-processing-model:the-script-element-26>
    fn script_children_changed(&self) {
        let element = self.element();
        if element.upcast::<Node>().is_connected() && !self.script_state().parser_inserted.get() {
            let script = DomRoot::from_ref(element);
            // This method can be invoked while there are script/layout blockers present
            // as DOM mutations have not yet settled. We use a delayed task to avoid
            // running any scripts until the DOM tree is safe for interactions.
            element.owner_document().add_delayed_task(
                task!(ScriptPrepare: |cx, script: DomRoot<Element>| {
                    script.as_script_element().prepare(cx, Some(IntroductionType::INJECTED_SCRIPT));
                }),
            );
        }
    }

    /// <https://html.spec.whatwg.org/multipage/#script-processing-model:the-script-element-20>
    fn script_post_connection_steps(&self, cx: &mut JSContext) {
        if self.element().upcast::<Node>().is_connected() &&
            !self.script_state().parser_inserted.get()
        {
            self.prepare(cx, Some(IntroductionType::INJECTED_SCRIPT));
        }
    }

    /// <https://html.spec.whatwg.org/multipage/#already-started>
    fn script_cloning_steps(&self, copy: &Node) {
        if self.script_state().already_started.get() {
            copy.downcast::<Element>()
                .unwrap()
                .as_script_element()
                .set_already_started(true);
        }
    }

    fn script_unbind_from_tree(&self) {
        if self.script_state().marked_as_render_blocking.replace(false) {
            let document = self.element().owner_document();
            document.decrement_render_blocking_element_count();
        }
    }
}

impl ScriptElement for HTMLScriptElement {
    fn element(&self) -> &Element {
        self.upcast()
    }

    fn script_state(&self) -> &ScriptElementState {
        &self.script_state
    }

    fn has_render_blocking_attribute(&self) -> bool {
        self.blocking
            .get()
            .is_some_and(|list| list.Contains(DOMString::from_static("render")))
    }
}

impl Element {
    /// The script processing model of this element, if it is an HTML or SVG `script` element.
    pub(crate) fn as_maybe_script_element(&self) -> Option<&dyn ScriptElement> {
        if let Some(script) = self.downcast::<HTMLScriptElement>() {
            return Some(script);
        }
        self.downcast::<SVGScriptElement>()
            .map(|script| script as &dyn ScriptElement)
    }

    /// The script processing model of this element, which must be an HTML or SVG `script`
    /// element.
    pub(crate) fn as_script_element(&self) -> &dyn ScriptElement {
        self.as_maybe_script_element()
            .expect("Element must be an HTML or SVG script element")
    }
}

impl VirtualMethods for HTMLScriptElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<HTMLElement>() as &dyn VirtualMethods)
    }

    fn attribute_mutated(
        &self,
        cx: &mut JSContext,
        attr: AttrRef<'_>,
        mutation: AttributeMutation,
    ) {
        self.super_type()
            .unwrap()
            .attribute_mutated(cx, attr, mutation);
        if *attr.local_name() == local_name!("src") {
            self.source_attribute_mutated(cx, mutation);
        } else if *attr.local_name() == local_name!("blocking") &&
            !self.has_render_blocking_attribute() &&
            self.script_state.marked_as_render_blocking.replace(false)
        {
            let document = self.owner_document();
            document.decrement_render_blocking_element_count();
        }
    }

    fn children_changed(&self, cx: &mut JSContext, mutation: &ChildrenMutation) {
        if let Some(s) = self.super_type() {
            s.children_changed(cx, mutation);
        }

        self.script_children_changed();
    }

    fn post_connection_steps(&self, cx: &mut JSContext) {
        if let Some(s) = self.super_type() {
            s.post_connection_steps(cx);
        }

        self.script_post_connection_steps(cx);
    }

    fn cloning_steps(
        &self,
        cx: &mut JSContext,
        copy: &Node,
        maybe_doc: Option<&Document>,
        clone_children: CloneChildrenFlag,
    ) {
        if let Some(s) = self.super_type() {
            s.cloning_steps(cx, copy, maybe_doc, clone_children);
        }

        self.script_cloning_steps(copy);
    }

    fn unbind_from_tree(&self, cx: &mut JSContext, context: &UnbindContext) {
        self.super_type().unwrap().unbind_from_tree(cx, context);

        self.script_unbind_from_tree();
    }
}

impl HTMLScriptElementMethods<crate::DomTypeHolder> for HTMLScriptElement {
    /// <https://html.spec.whatwg.org/multipage/#dom-script-src>
    fn Src(&self) -> TrustedScriptURLOrUSVString {
        let element = self.upcast::<Element>();
        element.get_trusted_type_url_attribute(&local_name!("src"))
    }

    /// <https://w3c.github.io/trusted-types/dist/spec/#the-src-idl-attribute>
    fn SetSrc(&self, cx: &mut JSContext, value: TrustedScriptURLOrUSVString) -> Fallible<()> {
        let element = self.upcast::<Element>();
        let local_name = &local_name!("src");
        let value = TrustedScriptURL::get_trusted_type_compliant_string(
            cx,
            &element.owner_global(),
            value,
            &format!("HTMLScriptElement {}", local_name),
        )?;
        element.set_attribute(cx, local_name, AttrValue::String(value.str().to_owned()));
        Ok(())
    }

    // https://html.spec.whatwg.org/multipage/#dom-script-type
    make_getter!(Type, "type");
    // https://html.spec.whatwg.org/multipage/#dom-script-type
    make_setter!(SetType, "type");

    // https://html.spec.whatwg.org/multipage/#dom-script-charset
    make_getter!(Charset, "charset");
    // https://html.spec.whatwg.org/multipage/#dom-script-charset
    make_setter!(SetCharset, "charset");

    /// <https://html.spec.whatwg.org/multipage/#dom-script-async>
    fn Async(&self) -> bool {
        self.script_state.non_blocking.get() ||
            self.upcast::<Element>()
                .has_attribute(&local_name!("async"))
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-script-async>
    fn SetAsync(&self, cx: &mut JSContext, value: bool) {
        self.script_state.non_blocking.set(false);
        self.upcast::<Element>()
            .set_bool_attribute(cx, &local_name!("async"), value);
    }

    // https://html.spec.whatwg.org/multipage/#dom-script-defer
    make_bool_getter!(Defer, "defer");
    // https://html.spec.whatwg.org/multipage/#dom-script-defer
    make_bool_setter!(SetDefer, "defer");

    /// <https://html.spec.whatwg.org/multipage/#attr-script-blocking>
    fn Blocking(&self, cx: &mut JSContext) -> DomRoot<DOMTokenList> {
        self.blocking.or_init(|| {
            DOMTokenList::new(
                cx,
                self.upcast(),
                &local_name!("blocking"),
                Some(vec![Atom::from("render")]),
            )
        })
    }

    // https://html.spec.whatwg.org/multipage/#dom-script-nomodule
    make_bool_getter!(NoModule, "nomodule");
    // https://html.spec.whatwg.org/multipage/#dom-script-nomodule
    make_bool_setter!(SetNoModule, "nomodule");

    // https://html.spec.whatwg.org/multipage/#dom-script-integrity
    make_getter!(Integrity, "integrity");
    // https://html.spec.whatwg.org/multipage/#dom-script-integrity
    make_setter!(SetIntegrity, "integrity");

    // https://html.spec.whatwg.org/multipage/#dom-script-event
    make_getter!(Event, "event");
    // https://html.spec.whatwg.org/multipage/#dom-script-event
    make_setter!(SetEvent, "event");

    // https://html.spec.whatwg.org/multipage/#dom-script-htmlfor
    make_getter!(HtmlFor, "for");
    // https://html.spec.whatwg.org/multipage/#dom-script-htmlfor
    make_setter!(SetHtmlFor, "for");

    /// <https://html.spec.whatwg.org/multipage/#dom-script-crossorigin>
    fn GetCrossOrigin(&self) -> Option<DOMString> {
        reflect_cross_origin_attribute(self.upcast::<Element>())
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-script-crossorigin>
    fn SetCrossOrigin(&self, cx: &mut JSContext, value: Option<DOMString>) {
        set_cross_origin_attribute(cx, self.upcast::<Element>(), value);
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-script-referrerpolicy>
    fn ReferrerPolicy(&self) -> DOMString {
        reflect_referrer_policy_attribute(self.upcast::<Element>())
    }

    // https://html.spec.whatwg.org/multipage/#dom-script-referrerpolicy
    make_setter!(SetReferrerPolicy, "referrerpolicy");

    /// <https://w3c.github.io/trusted-types/dist/spec/#dom-htmlscriptelement-innertext>
    fn InnerText(&self) -> TrustedScriptOrString {
        // Step 1: Return the result of running get the text steps with this.
        TrustedScriptOrString::String(self.upcast::<HTMLElement>().get_inner_outer_text())
    }

    /// <https://w3c.github.io/trusted-types/dist/spec/#the-innerText-idl-attribute>
    fn SetInnerText(&self, cx: &mut JSContext, input: TrustedScriptOrString) -> Fallible<()> {
        // Step 1: Let value be the result of calling Get Trusted Type compliant string with TrustedScript,
        // this's relevant global object, the given value, HTMLScriptElement innerText, and script.
        let value = TrustedScript::get_trusted_type_compliant_string(
            cx,
            &self.owner_global(),
            input,
            "HTMLScriptElement innerText",
        )?;
        *self.script_state.script_text.safe_borrow_mut(cx.no_gc()) = value.clone().traced();
        // Step 3: Run set the inner text steps with this and value.
        self.upcast::<HTMLElement>().set_inner_text(cx, value);
        Ok(())
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-script-text>
    fn Text(&self) -> TrustedScriptOrString {
        TrustedScriptOrString::String(self.upcast::<Node>().child_text_content())
    }

    /// <https://w3c.github.io/trusted-types/dist/spec/#the-text-idl-attribute>
    fn SetText(&self, cx: &mut JSContext, value: TrustedScriptOrString) -> Fallible<()> {
        // Step 1: Let value be the result of calling Get Trusted Type compliant string with TrustedScript,
        // this's relevant global object, the given value, HTMLScriptElement text, and script.
        let value = TrustedScript::get_trusted_type_compliant_string(
            cx,
            &self.owner_global(),
            value,
            "HTMLScriptElement text",
        )?;
        // Step 2: Set this's script text value to the given value.
        *self.script_state.script_text.safe_borrow_mut(cx.no_gc()) = value.clone().traced();
        // Step 3: String replace all with the given value within this.
        Node::string_replace_all(cx, value, self.upcast::<Node>());
        Ok(())
    }

    /// <https://w3c.github.io/trusted-types/dist/spec/#the-textContent-idl-attribute>
    fn GetTextContent(&self) -> Option<TrustedScriptOrString> {
        // Step 1: Return the result of running get text content with this.
        Some(TrustedScriptOrString::String(
            self.upcast::<Node>().GetTextContent()?,
        ))
    }

    /// <https://w3c.github.io/trusted-types/dist/spec/#the-textContent-idl-attribute>
    fn SetTextContent(
        &self,
        cx: &mut JSContext,
        value: Option<TrustedScriptOrString>,
    ) -> Fallible<()> {
        // Step 1: Let value be the result of calling Get Trusted Type compliant string with TrustedScript,
        // this's relevant global object, the given value, HTMLScriptElement textContent, and script.
        let value = TrustedScript::get_trusted_type_compliant_string(
            cx,
            &self.owner_global(),
            value.unwrap_or(TrustedScriptOrString::String(DOMString::new())),
            "HTMLScriptElement textContent",
        )?;
        // Step 2: Set this's script text value to value.
        *self.script_state.script_text.safe_borrow_mut(cx.no_gc()) = value.clone().traced();
        // Step 3: Run set text content with this and value.
        self.upcast::<Node>()
            .set_text_content_for_element(cx, Some(value));
        Ok(())
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-script-supports>
    fn Supports(_window: &Window, type_: DOMString) -> bool {
        // The type argument has to exactly match these values,
        // we do not perform an ASCII case-insensitive match.
        matches!(&*type_.str(), "classic" | "module" | "importmap")
    }
}

pub fn substitute_with_local_script(
    script_source: &str,
    script: &mut Cow<'_, str>,
    url: &ServoUrl,
) {
    let mut path = PathBuf::from(script_source);
    path = path.join(&url[url::Position::BeforeHost..url::Position::AfterPath]);
    debug!("Attempting to read script stored at: {:?}", path);
    match read_to_string(path.clone()) {
        Ok(local_script) => {
            debug!("Found script stored at: {:?}", path);
            *script = Cow::Owned(local_script);
        },
        Err(why) => warn!("Could not restore script from file {:?}", why),
    }
}

#[derive(Clone, Copy)]
pub(crate) enum ExternalScriptKind {
    Deferred,
    ParsingBlocking,
    AsapInOrder,
    Asap,
}
