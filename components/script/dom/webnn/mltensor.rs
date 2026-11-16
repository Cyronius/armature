/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::Cell;

use dom_struct::dom_struct;
use js::context::JSContext;
use js::rust::MutableHandleValue;
use script_bindings::reflector::{Reflector, reflect_dom_object};

use crate::dom::bindings::codegen::Bindings::WebNNBinding::{MLOperandDataType, MLTensorMethods};
use crate::dom::bindings::frozenarray::CachedFrozenArray;
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::globalscope::GlobalScope;
use crate::dom::webnn::backend::{self, BackendId, Request};
use crate::dom::webnn::convert::element_size;
use crate::dom::webnn::mlcontext::MLContext;

/// <https://webmachinelearning.github.io/webnn/#api-mltensor>
#[dom_struct]
pub(crate) struct MLTensor {
    reflector_: Reflector,
    context: Dom<MLContext>,
    /// The rustnn tensor on the backend thread.
    id: BackendId,
    data_type: MLOperandDataType,
    shape: Vec<u32>,
    readable: bool,
    writable: bool,
    /// For a constant tensor, its contents, which `MLGraphBuilder.constant(tensor)` copies into
    /// the graph.
    constant_data: Option<Vec<u8>>,
    destroyed: Cell<bool>,
    #[ignore_malloc_size_of = "mozjs"]
    frozen_shape: CachedFrozenArray,
}

impl MLTensor {
    #[expect(clippy::too_many_arguments)]
    fn new_inherited(
        context: &MLContext,
        id: BackendId,
        data_type: MLOperandDataType,
        shape: Vec<u32>,
        readable: bool,
        writable: bool,
        constant_data: Option<Vec<u8>>,
    ) -> Self {
        Self {
            reflector_: Reflector::new(),
            context: Dom::from_ref(context),
            id,
            data_type,
            shape,
            readable,
            writable,
            constant_data,
            destroyed: Cell::new(false),
            frozen_shape: CachedFrozenArray::new(),
        }
    }

    #[expect(clippy::too_many_arguments)]
    pub(crate) fn new(
        cx: &mut JSContext,
        global: &GlobalScope,
        context: &MLContext,
        id: BackendId,
        data_type: MLOperandDataType,
        shape: Vec<u32>,
        readable: bool,
        writable: bool,
        constant_data: Option<Vec<u8>>,
    ) -> DomRoot<Self> {
        reflect_dom_object(
            cx,
            Box::new(Self::new_inherited(
                context,
                id,
                data_type,
                shape,
                readable,
                writable,
                constant_data,
            )),
            global,
        )
    }

    pub(crate) fn context(&self) -> DomRoot<MLContext> {
        self.context.as_rooted()
    }

    pub(crate) fn backend_id(&self) -> BackendId {
        self.id
    }

    pub(crate) fn data_type(&self) -> MLOperandDataType {
        self.data_type
    }

    pub(crate) fn shape(&self) -> &Vec<u32> {
        &self.shape
    }

    pub(crate) fn readable(&self) -> bool {
        self.readable
    }

    pub(crate) fn writable(&self) -> bool {
        self.writable
    }

    pub(crate) fn constant_data(&self) -> Option<&[u8]> {
        self.constant_data.as_deref()
    }

    pub(crate) fn is_destroyed(&self) -> bool {
        self.destroyed.get()
    }

    pub(crate) fn byte_length(&self) -> u64 {
        self.shape
            .iter()
            .fold(element_size(self.data_type), |length, &dimension| {
                length * dimension as u64
            })
    }
}

impl MLTensorMethods<crate::DomTypeHolder> for MLTensor {
    /// <https://webmachinelearning.github.io/webnn/#dom-mltensor-datatype>
    fn DataType(&self) -> MLOperandDataType {
        self.data_type
    }

    /// <https://webmachinelearning.github.io/webnn/#dom-mltensor-shape>
    fn Shape(&self, cx: &mut JSContext, retval: MutableHandleValue) {
        self.frozen_shape
            .get_or_init(cx, || self.shape.clone(), retval);
    }

    /// <https://webmachinelearning.github.io/webnn/#dom-mltensor-readable>
    fn Readable(&self) -> bool {
        self.readable
    }

    /// <https://webmachinelearning.github.io/webnn/#dom-mltensor-writable>
    fn Writable(&self) -> bool {
        self.writable
    }

    /// <https://webmachinelearning.github.io/webnn/#dom-mltensor-constant>
    fn Constant(&self) -> bool {
        self.constant_data.is_some()
    }

    /// <https://webmachinelearning.github.io/webnn/#dom-mltensor-destroy>
    fn Destroy(&self, _cx: &mut JSContext) {
        if self.destroyed.replace(true) {
            return;
        }
        backend::send(Request::DestroyTensor { id: self.id });
    }
}
