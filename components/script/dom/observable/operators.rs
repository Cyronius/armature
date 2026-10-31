/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The operators of <https://wicg.github.io/observable/#observable-api> that return an
//! [`Observable`].
//!
//! Each operator is a native subscribe callback ([`SubscribeSteps`]) that subscribes to its source
//! with a native [`InternalObserverSteps`], and forwards to the subscriber it was given.

use std::cell::Cell;
use std::rc::Rc;

use js::context::JSContext;
use js::jsval::UndefinedValue;
use js::realm::CurrentRealm;
use js::rust::HandleValue;
use script_bindings::callback::{ExceptionHandling, RootedCallback, TracedCallback};

use crate::dom::abortsignal::AbortAlgorithm;
use crate::dom::bindings::codegen::Bindings::ObservableBinding::{
    Mapper, ObservableInspectorAbortHandler, ObservableInspectorUnion,
    ObservableSubscriptionCallback, Predicate,
};
use crate::dom::bindings::codegen::Bindings::VoidFunctionBinding::VoidFunction;
use crate::dom::bindings::reflector::DomGlobal;
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::observable::observable::{Observable, ObservableAbortAlgorithm, SubscribeSteps};
use crate::dom::observable::subscriber::{
    InternalObserver, InternalObserverSteps, Subscriber, error_subscriber_with,
};

fn new_operator(
    cx: &mut JSContext,
    source: &Observable,
    steps: impl SubscribeSteps + 'static,
) -> DomRoot<Observable> {
    Observable::new_native(cx, &source.global(), Box::new(steps))
}

/// Subscribes to `source` for the sake of `subscriber`, with an observer that forwards everything
/// to `subscriber`.
pub(crate) fn subscribe_forwarding(
    cx: &mut CurrentRealm,
    source: &Observable,
    subscriber: &Subscriber,
) {
    let observer = InternalObserver::new(ForwardingObserver {
        subscriber: Dom::from_ref(subscriber),
    });
    source.subscribe_internal(cx, observer, Some(&subscriber.subscription_signal()));
}

/// An internal observer that passes `next()`, `error()` and `complete()` on to `subscriber`.
#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct ForwardingObserver {
    subscriber: Dom<Subscriber>,
}

impl InternalObserverSteps for ForwardingObserver {
    fn next(&self, cx: &mut CurrentRealm, value: HandleValue) {
        self.subscriber.next(cx, value);
    }

    fn error(&self, cx: &mut CurrentRealm, error: HandleValue) {
        self.subscriber.error(cx, error);
    }

    fn complete(&self, cx: &mut CurrentRealm) {
        self.subscriber.complete(cx);
    }
}

