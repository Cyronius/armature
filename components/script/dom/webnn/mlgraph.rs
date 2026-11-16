/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::Cell;

use dom_struct::dom_struct;
use js::context::JSContext;
use script_bindings::reflector::{Reflector, reflect_dom_object};

use crate::dom::bindings::codegen::Bindings::WebNNBinding::{MLGraphMethods, MLOperandDataType};
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::globalscope::GlobalScope;
use crate::dom::webnn::backend::{self, BackendId, Request};
use crate::dom::webnn::mlcontext::MLContext;

/// A named input or output of a compiled graph, which `dispatch()` checks tensors against.
#[derive(JSTraceable, MallocSizeOf)]
pub(crate) struct GraphOperand {
    /// The operand's index in the rustnn graph.
    pub(crate) index: u32,
    pub(crate) name: String,
    pub(crate) data_type: MLOperandDataType,
    pub(crate) shape: Vec<u32>,
}

/// <https://webmachinelearning.github.io/webnn/#api-mlgraph>
#[dom_struct]
pub(crate) struct MLGraph {
    reflector_: Reflector,
    context: Dom<MLContext>,
    /// The compiled rustnn graph on the backend thread.
    id: BackendId,
    /// <https://webmachinelearning.github.io/webnn/#dom-mlgraph-inputdescriptors-slot>
    inputs: Vec<GraphOperand>,
    /// <https://webmachinelearning.github.io/webnn/#dom-mlgraph-outputdescriptors-slot>
    outputs: Vec<GraphOperand>,
    destroyed: Cell<bool>,
}

impl MLGraph {
    fn new_inherited(
        context: &MLContext,
        id: BackendId,
        inputs: Vec<GraphOperand>,
        outputs: Vec<GraphOperand>,
    ) -> Self {
        Self {
            reflector_: Reflector::new(),
            context: Dom::from_ref(context),
            id,
            inputs,
            outputs,
            destroyed: Cell::new(false),
        }
    }

    pub(crate) fn new(
        cx: &mut JSContext,
        global: &GlobalScope,
        context: &MLContext,
        id: BackendId,
        inputs: Vec<GraphOperand>,
        outputs: Vec<GraphOperand>,
    ) -> DomRoot<Self> {
        reflect_dom_object(
            cx,
            Box::new(Self::new_inherited(context, id, inputs, outputs)),
            global,
        )
    }

    pub(crate) fn context(&self) -> DomRoot<MLContext> {
        self.context.as_rooted()
    }

    pub(crate) fn backend_id(&self) -> BackendId {
        self.id
    }

    pub(crate) fn inputs(&self) -> &[GraphOperand] {
        &self.inputs
    }

    pub(crate) fn outputs(&self) -> &[GraphOperand] {
        &self.outputs
    }

    pub(crate) fn is_destroyed(&self) -> bool {
        self.destroyed.get()
    }
}

impl MLGraphMethods<crate::DomTypeHolder> for MLGraph {
    /// <https://webmachinelearning.github.io/webnn/#dom-mlgraph-destroy>
    fn Destroy(&self, _cx: &mut JSContext) {
        if self.destroyed.replace(true) {
            return;
        }
        backend::send(Request::DestroyGraph { id: self.id });
    }
}
