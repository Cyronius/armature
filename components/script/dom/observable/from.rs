/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! <https://wicg.github.io/observable/#observable-convert-to-an-observable>

use std::rc::Rc;

use js::context::JSContext;
use js::jsapi::{Heap, SymbolCode};
use js::jsval::{JSVal, ObjectValue, UndefinedValue};
use js::realm::CurrentRealm;
use js::rust::wrappers2::IsPromiseObject;
use js::rust::{HandleValue, MutableHandleValue};

use crate::dom::abortsignal::AbortAlgorithm;
use crate::dom::bindings::conversions::root_from_handlevalue;
use crate::dom::bindings::error::{Error, ErrorToJsval, Fallible};
use crate::dom::bindings::reflector::DomGlobal;
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::globalscope::GlobalScope;
use crate::dom::observable::iteration::{
    IteratorKind, IteratorRecord, async_iterator_call_return, get_iterator, get_method_of_symbol,
    iterator_close, iterator_complete, iterator_next, iterator_step_value, iterator_value,
};
use crate::dom::observable::observable::{Observable, ObservableAbortAlgorithm, SubscribeSteps};
use crate::dom::observable::subscriber::{Subscriber, error_subscriber_with};
use crate::dom::promise::Promise;
use crate::dom::promisenativehandler::{Callback, PromiseNativeHandler};

/// To [convert to an Observable] an any value.
///
/// [convert to an Observable]: https://wicg.github.io/observable/#observable-convert-to-an-observable
#[cfg_attr(crown, expect(crown::unrooted_must_root))]
pub(crate) fn convert_to_observable(
    cx: &mut JSContext,
    global: &GlobalScope,
    value: HandleValue,
) -> Fallible<DomRoot<Observable>> {
    // Step 1. If Type(value) is not Object, throw a TypeError.
    if !value.is_object() {
        return Err(Error::Type(c"Value is not an object".to_owned()));
    }

    // Step 2. From Observable: If value's specific type is an Observable, then return value.
    if let Ok(observable) = root_from_handlevalue::<Observable>(cx, value) {
        return Ok(observable);
    }

    rooted!(&in(cx) let object = value.to_object());
    rooted!(&in(cx) let mut method = UndefinedValue());
    let value = Heap::boxed(ObjectValue(object.get()));

    // Step 3. From async iterable: Let asyncIteratorMethod be ? GetMethod(value,
    // %Symbol.asyncIterator%). If it is not undefined or null, return a new Observable that
    // iterates over value asynchronously.
    if get_method_of_symbol(
        cx,
        object.handle(),
        SymbolCode::asyncIterator,
        method.handle_mut(),
    )? {
        return Ok(Observable::new_native(
            cx,
            global,
            Box::new(FromAsyncIterableSteps { value }),
        ));
    }

    // Step 4. From iterable: Let iteratorMethod be ? GetMethod(value, %Symbol.iterator%). If it
    // is not undefined, return a new Observable that iterates over value.
    if get_method_of_symbol(
        cx,
        object.handle(),
        SymbolCode::iterator,
        method.handle_mut(),
    )? {
        return Ok(Observable::new_native(
            cx,
            global,
            Box::new(FromIterableSteps { value }),
        ));
    }

    // Step 5. From Promise: If IsPromise(value) is true, then return a new Observable that
    // reacts to value.
    #[expect(unsafe_code)]
    if unsafe { IsPromiseObject(object.handle()) } {
        return Ok(Observable::new_native(
            cx,
            global,
            Box::new(FromPromiseSteps { value }),
        ));
    }

    // Step 6. Throw a TypeError.
    Err(Error::Type(
        c"Value cannot be converted to an Observable".to_owned(),
    ))
}

/// Reports the exception that `error` stands for to the global of the current realm.
fn report_error(cx: &mut CurrentRealm, error: Error) {
    let global = GlobalScope::from_current_realm(cx);
    rooted!(&in(cx) let mut exception = UndefinedValue());
    error.to_jsval(cx, &global, exception.handle_mut());
    global.report_an_exception(cx, exception.handle());
}

/// Creates a rejected promise that nothing handles, so that the rejection is reported as an
/// unhandled one.
fn reject_without_handler(cx: &mut CurrentRealm, reason: HandleValue) {
    let global = GlobalScope::from_current_realm(cx);
    Promise::new_rejected(cx, &global, reason.get());
}

/// <https://wicg.github.io/observable/#from-iterable-conversion>
#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct FromIterableSteps {
    #[ignore_malloc_size_of = "mozjs"]
    value: Box<Heap<JSVal>>,
}

