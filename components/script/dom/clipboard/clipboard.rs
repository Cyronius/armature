/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use dom_struct::dom_struct;
use embedder_traits::{
    AllowOrDeny, ClipboardChange, ClipboardData, ClipboardSnapshot, ClipboardWriteError,
    ClipboardWriteId, EmbedderMsg,
};
use html5ever::serialize::TraversalScope;
use js::context::JSContext;
use js::realm::CurrentRealm;
use js::rust::HandleValue;
use script_bindings::inheritance::Castable;
use script_bindings::reflector::reflect_dom_object;
use servo_base::generic_channel::GenericCallback;
use sha2::{Digest, Sha256};

use super::clipboard_style::inline_clipboard_styles;
use super::clipboardchangeevent::ClipboardChangeEvent;
use super::clipboarditem::{ClipboardItem, is_supported_format};
use crate::dom::bindings::codegen::Bindings::BlobBinding::BlobMethods;
use crate::dom::bindings::codegen::Bindings::ClipboardBinding::{
    ClipboardMethods, ClipboardUnsanitizedFormats,
};
use crate::dom::bindings::codegen::Bindings::DocumentBinding::DocumentMethods;
use crate::dom::bindings::codegen::Bindings::PermissionStatusBinding::{
    PermissionName, PermissionState,
};
use crate::dom::bindings::codegen::Bindings::WindowBinding::Window_Binding::WindowMethods;
use crate::dom::bindings::conversions::{
    ConversionResult, FromJSValConvertible, StringificationBehavior,
};
use crate::dom::bindings::error::Error;
use crate::dom::bindings::refcounted::{Trusted, TrustedPromise};
use crate::dom::bindings::reflector::DomGlobal;
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::bindings::str::DOMString;
use crate::dom::bindings::trace::RootedTraceableBox;
use crate::dom::blob::Blob;
use crate::dom::event::Event;
use crate::dom::eventtarget::EventTarget;
use crate::dom::globalscope::GlobalScope;
use crate::dom::permissions::{
    AsyncPermissionRequest, descriptor_permission_state, record_permission_request_result,
    request_permission_to_use_async,
};
use crate::dom::promise::{Promise, RootedPromise, TracedPromise};
use crate::dom::promisenativehandler::{Callback, PromiseNativeHandler};
use crate::dom::sanitizer::Sanitizer;
use crate::dom::window::Window;
use crate::realms::enter_auto_realm;
use crate::routed_promise::{RoutedPromiseListener, callback_promise};

#[dom_struct]
pub(crate) struct Clipboard {
    event_target: EventTarget,
    write_sequence: Cell<u64>,
}

#[derive(Clone, Copy)]
enum ReadKind {
    Text,
    Items { unsanitized_html: bool },
}

fn access_denied() -> Error {
    Error::NotAllowed(Some(
        "Clipboard access requires focus and permission.".into(),
    ))
}

impl Clipboard {
    pub(crate) fn new(cx: &mut JSContext, global: &GlobalScope) -> DomRoot<Clipboard> {
        reflect_dom_object(
            cx,
            Box::new(Clipboard {
                event_target: EventTarget::new_inherited(),
                write_sequence: Cell::new(0),
            }),
            global,
        )
    }

    pub(crate) fn dispatch_change(&self, cx: &mut JSContext, change: ClipboardChange) {
        let global = self.global();
        let window = global.as_window();
        if !global.is_secure_context() ||
            (!window.has_sticky_activation() &&
                clipboard_permission_state(window, PermissionName::Clipboard_read) !=
                    PermissionState::Granted)
        {
            return;
        }
        let key = global.obtain_storage_key_for_non_storage_purposes();
        let Ok(key) = serde_json::to_vec(&key) else {
            return;
        };
        let mut hash = Sha256::new();
        hash.update(change.nonce.to_be_bytes());
        hash.update(key);
        let hash = hash.finalize();
        let change_id = u128::from_be_bytes(hash[..16].try_into().unwrap());
        let types = change.types.into_iter().map(DOMString::from).collect();
        let mut realm = enter_auto_realm(cx, window);
        let cx = &mut realm.current_realm();
        if let Some(event) = ClipboardChangeEvent::for_change(cx, window, types, change_id) {
            event.upcast::<Event>().fire(cx, &self.event_target);
        }
    }

