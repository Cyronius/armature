/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! <https://wicg.github.io/observable/#event-target-integration>

use std::ptr;

use js::context::JSContext;
use js::jsapi::{
    CallArgs, GetFunctionNativeReserved, JS_GetFunctionObject, JSContext as RawJSContext,
    SetFunctionNativeReserved,
};
use js::jsval::{JSVal, ObjectValue, UndefinedValue};
use js::realm::CurrentRealm;
use js::rust::HandleValue;
use js::rust::wrappers2::NewFunctionWithReserved;
use script_bindings::domstring::TracedDOMString;
use script_bindings::reflector::DomObject;

use crate::dom::bindings::codegen::Bindings::EventListenerBinding::EventListener;
use crate::dom::bindings::codegen::Bindings::EventTargetBinding::{
    AddEventListenerOptions, EventListenerOptions,
};
use crate::dom::bindings::codegen::Bindings::ObservableBinding::ObservableEventListenerOptions;
use crate::dom::bindings::conversions::root_from_object;
use crate::dom::bindings::reflector::DomGlobal;
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::bindings::str::DOMString;
use crate::dom::eventtarget::EventTarget;
use crate::dom::observable::observable::{Observable, SubscribeSteps};
use crate::dom::observable::subscriber::Subscriber;

const SLOT_SUBSCRIBER: usize = 0;

/// The subscribe callback of the [`Observable`] that [`EventTarget.when()`] returns.
///
/// [`EventTarget.when()`]: https://wicg.github.io/observable/#dom-eventtarget-when
#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct WhenSteps {
    /// Note: the spec holds the event target weakly. A strong reference is fine here.
    target: Dom<EventTarget>,
    ty: TracedDOMString,
    capture: bool,
    passive: Option<bool>,
}

impl SubscribeSteps for WhenSteps {
    fn subscribe(&self, cx: &mut CurrentRealm, subscriber: &Subscriber) {
        // Step 2.1. If subscriber's subscription controller's signal is aborted, then return.
        let signal = subscriber.subscription_signal();
        if signal.aborted() {
            return;
        }

        // Step 2.2. Add an event listener with event target and an event listener whose type is
        // type, callback is an algorithm that runs subscriber's next() with the event, capture is
        // options["capture"], passive is options["passive"], once is false, and signal is
        // subscriber's signal.
        // Note: The listener is removed again by the abort algorithm of the signal, that matches
        // it by the identity of the function.
        let listener = create_event_listener(cx, subscriber);
        self.target.add_event_listener(
            self.ty.clone().root(),
            Some(listener),
            AddEventListenerOptions {
                parent: EventListenerOptions {
                    capture: self.capture,
                },
                once: false,
                passive: self.passive,
                signal: Some(signal),
            },
        );
    }
}

/// <https://wicg.github.io/observable/#dom-eventtarget-when>
pub(crate) fn when(
    cx: &mut JSContext,
    target: &EventTarget,
    ty: DOMString,
    options: &ObservableEventListenerOptions,
) -> DomRoot<Observable> {
    Observable::new_native(
        cx,
        &target.global(),
        Box::new(WhenSteps {
            target: Dom::from_ref(target),
            ty: ty.traced(),
            capture: options.capture,
            passive: options.passive,
        }),
    )
}

/// Creates a function that runs `subscriber.next()` with its first argument, wrapped as an
/// [`EventListener`].
#[expect(unsafe_code)]
fn create_event_listener(
    cx: &mut JSContext,
    subscriber: &Subscriber,
) -> script_bindings::callback::RootedCallback<EventListener> {
    unsafe {
        let function =
            NewFunctionWithReserved(cx, Some(event_listener_callback), 1, 0, ptr::null());
        assert!(!function.is_null());

        rooted!(&in(cx) let function_object = JS_GetFunctionObject(function));
        assert!(!function_object.is_null());
        SetFunctionNativeReserved(
            function_object.get(),
            SLOT_SUBSCRIBER,
            &ObjectValue(subscriber.reflector().get_jsobject().get()),
        );
        EventListener::new(cx, function_object.get())
    }
}

#[expect(unsafe_code)]
unsafe extern "C" fn event_listener_callback(
    cx: *mut RawJSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: it is safe to construct a JSContext from engine hook.
    let mut cx = unsafe { JSContext::from_ptr(ptr::NonNull::new(cx).unwrap()) };
    let mut cx = CurrentRealm::assert(&mut cx);
    let cx = &mut cx;

    let args = unsafe { CallArgs::from_vp(vp, argc) };
    let subscriber_value = unsafe { *GetFunctionNativeReserved(args.callee(), SLOT_SUBSCRIBER) };
    rooted!(&in(cx) let subscriber_value = subscriber_value);
    assert!(subscriber_value.get().is_object());

    let subscriber = unsafe { root_from_object::<Subscriber>(cx, subscriber_value.to_object()) }
        .expect("unexpected value for the subscriber of an event listener");

    // Run subscriber's next() with the event.
    subscriber.next(cx, unsafe { HandleValue::from_raw(args.get(0)) });

    args.rval().set(UndefinedValue());
    true
}
