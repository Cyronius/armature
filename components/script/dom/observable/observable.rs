/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::rc::Rc;

use dom_struct::dom_struct;
use js::context::JSContext;
use js::realm::CurrentRealm;
use js::rust::{HandleObject, HandleValue};
use malloc_size_of::MallocSizeOf;
use script_bindings::callback::{ExceptionHandling, RootedCallback, TracedCallback};
use script_bindings::cell::DomRefCell;
use script_bindings::inheritance::Castable;
use script_bindings::reflector::{Reflector, reflect_dom_object_with_proto};
use script_bindings::weakref::WeakRef;

use crate::dom::abortsignal::{AbortAlgorithm, AbortSignal};
use crate::dom::bindings::codegen::Bindings::AbortSignalBinding::AbortSignalMethods;
use crate::dom::bindings::codegen::Bindings::ObservableBinding::{
    Mapper, ObservableInspectorUnion, ObservableMethods, ObserverUnion, Predicate, Reducer,
    SubscribeCallback, SubscribeOptions, Visitor,
};
use crate::dom::bindings::codegen::Bindings::VoidFunctionBinding::VoidFunction;
use crate::dom::bindings::codegen::Bindings::WindowBinding::WindowMethods;
use crate::dom::bindings::reflector::DomGlobal;
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::bindings::trace::JSTraceable;
use crate::dom::globalscope::GlobalScope;
use crate::dom::observable::operators::{self, InspectAbortState};
use crate::dom::observable::promise_operators::{self, PredicateOperator};
use crate::dom::observable::subscriber::{
    InternalObserver, ScriptObserver, Subscriber, error_subscriber_with,
};
use crate::dom::promise::{RootedPromise, TracedPromise};
use crate::dom::window::Window;

/// Whether the global is a Window whose document is not fully active.
///
/// The spec checks this in many places and silently does nothing when it holds.
pub(crate) fn is_inactive_document(global: &GlobalScope) -> bool {
    global
        .downcast::<Window>()
        .is_some_and(|window| !window.Document().is_fully_active())
}

/// A subscribe callback that is implemented natively.
pub(crate) trait SubscribeSteps: JSTraceable + MallocSizeOf {
    fn subscribe(&self, cx: &mut CurrentRealm, subscriber: &Subscriber);
}

/// <https://wicg.github.io/observable/#observable-subscribe-callback>
#[derive(JSTraceable, MallocSizeOf)]
enum SubscribeCallbackKind {
    /// A callback given to the constructor.
    Script(TracedCallback<SubscribeCallback>),
    /// The subscribe callback of an Observable that the UA creates, such as the result of
    /// `EventTarget.when()` or of an operator.
    Native(Box<dyn SubscribeSteps>),
}

/// <https://wicg.github.io/observable/#observable>
#[dom_struct]
pub(crate) struct Observable {
    reflector_: Reflector,

    /// <https://wicg.github.io/observable/#observable-subscribe-callback>
    subscribe_callback: SubscribeCallbackKind,

    /// <https://wicg.github.io/observable/#observable-weak-subscriber>
    #[ignore_malloc_size_of = "WeakRef"]
    weak_subscriber: DomRefCell<Option<WeakRef<Subscriber>>>,
}

/// The algorithms that Observable adds to [`AbortSignal`]s.
#[derive(Clone, JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
pub(crate) enum ObservableAbortAlgorithm {
    /// <https://wicg.github.io/observable/#subscribe-to-an-observable> steps 4.3 and 8.2
    RemoveObserver {
        subscriber: Dom<Subscriber>,
        #[conditional_malloc_size_of]
        observer: Rc<InternalObserver>,
    },
    /// <https://wicg.github.io/observable/#dom-observable-inspect>
    InspectAbort(#[conditional_malloc_size_of] Rc<InspectAbortState>),
    /// Rejects the promise of a promise-returning operator with the abort reason.
    /// <https://wicg.github.io/observable/#promise-returning-operators>
    RejectPromise(TracedPromise),
}

impl ObservableAbortAlgorithm {
    pub(crate) fn run(&self, cx: &mut CurrentRealm, reason: HandleValue) {
        match self {
            ObservableAbortAlgorithm::RemoveObserver {
                subscriber,
                observer,
            } => {
                // If subscriber is not active, then return.
                if !subscriber.is_active() {
                    return;
                }

                // Remove internal observer from subscriber's internal observers.
                subscriber.remove_internal_observer(observer);

                // If subscriber's internal observers is empty, then close subscriber with the
                // signal's abort reason.
                if !subscriber.has_internal_observers() {
                    subscriber.close(cx, reason);
                }
            },
            ObservableAbortAlgorithm::InspectAbort(state) => state.run(cx, reason),
            ObservableAbortAlgorithm::RejectPromise(promise) => promise.reject(cx, reason),
        }
    }
}

impl Observable {
    fn new_inherited(subscribe_callback: SubscribeCallbackKind) -> Observable {
        Observable {
            reflector_: Reflector::new(),
            subscribe_callback,
            weak_subscriber: Default::default(),
        }
    }