    fn focused_permission(
        &self,
        cx: &mut JSContext,
        promise: &RootedPromise,
        permission: PermissionName,
    ) -> Option<PermissionState> {
        let global = self.global();
        let window = global.as_window();
        let document = window.Document();
        if !document.is_fully_active() {
            promise.resolve_native(cx, &());
            return None;
        }
        if !document.HasFocus() {
            promise.reject_error(cx, access_denied());
            return None;
        }

        Some(clipboard_permission_state(window, permission))
    }

    fn authorize_write(&self, cx: &mut JSContext, promise: &RootedPromise) -> bool {
        match self.focused_permission(cx, promise, PermissionName::Clipboard_write) {
            Some(PermissionState::Granted) => true,
            Some(PermissionState::Prompt)
                if self.global().as_window().has_transient_activation() =>
            {
                true
            },
            Some(_) => {
                promise.reject_error(cx, access_denied());
                false
            },
            None => false,
        }
    }

    fn begin_write(&self, promise: &RootedPromise) -> ClipboardWriteId {
        let global = self.global();
        let window = global.as_window();
        let sequence = self.write_sequence.get().wrapping_add(1);
        self.write_sequence.set(sequence);
        let id = ClipboardWriteId {
            pipeline: window.pipeline_id(),
            sequence,
        };
        let callback =
            callback_promise(promise, self, global.task_manager().clipboard_task_source());
        window.send_to_embedder(EmbedderMsg::BeginClipboardWrite(
            window.webview_id(),
            id,
            callback,
        ));
        id
    }

    fn request_read(&self, cx: &mut JSContext, promise: &RootedPromise, kind: ReadKind) {
        let permission = PermissionName::Clipboard_read;
        let Some(state) = self.focused_permission(cx, promise, permission) else {
            return;
        };
        if state == PermissionState::Granted {
            self.read_from_system_clipboard(promise, kind);
            return;
        }
        let global = self.global();
        if state == PermissionState::Denied || !global.as_window().has_transient_activation() {
            promise.reject_error(cx, access_denied());
            return;
        }

        let clipboard = Trusted::new(self);
        let mut pending_promise = Some(TrustedPromise::from(promise));
        let task_source = global.task_manager().clipboard_task_source().to_sendable();
        let callback = GenericCallback::new(move |response| {
            let Some(pending_promise) = pending_promise.take() else {
                return;
            };
            let clipboard = clipboard.clone();
            task_source.queue(task!(clipboard_read_permission: move |cx| {
                let clipboard = clipboard.root();
                let promise = pending_promise.root(cx);
                let global = clipboard.global();
                if !global.as_window().Document().is_fully_active() {
                    promise.resolve_native(cx, &());
                    return;
                }
                record_permission_request_result(
                    permission,
                    &global,
                    response.unwrap_or(AllowOrDeny::Deny),
                );
                if let Some(state) = clipboard.focused_permission(cx, &promise, permission) {
                    if state == PermissionState::Granted {
                        clipboard.read_from_system_clipboard(&promise, kind);
                    } else {
                        promise.reject_error(cx, access_denied());
                    }
                }
            }));
        })
        .expect("Could not create clipboard permission callback");
        match request_permission_to_use_async(permission, &global, callback) {
            AsyncPermissionRequest::Settled(PermissionState::Granted) => {
                self.read_from_system_clipboard(promise, kind);
            },
            AsyncPermissionRequest::Settled(_) => promise.reject_error(cx, access_denied()),
            AsyncPermissionRequest::Prompting => {},
        }
    }

    fn read_from_system_clipboard(&self, promise: &RootedPromise, kind: ReadKind) {
        let global = self.global();
        let window = global.as_window();
        let task_manager = global.task_manager();
        let task_source = task_manager.clipboard_task_source();
        let message = match kind {
            ReadKind::Text => EmbedderMsg::GetClipboardText(
                window.webview_id(),
                callback_promise(promise, self, task_source),
            ),
            ReadKind::Items { unsanitized_html } => {
                let clipboard = Trusted::new(self);
                let mut promise = Some(TrustedPromise::from(promise));
                let task_source = task_source.to_sendable();
                let callback = GenericCallback::new(
                    move |response: Result<Result<ClipboardSnapshot, String>, _>| {
                        let Some(promise) = promise.take() else {
                            return;
                        };
                        let clipboard = clipboard.clone();
                        let response = response.unwrap_or_else(|error| Err(error.to_string()));
                        task_source.queue(task!(clipboard_read_items: move |cx| {
                        let promise = promise.root(cx);
                        clipboard.root().resolve_items(cx, &promise, response, unsanitized_html);
                    }));
                    },
                )
                .expect("Could not create clipboard item read callback");
                EmbedderMsg::GetClipboardData(window.webview_id(), callback)
            },
        };
        window.send_to_embedder(message);
    }
}

