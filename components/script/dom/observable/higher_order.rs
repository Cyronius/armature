/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The operators of <https://wicg.github.io/observable/#observable-api> that convert values to
//! Observables, to subscribe to those: `takeUntil()`, `catch()`, `flatMap()` and `switchMap()`.
//!
//! In these, the spec's "calling from()" is [`convert_to_observable`]. The exceptions of it are
//! sent to the subscriber of the operator, instead of being thrown.

use std::cell::Cell;
use std::collections::VecDeque;
use std::rc::Rc;

use js::context::JSContext;
use js::jsapi::Heap;
use js::jsval::{JSVal, UndefinedValue};
use js::realm::CurrentRealm;
use js::rust::HandleValue;
use script_bindings::callback::{ExceptionHandling, RootedCallback, TracedCallback};
use script_bindings::cell::DomRefCell;

use crate::dom::abortsignal::AbortSignal;
use crate::dom::bindings::codegen::Bindings::ObservableBinding::{CatchCallback, Mapper};
use crate::dom::bindings::error::Fallible;
use crate::dom::bindings::reflector::DomGlobal;
use crate::dom::bindings::root::{Dom, DomRoot, MutNullableDom};
use crate::dom::observable::from::convert_to_observable;
use crate::dom::observable::observable::{Observable, SubscribeSteps};
use crate::dom::observable::operators::subscribe_forwarding;
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

/// <https://wicg.github.io/observable/#dom-observable-takeuntil>
pub(crate) fn take_until(
    cx: &mut JSContext,
    source: &Observable,
    value: HandleValue,
) -> Fallible<DomRoot<Observable>> {
    // Step 2. Let notifier be the result of converting value to an Observable.
    let notifier = convert_to_observable(cx, &source.global(), value)?;
    Ok(new_operator(
        cx,
        source,
        TakeUntilSteps {
            source: Dom::from_ref(source),
            notifier: Dom::from_ref(&notifier),
        },
    ))
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct TakeUntilSteps {
    source: Dom<Observable>,
    notifier: Dom<Observable>,
}

impl SubscribeSteps for TakeUntilSteps {
    fn subscribe(&self, cx: &mut CurrentRealm, subscriber: &Subscriber) {
        // Step 3.2. Subscribe to notifier given notifierObserver and options.
        let notifier_observer = InternalObserver::new(NotifierObserver {
            subscriber: Dom::from_ref(subscriber),
        });
        self.notifier.subscribe_internal(
            cx,
            notifier_observer,
            Some(&subscriber.subscription_signal()),
        );

        // Step 3.3. If subscriber's active is false, then return.
        // Note: The source is never subscribed to when the notifier emitted synchronously.
        if !subscriber.is_active() {
            return;
        }

        // Step 3.5. Subscribe to sourceObservable given sourceObserver and options.
        subscribe_forwarding(cx, &self.source, subscriber);
    }
}

/// The observer of the notifier of `takeUntil()`.
#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct NotifierObserver {
    subscriber: Dom<Subscriber>,
}

impl InternalObserverSteps for NotifierObserver {
    fn next(&self, cx: &mut CurrentRealm, _value: HandleValue) {
        // Run subscriber's complete() method.
        self.subscriber.complete(cx);
    }

    fn error(&self, cx: &mut CurrentRealm, _error: HandleValue) {
        // Run subscriber's complete() method.
        self.subscriber.complete(cx);
    }

    // Note: There are no complete steps, as the source is mirrored uninterrupted when the
    // notifier completes by itself.
}

