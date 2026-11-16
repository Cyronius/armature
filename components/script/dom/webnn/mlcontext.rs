/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::Cell;
use std::collections::HashSet;
use std::ptr;

use dom_struct::dom_struct;
use js::context::JSContext;
use js::jsapi::{Heap, JSObject};
use js::jsval::UndefinedValue;
use js::realm::CurrentRealm;
use js::rust::MutableHandleValue;
use js::rust::wrappers2::{JS_ClearPendingException, JS_ParseJSON};
use js::typedarray::{ArrayBuffer, ArrayBufferU8, ArrayBufferView};
use rustnn::limits::MLOpSupportLimits;
use rustnn::mlcontext::MLTensorDescriptor as RustnnTensorDescriptor;
use script_bindings::cell::DomRefCell;
use script_bindings::record::Record;
use script_bindings::reflector::{Reflector, reflect_dom_object};
use script_bindings::str::{DOMString, USVString};

use crate::dom::bindings::buffer_source::{create_buffer_source, get_buffer_source_copy};
use crate::dom::bindings::codegen::Bindings::WebNNBinding::{
    MLContextLostInfo, MLContextMethods, MLOperandDescriptor, MLTensorDescriptor,
};
use crate::dom::bindings::codegen::UnionTypes::ArrayBufferViewOrArrayBuffer;
use crate::dom::bindings::error::{Error, Fallible};
use crate::dom::bindings::refcounted::{Trusted, TrustedPromise};
use crate::dom::bindings::reflector::DomGlobal;
use crate::dom::bindings::root::DomRoot;
use crate::dom::globalscope::GlobalScope;
use crate::dom::promise::{Promise, RootedPromise, TracedPromise};
use crate::dom::webnn::backend::{self, BackendId, Request};
use crate::dom::webnn::convert::{
    MAX_TENSOR_BYTE_LENGTH, backend_error, checked_byte_length, to_rustnn_descriptor, type_error,
};
use crate::dom::webnn::mlgraph::MLGraph;
use crate::dom::webnn::mltensor::MLTensor;

/// An output buffer passed to `readTensor()`, kept alive until the read completes.
#[derive(JSTraceable)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct PendingRead {
    id: u64,
    buffer: Box<Heap<*mut JSObject>>,
}

/// <https://webmachinelearning.github.io/webnn/#api-mlcontext>
#[dom_struct]
pub(crate) struct MLContext {
    reflector_: Reflector,
    /// The rustnn context on the backend thread.
    id: BackendId,
    accelerated: bool,
    /// <https://webmachinelearning.github.io/webnn/#dom-mlcontext-lost-slot>
    lost: TracedPromise,
    is_lost: Cell<bool>,
    #[ignore_malloc_size_of = "mozjs"]
    pending_reads: DomRefCell<Vec<PendingRead>>,
}

impl MLContext {
    fn new_inherited(id: BackendId, accelerated: bool, lost: &RootedPromise) -> Self {
        Self {
            reflector_: Reflector::new(),
            id,
            accelerated,
            lost: lost.to_traced(),
            is_lost: Cell::new(false),
            pending_reads: Default::default(),
        }
    }

    pub(crate) fn new(
        cx: &mut JSContext,
        global: &GlobalScope,
        id: BackendId,
        accelerated: bool,
    ) -> DomRoot<Self> {
        let lost = Promise::new(cx, global);
        reflect_dom_object(
            cx,
            Box::new(Self::new_inherited(id, accelerated, &lost)),
            global,
        )
    }

    pub(crate) fn backend_id(&self) -> BackendId {
        self.id
    }

    pub(crate) fn is_lost(&self) -> bool {
        self.is_lost.get()
    }

    pub(crate) fn check_not_lost(&self) -> Fallible<()> {
        if self.is_lost() {
            return Err(Error::InvalidState(Some(
                "The MLContext has been lost".to_owned(),
            )));
        }
        Ok(())
    }

    fn check_tensor(&self, tensor: &MLTensor) -> Fallible<()> {
        if tensor.context().backend_id() != self.id {
            return Err(type_error("The MLTensor belongs to another MLContext"));
        }
        if tensor.is_destroyed() {
            return Err(type_error("The MLTensor has been destroyed"));
        }
        Ok(())
    }

