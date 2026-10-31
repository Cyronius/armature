/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! <https://wicg.github.io/observable/#promise-returning-operators>

use std::cell::Cell;

use js::jsapi::{Heap, JSPROP_ENUMERATE};
use js::jsval::{JSVal, ObjectValue, UndefinedValue};
use js::realm::CurrentRealm;
use js::rust::HandleValue;
use js::rust::wrappers2::{JS_DefineElement, NewArrayObject1};
use script_bindings::callback::{ExceptionHandling, RootedCallback, TracedCallback};
use script_bindings::cell::DomRefCell;

use crate::dom::abortsignal::{AbortAlgorithm, AbortSignal};
use crate::dom::bindings::codegen::Bindings::AbortSignalBinding::AbortSignalMethods;
use crate::dom::bindings::codegen::Bindings::ObservableBinding::{
    Predicate, Reducer, SubscribeOptions, Visitor,
};
use crate::dom::bindings::error::{Error, ErrorToJsval};
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::globalscope::GlobalScope;
use crate::dom::observable::observable::{Observable, ObservableAbortAlgorithm};
use crate::dom::observable::subscriber::{InternalObserver, InternalObserverSteps};
use crate::dom::promise::{Promise, RootedPromise, TracedPromise};

/// What the steps shared by the promise-returning operators produced.
struct Setup {
    promise: RootedPromise,
    /// The signal of the operator's "controller", for operators that have one.
    controller: Option<DomRoot<AbortSignal>>,
    /// The signal that the operator subscribes with.
    signal: Option<DomRoot<AbortSignal>>,
    /// Whether the signal was aborted already, and `promise` rejected as a result.
    aborted: bool,
}

/// The steps that all promise-returning operators start with.
///
/// If `use_controller` is true, the operator subscribes with a "dependent abort signal" of a new
/// controller and `options["signal"]`. Otherwise, it subscribes with `options["signal"]` itself.
fn setup(cx: &mut CurrentRealm, options: &SubscribeOptions, use_controller: bool) -> Setup {
    let global = GlobalScope::from_current_realm(cx);

    // Step 1. Let p a new promise.
    let promise = Promise::new_in_realm(cx);

    let (controller, signal) = if use_controller {
        // Step 2. Let controller be a new AbortController.
        let controller = AbortSignal::new_with_proto(cx, &global, None);
        // Step 3. Let internal options be a new SubscribeOptions whose signal is the result of
        // creating a dependent abort signal from the list «controller's signal, options's signal
        // if non-null».
        let mut signals = vec![controller.clone()];
        signals.extend(options.signal.clone());
        let signal = AbortSignal::create_dependent_abort_signal(cx, signals, &global);
        (Some(controller), Some(signal))
    } else {
        (None, options.signal.clone())
    };

    let mut setup = Setup {
        promise,
        controller,
        signal,
        aborted: false,
    };
    if let Some(signal) = &setup.signal {
        if signal.aborted() {
            // If the signal is aborted, then reject p with its abort reason and return p.
            rooted!(&in(cx) let mut reason = UndefinedValue());
            signal.Reason(reason.handle_mut());
            setup.promise.reject(cx, reason.handle());
            setup.aborted = true;
        } else {
            // Add an abort algorithm to the signal that rejects p with its abort reason.
            signal.add(&AbortAlgorithm::Observable(
                ObservableAbortAlgorithm::RejectPromise(setup.promise.to_traced()),
            ));
        }
    }
    setup
}

/// Subscribes to `source` with `observer` and the signal of `setup`, and returns the promise.
fn subscribe_and_return(
    cx: &mut CurrentRealm,
    source: &Observable,
    setup: Setup,
    observer: impl InternalObserverSteps + 'static,
) -> RootedPromise {
    if !setup.aborted {
        source.subscribe_internal(cx, InternalObserver::new(observer), setup.signal.as_deref());
    }
    setup.promise
}

/// Rejects `promise` with `error`, which is an exception that was thrown by a callback, and
/// signals abort on `controller` with it.
fn reject_and_abort_controller(
    cx: &mut CurrentRealm,
    promise: &Promise,
    controller: &AbortSignal,
    error: Error,
) {
    let global = GlobalScope::from_current_realm(cx);
    rooted!(&in(cx) let mut exception = UndefinedValue());
    error.to_jsval(cx, &global, exception.handle_mut());
    promise.reject(cx, exception.handle());
    controller.signal_abort(cx, exception.handle());
}