impl SubscribeSteps for FromIterableSteps {
    fn subscribe(&self, cx: &mut CurrentRealm, subscriber: &Subscriber) {
        // Step 1. If subscriber's subscription controller's signal is aborted, then return.
        let signal = subscriber.subscription_signal();
        if signal.aborted() {
            return;
        }

        // Step 2. Let iteratorRecordCompletion be GetIterator(value, sync).
        // Note: This reads `@@iterator` again.
        rooted!(&in(cx) let value = self.value.get());
        let record = match get_iterator(cx, value.handle(), IteratorKind::Sync) {
            Ok(record) => record,
            Err(error) => {
                // Step 3. If it is a throw completion, then run subscriber's error() method with
                // it and abort these steps.
                error_subscriber_with(cx, subscriber, error);
                return;
            },
        };

        // Step 5. If subscriber's subscription controller's signal is aborted, then return.
        // Note: Errors of the protocol are reported before the signal is looked at.
        if signal.aborted() {
            return;
        }

        // Step 6. Add an abort algorithm to the signal that runs IteratorClose.
        rooted!(&in(cx) let _iterator = record.iterator());
        signal.add(&AbortAlgorithm::Observable(
            ObservableAbortAlgorithm::CloseIterator(record.clone()),
        ));

        // Step 7. While true:
        loop {
            // Step 7.1. Let next be IteratorStepValue(iteratorRecord).
            rooted!(&in(cx) let mut next = UndefinedValue());
            match iterator_step_value(cx, &record, next.handle_mut()) {
                // Step 7.2. If next is a throw completion, then run subscriber's error() method
                // with it, and break.
                Err(error) => {
                    error_subscriber_with(cx, subscriber, error);
                    break;
                },
                // Step 7.4. If next is done, then run subscriber's complete() and return.
                Ok(false) => {
                    subscriber.complete(cx);
                    return;
                },
                // Step 7.5. Run subscriber's next() given next.
                Ok(true) => subscriber.next(cx, next.handle()),
            }
            // Step 7.6. If subscriber's subscription controller's signal is aborted, then break.
            if signal.aborted() {
                break;
            }
        }
    }
}

/// <https://wicg.github.io/observable/#from-async-iterable-conversion>
#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct FromAsyncIterableSteps {
    #[ignore_malloc_size_of = "mozjs"]
    value: Box<Heap<JSVal>>,
}

impl SubscribeSteps for FromAsyncIterableSteps {
    fn subscribe(&self, cx: &mut CurrentRealm, subscriber: &Subscriber) {
        // Step 1. If subscriber's subscription controller's signal is aborted, then return.
        let signal = subscriber.subscription_signal();
        if signal.aborted() {
            return;
        }

        // Step 2. Let iteratorRecordCompletion be GetIterator(value, async).
        // Note: This reads `@@asyncIterator` again, and falls back to `@@iterator`.
        rooted!(&in(cx) let value = self.value.get());
        let record = match get_iterator(cx, value.handle(), IteratorKind::Async) {
            Ok(record) => record,
            Err(error) => {
                // Step 3. If it is a throw completion, then run subscriber's error() method with
                // it and abort these steps. This is synchronous, unlike the later errors.
                error_subscriber_with(cx, subscriber, error);
                return;
            },
        };

        // Step 4. If subscriber's subscription controller's signal is aborted, then return.
        if signal.aborted() {
            return;
        }

        // Step 5. Add an abort algorithm to the signal that runs AsyncIteratorClose.
        rooted!(&in(cx) let _iterator = record.iterator());
        signal.add(&AbortAlgorithm::Observable(
            ObservableAbortAlgorithm::CloseIterator(record.clone()),
        ));

        // Step 6. Run nextAlgorithm given subscriber and iteratorRecord.
        next_algorithm(cx, subscriber, &record);
    }
}

/// The nextAlgorithm of <https://wicg.github.io/observable/#from-async-iterable-conversion>.
fn next_algorithm(cx: &mut CurrentRealm, subscriber: &Subscriber, record: &Rc<IteratorRecord>) {
    // Step 1. If subscriber's subscription controller's signal is aborted, then return.
    if subscriber.subscription_signal().aborted() {
        return;
    }

    let global = subscriber.global();
    let mut sync_result_done = None;

    // Step 2. Let nextPromise be a Promise-or-undefined, initially undefined.
    // Step 3. Let nextCompletion be IteratorNext(iteratorRecord).
    rooted!(&in(cx) let mut result = UndefinedValue());
    let completion = if record.is_async_from_sync() {
        // The steps of %AsyncFromSyncIteratorPrototype%.next(): the result of the sync iterator
        // is looked at synchronously, and its value is awaited.
        next_of_sync_iterator(cx, record, result.handle_mut(), &mut sync_result_done)
    } else {
        iterator_next(cx, record, result.handle_mut())
    };

    let next_promise = match completion {
        // Step 5. Otherwise, set nextPromise to a promise resolved with nextRecord's [[Value]].
        Ok(()) => Promise::new_resolved(cx, &global, result.get()),
        // Step 4. If nextCompletion is a throw completion, then set nextPromise to a promise
        // rejected with its value.
        Err(error) => {
            rooted!(&in(cx) let mut exception = UndefinedValue());
            error.to_jsval(cx, &global, exception.handle_mut());
            Promise::new_rejected(cx, &global, exception.get())
        },
    };

    // Step 6. React to nextPromise.
    let handler = PromiseNativeHandler::new(
        cx,
        &global,
        Some(Box::new(NextFulfilled {
            subscriber: Dom::from_ref(subscriber),
            record: record.clone(),
            sync_result_done,
        })),
        Some(Box::new(NextRejected {
            subscriber: Dom::from_ref(subscriber),
            record: record.clone(),
        })),
    );
    next_promise.append_native_handler(cx, &handler);
}