    /// Creates an Observable whose subscribe callback is implemented natively.
    pub(crate) fn new_native(
        cx: &mut JSContext,
        global: &GlobalScope,
        steps: Box<dyn SubscribeSteps>,
    ) -> DomRoot<Observable> {
        reflect_dom_object_with_proto(
            cx,
            Box::new(Observable::new_inherited(SubscribeCallbackKind::Native(
                steps,
            ))),
            global,
            None,
        )
    }

    fn new_with_proto(
        cx: &mut JSContext,
        global: &GlobalScope,
        proto: Option<HandleObject>,
        callback: RootedCallback<SubscribeCallback>,
    ) -> DomRoot<Observable> {
        reflect_dom_object_with_proto(
            cx,
            Box::new(Observable::new_inherited(SubscribeCallbackKind::Script(
                callback.to_traced(),
            ))),
            global,
            proto,
        )
    }

    /// <https://wicg.github.io/observable/#subscribe-to-an-observable>
    ///
    /// `observer` must not be the only thing keeping the objects it refers to alive: callers hold
    /// those in roots for the duration of the call.
    pub(crate) fn subscribe_internal(
        &self,
        cx: &mut CurrentRealm,
        observer: Rc<InternalObserver>,
        signal: Option<&AbortSignal>,
    ) {
        let global = self.global();

        // Step 1. If this's relevant global object is a Window object, and its associated
        // Document is not fully active, then return.
        if is_inactive_document(&global) {
            return;
        }

        // Step 2. If this has an active weak subscriber, then:
        let existing = self
            .weak_subscriber
            .borrow()
            .as_ref()
            .and_then(|weak| weak.root())
            .filter(|subscriber| subscriber.is_active());
        if let Some(subscriber) = existing {
            // Step 2.1. Append internal observer to the subscriber's internal observers.
            subscriber.add_internal_observer(observer.clone());

            // Step 2.2. If options["signal"] exists:
            if let Some(signal) = signal {
                if signal.aborted() {
                    // Step 2.2.1. If it is aborted, then remove the observer again, without
                    // closing the subscriber, even if no observers remain.
                    subscriber.remove_internal_observer(&observer);
                } else {
                    // Step 2.2.2. Otherwise, add the algorithm that removes the observer.
                    signal.add(&AbortAlgorithm::Observable(
                        ObservableAbortAlgorithm::RemoveObserver {
                            subscriber: Dom::from_ref(&subscriber),
                            observer,
                        },
                    ));
                }
            }

            // Step 2.3. Return.
            return;
        }

        // Step 3. Let subscriber be a new Subscriber, with internal observer as its only
        // internal observer, and this's weak subscriber pointing at it.
        // Note: The subscriber is allocated before the observer is stored anywhere that is traced.
        let subscriber = Subscriber::new(cx, &global);
        subscriber.add_internal_observer(observer.clone());
        *self.weak_subscriber.borrow_mut() = Some(WeakRef::new(&subscriber));

        // Step 4. If options["signal"] exists:
        if let Some(signal) = signal {
            if signal.aborted() {
                // Step 4.1. If it is aborted, then close subscriber with the signal's reason.
                rooted!(&in(cx) let mut reason = js::jsval::UndefinedValue());
                signal.Reason(reason.handle_mut());
                subscriber.close(cx, reason.handle());
            } else {
                // Step 4.2. Otherwise, add the algorithm that removes the observer.
                signal.add(&AbortAlgorithm::Observable(
                    ObservableAbortAlgorithm::RemoveObserver {
                        subscriber: Dom::from_ref(&subscriber),
                        observer,
                    },
                ));
            }
        }

        // Step 5. Invoke this's subscribe callback with subscriber, and "rethrow".
        // If an exception E was thrown, then run subscriber's error() with E.
        match &self.subscribe_callback {
            SubscribeCallbackKind::Script(callback) => {
                let callback = callback.root(cx);
                if let Err(error) = callback.Call__(cx, &subscriber, ExceptionHandling::Rethrow) {
                    error_subscriber_with(cx, &subscriber, error);
                }
            },
            SubscribeCallbackKind::Native(steps) => steps.subscribe(cx, &subscriber),
        }
    }
}

impl ObservableMethods<crate::DomTypeHolder> for Observable {
    /// <https://wicg.github.io/observable/#dom-observable-observable>
    fn Constructor(
        cx: &mut JSContext,
        global: &GlobalScope,
        proto: Option<HandleObject>,
        callback: RootedCallback<SubscribeCallback>,
    ) -> DomRoot<Observable> {
        // Step 1. Set this's subscribe callback to callback.
        Observable::new_with_proto(cx, global, proto, callback)
    }

