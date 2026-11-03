/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! <https://streams.spec.whatwg.org/#readable-stream-from-iterable>

use std::rc::Rc;

use js::context::JSContext;
use js::jsval::UndefinedValue;
use js::realm::CurrentRealm;
use js::rust::{HandleValue as SafeHandleValue, MutableHandleValue as SafeMutableHandleValue};

use crate::dom::bindings::error::{Error, ErrorToJsval, Fallible};
use crate::dom::bindings::iteration::{
    IteratorKind, IteratorRecord, async_from_sync_iterator_next, call_return, get_iterator,
    iterator_complete, iterator_next, iterator_value,
};
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::globalscope::GlobalScope;
use crate::dom::promise::{Promise, RootedPromise, TracedPromise};
use crate::dom::promisenativehandler::{Callback, PromiseNativeHandler};
use crate::dom::readablestream::{ReadableStream, create_readable_stream};
use crate::dom::stream::readablestreamdefaultcontroller::ReadableStreamDefaultController;
use crate::dom::stream::underlyingsourcecontainer::UnderlyingSourceType;
use crate::realms::enter_auto_realm;

/// <https://streams.spec.whatwg.org/#readable-stream-from-iterable>
pub(crate) fn readable_stream_from_iterable(
    cx: &mut JSContext,
    global: &GlobalScope,
    async_iterable: SafeHandleValue,
) -> Fallible<DomRoot<ReadableStream>> {
    // Note: The argument is an async_sequence<any>, whose conversion throws a TypeError for
    // values that are not objects.
    if !async_iterable.is_object() {
        return Err(Error::Type(c"Value is not an async iterable".to_owned()));
    }

    // Let iterator be the result of opening an async sequence asyncIterable.
    // Note: This is GetIterator(asyncIterable, async), which falls back to the sync iterator.
    let record = get_iterator(cx, async_iterable, IteratorKind::Async)?;

    // Let startAlgorithm be an algorithm that returns undefined.
    // Let pullAlgorithm and cancelAlgorithm be the steps below.
    // Set stream to ! CreateReadableStream(startAlgorithm, pullAlgorithm, cancelAlgorithm, 0).
    // Return stream.
    Ok(create_readable_stream(
        cx,
        global,
        UnderlyingSourceType::Iterable(&record),
        None,
        Some(0.0),
    ))
}

/// Rejects a new promise with `error`.
fn rejected_promise(cx: &mut JSContext, global: &GlobalScope, error: Error) -> RootedPromise {
    rooted!(&in(cx) let mut reason = UndefinedValue());
    error.to_jsval(cx, global, reason.handle_mut());
    Promise::new_rejected(cx, global, reason.handle())
}