/// The synchronous part of %AsyncFromSyncIteratorPrototype%.next(): calls `next()` of the sync
/// iterator, and reads `done` and `value` of the result. On success, `value` is the value, and
/// `done` is set.
fn next_of_sync_iterator(
    cx: &mut JSContext,
    record: &IteratorRecord,
    value: MutableHandleValue,
    done: &mut Option<bool>,
) -> Fallible<()> {
    rooted!(&in(cx) let mut result = UndefinedValue());
    iterator_next(cx, record, result.handle_mut())?;

    let fail = |error: Error| {
        record.set_done();
        Err(error)
    };
    if !result.is_object() {
        return fail(Error::Type(c"Iterator result is not an Object".to_owned()));
    }
    rooted!(&in(cx) let result = result.to_object());
    let is_done = match iterator_complete(cx, result.handle()) {
        Ok(is_done) => is_done,
        Err(error) => return fail(error),
    };
    if let Err(error) = iterator_value(cx, result.handle(), value) {
        return fail(error);
    }
    if is_done {
        record.set_done();
    }
    *done = Some(is_done);
    Ok(())
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct NextFulfilled {
    subscriber: Dom<Subscriber>,
    #[conditional_malloc_size_of]
    record: Rc<IteratorRecord>,
    /// Set if the record is of a sync iterator, in which case the value that the promise was
    /// fulfilled with is the `value` of the result, and this is the `done` of it.
    sync_result_done: Option<bool>,
}

impl Callback for NextFulfilled {
    fn callback(&self, cx: &mut CurrentRealm, result: HandleValue) {
        let subscriber = self.subscriber.as_rooted();

        rooted!(&in(cx) let mut value = UndefinedValue());
        match self.sync_result_done {
            Some(true) => {
                subscriber.complete(cx);
                return;
            },
            Some(false) => value.set(result.get()),
            None => {
                // If Type(iteratorResult) is not Object, then run subscriber's error() method
                // with a TypeError and abort these steps.
                if !result.is_object() {
                    self.record.set_done();
                    error_subscriber_with(
                        cx,
                        &subscriber,
                        Error::Type(c"Iterator result is not an Object".to_owned()),
                    );
                    return;
                }
                rooted!(&in(cx) let result = result.to_object());

                // Let done be IteratorComplete(iteratorResult). If it is a throw completion,
                // then run subscriber's error() method with it and abort these steps.
                // Note: This is read even if the signal was aborted in the meantime.
                match iterator_complete(cx, result.handle()) {
                    Err(error) => {
                        self.record.set_done();
                        error_subscriber_with(cx, &subscriber, error);
                        return;
                    },
                    // If done is true, then run subscriber's complete() and abort these steps.
                    Ok(true) => {
                        self.record.set_done();
                        subscriber.complete(cx);
                        return;
                    },
                    Ok(false) => {},
                }

                // Let value be IteratorValue(iteratorResult). If it is a throw completion, then
                // run subscriber's error() method with it and abort these steps.
                if let Err(error) = iterator_value(cx, result.handle(), value.handle_mut()) {
                    self.record.set_done();
                    error_subscriber_with(cx, &subscriber, error);
                    return;
                }
            },
        }

        // Run subscriber's next() given value's [[Value]].
        subscriber.next(cx, value.handle());
        // Run nextAlgorithm given subscriber and iteratorRecord.
        next_algorithm(cx, &subscriber, &self.record);
    }
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct NextRejected {
    subscriber: Dom<Subscriber>,
    #[conditional_malloc_size_of]
    record: Rc<IteratorRecord>,
}

impl Callback for NextRejected {
    fn callback(&self, cx: &mut CurrentRealm, reason: HandleValue) {
        // If nextPromise was rejected with reason r, then run subscriber's error() method given
        // r. As the iterator failed, it must not be closed.
        self.record.set_done();
        self.subscriber.error(cx, reason);
    }
}

/// <https://wicg.github.io/observable/#from-promise-conversion>
#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct FromPromiseSteps {
    #[ignore_malloc_size_of = "mozjs"]
    value: Box<Heap<JSVal>>,
}

impl SubscribeSteps for FromPromiseSteps {
    fn subscribe(&self, cx: &mut CurrentRealm, subscriber: &Subscriber) {
        // React to value.
        let global = subscriber.global();
        rooted!(&in(cx) let value = self.value.get());
        let promise = Promise::resolve_or_wrap_promise(cx, value.handle(), &global);
        let handler = PromiseNativeHandler::new(
            cx,
            &global,
            Some(Box::new(PromiseFulfilled {
                subscriber: Dom::from_ref(subscriber),
            })),
            Some(Box::new(PromiseRejected {
                subscriber: Dom::from_ref(subscriber),
            })),
        );
        promise.append_native_handler(cx, &handler);
    }
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct PromiseFulfilled {
    subscriber: Dom<Subscriber>,
}

impl Callback for PromiseFulfilled {
    fn callback(&self, cx: &mut CurrentRealm, value: HandleValue) {
        // If value was fulfilled with value v, then run subscriber's next() method given v, and
        // run subscriber's complete() method.
        self.subscriber.next(cx, value);
        self.subscriber.complete(cx);
    }
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct PromiseRejected {
    subscriber: Dom<Subscriber>,
}

impl Callback for PromiseRejected {
    fn callback(&self, cx: &mut CurrentRealm, reason: HandleValue) {
        // If value was rejected with reason r, then run subscriber's error() method given r.
        self.subscriber.error(cx, reason);
    }
}

/// The abort algorithm that closes the iterator of a `from()` subscription.
///
/// <https://wicg.github.io/observable/#from-iterable-conversion> step 6, and
/// <https://wicg.github.io/observable/#from-async-iterable-conversion> step 5.
pub(crate) fn close_iterator(cx: &mut CurrentRealm, record: &IteratorRecord, reason: HandleValue) {
    // Note: There is nothing to close when the iterator is exhausted, or threw. The WPT test
    // "Abort after complete does NOT call IteratorRecord#return()" depends on this.
    if record.is_done() {
        return;
    }
    record.set_done();

    match record.kind() {
        IteratorKind::Sync => {
            // Run IteratorClose(iteratorRecord, NormalCompletion(UNUSED)).
            // Note: The spec propagates an exception from the method out of the `abort()` call
            // that this runs in. As signaling abort is not fallible here, the exception is
            // reported.
            if let Err(error) = iterator_close(cx, record) {
                report_error(cx, error);
            }
        },
        IteratorKind::Async => {
            // Run AsyncIteratorClose(iteratorRecord, NormalCompletion(abort reason)).
            // Note: Exceptions are in the form of rejected promises that nothing handles.
            rooted!(&in(cx) let mut result = UndefinedValue());
            match async_iterator_call_return(cx, record, reason, result.handle_mut()) {
                Ok(false) => {},
                Err(error) => {
                    let global = GlobalScope::from_current_realm(cx);
                    rooted!(&in(cx) let mut exception = UndefinedValue());
                    error.to_jsval(cx, &global, exception.handle_mut());
                    reject_without_handler(cx, exception.handle());
                },
                Ok(true) => {
                    let global = GlobalScope::from_current_realm(cx);
                    let promise = Promise::new_resolved(cx, &global, result.get());
                    let handler = PromiseNativeHandler::new(
                        cx,
                        &global,
                        Some(Box::new(CloseFulfilled)),
                        Some(Box::new(CloseRejected)),
                    );
                    promise.append_native_handler(cx, &handler);
                },
            }
        },
    }
}

#[derive(JSTraceable, MallocSizeOf)]
struct CloseFulfilled;

impl Callback for CloseFulfilled {
    fn callback(&self, cx: &mut CurrentRealm, result: HandleValue) {
        // If the result of awaiting the return() method is not an Object, then it is an error.
        if !result.is_object() {
            let global = GlobalScope::from_current_realm(cx);
            rooted!(&in(cx) let mut exception = UndefinedValue());
            Error::Type(c"Iterator's return() method must return an Object".to_owned()).to_jsval(
                cx,
                &global,
                exception.handle_mut(),
            );
            reject_without_handler(cx, exception.handle());
        }
    }
}

#[derive(JSTraceable, MallocSizeOf)]
struct CloseRejected;

impl Callback for CloseRejected {
    fn callback(&self, cx: &mut CurrentRealm, reason: HandleValue) {
        reject_without_handler(cx, reason);
    }
}