/// <https://wicg.github.io/observable/#dom-observable-map>
pub(crate) fn map(
    cx: &mut JSContext,
    source: &Observable,
    mapper: RootedCallback<Mapper>,
) -> DomRoot<Observable> {
    new_operator(
        cx,
        source,
        MapSteps {
            source: Dom::from_ref(source),
            mapper: mapper.to_traced(),
        },
    )
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct MapSteps {
    source: Dom<Observable>,
    mapper: TracedCallback<Mapper>,
}

impl SubscribeSteps for MapSteps {
    fn subscribe(&self, cx: &mut CurrentRealm, subscriber: &Subscriber) {
        // Step 1. Let idx be 0.
        let observer = InternalObserver::new(MapObserver {
            subscriber: Dom::from_ref(subscriber),
            mapper: self.mapper.clone(),
            idx: Cell::new(0),
        });
        // Step 2. Let sourceObserver be a new internal observer.
        // Step 3. Subscribe to this given sourceObserver and options.
        self.source
            .subscribe_internal(cx, observer, Some(&subscriber.subscription_signal()));
    }
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct MapObserver {
    subscriber: Dom<Subscriber>,
    mapper: TracedCallback<Mapper>,
    idx: Cell<u64>,
}

impl InternalObserverSteps for MapObserver {
    fn next(&self, cx: &mut CurrentRealm, value: HandleValue) {
        // Step 1. Invoke mapper with value and idx, and "rethrow".
        let mapper = self.mapper.root(cx);
        let idx = self.idx.get();
        rooted!(&in(cx) let mut mapped_value = UndefinedValue());
        if let Err(error) = mapper.Call__(
            cx,
            value,
            idx,
            mapped_value.handle_mut(),
            ExceptionHandling::Rethrow,
        ) {
            // If an exception E was thrown, then run subscriber's error() with E.
            error_subscriber_with(cx, &self.subscriber, error);
            return;
        }
        // Step 2. Increment idx.
        self.idx.set(idx + 1);
        // Step 3. Run subscriber's next() with mappedValue.
        self.subscriber.next(cx, mapped_value.handle());
    }

    fn error(&self, cx: &mut CurrentRealm, error: HandleValue) {
        self.subscriber.error(cx, error);
    }

    fn complete(&self, cx: &mut CurrentRealm) {
        self.subscriber.complete(cx);
    }
}

/// <https://wicg.github.io/observable/#dom-observable-filter>
pub(crate) fn filter(
    cx: &mut JSContext,
    source: &Observable,
    predicate: RootedCallback<Predicate>,
) -> DomRoot<Observable> {
    new_operator(
        cx,
        source,
        FilterSteps {
            source: Dom::from_ref(source),
            predicate: predicate.to_traced(),
        },
    )
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct FilterSteps {
    source: Dom<Observable>,
    predicate: TracedCallback<Predicate>,
}

impl SubscribeSteps for FilterSteps {
    fn subscribe(&self, cx: &mut CurrentRealm, subscriber: &Subscriber) {
        let observer = InternalObserver::new(FilterObserver {
            subscriber: Dom::from_ref(subscriber),
            predicate: self.predicate.clone(),
            idx: Cell::new(0),
        });
        self.source
            .subscribe_internal(cx, observer, Some(&subscriber.subscription_signal()));
    }
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct FilterObserver {
    subscriber: Dom<Subscriber>,
    predicate: TracedCallback<Predicate>,
    idx: Cell<u64>,
}

impl InternalObserverSteps for FilterObserver {
    fn next(&self, cx: &mut CurrentRealm, value: HandleValue) {
        // Step 1. Invoke predicate with value and idx, and "rethrow".
        let predicate = self.predicate.root(cx);
        let idx = self.idx.get();
        let passed = match predicate.Call__(cx, value, idx, ExceptionHandling::Rethrow) {
            Ok(passed) => passed,
            Err(error) => {
                // If an exception E was thrown, then run subscriber's error() with E.
                error_subscriber_with(cx, &self.subscriber, error);
                return;
            },
        };
        // Step 2. Increment idx.
        self.idx.set(idx + 1);
        // Step 3. If passed is true, then run subscriber's next() with value.
        if passed {
            self.subscriber.next(cx, value);
        }
    }

    fn error(&self, cx: &mut CurrentRealm, error: HandleValue) {
        self.subscriber.error(cx, error);
    }

    fn complete(&self, cx: &mut CurrentRealm) {
        self.subscriber.complete(cx);
    }
}

/// <https://wicg.github.io/observable/#dom-observable-take>
pub(crate) fn take(cx: &mut JSContext, source: &Observable, amount: u64) -> DomRoot<Observable> {
    new_operator(
        cx,
        source,
        TakeSteps {
            source: Dom::from_ref(source),
            amount,
        },
    )
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct TakeSteps {
    source: Dom<Observable>,
    amount: u64,
}

impl SubscribeSteps for TakeSteps {
    fn subscribe(&self, cx: &mut CurrentRealm, subscriber: &Subscriber) {
        // Step 1. Let remaining be amount.
        // Step 2. If remaining is 0, then run subscriber's complete() and return.
        if self.amount == 0 {
            subscriber.complete(cx);
            return;
        }
        let observer = InternalObserver::new(TakeObserver {
            subscriber: Dom::from_ref(subscriber),
            remaining: Cell::new(self.amount),
        });
        self.source
            .subscribe_internal(cx, observer, Some(&subscriber.subscription_signal()));
    }
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct TakeObserver {
    subscriber: Dom<Subscriber>,
    remaining: Cell<u64>,
}

impl InternalObserverSteps for TakeObserver {
    fn next(&self, cx: &mut CurrentRealm, value: HandleValue) {
        // Step 1. Run subscriber's next() with value.
        self.subscriber.next(cx, value);
        // Step 2. Decrement remaining.
        let remaining = self.remaining.get().saturating_sub(1);
        self.remaining.set(remaining);
        // Step 3. If remaining is 0, then run subscriber's complete().
        if remaining == 0 {
            self.subscriber.complete(cx);
        }
    }

    fn error(&self, cx: &mut CurrentRealm, error: HandleValue) {
        self.subscriber.error(cx, error);
    }

    fn complete(&self, cx: &mut CurrentRealm) {
        self.subscriber.complete(cx);
    }
}

/// <https://wicg.github.io/observable/#dom-observable-drop>
pub(crate) fn drop_values(
    cx: &mut JSContext,
    source: &Observable,
    amount: u64,
) -> DomRoot<Observable> {
    new_operator(
        cx,
        source,
        DropSteps {
            source: Dom::from_ref(source),
            amount,
        },
    )
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct DropSteps {
    source: Dom<Observable>,
    amount: u64,
}

impl SubscribeSteps for DropSteps {
    fn subscribe(&self, cx: &mut CurrentRealm, subscriber: &Subscriber) {
        let observer = InternalObserver::new(DropObserver {
            subscriber: Dom::from_ref(subscriber),
            remaining: Cell::new(self.amount),
        });
        self.source
            .subscribe_internal(cx, observer, Some(&subscriber.subscription_signal()));
    }
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct DropObserver {
    subscriber: Dom<Subscriber>,
    remaining: Cell<u64>,
}

impl InternalObserverSteps for DropObserver {
    fn next(&self, cx: &mut CurrentRealm, value: HandleValue) {
        // Step 1. If remaining is greater than 0, then decrement remaining and return.
        let remaining = self.remaining.get();
        if remaining > 0 {
            self.remaining.set(remaining - 1);
            return;
        }
        // Step 2. Run subscriber's next() with value.
        self.subscriber.next(cx, value);
    }

    fn error(&self, cx: &mut CurrentRealm, error: HandleValue) {
        self.subscriber.error(cx, error);
    }

    fn complete(&self, cx: &mut CurrentRealm) {
        self.subscriber.complete(cx);
    }
}

/// <https://wicg.github.io/observable/#dom-observable-finally>
pub(crate) fn finally(
    cx: &mut JSContext,
    source: &Observable,
    callback: RootedCallback<VoidFunction>,
) -> DomRoot<Observable> {
    new_operator(
        cx,
        source,
        FinallySteps {
            source: Dom::from_ref(source),
            callback: callback.to_traced(),
        },
    )
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct FinallySteps {
    source: Dom<Observable>,
    callback: TracedCallback<VoidFunction>,
}

impl SubscribeSteps for FinallySteps {
    fn subscribe(&self, cx: &mut CurrentRealm, subscriber: &Subscriber) {
        // Step 1. Run subscriber's addTeardown() with callback.
        subscriber.add_teardown(cx, self.callback.root(cx));
        // Step 2. Subscribe to this given an internal observer that forwards to subscriber.
        subscribe_forwarding(cx, &self.source, subscriber);
    }
}

/// The state of the abort algorithm that `inspect()` adds to the signal of a subscriber.
#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
pub(crate) struct InspectAbortState {
    callback: TracedCallback<ObservableInspectorAbortHandler>,
    /// Set instead of removing the algorithm from the signal, as that is not possible while the
    /// signal runs its abort algorithms.
    cancelled: Cell<bool>,
}

impl InspectAbortState {
    pub(crate) fn run(&self, cx: &mut CurrentRealm, reason: HandleValue) {
        if self.cancelled.get() {
            return;
        }
        let callback = self.callback.root(cx);
        let _ = callback.Call__(cx, reason, ExceptionHandling::Report);
    }
}

/// <https://wicg.github.io/observable/#dom-observable-inspect>
pub(crate) fn inspect(
    cx: &mut JSContext,
    source: &Observable,
    inspector: ObservableInspectorUnion,
) -> DomRoot<Observable> {
    let steps = match inspector {
        ObservableInspectorUnion::ObservableSubscriptionCallback(next) => InspectSteps {
            source: Dom::from_ref(source),
            next: Some(next.to_traced()),
            error: None,
            complete: None,
            subscribe: None,
            abort: None,
        },
        ObservableInspectorUnion::ObservableInspector(inspector) => InspectSteps {
            source: Dom::from_ref(source),
            next: inspector.next.clone(),
            error: inspector.error.clone(),
            complete: inspector.complete.clone(),
            subscribe: inspector.subscribe.clone(),
            abort: inspector.abort.clone(),
        },
    };
    new_operator(cx, source, steps)
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct InspectSteps {
    source: Dom<Observable>,
    next: Option<TracedCallback<ObservableSubscriptionCallback>>,
    error: Option<TracedCallback<ObservableSubscriptionCallback>>,
    complete: Option<TracedCallback<VoidFunction>>,
    subscribe: Option<TracedCallback<VoidFunction>>,
    abort: Option<TracedCallback<ObservableInspectorAbortHandler>>,
}

impl SubscribeSteps for InspectSteps {
    fn subscribe(&self, cx: &mut CurrentRealm, subscriber: &Subscriber) {
        // Step 1. If inspector's subscribe callback exists, then invoke it with "rethrow".
        if let Some(subscribe) = &self.subscribe {
            let subscribe = subscribe.root(cx);
            if let Err(error) = subscribe.Call__(cx, ExceptionHandling::Rethrow) {
                // If an exception E was thrown, then run subscriber's error() with E and return.
                error_subscriber_with(cx, subscriber, error);
                return;
            }
        }

        // Step 2. If inspector's abort callback exists, then add an abort algorithm to
        // subscriber's signal that invokes it with the signal's abort reason.
        let abort_state = self.abort.as_ref().map(|callback| {
            Rc::new(InspectAbortState {
                callback: callback.clone(),
                cancelled: Cell::new(false),
            })
        });
        if let Some(state) = &abort_state {
            subscriber
                .subscription_signal()
                .add(&AbortAlgorithm::Observable(
                    ObservableAbortAlgorithm::InspectAbort(state.clone()),
                ));
        }

        // Step 3. Subscribe to this given an internal observer that runs the inspector's
        // callbacks before forwarding to subscriber.
        let observer = InternalObserver::new(InspectObserver {
            subscriber: Dom::from_ref(subscriber),
            next: self.next.clone(),
            error: self.error.clone(),
            complete: self.complete.clone(),
            abort_state,
        });
        self.source
            .subscribe_internal(cx, observer, Some(&subscriber.subscription_signal()));
    }
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct InspectObserver {
    subscriber: Dom<Subscriber>,
    next: Option<TracedCallback<ObservableSubscriptionCallback>>,
    error: Option<TracedCallback<ObservableSubscriptionCallback>>,
    complete: Option<TracedCallback<VoidFunction>>,
    #[conditional_malloc_size_of]
    abort_state: Option<Rc<InspectAbortState>>,
}

impl InspectObserver {
    /// Removes the abort callback from the signal.
    fn cancel_abort_callback(&self) {
        if let Some(state) = &self.abort_state {
            state.cancelled.set(true);
        }
    }
}

impl InternalObserverSteps for InspectObserver {
    fn next(&self, cx: &mut CurrentRealm, value: HandleValue) {
        // Step 1. If inspector's next callback exists, then invoke it with value and "rethrow".
        if let Some(next) = &self.next {
            let next = next.root(cx);
            if let Err(error) = next.Call__(cx, value, ExceptionHandling::Rethrow) {
                // If an exception E was thrown, then remove the abort callback, run subscriber's
                // error() with E, and return.
                self.cancel_abort_callback();
                error_subscriber_with(cx, &self.subscriber, error);
                return;
            }
        }
        // Step 2. Run subscriber's next() with value.
        self.subscriber.next(cx, value);
    }

    fn error(&self, cx: &mut CurrentRealm, error: HandleValue) {
        // Step 1. Remove the abort callback.
        self.cancel_abort_callback();
        // Step 2. If inspector's error callback exists, then invoke it with error and "rethrow".
        if let Some(callback) = &self.error {
            let callback = callback.root(cx);
            if let Err(thrown) = callback.Call__(cx, error, ExceptionHandling::Rethrow) {
                error_subscriber_with(cx, &self.subscriber, thrown);
                return;
            }
        }
        // Step 3. Run subscriber's error() with error.
        self.subscriber.error(cx, error);
    }

    fn complete(&self, cx: &mut CurrentRealm) {
        // Step 1. Remove the abort callback.
        self.cancel_abort_callback();
        // Step 2. If inspector's complete callback exists, then invoke it with "rethrow".
        if let Some(callback) = &self.complete {
            let callback = callback.root(cx);
            if let Err(thrown) = callback.Call__(cx, ExceptionHandling::Rethrow) {
                error_subscriber_with(cx, &self.subscriber, thrown);
                return;
            }
        }
        // Step 3. Run subscriber's complete().
        self.subscriber.complete(cx);
    }
}
