/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The ECMAScript iteration abstract operations that `Observable.from()` needs.
//!
//! <https://tc39.es/ecma262/#sec-operations-on-iterator-objects>
//!
//! These cannot use `js::rust::ForOfIterator`, as that can neither be stored, nor be driven
//! asynchronously, and `from()` has to read the properties of the iterator in an exact order.
//!
//! Failing operations return `Err(Error::JSFailed)` with the exception pending, or another
//! [`Error`] that stands for an exception that was not thrown yet.

use std::cell::Cell;
use std::rc::Rc;

use js::context::JSContext;
use js::jsapi::{Heap, IsCallable, JSObject, SymbolCode};
use js::jsval::{JSVal, ObjectValue, UndefinedValue};
use js::rust::wrappers2::{Call, GetWellKnownSymbolKey};
use js::rust::{HandleObject, HandleValue, MutableHandleValue};
use script_bindings::js::{JS_GetProperty, JS_GetPropertyById};

use crate::dom::bindings::error::{Error, Fallible};

#[derive(Clone, Copy, Eq, JSTraceable, MallocSizeOf, PartialEq)]
pub(crate) enum IteratorKind {
    Sync,
    Async,
}

/// <https://tc39.es/ecma262/#sec-iterator-records>
#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::allow_unrooted_in_rc)]
pub(crate) struct IteratorRecord {
    #[ignore_malloc_size_of = "mozjs"]
    iterator: Heap<*mut JSObject>,

    /// The `next` method of the iterator.
    ///
    /// This is read when it is first needed, instead of when the record is created. The effect
    /// is that an error from a `next` getter surfaces from the first call to the method, which
    /// is what `from()` of an async iterable is tested to do.
    #[ignore_malloc_size_of = "mozjs"]
    next_method: Heap<JSVal>,
    next_method_loaded: Cell<bool>,

    /// <https://tc39.es/ecma262/#table-iterator-record-fields>
    ///
    /// This is also set when the iterator has returned a result that has `done` set, or when a
    /// call or property access on the iterator has thrown, as then the iterator must not be closed
    /// anymore.
    done: Cell<bool>,

    kind: IteratorKind,

    /// Whether this is the iterator of an object that does not implement the async iterator
    /// protocol, but the sync iterator protocol. That is what
    /// <https://tc39.es/ecma262/#sec-createasyncfromsynciterator> wraps.
    async_from_sync: bool,
}

impl IteratorRecord {
    pub(crate) fn iterator(&self) -> *mut JSObject {
        self.iterator.get()
    }

    pub(crate) fn is_done(&self) -> bool {
        self.done.get()
    }

    pub(crate) fn set_done(&self) {
        self.done.set(true);
    }

    pub(crate) fn kind(&self) -> IteratorKind {
        self.kind
    }

    pub(crate) fn is_async_from_sync(&self) -> bool {
        self.async_from_sync
    }
}

/// Returns whether `value` is a callable object.
#[expect(unsafe_code)]
fn is_callable(cx: &mut JSContext, value: HandleValue) -> bool {
    if !value.is_object() {
        return false;
    }
    rooted!(&in(cx) let object = value.to_object());
    unsafe { IsCallable(object.get()) }
}

/// The part of <https://tc39.es/ecma262/#sec-getmethod> that comes after the property was read.
fn check_method(cx: &mut JSContext, method: HandleValue) -> Fallible<bool> {
    // If func is either undefined or null, return undefined.
    if method.is_undefined() || method.is_null() {
        return Ok(false);
    }
    // If IsCallable(func) is false, throw a TypeError exception.
    if !is_callable(cx, method) {
        return Err(Error::Type(c"Property is not callable".to_owned()));
    }
    Ok(true)
}

/// <https://tc39.es/ecma262/#sec-getmethod>
///
/// Returns whether there is a method. If so, `rval` is that.
pub(crate) fn get_method_of_symbol(
    cx: &mut JSContext,
    object: HandleObject,
    symbol: SymbolCode,
    mut rval: MutableHandleValue,
) -> Fallible<bool> {
    #[expect(unsafe_code)]
    let key = unsafe { GetWellKnownSymbolKey(cx, symbol) };
    rooted!(&in(cx) let key = key);
    if !JS_GetPropertyById(cx, object, key.handle(), rval.reborrow()) {
        return Err(Error::JSFailed);
    }
    check_method(cx, rval.handle())
}