    /// Create a tensor on the backend and resolve `promise` with it.
    fn create_tensor(
        &self,
        promise: &RootedPromise,
        descriptor: &MLOperandDescriptor,
        readable: bool,
        writable: bool,
        constant_data: Option<Vec<u8>>,
    ) -> Fallible<()> {
        let rustnn_descriptor = to_rustnn_descriptor(descriptor)?;
        let mut tensor_descriptor =
            RustnnTensorDescriptor::from_operand_descriptor(&rustnn_descriptor);
        tensor_descriptor.set_readable(readable);
        // A constant tensor is written once, on the backend, when it's created.
        tensor_descriptor.set_writable(writable || constant_data.is_some());

        let id = backend::next_id();
        let data_type = descriptor.dataType;
        let shape = descriptor.shape.clone();
        let trusted_promise = TrustedPromise::new(promise);
        let this = Trusted::new(self);
        let task_source = self
            .global()
            .task_manager()
            .dom_manipulation_task_source()
            .to_sendable();
        backend::send(Request::CreateTensor {
            context: self.id,
            id,
            descriptor: tensor_descriptor,
            data: constant_data.clone(),
            reply: Box::new(move |result| {
                task_source.queue(task!(webnn_tensor_created: move |cx| {
                    let promise = trusted_promise.root(cx);
                    let context = this.root();
                    match result {
                        Ok(()) => {
                            let tensor = MLTensor::new(
                                cx,
                                &context.global(),
                                &context,
                                id,
                                data_type,
                                shape,
                                readable,
                                writable,
                                constant_data,
                            );
                            promise.resolve_native(cx, &tensor);
                        },
                        Err(error) => promise.reject_error(cx, backend_error(error)),
                    }
                }));
            }),
        });
        Ok(())
    }

    /// <https://webmachinelearning.github.io/webnn/#dom-mlcontext-readtensor>
    #[expect(unsafe_code)]
    fn read_tensor(
        &self,
        realm: &mut CurrentRealm,
        tensor: &MLTensor,
        output: Option<&ArrayBufferViewOrArrayBuffer>,
    ) -> RootedPromise {
        let promise = Promise::new_in_realm(realm);
        if let Err(error) = self
            .check_not_lost()
            .and_then(|_| self.check_tensor(tensor))
        {
            promise.reject_error(realm, error);
            return promise;
        }
        if !tensor.readable() {
            promise.reject_error(realm, type_error("The MLTensor is not readable"));
            return promise;
        }

        let read_id = backend::next_id();
        if let Some(output) = output {
            let (object, length) = match output {
                ArrayBufferViewOrArrayBuffer::ArrayBufferView(view) => {
                    // Safety: the object is only read, to keep it alive until the read ends.
                    (unsafe { view.underlying_object().get() }, view.len())
                },
                ArrayBufferViewOrArrayBuffer::ArrayBuffer(buffer) => {
                    (unsafe { buffer.underlying_object().get() }, buffer.len())
                },
            };
            if (length as u64) < tensor.byte_length() {
                promise.reject_error(realm, type_error("The output buffer is too small"));
                return promise;
            }
            self.pending_reads.borrow_mut().push(PendingRead {
                id: read_id,
                buffer: Heap::boxed(object),
            });
        }
        let has_output = output.is_some();

        let trusted_promise = TrustedPromise::new(&promise);
        let this = Trusted::new(self);
        let task_source = self
            .global()
            .task_manager()
            .dom_manipulation_task_source()
            .to_sendable();
        backend::send(Request::ReadTensor {
            context: self.id,
            tensor: tensor.backend_id(),
            reply: Box::new(move |result| {
                task_source.queue(task!(webnn_tensor_read: move |cx| {
                    let promise = trusted_promise.root(cx);
                    let context = this.root();
                    let data = match result {
                        Ok(data) => data,
                        Err(error) => {
                            if has_output {
                                let _ = context.finish_pending_read(cx, read_id, None);
                            }
                            promise.reject_error(cx, backend_error(error));
                            return;
                        },
                    };
                    if has_output {
                        match context.finish_pending_read(cx, read_id, Some(&data)) {
                            Ok(()) => promise.resolve_native(cx, &()),
                            Err(error) => promise.reject_error(cx, error),
                        }
                    } else {
                            rooted!(&in(cx) let mut array_buffer = ptr::null_mut::<JSObject>());
                            match create_buffer_source::<ArrayBufferU8>(
                                cx,
                                &data,
                                array_buffer.handle_mut(),
                            ) {
                                Ok(_) => promise.resolve_native(cx, &*array_buffer),
                                Err(_) => promise.reject_error(cx, Error::JSFailed),
                            }
                    }
                }));
            }),
        });
        promise
    }

