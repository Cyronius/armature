/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Implementation of value `async_iterable<...>` WebIDL declarations.
//!
//! <https://webidl.spec.whatwg.org/#es-asynchronous-iterator-prototype-object>
//!
//! The asynchronous iterators of all interfaces are [`AsyncIterableIterator`]s, which do the
//! promise handling of the `next()` and `return()` methods, and call into the [`AsyncIterable`]
//! implementation of the interface for the parts that differ between interfaces.

use std::cell::Cell;

use dom_struct::dom_struct;
use js::context::JSContext;
use js::jsapi::{Heap, JSObject};
use js::jsval::{JSVal, ObjectValue, UndefinedValue};
use js::realm::CurrentRealm;
use js::rust::{HandleObject, HandleValue};
use malloc_size_of::MallocSizeOf;
use script_bindings::cell::DomRefCell;
use script_bindings::iterable::AsyncIterator;
use script_bindings::js::JS_NewObjectWithGivenProto;
use script_bindings::reflector::{DomObject, DomObjectWrap, Reflector, reflect_dom_object};

use crate::DomTypeHolder;
use crate::dom::bindings::codegen::Bindings::IterableIteratorBinding::IterableKeyOrValueResult;
use crate::dom::bindings::inheritance::HasParent;
use crate::dom::bindings::reflector::DomGlobal;
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::bindings::trace::JSTraceable;
use crate::dom::globalscope::GlobalScope;
use crate::dom::promise::{Promise, RootedPromise, TracedPromise};
use crate::dom::promisenativehandler::{Callback, PromiseNativeHandler};
use crate::realms::enter_auto_realm;

/// An interface with a value `async_iterable<...>` declaration.
///
/// The `values()` method of the interface runs the asynchronous iterator initialization steps,
/// and creates the iterator with [`AsyncIterableIterator::new`].
pub(crate) trait AsyncIterable:
    DomObject + JSTraceable + MallocSizeOf + Sized + 'static
{
    /// The state of an asynchronous iterator, which the asynchronous iterator initialization
    /// steps set up.
    #[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
    type IteratorState: JSTraceable + MallocSizeOf + 'static;

    /// <https://webidl.spec.whatwg.org/#dfn-get-the-next-iteration-result>
    ///
    /// The promise is to be resolved with the next value, or with
    /// [`AsyncIterableIterator::resolve_with_end_of_iteration`].
    fn get_next_iteration_result(
        &self,
        cx: &mut JSContext,
        iterator: &AsyncIterableIterator<Self>,
    ) -> RootedPromise;

    /// <https://webidl.spec.whatwg.org/#asynchronous-iterator-return>
    fn async_iterator_return(
        &self,
        cx: &mut JSContext,
        iterator: &AsyncIterableIterator<Self>,
        value: HandleValue,
    ) -> RootedPromise;
}

/// <https://webidl.spec.whatwg.org/#default-asynchronous-iterator-object>
// Note: `HasParent` is implemented below, as `dom_struct` can only implement it for structs that
// are generic over `DomTypes`.
#[dom_struct(no_has_parent)]
pub(crate) struct AsyncIterableIterator<T: AsyncIterable> {
    reflector_: Reflector,

    /// <https://webidl.spec.whatwg.org/#default-asynchronous-iterator-object-target>
    target: Dom<T>,

    /// The state that is specific to the interface of the target.
    state: T::IteratorState,

    /// <https://webidl.spec.whatwg.org/#default-asynchronous-iterator-object-ongoing-promise>
    ongoing_promise: DomRefCell<Option<TracedPromise>>,

    /// <https://webidl.spec.whatwg.org/#default-asynchronous-iterator-object-is-finished>
    is_finished: Cell<bool>,

    /// The value that stands for "end of iteration" in the promises of
    /// [`AsyncIterable::get_next_iteration_result`]. It is an object without a prototype and
    /// without properties, so that resolving a promise with it can not run script.
    #[ignore_malloc_size_of = "mozjs"]
    end_of_iteration: Heap<*mut JSObject>,
}

impl<T: AsyncIterable> HasParent for AsyncIterableIterator<T> {
    type Parent = Reflector;

    fn as_parent(&self) -> &Reflector {
        &self.reflector_
    }
}

