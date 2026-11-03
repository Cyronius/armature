/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! <https://streams.spec.whatwg.org/#rs-asynciterator>

use js::context::JSContext;
use js::rust::HandleValue as SafeHandleValue;

use crate::dom::bindings::asynciterable::{AsyncIterable, AsyncIterableIterator};
use crate::dom::bindings::reflector::DomGlobal;
use crate::dom::bindings::root::Dom;
use crate::dom::promise::{Promise, RootedPromise};
use crate::dom::readablestream::ReadableStream;
use crate::dom::stream::readablestreamdefaultreader::{ReadRequest, ReadableStreamDefaultReader};
use crate::dom::stream::readablestreamgenericreader::ReadableStreamGenericReader;

/// The internal slots of an asynchronous iterator of a [`ReadableStream`].
#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
pub(crate) struct ReadableStreamAsyncIteratorState {
    /// <https://streams.spec.whatwg.org/#readablestream-async-iterator-reader>
    reader: Dom<ReadableStreamDefaultReader>,

    /// <https://streams.spec.whatwg.org/#readablestream-async-iterator-prevent-cancel>
    prevent_cancel: bool,
}

impl ReadableStreamAsyncIteratorState {
    /// Steps 2 and 4 of the asynchronous iterator initialization steps of
    /// <https://streams.spec.whatwg.org/#rs-asynciterator>.
    pub(crate) fn new(
        reader: &ReadableStreamDefaultReader,
        prevent_cancel: bool,
    ) -> ReadableStreamAsyncIteratorState {
        ReadableStreamAsyncIteratorState {
            reader: Dom::from_ref(reader),
            prevent_cancel,
        }
    }

    pub(crate) fn reader(&self) -> &ReadableStreamDefaultReader {
        &self.reader
    }
}

impl AsyncIterable for ReadableStream {
    type IteratorState = ReadableStreamAsyncIteratorState;

    /// The get the next iteration result steps of
    /// <https://streams.spec.whatwg.org/#rs-asynciterator>.
    fn get_next_iteration_result(
        &self,
        cx: &mut JSContext,
        iterator: &AsyncIterableIterator<Self>,
    ) -> RootedPromise {
        // Let reader be iterator’s reader.
        let reader = iterator.state().reader();

        // Assert: reader.[[stream]] is not undefined.
        assert!(reader.get_stream().is_some());

        // Let promise be a new promise.
        let promise = Promise::new(cx, &self.global());

        // Let readRequest be a new read request with the following items:
        // chunk steps, given chunk: Resolve promise with chunk.
        // close steps: Perform ! ReadableStreamDefaultReaderRelease(reader). Resolve promise with
        // end of iteration.
        // error steps, given e: Perform ! ReadableStreamDefaultReaderRelease(reader). Reject
        // promise with e.
        rooted!(&in(cx) let read_request = ReadRequest::AsyncIterator {
            promise: promise.to_traced(),
            iterator: Dom::from_ref(iterator),
        });

        // Perform ! ReadableStreamDefaultReaderRead(this, readRequest).
        reader.read(cx, &read_request);

        // Return promise.
        promise
    }

    /// The asynchronous iterator return steps of
    /// <https://streams.spec.whatwg.org/#rs-asynciterator>.
    fn async_iterator_return(
        &self,
        cx: &mut JSContext,
        iterator: &AsyncIterableIterator<Self>,
        arg: SafeHandleValue,
    ) -> RootedPromise {
        // Let reader be iterator’s reader.
        let state = iterator.state();
        let reader = state.reader();

        // Assert: reader.[[stream]] is not undefined.
        assert!(reader.get_stream().is_some());

        // Assert: reader.[[readRequests]] is empty, as the async iterator machinery guarantees
        // that any previous calls to next() have settled before this is called.
        assert_eq!(reader.get_num_read_requests(), 0);

        let global = self.global();

        // If iterator’s prevent cancel is false:
        if !state.prevent_cancel {
            // Let result be ! ReadableStreamReaderGenericCancel(reader, arg).
            let result = reader.reader_generic_cancel(cx, &global, arg);

            // Perform ! ReadableStreamDefaultReaderRelease(reader).
            reader
                .release(cx)
                .expect("Releasing the reader of an async iterator should succeed");

            // Return result.
            return result;
        }

        // Perform ! ReadableStreamDefaultReaderRelease(reader).
        reader
            .release(cx)
            .expect("Releasing the reader of an async iterator should succeed");

        // Return a promise resolved with undefined.
        Promise::new_resolved(cx, &global, ())
    }
}