    /// Copy `data` into the output buffer of the pending read `id`, if any, and forget the
    /// buffer. The buffer is written in place so that it stays rooted by the traced list.
    fn finish_pending_read(
        &self,
        cx: &mut JSContext,
        id: u64,
        data: Option<&[u8]>,
    ) -> Fallible<()> {
        let mut pending_reads = self.pending_reads.borrow_mut();
        let Some(index) = pending_reads.iter().position(|read| read.id == id) else {
            return Err(Error::JSFailed);
        };
        let result = match data {
            Some(data) => write_into_buffer(cx, pending_reads[index].buffer.get(), data),
            None => Ok(()),
        };
        pending_reads.swap_remove(index);
        result
    }

    /// Check a `dispatch()` binding against the graph's inputs or outputs.
    fn validate_bindings(
        &self,
        tensors: &Record<USVString, DomRoot<MLTensor>>,
        expected: &[super::mlgraph::GraphOperand],
        all_tensors: &mut HashSet<BackendId>,
    ) -> Fallible<Vec<(String, BackendId)>> {
        if tensors.len() != expected.len() {
            return Err(type_error(format!(
                "Expected {} tensors but got {}",
                expected.len(),
                tensors.len()
            )));
        }
        tensors
            .iter()
            .map(|(name, tensor)| {
                let operand = expected
                    .iter()
                    .find(|operand| operand.name == name.0)
                    .ok_or_else(|| {
                        type_error(format!("The graph has no operand named {:?}", name.0))
                    })?;
                self.check_tensor(tensor)?;
                if tensor.data_type() != operand.data_type || *tensor.shape() != operand.shape {
                    return Err(type_error(format!(
                        "The MLTensor bound to {:?} doesn't match the graph's operand",
                        name.0
                    )));
                }
                if !all_tensors.insert(tensor.backend_id()) {
                    return Err(type_error("An MLTensor is bound more than once"));
                }
                Ok((name.0.clone(), tensor.backend_id()))
            })
            .collect()
    }
}

/// Copy `data` into the ArrayBuffer or ArrayBufferView `object`.
fn write_into_buffer(cx: &mut JSContext, object: *mut JSObject, data: &[u8]) -> Fallible<()> {
    let detached = || type_error("The output buffer was detached");
    if let Ok(mut view) = ArrayBufferView::from(object) {
        let slice = view.as_mut_slice_safe(cx.no_gc()).ok_or_else(detached)?;
        if slice.len() < data.len() {
            return Err(detached());
        }
        slice[..data.len()].copy_from_slice(data);
        return Ok(());
    }
    let mut buffer = ArrayBuffer::from(object).map_err(|_| detached())?;
    let slice = buffer.as_mut_slice_safe(cx.no_gc()).ok_or_else(detached)?;
    if slice.len() < data.len() {
        return Err(detached());
    }
    slice[..data.len()].copy_from_slice(data);
    Ok(())
}

impl MLContextMethods<crate::DomTypeHolder> for MLContext {
    /// <https://webmachinelearning.github.io/webnn/#dom-mlcontext-dispatch>
    fn Dispatch(
        &self,
        _cx: &mut JSContext,
        graph: &MLGraph,
        inputs: Record<USVString, DomRoot<MLTensor>>,
        outputs: Record<USVString, DomRoot<MLTensor>>,
    ) -> Fallible<()> {
        if graph.context().backend_id() != self.id {
            return Err(type_error("The MLGraph belongs to another MLContext"));
        }
        self.check_not_lost()?;
        if graph.is_destroyed() {
            return Err(Error::InvalidState(Some(
                "The MLGraph has been destroyed".to_owned(),
            )));
        }
        let mut all_tensors = HashSet::new();
        let inputs = self.validate_bindings(&inputs, graph.inputs(), &mut all_tensors)?;
        let outputs = self.validate_bindings(&outputs, graph.outputs(), &mut all_tensors)?;
        backend::send(Request::Dispatch {
            context: self.id,
            graph: graph.backend_id(),
            inputs,
            outputs,
        });
        Ok(())
    }

    /// <https://webmachinelearning.github.io/webnn/#dom-mlcontext-createtensor>
    fn CreateTensor(
        &self,
        realm: &mut CurrentRealm,
        descriptor: &MLTensorDescriptor,
    ) -> RootedPromise {
        let promise = Promise::new_in_realm(realm);
        let result = self.check_not_lost().and_then(|_| {
            self.create_tensor(
                &promise,
                &descriptor.parent,
                descriptor.readable,
                descriptor.writable,
                None,
            )
        });
        if let Err(error) = result {
            promise.reject_error(realm, error);
        }
        promise
    }