impl<T: AsyncIterable> AsyncIterableIterator<T>
where
    Self: DomObjectWrap<DomTypeHolder>,
{
    /// Creates an asynchronous iterator for `target` in the current realm, whose asynchronous
    /// iterator initialization steps resulted in `state`.
    ///
    /// <https://webidl.spec.whatwg.org/#es-asynchronous-iterable>
    pub(crate) fn new(
        realm: &mut CurrentRealm,
        target: &T,
        state: Box<T::IteratorState>,
    ) -> DomRoot<Self> {
        let global = GlobalScope::from_current_realm(realm);
        let iterator = reflect_dom_object(
            realm,
            Box::new(AsyncIterableIterator {
                reflector_: Reflector::new(),
                target: Dom::from_ref(target),
                state: *state,
                ongoing_promise: DomRefCell::new(None),
                is_finished: Cell::new(false),
                end_of_iteration: Heap::default(),
            }),
            &*global,
        );
        let end_of_iteration = JS_NewObjectWithGivenProto(realm, None, HandleObject::null());
        iterator.end_of_iteration.set(end_of_iteration);
        iterator
    }
}

impl<T: AsyncIterable> AsyncIterableIterator<T> {
    /// The state that the asynchronous iterator initialization steps set up.
    pub(crate) fn state(&self) -> &T::IteratorState {
        &self.state
    }

    /// Resolves a promise of [`AsyncIterable::get_next_iteration_result`] with end of iteration.
    pub(crate) fn resolve_with_end_of_iteration(&self, cx: &mut JSContext, promise: &Promise) {
        rooted!(&in(cx) let end_of_iteration = ObjectValue(self.end_of_iteration.get()));
        promise.resolve(cx, end_of_iteration.handle());
    }

    fn is_end_of_iteration(&self, value: HandleValue) -> bool {
        value.is_object() && value.to_object() == self.end_of_iteration.get()
    }

    /// Runs the nextSteps, or the returnSteps with `return_value` if it is `Some`, after
    /// `ongoing_promise` settled, or now, if there is no ongoing promise, and sets the ongoing
    /// promise to the result.
    ///
    /// Steps 9 to 11 of the `next()` and the `return()` methods of
    /// <https://webidl.spec.whatwg.org/#es-asynchronous-iterator-prototype-object>.
    fn chain_to_ongoing_promise(
        &self,
        cx: &mut JSContext,
        return_value: Option<HandleValue>,
    ) -> RootedPromise {
        // Let ongoingPromise be object’s ongoing promise.
        let ongoing_promise = self
            .ongoing_promise
            .borrow()
            .as_ref()
            .map(|promise| promise.root(cx));

        let promise = match ongoing_promise {
            // If ongoingPromise is not null, then:
            Some(ongoing_promise) => {
                // Let afterOngoingPromiseCapability be ! NewPromiseCapability(%Promise%).
                let global = self.global();
                let after_ongoing_promise = Promise::new(cx, &global);

                // Let onSettled be CreateBuiltinFunction(steps, « »).
                // Perform PerformPromiseThen(ongoingPromise, onSettled, onSettled,
                // afterOngoingPromiseCapability).
                let handler = PromiseNativeHandler::new(
                    cx,
                    &global,
                    Some(Box::new(RunIteratorSteps {
                        iterator: Dom::from_ref(self),
                        steps: IteratorSteps::new(return_value),
                        capability: after_ongoing_promise.to_traced(),
                    })),
                    Some(Box::new(RunIteratorSteps {
                        iterator: Dom::from_ref(self),
                        steps: IteratorSteps::new(return_value),
                        capability: after_ongoing_promise.to_traced(),
                    })),
                );
                let mut realm = enter_auto_realm(cx, &*global);
                ongoing_promise.append_native_handler(&mut realm.current_realm(), &handler);

                // Set object’s ongoing promise to afterOngoingPromiseCapability.[[Promise]].
                after_ongoing_promise
            },
            // Otherwise: Set object’s ongoing promise to the result of running steps.
            None => match return_value {
                None => self.next_steps(cx),
                Some(value) => self.return_steps(cx, value),
            },
        };
        *self.ongoing_promise.borrow_mut() = Some(promise.to_traced());
        promise
    }