impl ClipboardMethods<crate::DomTypeHolder> for Clipboard {
    event_handler!(clipboardchange, GetOnclipboardchange, SetOnclipboardchange);

    /// <https://w3c.github.io/clipboard-apis/#dom-clipboard-read>
    fn Read(
        &self,
        realm: &mut CurrentRealm,
        formats: &ClipboardUnsanitizedFormats,
    ) -> RootedPromise {
        let promise = Promise::new_in_realm(realm);
        if formats
            .unsanitized
            .as_ref()
            .is_some_and(|formats| formats.iter().any(|format| format != "text/html"))
        {
            promise.reject_error(
                realm,
                Error::NotAllowed(Some("Only HTML can be requested unsanitized.".into())),
            );
            return promise;
        }

        self.request_read(
            realm,
            &promise,
            ReadKind::Items {
                unsanitized_html: formats
                    .unsanitized
                    .as_ref()
                    .is_some_and(|formats| !formats.is_empty()),
            },
        );
        promise
    }

    /// <https://w3c.github.io/clipboard-apis/#dom-clipboard-write>
    fn Write(&self, realm: &mut CurrentRealm, items: Vec<DomRoot<ClipboardItem>>) -> RootedPromise {
        let promise = Promise::new_in_realm(realm);
        if !self.authorize_write(realm, &promise) {
            return promise;
        }
        let item = match items.as_slice() {
            [] => {
                promise.resolve_native(realm, &());
                return promise;
            },
            [item] => item,
            _ => {
                promise.reject_error(
                    realm,
                    Error::NotAllowed(Some("The clipboard holds one item.".into())),
                );
                return promise;
            },
        };
        let representations = match item.data_promises(realm) {
            Ok(representations) => representations,
            Err(error) => {
                promise.reject_error(realm, error);
                return promise;
            },
        };
        if representations
            .iter()
            .any(|(format, _)| !is_supported_format(format)) ||
            representations
                .iter()
                .filter(|(format, _)| format.starts_with("web "))
                .count() >
                100
        {
            promise.reject_error(
                realm,
                Error::NotAllowed(Some(
                    "Unsupported clipboard formats or too many custom formats.".into(),
                )),
            );
            return promise;
        }

        let id = self.begin_write(&promise);
        let global = self.global();
        let pending = Rc::new(RefCell::new(PendingRepresentations {
            data: representations
                .iter()
                .map(|(format, _)| (format.clone(), Vec::new()))
                .collect(),
            remaining: representations.len(),
        }));
        for (index, (_, data)) in representations.iter().enumerate() {
            rooted!(&in(realm) let mut handler = Some(RepresentationWriteHandler {
                clipboard: Dom::from_ref(self),
                promise: promise.to_traced(),
                pending: pending.clone(),
                id,
                index,
            }));
            rooted!(&in(realm) let mut rejection = Some(WriteRejectionHandler {
                promise: promise.to_traced(),
                clipboard: Dom::from_ref(self),
                id,
            }));
            let handler = PromiseNativeHandler::new(
                realm,
                &global,
                handler
                    .take()
                    .map(|handler| Box::new(handler) as Box<dyn Callback>),
                rejection
                    .take()
                    .map(|handler| Box::new(handler) as Box<dyn Callback>),
            );
            data.append_native_handler(realm, &handler);
        }
        promise
    }

    /// <https://w3c.github.io/clipboard-apis/#dom-clipboard-readtext>
    fn ReadText(&self, realm: &mut CurrentRealm) -> RootedPromise {
        let promise = Promise::new_in_realm(realm);
        self.request_read(realm, &promise, ReadKind::Text);
        promise
    }