/// <https://wicg.github.io/observable/#dom-observable-toarray>
#[cfg_attr(crown, expect(crown::unrooted_must_root))]
pub(crate) fn to_array(
    cx: &mut CurrentRealm,
    source: &Observable,
    options: &SubscribeOptions,
) -> RootedPromise {
    let setup = setup(cx, options, false);
    let observer = ToArrayObserver {
        promise: setup.promise.to_traced(),
        values: Default::default(),
    };
    subscribe_and_return(cx, source, setup, observer)
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct ToArrayObserver {
    promise: TracedPromise,
    #[ignore_malloc_size_of = "mozjs"]
    values: DomRefCell<Vec<Box<Heap<JSVal>>>>,
}

impl InternalObserverSteps for ToArrayObserver {
    fn next(&self, _cx: &mut CurrentRealm, value: HandleValue) {
        // Append the passed in value to values.
        self.values.borrow_mut().push(Heap::boxed(value.get()));
    }

    fn error(&self, cx: &mut CurrentRealm, error: HandleValue) {
        // Reject p with the passed in error.
        self.promise.reject(cx, error);
    }

    #[expect(unsafe_code)]
    fn complete(&self, cx: &mut CurrentRealm) {
        // Resolve p with values.
        let length = self.values.borrow().len();
        rooted!(&in(cx) let array = unsafe { NewArrayObject1(cx, length) });
        assert!(!array.get().is_null());
        for index in 0..length {
            rooted!(&in(cx) let value = self.values.borrow()[index].get());
            let defined = unsafe {
                JS_DefineElement(
                    cx,
                    array.handle(),
                    index as u32,
                    value.handle(),
                    JSPROP_ENUMERATE as u32,
                )
            };
            assert!(defined);
        }
        rooted!(&in(cx) let array_value = ObjectValue(array.get()));
        self.promise.resolve(cx, array_value.handle());
    }
}

/// <https://wicg.github.io/observable/#dom-observable-foreach>
#[cfg_attr(crown, expect(crown::unrooted_must_root))]
pub(crate) fn for_each(
    cx: &mut CurrentRealm,
    source: &Observable,
    callback: RootedCallback<Visitor>,
    options: &SubscribeOptions,
) -> RootedPromise {
    let setup = setup(cx, options, true);
    let observer = ForEachObserver {
        promise: setup.promise.to_traced(),
        controller: Dom::from_ref(setup.controller.as_deref().expect("Has a controller")),
        callback: callback.to_traced(),
        idx: Cell::new(0),
    };
    subscribe_and_return(cx, source, setup, observer)
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct ForEachObserver {
    promise: TracedPromise,
    controller: Dom<AbortSignal>,
    callback: TracedCallback<Visitor>,
    idx: Cell<u64>,
}

impl InternalObserverSteps for ForEachObserver {
    fn next(&self, cx: &mut CurrentRealm, value: HandleValue) {
        // Step 1. Invoke callback with «the passed in value, idx» and "rethrow".
        let callback = self.callback.root(cx);
        let idx = self.idx.get();
        if let Err(error) = callback.Call__(cx, value, idx, ExceptionHandling::Rethrow) {
            // Step 2. If an exception E was thrown, then reject p with E, and signal abort
            // visitor callback controller with E.
            reject_and_abort_controller(cx, &self.promise, &self.controller, error);
            return;
        }
        // Step 3. Increment idx.
        self.idx.set(idx + 1);
    }

    fn error(&self, cx: &mut CurrentRealm, error: HandleValue) {
        self.promise.reject(cx, error);
    }

    fn complete(&self, cx: &mut CurrentRealm) {
        // Resolve p with undefined.
        self.promise.resolve_native(cx, &());
    }
}

#[derive(Clone, Copy, JSTraceable, MallocSizeOf)]
pub(crate) enum PredicateOperator {
    /// <https://wicg.github.io/observable/#dom-observable-every>
    Every,
    /// <https://wicg.github.io/observable/#dom-observable-find>
    Find,
    /// <https://wicg.github.io/observable/#dom-observable-some>
    Some,
}

/// `every()`, `find()` and `some()`, that differ only in when and with what they resolve.
#[cfg_attr(crown, expect(crown::unrooted_must_root))]
pub(crate) fn with_predicate(
    cx: &mut CurrentRealm,
    source: &Observable,
    operator: PredicateOperator,
    predicate: RootedCallback<Predicate>,
    options: &SubscribeOptions,
) -> RootedPromise {
    let setup = setup(cx, options, true);
    let observer = PredicateObserver {
        operator,
        promise: setup.promise.to_traced(),
        controller: Dom::from_ref(setup.controller.as_deref().expect("Has a controller")),
        predicate: predicate.to_traced(),
        idx: Cell::new(0),
    };
    subscribe_and_return(cx, source, setup, observer)
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct PredicateObserver {
    operator: PredicateOperator,
    promise: TracedPromise,
    controller: Dom<AbortSignal>,
    predicate: TracedCallback<Predicate>,
    idx: Cell<u64>,
}

impl InternalObserverSteps for PredicateObserver {
    fn next(&self, cx: &mut CurrentRealm, value: HandleValue) {
        // Step 1. Invoke predicate with «the passed in value, idx» and "rethrow"; let passed be
        // the returned value.
        let predicate = self.predicate.root(cx);
        let idx = self.idx.get();
        let passed = match predicate.Call__(cx, value, idx, ExceptionHandling::Rethrow) {
            Ok(passed) => passed,
            Err(error) => {
                // Step 2. If an exception E was thrown, then reject p with E, and signal abort
                // controller with E.
                reject_and_abort_controller(cx, &self.promise, &self.controller, error);
                return;
            },
        };
        // Step 3. Set idx to idx + 1.
        self.idx.set(idx + 1);

        // Step 4. Resolve p and signal abort controller, depending on the operator.
        let settled = match self.operator {
            PredicateOperator::Every if !passed => {
                self.promise.resolve_native(cx, &false);
                true
            },
            PredicateOperator::Some if passed => {
                self.promise.resolve_native(cx, &true);
                true
            },
            PredicateOperator::Find if passed => {
                self.promise.resolve(cx, value);
                true
            },
            _ => false,
        };
        if settled {
            self.controller.signal_abort(cx, HandleValue::undefined());
        }
    }

    fn error(&self, cx: &mut CurrentRealm, error: HandleValue) {
        self.promise.reject(cx, error);
    }

    fn complete(&self, cx: &mut CurrentRealm) {
        match self.operator {
            PredicateOperator::Every => self.promise.resolve_native(cx, &true),
            PredicateOperator::Some => self.promise.resolve_native(cx, &false),
            PredicateOperator::Find => self.promise.resolve_native(cx, &()),
        }
    }
}

/// <https://wicg.github.io/observable/#dom-observable-first>
#[cfg_attr(crown, expect(crown::unrooted_must_root))]
pub(crate) fn first(
    cx: &mut CurrentRealm,
    source: &Observable,
    options: &SubscribeOptions,
) -> RootedPromise {
    let setup = setup(cx, options, true);
    let observer = FirstObserver {
        promise: setup.promise.to_traced(),
        controller: Dom::from_ref(setup.controller.as_deref().expect("Has a controller")),
    };
    subscribe_and_return(cx, source, setup, observer)
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct FirstObserver {
    promise: TracedPromise,
    controller: Dom<AbortSignal>,
}

impl InternalObserverSteps for FirstObserver {
    fn next(&self, cx: &mut CurrentRealm, value: HandleValue) {
        // Resolve p with the passed in value.
        self.promise.resolve(cx, value);
        // Signal abort controller.
        self.controller.signal_abort(cx, HandleValue::undefined());
    }

    fn error(&self, cx: &mut CurrentRealm, error: HandleValue) {
        self.promise.reject(cx, error);
    }

    fn complete(&self, cx: &mut CurrentRealm) {
        // Reject p with a new RangeError.
        // Note: This is only reached when the source completes before it emits a single value.
        self.promise
            .reject_error(cx, Error::Range(c"No values in Observable".to_owned()));
    }
}

/// <https://wicg.github.io/observable/#dom-observable-last>
#[cfg_attr(crown, expect(crown::unrooted_must_root))]
pub(crate) fn last(
    cx: &mut CurrentRealm,
    source: &Observable,
    options: &SubscribeOptions,
) -> RootedPromise {
    let setup = setup(cx, options, false);
    let observer = LastObserver {
        promise: setup.promise.to_traced(),
        last_value: Default::default(),
    };
    subscribe_and_return(cx, source, setup, observer)
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct LastObserver {
    promise: TracedPromise,
    /// `None` until the first value arrives.
    #[ignore_malloc_size_of = "mozjs"]
    last_value: DomRefCell<Option<Box<Heap<JSVal>>>>,
}

impl InternalObserverSteps for LastObserver {
    fn next(&self, _cx: &mut CurrentRealm, value: HandleValue) {
        // Set hasLastValue to true, and lastValue to the passed in value.
        *self.last_value.borrow_mut() = Some(Heap::boxed(value.get()));
    }

    fn error(&self, cx: &mut CurrentRealm, error: HandleValue) {
        self.promise.reject(cx, error);
    }

    fn complete(&self, cx: &mut CurrentRealm) {
        // If hasLastValue is true, resolve p with lastValue. Otherwise, reject p with a new
        // RangeError.
        let last_value = self.last_value.borrow().as_ref().map(|value| value.get());
        match last_value {
            Some(value) => {
                rooted!(&in(cx) let value = value);
                self.promise.resolve(cx, value.handle());
            },
            None => self
                .promise
                .reject_error(cx, Error::Range(c"No values in Observable".to_owned())),
        }
    }
}

/// <https://wicg.github.io/observable/#dom-observable-reduce>
#[cfg_attr(crown, expect(crown::unrooted_must_root))]
pub(crate) fn reduce(
    cx: &mut CurrentRealm,
    source: &Observable,
    reducer: RootedCallback<Reducer>,
    initial_value: HandleValue,
    options: &SubscribeOptions,
) -> RootedPromise {
    let setup = setup(cx, options, true);
    // Note: An `initialValue` of `undefined` is not told apart from one that is not given.
    let accumulator = (!initial_value.is_undefined()).then(|| Heap::boxed(initial_value.get()));
    let observer = ReduceObserver {
        promise: setup.promise.to_traced(),
        controller: Dom::from_ref(setup.controller.as_deref().expect("Has a controller")),
        reducer: reducer.to_traced(),
        idx: Cell::new(0),
        accumulator: DomRefCell::new(accumulator),
    };
    subscribe_and_return(cx, source, setup, observer)
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct ReduceObserver {
    promise: TracedPromise,
    controller: Dom<AbortSignal>,
    reducer: TracedCallback<Reducer>,
    idx: Cell<u64>,
    /// `None` while uninitialized.
    #[ignore_malloc_size_of = "mozjs"]
    accumulator: DomRefCell<Option<Box<Heap<JSVal>>>>,
}

impl InternalObserverSteps for ReduceObserver {
    fn next(&self, cx: &mut CurrentRealm, value: HandleValue) {
        let idx = self.idx.get();

        // Step 1. If accumulator is uninitialized, then set accumulator to the passed in value,
        // set idx to idx + 1, and abort these steps.
        let accumulator = self.accumulator.borrow().as_ref().map(|value| value.get());
        let Some(accumulator) = accumulator else {
            *self.accumulator.borrow_mut() = Some(Heap::boxed(value.get()));
            self.idx.set(idx + 1);
            return;
        };

        // Step 2. Invoke reducer with «accumulator as accumulator, the passed in value as
        // currentValue, idx as index» and "rethrow". Let result be the returned value.
        let reducer = self.reducer.root(cx);
        rooted!(&in(cx) let accumulator = accumulator);
        rooted!(&in(cx) let mut result = UndefinedValue());
        if let Err(error) = reducer.Call__(
            cx,
            accumulator.handle(),
            value,
            idx,
            result.handle_mut(),
            ExceptionHandling::Rethrow,
        ) {
            // Step 3. If an exception E was thrown, then reject p with E, and signal abort
            // controller with E.
            reject_and_abort_controller(cx, &self.promise, &self.controller, error);
            return;
        }

        // Step 4. Set idx to idx + 1.
        self.idx.set(idx + 1);
        // Step 5. Set accumulator to result.
        *self.accumulator.borrow_mut() = Some(Heap::boxed(result.get()));
    }

    fn error(&self, cx: &mut CurrentRealm, error: HandleValue) {
        self.promise.reject(cx, error);
    }

    fn complete(&self, cx: &mut CurrentRealm) {
        // If accumulator is not "unset", then resolve p with accumulator. Otherwise, reject p
        // with a TypeError.
        let accumulator = self.accumulator.borrow().as_ref().map(|value| value.get());
        match accumulator {
            Some(accumulator) => {
                rooted!(&in(cx) let accumulator = accumulator);
                self.promise.resolve(cx, accumulator.handle());
            },
            None => self.promise.reject_error(
                cx,
                Error::Type(
                    c"Reduce of an Observable with no values and no initial value".to_owned(),
                ),
            ),
        }
    }
}
