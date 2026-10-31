/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::Cell;
use std::rc::Rc;

use dom_struct::dom_struct;
use js::context::JSContext;
use js::jsval::UndefinedValue;
use js::realm::CurrentRealm;
use js::rust::HandleValue;
use malloc_size_of::MallocSizeOf;
use script_bindings::callback::{ExceptionHandling, RootedCallback, TracedCallback};
use script_bindings::cell::DomRefCell;
use script_bindings::reflector::{Reflector, reflect_weak_referenceable_dom_object};

use crate::dom::abortsignal::AbortSignal;
use crate::dom::bindings::codegen::Bindings::ObservableBinding::ObservableSubscriptionCallback;
use crate::dom::bindings::codegen::Bindings::SubscriberBinding::SubscriberMethods;
use crate::dom::bindings::codegen::Bindings::VoidFunctionBinding::VoidFunction;
use crate::dom::bindings::error::{Error, ErrorToJsval};
use crate::dom::bindings::reflector::DomGlobal;
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::bindings::trace::JSTraceable;
use crate::dom::globalscope::GlobalScope;
use crate::dom::observable::observable::is_inactive_document;

/// The steps of an [internal observer].
///
/// Implementors follow the same rooting requirements as types that use the `#[dom_struct]`
/// attribute: store `Dom<T>` members instead of `DomRoot<T>`.
///
/// [internal observer]: https://wicg.github.io/observable/#internal-observer
pub(crate) trait InternalObserverSteps: JSTraceable + MallocSizeOf {
    /// <https://wicg.github.io/observable/#internal-observer-next-steps>
    fn next(&self, _cx: &mut CurrentRealm, _value: HandleValue) {}

    /// <https://wicg.github.io/observable/#internal-observer-error-steps>
    ///
    /// The default is the spec's "default error algorithm".
    fn error(&self, cx: &mut CurrentRealm, error: HandleValue) {
        default_error_algorithm(cx, error)
    }

    /// <https://wicg.github.io/observable/#internal-observer-complete-steps>
    fn complete(&self, _cx: &mut CurrentRealm) {}
}

/// <https://wicg.github.io/observable/#default-error-algorithm>
fn default_error_algorithm(cx: &mut CurrentRealm, error: HandleValue) {
    // Report the exception given error.
    GlobalScope::from_current_realm(cx).report_an_exception(cx, error);
}

/// <https://wicg.github.io/observable/#internal-observer>
#[derive(JSTraceable, MallocSizeOf)]
pub(crate) struct InternalObserver(Box<dyn InternalObserverSteps>);

impl InternalObserver {
    pub(crate) fn new(steps: impl InternalObserverSteps + 'static) -> Rc<InternalObserver> {
        Rc::new(InternalObserver(Box::new(steps)))
    }
}

/// An internal observer made from the [`ObserverUnion`] that script passed to `subscribe()`.
///
/// <https://wicg.github.io/observable/#observable-convert-to-an-observer>
///
/// [`ObserverUnion`]: crate::dom::bindings::codegen::Bindings::ObservableBinding::ObserverUnion
#[derive(JSTraceable, MallocSizeOf)]
pub(crate) struct ScriptObserver {
    pub(crate) next: Option<TracedCallback<ObservableSubscriptionCallback>>,
    pub(crate) error: Option<TracedCallback<ObservableSubscriptionCallback>>,
    pub(crate) complete: Option<TracedCallback<VoidFunction>>,
}

impl InternalObserverSteps for ScriptObserver {
    fn next(&self, cx: &mut CurrentRealm, value: HandleValue) {
        if let Some(next) = &self.next {
            let next = next.root(cx);
            let _ = next.Call__(cx, value, ExceptionHandling::Report);
        }
    }

    fn error(&self, cx: &mut CurrentRealm, error: HandleValue) {
        match &self.error {
            Some(callback) => {
                let callback = callback.root(cx);
                let _ = callback.Call__(cx, error, ExceptionHandling::Report);
            },
            None => default_error_algorithm(cx, error),
        }
    }

    fn complete(&self, cx: &mut CurrentRealm) {
        if let Some(complete) = &self.complete {
            let complete = complete.root(cx);
            let _ = complete.Call__(cx, ExceptionHandling::Report);
        }
    }
}