    /// <https://w3c.github.io/clipboard-apis/#dom-clipboard-writetext>
    fn WriteText(&self, realm: &mut CurrentRealm, data: DOMString) -> RootedPromise {
        let promise = Promise::new_in_realm(realm);
        if !self.authorize_write(realm, &promise) {
            return promise;
        }

        let id = self.begin_write(&promise);
        let data = vec![("text/plain".into(), String::from(data).into_bytes())];
        self.global()
            .as_window()
            .send_to_embedder(EmbedderMsg::CommitClipboardWrite(id, data));
        promise
    }
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct RepresentationWriteHandler {
    clipboard: Dom<Clipboard>,
    promise: TracedPromise,
    #[no_trace]
    #[ignore_malloc_size_of = "Shared by all handlers for this write"]
    pending: Rc<RefCell<PendingRepresentations>>,
    index: usize,
    #[no_trace]
    #[ignore_malloc_size_of = "Contains only scalar values"]
    id: ClipboardWriteId,
}

struct PendingRepresentations {
    data: ClipboardData,
    remaining: usize,
}

impl js::gc::Rootable for RepresentationWriteHandler {}

impl Callback for RepresentationWriteHandler {
    fn callback(&self, cx: &mut CurrentRealm, value: HandleValue) {
        let promise = self.promise.root(cx);
        if !promise.is_pending() {
            return;
        }
        let global = self.clipboard.global();
        let window = global.as_window();
        if !window.Document().is_fully_active() {
            window.send_to_embedder(EmbedderMsg::AbortClipboardWrite(self.id));
            promise.resolve_native(cx, &());
            return;
        }

        let format = self.pending.borrow().data[self.index].0.clone();
        let mut bytes = if let Ok(ConversionResult::Success(blob)) =
            DomRoot::<Blob>::from_jsval(cx, value, ())
        {
            let blob_type = blob.Type();
            if blob_type != format.as_str() &&
                !format
                    .strip_prefix("web ")
                    .is_some_and(|unprefixed| blob_type == unprefixed)
            {
                window.send_to_embedder(EmbedderMsg::AbortClipboardWrite(self.id));
                promise.reject_error(
                    cx,
                    Error::NotAllowed(Some(
                        "The Blob type must match the clipboard format.".into(),
                    )),
                );
                return;
            }
            let Ok(bytes) = blob.get_bytes() else {
                window.send_to_embedder(EmbedderMsg::AbortClipboardWrite(self.id));
                promise.reject_error(
                    cx,
                    Error::Data(Some("Could not read clipboard Blob.".into())),
                );
                return;
            };
            bytes
        } else {
            if format == "image/png" {
                window.send_to_embedder(EmbedderMsg::AbortClipboardWrite(self.id));
                promise.reject_error(
                    cx,
                    Error::Type(c"PNG clipboard data must be a Blob".to_owned()),
                );
                return;
            }
            let Ok(ConversionResult::Success(text)) =
                DOMString::from_jsval(cx, value, StringificationBehavior::Default)
            else {
                window.send_to_embedder(EmbedderMsg::AbortClipboardWrite(self.id));
                promise.reject_error(cx, Error::JSFailed);
                return;
            };
            String::from(text).into_bytes()
        };
        if matches!(format.as_str(), "text/html" | "image/svg+xml") {
            match sanitize_markup(cx, window, &bytes) {
                Ok(sanitized) => bytes = sanitized,
                Err(error) => {
                    window.send_to_embedder(EmbedderMsg::AbortClipboardWrite(self.id));
                    promise.reject_error(cx, error);
                    return;
                },
            }
        }
        let data = {
            let mut pending = self.pending.borrow_mut();
            pending.data[self.index].1 = bytes;
            pending.remaining -= 1;
            if pending.remaining != 0 {
                return;
            }
            std::mem::take(&mut pending.data)
        };
        window.send_to_embedder(EmbedderMsg::CommitClipboardWrite(self.id, data));
    }
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct WriteRejectionHandler {
    promise: TracedPromise,
    clipboard: Dom<Clipboard>,
    #[no_trace]
    #[ignore_malloc_size_of = "Contains only scalar values"]
    id: ClipboardWriteId,
}

impl js::gc::Rootable for WriteRejectionHandler {}

impl Callback for WriteRejectionHandler {
    fn callback(&self, cx: &mut CurrentRealm, _value: HandleValue) {
        self.clipboard
            .global()
            .as_window()
            .send_to_embedder(EmbedderMsg::AbortClipboardWrite(self.id));
        self.promise.reject_error(
            cx,
            Error::NotAllowed(Some("Clipboard data could not be resolved.".into())),
        );
    }
}

impl Clipboard {
    fn resolve_items(
        &self,
        cx: &mut JSContext,
        promise: &RootedPromise,
        response: Result<ClipboardSnapshot, String>,
        unsanitized_html: bool,
    ) {
        let global = self.global();
        let window = global.as_window();
        if !window.Document().is_fully_active() {
            promise.resolve_native(cx, &());
            return;
        }
        match response {
            Ok(ClipboardSnapshot { mut data, revision }) => {
                data.retain(|(format, _)| is_supported_format(format));
                for (format, bytes) in &mut data {
                    if format == "image/svg+xml" || (format == "text/html" && !unsanitized_html) {
                        match sanitize_markup(cx, window, bytes) {
                            Ok(sanitized) => *bytes = sanitized,
                            Err(error) => {
                                promise.reject_error(cx, error);
                                return;
                            },
                        }
                    }
                }
                let items = if data.is_empty() {
                    Vec::new()
                } else {
                    vec![ClipboardItem::from_data(cx, window, data, revision)]
                };
                promise.resolve_native(cx, &items);
            },
            Err(message) => promise.reject_error(cx, Error::NotReadable(Some(message))),
        }
    }
}

impl RoutedPromiseListener<crate::DomTypeHolder, Result<(), ClipboardWriteError>> for Clipboard {
    fn handle_response(
        &self,
        cx: &mut JSContext,
        response: Result<(), ClipboardWriteError>,
        promise: &RootedPromise,
    ) {
        match response {
            Ok(()) => promise.resolve_native(cx, &()),
            Err(ClipboardWriteError::Superseded) => promise.reject_error(
                cx,
                Error::NotAllowed(Some(
                    "A newer clipboard write superseded this request.".into(),
                )),
            ),
            Err(ClipboardWriteError::Platform(message)) => {
                promise.reject_error(cx, Error::Data(Some(message)))
            },
        }
    }
}

impl RoutedPromiseListener<crate::DomTypeHolder, Result<String, String>> for Clipboard {
    fn handle_response(
        &self,
        cx: &mut JSContext,
        response: Result<String, String>,
        promise: &RootedPromise,
    ) {
        if self.global().as_window().Document().is_fully_active() {
            promise.resolve_native(cx, &DOMString::from(response.unwrap_or_default()));
        } else {
            promise.resolve_native(cx, &());
        }
    }
}

pub(crate) fn sanitize_markup(
    cx: &mut JSContext,
    window: &Window,
    bytes: &[u8],
) -> Result<Vec<u8>, Error> {
    let mut realm = enter_auto_realm(cx, window);
    let cx = &mut realm.current_realm();
    let document = window.Document();
    let context = document
        .GetBody()
        .map(DomRoot::upcast)
        .or_else(|| document.GetDocumentElement())
        .ok_or_else(|| {
            Error::InvalidState(Some("The document has no HTML parsing context.".into()))
        })?;
    let options = RootedTraceableBox::new(Sanitizer::clipboard_html_options());
    let fragment = Sanitizer::parse_and_filter_html(
        cx,
        &context,
        DOMString::from(String::from_utf8_lossy(bytes)),
        &*options,
        true,
    )?;
    Ok(fragment.map_or_else(Vec::new, |fragment| {
        inline_clipboard_styles(cx, &fragment);
        String::from(fragment.html_serialize(
            cx,
            TraversalScope::ChildrenOnly(None),
            false,
            Vec::new(),
        ))
        .into_bytes()
    }))
}

pub(super) fn clipboard_permission_state(
    window: &Window,
    permission: PermissionName,
) -> PermissionState {
    let mut permission_state =
        descriptor_permission_state(permission, Some(window.as_global_scope()));
    // Permission overrides currently live on globals. A same-origin frame shares
    // the permission decision of its embedding document unless it has its own.
    let proxy = window.window_proxy();
    let mut ancestor = proxy.parent();
    while permission_state == PermissionState::Prompt {
        let Some(parent) = ancestor else { break };
        let Some(parent_document) = parent.document() else {
            break;
        };
        if !window.origin().same_origin(&parent_document.origin()) {
            break;
        }
        permission_state = descriptor_permission_state(permission, Some(&parent_document.global()));
        ancestor = parent.parent();
    }

    permission_state
}