    /// The nextSteps of the `next()` method of
    /// <https://webidl.spec.whatwg.org/#es-asynchronous-iterator-prototype-object>.
    fn next_steps(&self, cx: &mut JSContext) -> RootedPromise {
        // Let nextPromiseCapability be ! NewPromiseCapability(%Promise%).
        let global = self.global();
        let next_promise_capability = Promise::new(cx, &global);

        // If object’s is finished is true, then:
        if self.is_finished.get() {
            // Let result be CreateIteratorResultObject(undefined, true).
            // Perform ! Call(nextPromiseCapability.[[Resolve]], undefined, « result »).
            // Return nextPromiseCapability.[[Promise]].
            resolve_with_iterator_result(cx, &next_promise_capability, UndefinedValue(), true);
            return next_promise_capability;
        }

        // Let nextPromise be the result of getting the next iteration result with object’s
        // target and object.
        let next_promise = self.target.get_next_iteration_result(cx, self);

        // Perform PerformPromiseThen(nextPromise, onFulfilled, onRejected,
        // nextPromiseCapability).
        let handler = PromiseNativeHandler::new(
            cx,
            &global,
            Some(Box::new(NextFulfilled {
                iterator: Dom::from_ref(self),
                capability: next_promise_capability.to_traced(),
            })),
            Some(Box::new(NextRejected {
                iterator: Dom::from_ref(self),
                capability: next_promise_capability.to_traced(),
            })),
        );
        let mut realm = enter_auto_realm(cx, &*global);
        next_promise.append_native_handler(&mut realm.current_realm(), &handler);

        // Return nextPromiseCapability.[[Promise]].
        next_promise_capability
    }

    /// The returnSteps of the `return()` method of
    /// <https://webidl.spec.whatwg.org/#es-asynchronous-iterator-prototype-object>.
    fn return_steps(&self, cx: &mut JSContext, value: HandleValue) -> RootedPromise {
        // If object’s is finished is true, then:
        if self.is_finished.get() {
            // Let result be CreateIteratorResultObject(value, true).
            // Perform ! Call(returnPromiseCapability.[[Resolve]], undefined, « result »).
            // Return returnPromiseCapability.[[Promise]].
            let return_promise_capability = Promise::new(cx, &self.global());
            resolve_with_iterator_result(cx, &return_promise_capability, value.get(), true);
            return return_promise_capability;
        }

        // Set object’s is finished to true.
        self.is_finished.set(true);

        // Return the result of running the asynchronous iterator return algorithm for interface,
        // given object’s target, object, and value.
        self.target.async_iterator_return(cx, self, value)
    }
}

impl<T: AsyncIterable> AsyncIterator<DomTypeHolder> for AsyncIterableIterator<T> {
    /// The `next()` method of
    /// <https://webidl.spec.whatwg.org/#es-asynchronous-iterator-prototype-object>.
    ///
    /// Note: The bindings check that the this value is an asynchronous iterator of the
    /// interface.
    fn next(&self, cx: &mut JSContext) -> RootedPromise {
        self.chain_to_ongoing_promise(cx, None)
    }

    /// The `return()` method of
    /// <https://webidl.spec.whatwg.org/#es-asynchronous-iterator-prototype-object>.
    fn r#return(&self, cx: &mut JSContext, value: HandleValue) -> RootedPromise {
        // Steps 9 to 11, which run the returnSteps.
        let ongoing_promise = self.chain_to_ongoing_promise(cx, Some(value));

        // Let returnPromiseCapability be ! NewPromiseCapability(%Promise%).
        let global = self.global();
        let return_promise_capability = Promise::new(cx, &global);

        // Let onFulfilled be CreateBuiltinFunction(fulfillSteps, « »), where fulfillSteps
        // return CreateIteratorResultObject(value, true).
        // Perform PerformPromiseThen(object’s ongoing promise, onFulfilled, undefined,
        // returnPromiseCapability).
        let handler = PromiseNativeHandler::new(
            cx,
            &global,
            Some(Box::new(ReturnFulfilled {
                value: Heap::boxed(value.get()),
                capability: return_promise_capability.to_traced(),
            })),
            Some(Box::new(RejectCapability {
                capability: return_promise_capability.to_traced(),
            })),
        );
        let mut realm = enter_auto_realm(cx, &*global);
        ongoing_promise.append_native_handler(&mut realm.current_realm(), &handler);