/// The pullAlgorithm of <https://streams.spec.whatwg.org/#readable-stream-from-iterable>.
pub(crate) fn pull_algorithm(
    cx: &mut JSContext,
    global: &GlobalScope,
    record: &Rc<IteratorRecord>,
    controller: &ReadableStreamDefaultController,
) -> RootedPromise {
    // Let nextPromise be the result of getting the next value of iterator.
    // Note: That is, let nextResult be IteratorNext(iteratorRecord). If nextResult is an abrupt
    // completion, return a promise rejected with nextResult.[[Value]]. Let nextPromise be a
    // promise resolved with nextResult.[[Value]].
    let mut sync_result_done = None;
    rooted!(&in(cx) let mut next_result = UndefinedValue());
    let completion = if record.is_async_from_sync() {
        // The steps of %AsyncFromSyncIteratorPrototype%.next(): the result of the sync iterator
        // is looked at synchronously, and its value is awaited.
        async_from_sync_iterator_next(cx, record, next_result.handle_mut(), &mut sync_result_done)
    } else {
        iterator_next(cx, record, next_result.handle_mut())
    };
    if let Err(error) = completion {
        return rejected_promise(cx, global, error);
    }
    let next_promise = Promise::new_resolved(cx, global, next_result.get());

    // Return the result of reacting to nextPromise.
    let result = Promise::new(cx, global);
    let handler = PromiseNativeHandler::new(
        cx,
        global,
        Some(Box::new(PullFulfilled {
            controller: Dom::from_ref(controller),
            sync_result_done,
            result: result.to_traced(),
        })),
        Some(Box::new(PullRejected {
            controller: Dom::from_ref(controller),
            result: result.to_traced(),
        })),
    );
    let mut realm = enter_auto_realm(cx, global);
    next_promise.append_native_handler(&mut realm.current_realm(), &handler);
    result
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct PullFulfilled {
    controller: Dom<ReadableStreamDefaultController>,
    /// Set if the iterator is of a sync iterable, in which case the value that the promise was
    /// fulfilled with is the `value` of the result, and this is the `done` of it.
    sync_result_done: Option<bool>,
    result: TracedPromise,
}

impl PullFulfilled {
    /// The fulfillment steps of getting the next value of an iterator. Returns whether there is
    /// a value, which is then `value`, or whether this is the end of iteration.
    fn next_value(
        &self,
        cx: &mut JSContext,
        iter_result: SafeHandleValue,
        mut value: SafeMutableHandleValue,
    ) -> Fallible<bool> {
        match self.sync_result_done {
            Some(done) => {
                value.set(iter_result.get());
                return Ok(!done);
            },
            None => {},
        }

        // If Type(iterResult) is not Object, throw a TypeError.
        if !iter_result.is_object() {
            return Err(Error::Type(c"Iterator result is not an Object".to_owned()));
        }
        rooted!(&in(cx) let iter_result = iter_result.to_object());

        // Let done be ? IteratorComplete(iterResult).
        // If done is true, return end of iteration.
        if iterator_complete(cx, iter_result.handle())? {
            return Ok(false);
        }

        // Return ? IteratorValue(iterResult).
        iterator_value(cx, iter_result.handle(), value)?;
        Ok(true)
    }
}

impl Callback for PullFulfilled {
    fn callback(&self, cx: &mut CurrentRealm, iter_result: SafeHandleValue) {
        rooted!(&in(cx) let mut value = UndefinedValue());
        match self.next_value(cx, iter_result, value.handle_mut()) {
            Err(error) => {
                // If getting the value threw, nextPromise is rejected with the exception.
                rooted!(&in(cx) let mut reason = UndefinedValue());
                let global = GlobalScope::from_current_realm(cx);
                error.to_jsval(cx, &global, reason.handle_mut());
                self.controller.error(cx, reason.handle());
            },
            // If v is end of iteration, perform ! ReadableStreamDefaultControllerClose(
            // stream.[[controller]]).
            Ok(false) => self.controller.close(cx),
            // Otherwise, perform ! ReadableStreamDefaultControllerEnqueue(stream.[[controller]], v).
            Ok(true) => {
                self.controller
                    .enqueue(cx, value.handle())
                    .expect("Enqueuing a chunk with the default size algorithm cannot fail");
            },
        }
        self.result.resolve_native(cx, &());
    }
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct PullRejected {
    controller: Dom<ReadableStreamDefaultController>,
    result: TracedPromise,
}

impl Callback for PullRejected {
    fn callback(&self, cx: &mut CurrentRealm, reason: SafeHandleValue) {
        // If nextPromise was rejected with reason r, perform !
        // ReadableStreamDefaultControllerError(stream.[[controller]], r).
        self.controller.error(cx, reason);
        self.result.resolve_native(cx, &());
    }
}

/// The cancelAlgorithm of <https://streams.spec.whatwg.org/#readable-stream-from-iterable>.
pub(crate) fn cancel_algorithm(
    cx: &mut JSContext,
    global: &GlobalScope,
    record: &IteratorRecord,
    reason: SafeHandleValue,
) -> RootedPromise {
    // Return the result of closing iterator with reason.
    // Note: That is, let returnMethod be GetMethod(iterator, "return"). If it is undefined,
    // return a promise resolved with undefined. Let returnResult be Call(returnMethod.[[Value]],
    // iterator, « reason »). If either is an abrupt completion, return a promise rejected with
    // its value.
    rooted!(&in(cx) let mut return_result = UndefinedValue());
    match call_return(cx, record, Some(reason), return_result.handle_mut()) {
        Err(error) => return rejected_promise(cx, global, error),
        Ok(false) => return Promise::new_resolved(cx, global, ()),
        Ok(true) => {},
    }

    // Let returnPromise be a promise resolved with returnResult.[[Value]].
    let return_promise = Promise::new_resolved(cx, global, return_result.get());

    // Return the result of reacting to returnPromise with the following fulfillment steps,
    // given returnPromiseResult:
    let result = Promise::new(cx, global);
    let handler = PromiseNativeHandler::new(
        cx,
        global,
        Some(Box::new(CancelFulfilled {
            result: result.to_traced(),
        })),
        Some(Box::new(CancelRejected {
            result: result.to_traced(),
        })),
    );
    let mut realm = enter_auto_realm(cx, global);
    return_promise.append_native_handler(&mut realm.current_realm(), &handler);
    result
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct CancelFulfilled {
    result: TracedPromise,
}

impl Callback for CancelFulfilled {
    fn callback(&self, cx: &mut CurrentRealm, return_promise_result: SafeHandleValue) {
        // If Type(returnPromiseResult) is not Object, throw a TypeError exception.
        if !return_promise_result.is_object() {
            self.result.reject_error(
                cx,
                Error::Type(c"Iterator's return() method must return an Object".to_owned()),
            );
            return;
        }
        // Return undefined.
        self.result.resolve_native(cx, &());
    }
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct CancelRejected {
    result: TracedPromise,
}

impl Callback for CancelRejected {
    fn callback(&self, cx: &mut CurrentRealm, reason: SafeHandleValue) {
        self.result.reject(cx, reason);
    }
}