/// A copy of a [`Subscriber`]'s internal observers, that is rooted while script runs.
///
/// <https://wicg.github.io/observable/#subscriber-internal-observers>
#[derive(Clone, JSTraceable)]
struct ObserverSnapshot(Vec<Rc<InternalObserver>>);

impl js::gc::Rootable for ObserverSnapshot {}

/// <https://wicg.github.io/observable/#subscriber>
#[dom_struct]
pub(crate) struct Subscriber {
    reflector_: Reflector,

    /// <https://wicg.github.io/observable/#subscriber-internal-observers>
    #[ignore_malloc_size_of = "Rc"]
    internal_observers: DomRefCell<Vec<Rc<InternalObserver>>>,

    /// <https://wicg.github.io/observable/#subscriber-teardown-callbacks>
    teardowns: DomRefCell<Vec<TracedCallback<VoidFunction>>>,

    /// The signal of the [subscription controller].
    ///
    /// [subscription controller]: https://wicg.github.io/observable/#subscriber-subscription-controller
    signal: Dom<AbortSignal>,

    /// <https://wicg.github.io/observable/#subscriber-active>
    active: Cell<bool>,
}

impl Subscriber {
    /// Creates a new active subscriber with no internal observers.
    pub(crate) fn new(cx: &mut JSContext, global: &GlobalScope) -> DomRoot<Subscriber> {
        let signal = AbortSignal::new_with_proto(cx, global, None);
        reflect_weak_referenceable_dom_object(
            cx,
            Rc::new(Subscriber {
                reflector_: Reflector::new(),
                internal_observers: Default::default(),
                teardowns: Default::default(),
                signal: Dom::from_ref(&signal),
                active: Cell::new(true),
            }),
            global,
        )
    }

    pub(crate) fn add_internal_observer(&self, observer: Rc<InternalObserver>) {
        self.internal_observers.borrow_mut().push(observer);
    }

    pub(crate) fn remove_internal_observer(&self, observer: &Rc<InternalObserver>) {
        self.internal_observers
            .borrow_mut()
            .retain(|candidate| !Rc::ptr_eq(candidate, observer));
    }

    pub(crate) fn has_internal_observers(&self) -> bool {
        !self.internal_observers.borrow().is_empty()
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active.get()
    }

    pub(crate) fn subscription_signal(&self) -> DomRoot<AbortSignal> {
        self.signal.as_rooted()
    }

    fn observers(&self) -> ObserverSnapshot {
        ObserverSnapshot(self.internal_observers.borrow().clone())
    }

    /// <https://wicg.github.io/observable/#dom-subscriber-next>
    pub(crate) fn next(&self, cx: &mut CurrentRealm, value: HandleValue) {
        // Step 1. If this's relevant global object is a Window object, and its associated
        // Document is not fully active, then return.
        // Step 2. If this is not active, then return.
        if !self.is_active() || is_inactive_document(&self.global()) {
            return;
        }

        // Step 3. Otherwise, run next steps with value for each internal observer in a copy of
        // this's internal observers.
        rooted!(&in(cx) let observers = self.observers());
        for observer in observers.0.iter() {
            observer.0.next(cx, value);
        }
    }

    /// <https://wicg.github.io/observable/#dom-subscriber-error>
    pub(crate) fn error(&self, cx: &mut CurrentRealm, error: HandleValue) {
        // Step 1. If this is not active, then report the exception error, and return.
        if !self.is_active() {
            self.global().report_an_exception(cx, error);
            return;
        }

        // Step 2. If this's relevant global object is a Window object, and its associated
        // Document is not fully active, then return.
        if is_inactive_document(&self.global()) {
            return;
        }

        // Step 3. Close this.
        // Note: The WPT test "Subscriber#error() value is stored as Subscriber's AbortSignal's
        // reason" expects the error to be the signal's reason, as in Chromium.
        self.close(cx, error);

        // Step 4. Run error steps with error for each internal observer in a copy of this's
        // internal observers.
        rooted!(&in(cx) let observers = self.observers());
        for observer in observers.0.iter() {
            observer.0.error(cx, error);
        }
    }