/// <https://tc39.es/ecma262/#sec-getmethod>
fn get_method_of_name(
    cx: &mut JSContext,
    object: HandleObject,
    name: &std::ffi::CStr,
    mut rval: MutableHandleValue,
) -> Fallible<bool> {
    if !JS_GetProperty(cx, object, name, rval.reborrow()) {
        return Err(Error::JSFailed);
    }
    check_method(cx, rval.handle())
}

/// <https://tc39.es/ecma262/#sec-getiterator>
///
/// `value` must be an object.
pub(crate) fn get_iterator(
    cx: &mut JSContext,
    value: HandleValue,
    kind: IteratorKind,
) -> Fallible<Rc<IteratorRecord>> {
    rooted!(&in(cx) let object = value.to_object());
    rooted!(&in(cx) let mut method = UndefinedValue());
    let mut async_from_sync = false;

    let has_method = match kind {
        IteratorKind::Async => {
            // Let method be ? GetMethod(obj, %Symbol.asyncIterator%).
            if get_method_of_symbol(
                cx,
                object.handle(),
                SymbolCode::asyncIterator,
                method.handle_mut(),
            )? {
                true
            } else {
                // If method is undefined, then let syncMethod be ? GetMethod(obj,
                // %Symbol.iterator%), and wrap the iterator of it in an async-from-sync iterator.
                async_from_sync = true;
                get_method_of_symbol(
                    cx,
                    object.handle(),
                    SymbolCode::iterator,
                    method.handle_mut(),
                )?
            }
        },
        IteratorKind::Sync => get_method_of_symbol(
            cx,
            object.handle(),
            SymbolCode::iterator,
            method.handle_mut(),
        )?,
    };
    // If method is undefined, throw a TypeError exception.
    if !has_method {
        return Err(Error::Type(c"Object is not iterable".to_owned()));
    }

    // Return ? GetIteratorFromMethod(obj, method).
    rooted!(&in(cx) let mut iterator = UndefinedValue());
    #[expect(unsafe_code)]
    let called = unsafe {
        Call(
            cx,
            value,
            method.handle(),
            &js::jsapi::HandleValueArray::empty(),
            iterator.handle_mut(),
        )
    };
    if !called {
        return Err(Error::JSFailed);
    }
    // If iterator is not an Object, throw a TypeError exception.
    if !iterator.is_object() {
        return Err(Error::Type(c"Iterator is not an Object".to_owned()));
    }

    let record = Rc::new(IteratorRecord {
        iterator: Heap::default(),
        next_method: Heap::default(),
        next_method_loaded: Cell::new(false),
        done: Cell::new(false),
        kind,
        async_from_sync,
    });
    record.iterator.set(iterator.to_object());
    Ok(record)
}

/// <https://tc39.es/ecma262/#sec-iteratornext>
///
/// A failure marks the record as done.
pub(crate) fn iterator_next(
    cx: &mut JSContext,
    record: &IteratorRecord,
    rval: MutableHandleValue,
) -> Fallible<()> {
    rooted!(&in(cx) let iterator = record.iterator.get());

    if !record.next_method_loaded.get() {
        record.next_method_loaded.set(true);
        rooted!(&in(cx) let mut next_method = UndefinedValue());
        if !JS_GetProperty(cx, iterator.handle(), c"next", next_method.handle_mut()) {
            record.done.set(true);
            return Err(Error::JSFailed);
        }
        record.next_method.set(next_method.get());
    }

    rooted!(&in(cx) let this_value = ObjectValue(iterator.get()));
    rooted!(&in(cx) let next_method = record.next_method.get());
    #[expect(unsafe_code)]
    let called = unsafe {
        Call(
            cx,
            this_value.handle(),
            next_method.handle(),
            &js::jsapi::HandleValueArray::empty(),
            rval,
        )
    };
    if !called {
        record.done.set(true);
        return Err(Error::JSFailed);
    }
    Ok(())
}

/// <https://tc39.es/ecma262/#sec-iteratorcomplete>
pub(crate) fn iterator_complete(cx: &mut JSContext, result: HandleObject) -> Fallible<bool> {
    rooted!(&in(cx) let mut done = UndefinedValue());
    if !JS_GetProperty(cx, result, c"done", done.handle_mut()) {
        return Err(Error::JSFailed);
    }
    #[expect(unsafe_code)]
    Ok(unsafe { js::rust::ToBoolean(done.handle()) })
}