        // Return returnPromiseCapability.[[Promise]].
        return_promise_capability
    }
}

/// Resolves `promise` with CreateIteratorResultObject(value, done).
fn resolve_with_iterator_result(cx: &mut JSContext, promise: &Promise, value: JSVal, done: bool) {
    promise.resolve_native(
        cx,
        &IterableKeyOrValueResult {
            done,
            value: Heap::boxed(value),
        },
    );
}

/// The steps that the `next()` and `return()` methods chain to the ongoing promise.
#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
enum IteratorSteps {
    Next,
    Return(#[ignore_malloc_size_of = "mozjs"] Box<Heap<JSVal>>),
}

impl IteratorSteps {
    /// The nextSteps, or the returnSteps with `return_value` if it is `Some`.
    fn new(return_value: Option<HandleValue>) -> IteratorSteps {
        match return_value {
            None => IteratorSteps::Next,
            Some(value) => IteratorSteps::Return(Heap::boxed(value.get())),
        }
    }
}

/// Runs the steps once the ongoing promise settled, and resolves the capability with the result.
#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct RunIteratorSteps<T: AsyncIterable> {
    iterator: Dom<AsyncIterableIterator<T>>,
    steps: IteratorSteps,
    capability: TracedPromise,
}

impl<T: AsyncIterable> Callback for RunIteratorSteps<T> {
    fn callback(&self, cx: &mut CurrentRealm, _value: HandleValue) {
        let promise = match &self.steps {
            IteratorSteps::Next => self.iterator.next_steps(cx),
            IteratorSteps::Return(value) => {
                rooted!(&in(cx) let value = value.get());
                self.iterator.return_steps(cx, value.handle())
            },
        };
        self.capability.resolve_native(cx, &promise);
    }
}

/// The fulfillSteps of the nextSteps of the `next()` method.
#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct NextFulfilled<T: AsyncIterable> {
    iterator: Dom<AsyncIterableIterator<T>>,
    capability: TracedPromise,
}

impl<T: AsyncIterable> Callback for NextFulfilled<T> {
    fn callback(&self, cx: &mut CurrentRealm, next: HandleValue) {
        // Set object’s ongoing promise to null.
        *self.iterator.ongoing_promise.borrow_mut() = None;

        // If next is end of iteration, then:
        if self.iterator.is_end_of_iteration(next) {
            // Set object’s is finished to true.
            self.iterator.is_finished.set(true);
            // Return CreateIteratorResultObject(undefined, true).
            resolve_with_iterator_result(cx, &self.capability, UndefinedValue(), true);
            return;
        }

        // Otherwise: Return CreateIteratorResultObject(next, false).
        resolve_with_iterator_result(cx, &self.capability, next.get(), false);
    }
}

/// The rejectSteps of the nextSteps of the `next()` method.
#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct NextRejected<T: AsyncIterable> {
    iterator: Dom<AsyncIterableIterator<T>>,
    capability: TracedPromise,
}

impl<T: AsyncIterable> Callback for NextRejected<T> {
    fn callback(&self, cx: &mut CurrentRealm, reason: HandleValue) {
        // Set object’s ongoing promise to null.
        *self.iterator.ongoing_promise.borrow_mut() = None;
        // Set object’s is finished to true.
        self.iterator.is_finished.set(true);
        // Throw reason.
        self.capability.reject(cx, reason);
    }
}

/// The fulfillSteps of the `return()` method.
#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct ReturnFulfilled {
    #[ignore_malloc_size_of = "mozjs"]
    value: Box<Heap<JSVal>>,
    capability: TracedPromise,
}

impl Callback for ReturnFulfilled {
    fn callback(&self, cx: &mut CurrentRealm, _value: HandleValue) {
        // Return CreateIteratorResultObject(value, true).
        resolve_with_iterator_result(cx, &self.capability, self.value.get(), true);
    }
}

/// Passes on a rejection to the capability, as PerformPromiseThen does for an undefined
/// onRejected.
#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct RejectCapability {
    capability: TracedPromise,
}

impl Callback for RejectCapability {
    fn callback(&self, cx: &mut CurrentRealm, reason: HandleValue) {
        self.capability.reject(cx, reason);
    }
}