    /// <https://wicg.github.io/observable/#dom-observable-subscribe>
    fn Subscribe(
        &self,
        cx: &mut CurrentRealm,
        observer: ObserverUnion,
        options: &SubscribeOptions,
    ) {
        // Step 1. Let internal observer be the result of converting observer to an internal
        // observer.
        let script_observer = match observer {
            ObserverUnion::ObservableSubscriptionCallback(next) => ScriptObserver {
                next: Some(next.to_traced()),
                error: None,
                complete: None,
            },
            ObserverUnion::SubscriptionObserver(dictionary) => ScriptObserver {
                next: dictionary.next.clone(),
                error: dictionary.error.clone(),
                complete: dictionary.complete.clone(),
            },
        };

        // Step 2. Subscribe to this given internal observer and options.
        self.subscribe_internal(
            cx,
            InternalObserver::new(script_observer),
            options.signal.as_deref(),
        );
    }

    /// <https://wicg.github.io/observable/#dom-observable-map>
    fn Map(&self, cx: &mut CurrentRealm, mapper: RootedCallback<Mapper>) -> DomRoot<Observable> {
        operators::map(cx, self, mapper)
    }

    /// <https://wicg.github.io/observable/#dom-observable-filter>
    fn Filter(
        &self,
        cx: &mut CurrentRealm,
        predicate: RootedCallback<Predicate>,
    ) -> DomRoot<Observable> {
        operators::filter(cx, self, predicate)
    }

    /// <https://wicg.github.io/observable/#dom-observable-take>
    fn Take(&self, cx: &mut CurrentRealm, amount: u64) -> DomRoot<Observable> {
        operators::take(cx, self, amount)
    }

    /// <https://wicg.github.io/observable/#dom-observable-drop>
    fn Drop(&self, cx: &mut CurrentRealm, amount: u64) -> DomRoot<Observable> {
        operators::drop_values(cx, self, amount)
    }

    /// <https://wicg.github.io/observable/#dom-observable-inspect>
    fn Inspect(
        &self,
        cx: &mut CurrentRealm,
        inspector_union: ObservableInspectorUnion,
    ) -> DomRoot<Observable> {
        operators::inspect(cx, self, inspector_union)
    }

    /// <https://wicg.github.io/observable/#dom-observable-finally>
    fn Finally(
        &self,
        cx: &mut CurrentRealm,
        callback: RootedCallback<VoidFunction>,
    ) -> DomRoot<Observable> {
        operators::finally(cx, self, callback)
    }

    /// <https://wicg.github.io/observable/#dom-observable-toarray>
    fn ToArray(&self, cx: &mut CurrentRealm, options: &SubscribeOptions) -> RootedPromise {
        promise_operators::to_array(cx, self, options)
    }

    /// <https://wicg.github.io/observable/#dom-observable-foreach>
    fn ForEach(
        &self,
        cx: &mut CurrentRealm,
        callback: RootedCallback<Visitor>,
        options: &SubscribeOptions,
    ) -> RootedPromise {
        promise_operators::for_each(cx, self, callback, options)
    }

    /// <https://wicg.github.io/observable/#dom-observable-every>
    fn Every(
        &self,
        cx: &mut CurrentRealm,
        predicate: RootedCallback<Predicate>,
        options: &SubscribeOptions,
    ) -> RootedPromise {
        promise_operators::with_predicate(cx, self, PredicateOperator::Every, predicate, options)
    }

    /// <https://wicg.github.io/observable/#dom-observable-first>
    fn First(&self, cx: &mut CurrentRealm, options: &SubscribeOptions) -> RootedPromise {
        promise_operators::first(cx, self, options)
    }

    /// <https://wicg.github.io/observable/#dom-observable-last>
    fn Last(&self, cx: &mut CurrentRealm, options: &SubscribeOptions) -> RootedPromise {
        promise_operators::last(cx, self, options)
    }

    /// <https://wicg.github.io/observable/#dom-observable-find>
    fn Find(
        &self,
        cx: &mut CurrentRealm,
        predicate: RootedCallback<Predicate>,
        options: &SubscribeOptions,
    ) -> RootedPromise {
        promise_operators::with_predicate(cx, self, PredicateOperator::Find, predicate, options)
    }

    /// <https://wicg.github.io/observable/#dom-observable-some>
    fn Some(
        &self,
        cx: &mut CurrentRealm,
        predicate: RootedCallback<Predicate>,
        options: &SubscribeOptions,
    ) -> RootedPromise {
        promise_operators::with_predicate(cx, self, PredicateOperator::Some, predicate, options)
    }

    /// <https://wicg.github.io/observable/#dom-observable-reduce>
    fn Reduce(
        &self,
        cx: &mut CurrentRealm,
        reducer: RootedCallback<Reducer>,
        initial_value: HandleValue,
        options: &SubscribeOptions,
    ) -> RootedPromise {
        promise_operators::reduce(cx, self, reducer, initial_value, options)
    }
}