/// <https://tc39.es/ecma262/#sec-iteratorvalue>
pub(crate) fn iterator_value(
    cx: &mut JSContext,
    result: HandleObject,
    rval: MutableHandleValue,
) -> Fallible<()> {
    if !JS_GetProperty(cx, result, c"value", rval) {
        return Err(Error::JSFailed);
    }
    Ok(())
}

/// <https://tc39.es/ecma262/#sec-iteratorstepvalue>
///
/// Returns whether there is a value. If so, `rval` is that. A failure marks the record as done.
pub(crate) fn iterator_step_value(
    cx: &mut JSContext,
    record: &IteratorRecord,
    rval: MutableHandleValue,
) -> Fallible<bool> {
    rooted!(&in(cx) let mut result = UndefinedValue());
    iterator_next(cx, record, result.handle_mut())?;

    let fail = |error: Error| {
        record.done.set(true);
        Err(error)
    };

    // If result is not an Object, throw a TypeError exception.
    if !result.is_object() {
        return fail(Error::Type(c"Iterator result is not an Object".to_owned()));
    }
    rooted!(&in(cx) let result = result.to_object());

    // Let done be Completion(IteratorComplete(result)).
    match iterator_complete(cx, result.handle()) {
        Err(error) => return fail(error),
        Ok(true) => {
            // If done is true, set iteratorRecord.[[Done]] to true and return done.
            record.done.set(true);
            return Ok(false);
        },
        Ok(false) => {},
    }

    // Let value be Completion(IteratorValue(result)).
    if let Err(error) = iterator_value(cx, result.handle(), rval) {
        return fail(error);
    }
    Ok(true)
}

/// Calls the `return` method of the iterator of `record`, if there is one, with `arguments`.
///
/// Returns the result of the call, which is `None` when there is no `return` method.
fn call_return(
    cx: &mut JSContext,
    record: &IteratorRecord,
    argument: Option<HandleValue>,
    rval: MutableHandleValue,
) -> Fallible<bool> {
    rooted!(&in(cx) let iterator = record.iterator.get());
    rooted!(&in(cx) let mut return_method = UndefinedValue());
    if !get_method_of_name(cx, iterator.handle(), c"return", return_method.handle_mut())? {
        return Ok(false);
    }

    rooted!(&in(cx) let this_value = ObjectValue(iterator.get()));
    rooted_vec!(let mut arguments);
    if let Some(argument) = argument {
        arguments.push(argument.get());
    }
    let arguments = js::jsapi::HandleValueArray::from(&arguments);
    #[expect(unsafe_code)]
    let called = unsafe {
        Call(
            cx,
            this_value.handle(),
            return_method.handle(),
            &arguments,
            rval,
        )
    };
    if !called {
        return Err(Error::JSFailed);
    }
    Ok(true)
}

/// <https://tc39.es/ecma262/#sec-iteratorclose>, with a normal completion.
pub(crate) fn iterator_close(cx: &mut JSContext, record: &IteratorRecord) -> Fallible<()> {
    rooted!(&in(cx) let mut result = UndefinedValue());
    // If the iterator has a return method, call it. If the result is not an Object, throw a
    // TypeError exception.
    if call_return(cx, record, None, result.handle_mut())? && !result.is_object() {
        return Err(Error::Type(
            c"Iterator's return() method must return an Object".to_owned(),
        ));
    }
    Ok(())
}

/// The call of <https://tc39.es/ecma262/#sec-asynciteratorclose> that precedes awaiting the
/// result of the `return` method. That is told apart from the later steps, as the result of it is
/// a promise.
///
/// Returns whether there is a `return` method. If so, `rval` is the result of calling it. The
/// `reason` is passed on to the method, unless this is the iterator of a sync iterable.
pub(crate) fn async_iterator_call_return(
    cx: &mut JSContext,
    record: &IteratorRecord,
    reason: HandleValue,
    rval: MutableHandleValue,
) -> Fallible<bool> {
    let argument = (!record.async_from_sync).then_some(reason);
    call_return(cx, record, argument, rval)
}