    /// <https://webmachinelearning.github.io/webnn/#dom-mlcontext-createconstanttensor>
    fn CreateConstantTensor(
        &self,
        realm: &mut CurrentRealm,
        descriptor: &MLOperandDescriptor,
        input_data: ArrayBufferViewOrArrayBuffer,
    ) -> RootedPromise {
        let promise = Promise::new_in_realm(realm);
        if let Err(error) = self.check_not_lost() {
            promise.reject_error(realm, error);
            return promise;
        }
        let byte_length = match checked_byte_length(descriptor) {
            Ok(length) => length,
            Err(error) => {
                promise.reject_error(realm, error);
                return promise;
            },
        };
        let data = get_buffer_source_copy((&input_data).into());
        if data.len() as u64 != byte_length {
            promise.reject_error(
                realm,
                type_error("The data size doesn't match the descriptor"),
            );
            return promise;
        }
        if let Err(error) = self.create_tensor(&promise, descriptor, false, false, Some(data)) {
            promise.reject_error(realm, error);
        }
        promise
    }

    /// <https://webmachinelearning.github.io/webnn/#dom-mlcontext-readtensor>
    fn ReadTensor(&self, realm: &mut CurrentRealm, tensor: &MLTensor) -> RootedPromise {
        self.read_tensor(realm, tensor, None)
    }

    /// <https://webmachinelearning.github.io/webnn/#dom-mlcontext-readtensor-tensor-outputdata>
    fn ReadTensor_(
        &self,
        realm: &mut CurrentRealm,
        tensor: &MLTensor,
        output_data: ArrayBufferViewOrArrayBuffer,
    ) -> RootedPromise {
        self.read_tensor(realm, tensor, Some(&output_data))
    }

    /// <https://webmachinelearning.github.io/webnn/#dom-mlcontext-writetensor>
    fn WriteTensor(
        &self,
        _cx: &mut JSContext,
        tensor: &MLTensor,
        input_data: ArrayBufferViewOrArrayBuffer,
    ) -> Fallible<()> {
        self.check_not_lost()?;
        self.check_tensor(tensor)?;
        if !tensor.writable() {
            return Err(type_error("The MLTensor is not writable"));
        }
        let data = get_buffer_source_copy((&input_data).into());
        if data.len() as u64 != tensor.byte_length() {
            return Err(type_error("The data size doesn't match the MLTensor"));
        }
        backend::send(Request::WriteTensor {
            context: self.id,
            tensor: tensor.backend_id(),
            data,
        });
        Ok(())
    }

    /// <https://webmachinelearning.github.io/webnn/#dom-mlcontext-opsupportlimits>
    ///
    /// rustnn's limits serialize to the dictionary's JSON form, so the dictionary is built by
    /// parsing that JSON rather than member by member.
    #[expect(unsafe_code)]
    fn OpSupportLimits(&self, cx: &mut JSContext, mut retval: MutableHandleValue) {
        let limits = MLOpSupportLimits {
            max_tensor_byte_length: MAX_TENSOR_BYTE_LENGTH,
            ..Default::default()
        };
        // The bindings don't expose rustnn's int4 and uint4.
        let json = serde_json::to_string(&limits).unwrap_or_else(|_| "{}".to_owned());
        let json = json.replace(r#""int4","#, "").replace(r#","int4""#, "");
        let json = json.replace(r#""uint4","#, "").replace(r#","uint4""#, "");
        // serde spells these gatherNd and scatterNd.
        let json = json
            .replace(r#""gatherNd":"#, r#""gatherND":"#)
            .replace(r#""scatterNd":"#, r#""scatterND":"#);
        let json: Vec<u16> = json.encode_utf16().collect();
        rooted!(&in(cx) let mut value = UndefinedValue());
        unsafe {
            if !JS_ParseJSON(cx, json.as_ptr(), json.len() as u32, value.handle_mut()) {
                JS_ClearPendingException(cx);
            }
        }
        retval.set(value.get());
    }

    /// <https://webmachinelearning.github.io/webnn/#dom-mlcontext-destroy>
    fn Destroy(&self, cx: &mut JSContext) {
        if self.is_lost.replace(true) {
            return;
        }
        backend::send(Request::DestroyContext { id: self.id });
        let info = MLContextLostInfo {
            message: Some(DOMString::from_static("The MLContext was destroyed")),
        };
        self.lost.root(cx).resolve_native(cx, &info);
    }

    /// <https://webmachinelearning.github.io/webnn/#dom-mlcontext-accelerated>
    fn Accelerated(&self) -> bool {
        self.accelerated
    }

    /// <https://webmachinelearning.github.io/webnn/#dom-mlcontext-lost>
    fn Lost(&self, cx: &mut JSContext) -> RootedPromise {
        self.lost.root(cx)
    }
}