/// <https://wicg.github.io/observable/#dom-observable-catch>
pub(crate) fn catch(
    cx: &mut JSContext,
    source: &Observable,
    callback: RootedCallback<CatchCallback>,
) -> DomRoot<Observable> {
    new_operator(
        cx,
        source,
        CatchSteps {
            source: Dom::from_ref(source),
            callback: callback.to_traced(),
        },
    )
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct CatchSteps {
    source: Dom<Observable>,
    callback: TracedCallback<CatchCallback>,
}

impl SubscribeSteps for CatchSteps {
    fn subscribe(&self, cx: &mut CurrentRealm, subscriber: &Subscriber) {
        let observer = InternalObserver::new(CatchObserver {
            subscriber: Dom::from_ref(subscriber),
            callback: self.callback.clone(),
        });
        self.source
            .subscribe_internal(cx, observer, Some(&subscriber.subscription_signal()));
    }
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct CatchObserver {
    subscriber: Dom<Subscriber>,
    callback: TracedCallback<CatchCallback>,
}

impl InternalObserverSteps for CatchObserver {
    fn next(&self, cx: &mut CurrentRealm, value: HandleValue) {
        self.subscriber.next(cx, value);
    }

    fn error(&self, cx: &mut CurrentRealm, error: HandleValue) {
        // Step 1. Invoke callback with «the passed in error» and "rethrow". Let result be the
        // returned value.
        let callback = self.callback.root(cx);
        rooted!(&in(cx) let mut result = UndefinedValue());
        if let Err(thrown) =
            callback.Call__(cx, error, result.handle_mut(), ExceptionHandling::Rethrow)
        {
            // Step 2. If an exception E was thrown, then run subscriber's error() with E, and
            // abort these steps.
            error_subscriber_with(cx, &self.subscriber, thrown);
            return;
        }

        // Step 3. Let innerObservable be the result of calling from() with result.
        let global = self.subscriber.global();
        let inner = match convert_to_observable(cx, &global, result.handle()) {
            Ok(inner) => inner,
            Err(thrown) => {
                // Step 4. If an exception E was thrown, then run subscriber's error() method,
                // given E, and abort these steps.
                error_subscriber_with(cx, &self.subscriber, thrown);
                return;
            },
        };

        // Step 6. Subscribe to innerObservable given innerObserver and innerOptions.
        // Note: The source is already done at this point, so there is nothing to unsubscribe from.
        subscribe_forwarding(cx, &inner, &self.subscriber);
    }

    fn complete(&self, cx: &mut CurrentRealm) {
        self.subscriber.complete(cx);
    }
}

/// <https://wicg.github.io/observable/#dom-observable-flatmap>
pub(crate) fn flat_map(
    cx: &mut JSContext,
    source: &Observable,
    mapper: RootedCallback<Mapper>,
) -> DomRoot<Observable> {
    new_operator(
        cx,
        source,
        FlatMapSteps {
            source: Dom::from_ref(source),
            mapper: mapper.to_traced(),
        },
    )
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct FlatMapSteps {
    source: Dom<Observable>,
    mapper: TracedCallback<Mapper>,
}

impl SubscribeSteps for FlatMapSteps {
    fn subscribe(&self, cx: &mut CurrentRealm, subscriber: &Subscriber) {
        let state = Rc::new(FlatMapState {
            subscriber: Dom::from_ref(subscriber),
            mapper: self.mapper.clone(),
            idx: Cell::new(0),
            outer_completed: Cell::new(false),
            active_inner: Cell::new(false),
            queue: Default::default(),
        });
        let observer = InternalObserver::new(FlatMapSourceObserver { state });
        self.source
            .subscribe_internal(cx, observer, Some(&subscriber.subscription_signal()));
    }
}

/// The variables that the steps of `flatMap()` share, which are references in the spec.
#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct FlatMapState {
    subscriber: Dom<Subscriber>,
    mapper: TracedCallback<Mapper>,
    idx: Cell<u64>,
    /// outerSubscriptionHasCompleted
    outer_completed: Cell<bool>,
    /// activeInnerSubscription
    active_inner: Cell<bool>,
    /// The values from the source that are waiting for the active inner subscription to end.
    #[ignore_malloc_size_of = "mozjs"]
    queue: DomRefCell<VecDeque<Box<Heap<JSVal>>>>,
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct FlatMapSourceObserver {
    #[conditional_malloc_size_of]
    state: Rc<FlatMapState>,
}

impl InternalObserverSteps for FlatMapSourceObserver {
    fn next(&self, cx: &mut CurrentRealm, value: HandleValue) {
        let state = &self.state;
        if state.active_inner.get() {
            // If activeInnerSubscription is true, then append value to queue.
            state.queue.borrow_mut().push_back(Heap::boxed(value.get()));
        } else {
            // Otherwise, set activeInnerSubscription to true, and run the flatmap process next
            // value steps.
            state.active_inner.set(true);
            flat_map_process_next_value(cx, state, value);
        }
    }

    fn error(&self, cx: &mut CurrentRealm, error: HandleValue) {
        self.state.subscriber.error(cx, error);
    }

    fn complete(&self, cx: &mut CurrentRealm) {
        // Set outerSubscriptionHasCompleted to true. If activeInnerSubscription is false and
        // queue is empty, run subscriber's complete() method.
        self.state.outer_completed.set(true);
        if !self.state.active_inner.get() && self.state.queue.borrow().is_empty() {
            self.state.subscriber.complete(cx);
        }
    }
}

/// The flatmap process next value steps.
fn flat_map_process_next_value(
    cx: &mut CurrentRealm,
    state: &Rc<FlatMapState>,
    value: HandleValue,
) {
    // Step 1. Let mappedResult be the result of invoking mapper with «value, idx» and "rethrow".
    let mapper = state.mapper.root(cx);
    let idx = state.idx.get();
    rooted!(&in(cx) let mut mapped_result = UndefinedValue());
    if let Err(error) = mapper.Call__(
        cx,
        value,
        idx,
        mapped_result.handle_mut(),
        ExceptionHandling::Rethrow,
    ) {
        // Step 2. If an exception E was thrown, then run subscriber's error() method, given E,
        // and abort these steps.
        error_subscriber_with(cx, &state.subscriber, error);
        return;
    }

    // Step 3. Set idx to idx + 1.
    state.idx.set(idx + 1);

    // Step 4. Let innerObservable be the result of calling from() with mappedResult.
    let global = state.subscriber.global();
    let inner = match convert_to_observable(cx, &global, mapped_result.handle()) {
        Ok(inner) => inner,
        Err(error) => {
            // Step 5. If an exception E was thrown, then run subscriber's error() method, given
            // E, and abort these steps.
            error_subscriber_with(cx, &state.subscriber, error);
            return;
        },
    };

    // Step 7. Subscribe to innerObservable given innerObserver and innerOptions.
    let observer = InternalObserver::new(FlatMapInnerObserver {
        state: state.clone(),
    });
    inner.subscribe_internal(cx, observer, Some(&state.subscriber.subscription_signal()));
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct FlatMapInnerObserver {
    #[conditional_malloc_size_of]
    state: Rc<FlatMapState>,
}

impl InternalObserverSteps for FlatMapInnerObserver {
    fn next(&self, cx: &mut CurrentRealm, value: HandleValue) {
        self.state.subscriber.next(cx, value);
    }

    fn error(&self, cx: &mut CurrentRealm, error: HandleValue) {
        self.state.subscriber.error(cx, error);
    }

    fn complete(&self, cx: &mut CurrentRealm) {
        let state = &self.state;
        let next_value = state.queue.borrow_mut().pop_front();
        match next_value {
            // If queue is not empty, then let nextValue be the first item in queue, remove it,
            // and run the flatmap process next value steps with it.
            Some(next_value) => {
                rooted!(&in(cx) let next_value = next_value.get());
                flat_map_process_next_value(cx, state, next_value.handle());
            },
            // Otherwise, set activeInnerSubscription to false, and if
            // outerSubscriptionHasCompleted is true, run subscriber's complete() method.
            None => {
                state.active_inner.set(false);
                if state.outer_completed.get() {
                    state.subscriber.complete(cx);
                }
            },
        }
    }
}

/// <https://wicg.github.io/observable/#dom-observable-switchmap>
pub(crate) fn switch_map(
    cx: &mut JSContext,
    source: &Observable,
    mapper: RootedCallback<Mapper>,
) -> DomRoot<Observable> {
    new_operator(
        cx,
        source,
        SwitchMapSteps {
            source: Dom::from_ref(source),
            mapper: mapper.to_traced(),
        },
    )
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct SwitchMapSteps {
    source: Dom<Observable>,
    mapper: TracedCallback<Mapper>,
}

impl SubscribeSteps for SwitchMapSteps {
    fn subscribe(&self, cx: &mut CurrentRealm, subscriber: &Subscriber) {
        let state = Rc::new(SwitchMapState {
            subscriber: Dom::from_ref(subscriber),
            mapper: self.mapper.clone(),
            idx: Cell::new(0),
            outer_completed: Cell::new(false),
            active_inner: Default::default(),
        });
        let observer = InternalObserver::new(SwitchMapSourceObserver { state });
        self.source
            .subscribe_internal(cx, observer, Some(&subscriber.subscription_signal()));
    }
}

/// The variables that the steps of `switchMap()` share, which are references in the spec.
#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct SwitchMapState {
    subscriber: Dom<Subscriber>,
    mapper: TracedCallback<Mapper>,
    idx: Cell<u64>,
    /// outerSubscriptionHasCompleted
    outer_completed: Cell<bool>,
    /// The signal of activeInnerAbortController, which is also the marker of whether there is an
    /// active inner subscription.
    active_inner: MutNullableDom<AbortSignal>,
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct SwitchMapSourceObserver {
    #[conditional_malloc_size_of]
    state: Rc<SwitchMapState>,
}

impl InternalObserverSteps for SwitchMapSourceObserver {
    fn next(&self, cx: &mut CurrentRealm, value: HandleValue) {
        let state = &self.state;

        // Step 1. If activeInnerAbortController is not null, then signal abort
        // activeInnerAbortController.
        if let Some(active_inner) = state.active_inner.get() {
            active_inner.signal_abort(cx, HandleValue::undefined());
        }

        // Step 2. Set activeInnerAbortController to a new AbortController.
        let controller = AbortSignal::new_with_proto(cx, &state.subscriber.global(), None);
        state.active_inner.set(Some(&controller));

        // Step 3. Run the switchmap process next value steps.
        switch_map_process_next_value(cx, state, &controller, value);
    }

    fn error(&self, cx: &mut CurrentRealm, error: HandleValue) {
        self.state.subscriber.error(cx, error);
    }

    fn complete(&self, cx: &mut CurrentRealm) {
        // Set outerSubscriptionHasCompleted to true. If activeInnerAbortController is null, run
        // subscriber's complete() method.
        self.state.outer_completed.set(true);
        if self.state.active_inner.get().is_none() {
            self.state.subscriber.complete(cx);
        }
    }
}

/// The switchmap process next value steps.
fn switch_map_process_next_value(
    cx: &mut CurrentRealm,
    state: &Rc<SwitchMapState>,
    controller: &AbortSignal,
    value: HandleValue,
) {
    // Step 1. Let mappedResult be the result of invoking mapper with «value, idx» and "rethrow".
    let mapper = state.mapper.root(cx);
    let idx = state.idx.get();
    rooted!(&in(cx) let mut mapped_result = UndefinedValue());
    if let Err(error) = mapper.Call__(
        cx,
        value,
        idx,
        mapped_result.handle_mut(),
        ExceptionHandling::Rethrow,
    ) {
        // Step 2. If an exception E was thrown, then run subscriber's error() method, given E,
        // and abort these steps.
        error_subscriber_with(cx, &state.subscriber, error);
        return;
    }

    // Step 3. Set idx to idx + 1.
    state.idx.set(idx + 1);

    // Step 4. Let innerObservable be the result of calling from() with mappedResult.
    let global = state.subscriber.global();
    let inner = match convert_to_observable(cx, &global, mapped_result.handle()) {
        Ok(inner) => inner,
        Err(error) => {
            // Step 5. If an exception E was thrown, then run subscriber's error() method, given
            // E, and abort these steps.
            error_subscriber_with(cx, &state.subscriber, error);
            return;
        },
    };

    // Step 7. Let innerOptions be a new SubscribeOptions whose signal is the result of creating
    // a dependent abort signal from the list «activeInnerAbortController's signal, subscriber's
    // subscription controller's signal».
    let inner_signal = AbortSignal::create_dependent_abort_signal(
        cx,
        vec![
            DomRoot::from_ref(controller),
            state.subscriber.subscription_signal(),
        ],
        &global,
    );

    // Step 8. Subscribe to innerObservable given innerObserver and innerOptions.
    let observer = InternalObserver::new(SwitchMapInnerObserver {
        state: state.clone(),
    });
    inner.subscribe_internal(cx, observer, Some(&inner_signal));
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct SwitchMapInnerObserver {
    #[conditional_malloc_size_of]
    state: Rc<SwitchMapState>,
}

impl InternalObserverSteps for SwitchMapInnerObserver {
    fn next(&self, cx: &mut CurrentRealm, value: HandleValue) {
        self.state.subscriber.next(cx, value);
    }

    fn error(&self, cx: &mut CurrentRealm, error: HandleValue) {
        // Note: There is no need to mark the inner subscription as inactive, as erroring the
        // subscriber also ends the subscription to the source.
        self.state.subscriber.error(cx, error);
    }

    fn complete(&self, cx: &mut CurrentRealm) {
        // If outerSubscriptionHasCompleted is true, run subscriber's complete() method.
        // Otherwise, set activeInnerAbortController to null.
        if self.state.outer_completed.get() {
            self.state.subscriber.complete(cx);
        } else {
            self.state.active_inner.set(None);
        }
    }
}
