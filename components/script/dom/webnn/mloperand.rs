/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use js::context::JSContext;
use js::rust::MutableHandleValue;
use script_bindings::reflector::{Reflector, reflect_dom_object};

use crate::dom::bindings::codegen::Bindings::WebNNBinding::{MLOperandDataType, MLOperandMethods};
use crate::dom::bindings::frozenarray::CachedFrozenArray;
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::globalscope::GlobalScope;
use crate::dom::webnn::mlgraphbuilder::MLGraphBuilder;

/// The kind of an operand, which `build()` checks.
#[derive(Clone, Copy, JSTraceable, MallocSizeOf, PartialEq)]
pub(crate) enum OperandKind {
    Input,
    Constant,
    /// The result of an operation.
    Operation,
}

/// <https://webmachinelearning.github.io/webnn/#api-mloperand>
#[dom_struct]
pub(crate) struct MLOperand {
    reflector_: Reflector,
    /// <https://webmachinelearning.github.io/webnn/#dom-mloperand-builder-slot>
    builder: Dom<MLGraphBuilder>,
    /// The operand's index in the rustnn graph.
    index: u32,
    kind: OperandKind,
    data_type: MLOperandDataType,
    shape: Vec<u32>,
    #[ignore_malloc_size_of = "mozjs"]
    frozen_shape: CachedFrozenArray,
}

impl MLOperand {
    fn new_inherited(
        builder: &MLGraphBuilder,
        index: u32,
        kind: OperandKind,
        data_type: MLOperandDataType,
        shape: Vec<u32>,
    ) -> Self {
        Self {
            reflector_: Reflector::new(),
            builder: Dom::from_ref(builder),
            index,
            kind,
            data_type,
            shape,
            frozen_shape: CachedFrozenArray::new(),
        }
    }

    pub(crate) fn new(
        cx: &mut JSContext,
        global: &GlobalScope,
        builder: &MLGraphBuilder,
        index: u32,
        kind: OperandKind,
        data_type: MLOperandDataType,
        shape: Vec<u32>,
    ) -> DomRoot<Self> {
        reflect_dom_object(
            cx,
            Box::new(Self::new_inherited(builder, index, kind, data_type, shape)),
            global,
        )
    }

    pub(crate) fn builder(&self) -> DomRoot<MLGraphBuilder> {
        self.builder.as_rooted()
    }

    pub(crate) fn index(&self) -> u32 {
        self.index
    }

    pub(crate) fn kind(&self) -> OperandKind {
        self.kind
    }

    pub(crate) fn data_type(&self) -> MLOperandDataType {
        self.data_type
    }

    pub(crate) fn shape(&self) -> &Vec<u32> {
        &self.shape
    }
}

impl MLOperandMethods<crate::DomTypeHolder> for MLOperand {
    /// <https://webmachinelearning.github.io/webnn/#dom-mloperand-datatype>
    fn DataType(&self) -> MLOperandDataType {
        self.data_type
    }

    /// <https://webmachinelearning.github.io/webnn/#dom-mloperand-shape>
    fn Shape(&self, cx: &mut JSContext, retval: MutableHandleValue) {
        self.frozen_shape
            .get_or_init(cx, || self.shape.clone(), retval);
    }
}