    /// <https://wicg.github.io/observable/#dom-subscriber-complete>
    pub(crate) fn complete(&self, cx: &mut CurrentRealm) {
        // Step 1. If this is not active, then return.
        // Step 2. If this's relevant global object is a Window object, and its associated
        // Document is not fully active, then return.
        if !self.is_active() || is_inactive_document(&self.global()) {
            return;
        }

        // Step 3. Close this.
        self.close(cx, HandleValue::undefined());

        // Step 4. Run complete steps for each internal observer in a copy of this's internal
        // observers.
        rooted!(&in(cx) let observers = self.observers());
        for observer in observers.0.iter() {
            observer.0.complete(cx);
        }
    }

    /// <https://wicg.github.io/observable/#dom-subscriber-addteardown>
    pub(crate) fn add_teardown(
        &self,
        cx: &mut CurrentRealm,
        teardown: RootedCallback<VoidFunction>,
    ) {
        // Step 1. If this's relevant global object is a Window object, and its associated
        // Document is not fully active, then return.
        if is_inactive_document(&self.global()) {
            return;
        }

        if self.is_active() {
            // Step 2. If this is active, then append teardown to this's teardown callbacks list.
            self.teardowns.borrow_mut().push(teardown.to_traced());
        } else {
            // Step 3. Otherwise, invoke teardown.
            let _ = teardown.Call__(cx, ExceptionHandling::Report);
        }
    }

    /// <https://wicg.github.io/observable/#close-a-subscription>
    pub(crate) fn close(&self, cx: &mut CurrentRealm, reason: HandleValue) {
        // Step 1. If subscriber is not active, then return.
        // Note: This also guards against re-entrancy.
        if !self.is_active() {
            return;
        }

        // Step 2. Set subscriber's active boolean to false.
        self.active.set(false);

        // Step 3. Signal abort on subscriber's subscription controller with reason, if it is
        // given.
        self.signal.signal_abort(cx, reason);

        // Step 4. For each teardown of subscriber's teardown callbacks sorted in reverse
        // insertion order:
        // Note: Teardowns added while closing run immediately, as the subscriber is no longer
        // active, so taking the list is equivalent to the spec.
        let teardowns = std::mem::take(&mut *self.teardowns.borrow_mut());
        let teardowns: Vec<_> = teardowns.iter().map(|teardown| teardown.root(cx)).collect();
        for teardown in teardowns.iter().rev() {
            // Step 4.1. If subscriber's relevant global object is a Window object, and its
            // associated Document is not fully active, then abort these steps.
            if is_inactive_document(&self.global()) {
                return;
            }

            // Step 4.2. Invoke teardown.
            let _ = teardown.Call__(cx, ExceptionHandling::Report);
        }
    }
}

/// Runs the "error" steps of `subscriber` with the exception that `error` stands for.
///
/// This is the "rethrow, and if an exception E was thrown, run subscriber's error() with E"
/// pattern of the spec.
pub(crate) fn error_subscriber_with(cx: &mut CurrentRealm, subscriber: &Subscriber, error: Error) {
    rooted!(&in(cx) let mut exception = UndefinedValue());
    // For `Error::JSFailed` this takes and clears the pending exception.
    error.to_jsval(cx, &subscriber.global(), exception.handle_mut());
    subscriber.error(cx, exception.handle());
}

impl SubscriberMethods<crate::DomTypeHolder> for Subscriber {
    /// <https://wicg.github.io/observable/#dom-subscriber-next>
    fn Next(&self, cx: &mut CurrentRealm, value: HandleValue) {
        self.next(cx, value);
    }

    /// <https://wicg.github.io/observable/#dom-subscriber-error>
    fn Error(&self, cx: &mut CurrentRealm, error: HandleValue) {
        self.error(cx, error);
    }

    /// <https://wicg.github.io/observable/#dom-subscriber-complete>
    fn Complete(&self, cx: &mut CurrentRealm) {
        self.complete(cx);
    }

    /// <https://wicg.github.io/observable/#dom-subscriber-addteardown>
    fn AddTeardown(&self, cx: &mut CurrentRealm, teardown: RootedCallback<VoidFunction>) {
        self.add_teardown(cx, teardown);
    }

    /// <https://wicg.github.io/observable/#dom-subscriber-active>
    fn Active(&self) -> bool {
        self.is_active()
    }

    /// <https://wicg.github.io/observable/#dom-subscriber-signal>
    fn Signal(&self) -> DomRoot<AbortSignal> {
        self.subscription_signal()
    }
}
