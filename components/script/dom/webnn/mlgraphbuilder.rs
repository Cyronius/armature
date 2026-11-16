/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::Cell;
use std::collections::BTreeMap;

use dom_struct::dom_struct;
use js::context::JSContext;
use js::conversions::{ConversionResult, FromJSValConvertible};
use js::realm::CurrentRealm;
use js::rust::{HandleObject, HandleValue};
use rustnn::mlcontext::MLOperand as RustnnOperand;
use rustnn::mlgraphbuilder::MLGraphBuilder as RustnnBuilder;
use rustnn::operator_options::{MLDimension, MLOperatorOptions as RustnnOperatorOptions};
use script_bindings::cell::DomRefCell;
use script_bindings::record::Record;
use script_bindings::reflector::{Reflector, reflect_dom_object_with_proto};
use script_bindings::str::USVString;
use servo_base::panic::catch_unwind_without_abort;

use crate::dom::bindings::buffer_source::get_buffer_source_copy;
use crate::dom::bindings::codegen::Bindings::WebNNBinding::{
    MLArgMinMaxOptions, MLBatchNormalizationOptions, MLClampOptions, MLConv2dOptions,
    MLConvTranspose2dOptions, MLCumulativeSumOptions, MLEluOptions, MLGatherOptions, MLGemmOptions,
    MLGraphBuilderMethods, MLGruCellOptions, MLGruOptions, MLHardSigmoidOptions,
    MLInstanceNormalizationOptions, MLLayerNormalizationOptions, MLLeakyReluOptions,
    MLLinearOptions, MLLstmCellOptions, MLLstmOptions, MLOperandDataType, MLOperandDescriptor,
    MLOperatorOptions, MLPadOptions, MLPool2dOptions, MLReduceOptions, MLResample2dOptions,
    MLReverseOptions, MLScatterOptions, MLSliceOptions, MLSplitOptions, MLTransposeOptions,
    MLTriangularOptions,
};
use crate::dom::bindings::codegen::UnionTypes::{
    ArrayBufferViewOrArrayBuffer, RangeEnforcedUnsignedLongOrRangeEnforcedUnsignedLongSequence,
};
use crate::dom::bindings::error::{Error, Fallible};
use crate::dom::bindings::refcounted::{Trusted, TrustedPromise};
use crate::dom::bindings::reflector::DomGlobal;
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::globalscope::GlobalScope;
use crate::dom::promise::{Promise, RootedPromise};
use crate::dom::webnn::backend::{self, Request};
use crate::dom::webnn::convert::{
    backend_error, checked_byte_length, from_rustnn_data_type, to_rustnn_data_type,
    to_rustnn_descriptor, type_error,
};
use crate::dom::webnn::mlcontext::MLContext;
use crate::dom::webnn::mlgraph::{GraphOperand, MLGraph};
use crate::dom::webnn::mloperand::{MLOperand, OperandKind};
use crate::dom::webnn::mltensor::MLTensor;
use crate::dom::webnn::options;

/// <https://webmachinelearning.github.io/webnn/#api-mlgraphbuilder>
///
/// Operations are recorded into a rustnn builder that has no backend. rustnn validates each
/// operation and infers its output shape as it's recorded, so errors are reported by the method
/// that caused them. `build()` hands the recorded graph to the backend thread to compile.
#[dom_struct]
pub(crate) struct MLGraphBuilder {
    reflector_: Reflector,
    context: Dom<MLContext>,
    #[no_trace]
    #[ignore_malloc_size_of = "Defined in rustnn"]
    builder: DomRefCell<RustnnBuilder<'static, 'static>>,
    /// <https://webmachinelearning.github.io/webnn/#dom-mlgraphbuilder-hasbuilt-slot>
    has_built: Cell<bool>,
    /// The graph inputs declared so far.
    inputs: DomRefCell<Vec<GraphOperand>>,
}

impl MLGraphBuilder {
    fn new_inherited(context: &MLContext) -> Self {
        Self {
            reflector_: Reflector::new(),
            context: Dom::from_ref(context),
            builder: DomRefCell::new(RustnnBuilder::new_uncompiled()),
            has_built: Cell::new(false),
            inputs: Default::default(),
        }
    }

    /// <https://webmachinelearning.github.io/webnn/#mlgraphbuilder-can-build>
    fn check_can_build(&self) -> Fallible<()> {
        if self.has_built.get() {
            return Err(Error::InvalidState(Some(
                "The MLGraphBuilder has already built a graph".to_owned(),
            )));
        }
        self.context.check_not_lost()
    }

    /// The rustnn operand for `operand`, which must come from this builder.
    pub(crate) fn operand(&self, operand: &MLOperand) -> Fallible<RustnnOperand> {
        if *operand.builder() != *self {
            return Err(type_error(
                "The MLOperand belongs to another MLGraphBuilder",
            ));
        }
        Ok(RustnnOperand::from(operand.index()))
    }

    /// Call into the rustnn builder. rustnn validates operations by recording them; a panic in
    /// that code is reported as a `TypeError` instead of taking down the script thread.
    fn with_builder<T, E: std::fmt::Display>(
        &self,
        f: impl FnOnce(&mut RustnnBuilder<'static, 'static>) -> Result<T, E>,
    ) -> Fallible<T> {
        let mut builder = self.builder.borrow_mut();
        match catch_unwind_without_abort(|| f(&mut builder)) {
            Ok(result) => result.map_err(type_error),
            Err(_) => Err(type_error("The operation is invalid")),
        }
    }

    /// Wrap a newly recorded rustnn operand.
    fn new_operand(
        &self,
        cx: &mut JSContext,
        operand: RustnnOperand,
        kind: OperandKind,
    ) -> Fallible<DomRoot<MLOperand>> {
        let data_type = self.with_builder(|builder| builder.rustnn_operand_data_type(operand))?;
        let shape = self.with_builder(|builder| builder.rustnn_operand_shape(operand))?;
        let shape = shape
            .into_iter()
            .map(|dimension| u32::try_from(dimension).map_err(|_| type_error("Invalid shape")))
            .collect::<Fallible<_>>()?;
        Ok(MLOperand::new(
            cx,
            &self.global(),
            self,
            operand.rustnn_index(),
            kind,
            from_rustnn_data_type(data_type),
            shape,
        ))
    }

    /// Record an operation with `record`, after the checks every operation needs.
    fn operation(
        &self,
        cx: &mut JSContext,
        label: &str,
        record: impl FnOnce(
            &mut RustnnBuilder<'static, 'static>,
        ) -> rustnn::mlgraphbuilder::Result<RustnnOperand>,
    ) -> Fallible<DomRoot<MLOperand>> {
        self.check_can_build()?;
        let operand = self
            .with_builder(record)
            .map_err(|error| with_label(error, label))?;
        self.new_operand(cx, operand, OperandKind::Operation)
    }

    /// Record an operation that returns several operands.
    fn operation_with_outputs(
        &self,
        cx: &mut JSContext,
        label: &str,
        record: impl FnOnce(
            &mut RustnnBuilder<'static, 'static>,
        ) -> rustnn::mlgraphbuilder::Result<Vec<RustnnOperand>>,
    ) -> Fallible<Vec<DomRoot<MLOperand>>> {
        self.check_can_build()?;
        let operands = self
            .with_builder(record)
            .map_err(|error| with_label(error, label))?;
        operands
            .into_iter()
            .map(|operand| self.new_operand(cx, operand, OperandKind::Operation))
            .collect()
    }

    /// Record a constant operand with the given contents.
    fn constant(
        &self,
        cx: &mut JSContext,
        descriptor: &MLOperandDescriptor,
        data: Vec<u8>,
    ) -> Fallible<DomRoot<MLOperand>> {
        self.check_can_build()?;
        let byte_length = checked_byte_length(descriptor)?;
        if data.len() as u64 != byte_length {
            return Err(type_error("The data size doesn't match the descriptor"));
        }
        let rustnn_descriptor = to_rustnn_descriptor(descriptor)?;
        let operand =
            self.with_builder(|builder| builder.constant_from_bytes(&rustnn_descriptor, data))?;
        self.new_operand(cx, operand, OperandKind::Constant)
    }
}

/// Prefix an operation's error message with its label, as
/// <https://webmachinelearning.github.io/webnn/#mloperatoroptions-label> suggests, so that the
/// failing operation can be identified.
fn with_label(error: Error, label: &str) -> Error {
    match error {
        Error::Type(message) => type_error(format!("[{label}] {}", message.to_string_lossy())),
        error => error,
    }
}

pub(crate) fn operator_options(options: &MLOperatorOptions) -> RustnnOperatorOptions {
    RustnnOperatorOptions {
        label: options.label.0.clone(),
    }
}

/// <https://webmachinelearning.github.io/webnn/#cast>
///
/// Convert an MLNumber (a bigint or a double) to the bytes of a scalar of `data_type`.
#[expect(unsafe_code)]
fn number_bytes(
    cx: &mut JSContext,
    data_type: MLOperandDataType,
    value: HandleValue,
) -> Fallible<Vec<u8>> {
    enum Number {
        Double(f64),
        Int64(i64),
        Uint64(u64),
    }
    let number = if value.get().is_bigint() {
        let bigint = value.get().to_bigint();
        match data_type {
            MLOperandDataType::Int64 => Number::Int64(unsafe { js::jsapi::ToBigInt64(bigint) }),
            MLOperandDataType::Uint64 => Number::Uint64(unsafe { js::jsapi::ToBigUint64(bigint) }),
            _ => Number::Double(unsafe { js::jsapi::BigIntToNumber(bigint) }),
        }
    } else {
        match f64::from_jsval(cx, value, ()) {
            Ok(ConversionResult::Success(number)) => Number::Double(number),
            _ => return Err(Error::JSFailed),
        }
    };
    let as_f64 = |number: &Number| match *number {
        Number::Double(number) => number,
        Number::Int64(number) => number as f64,
        Number::Uint64(number) => number as f64,
    };
    Ok(match data_type {
        MLOperandDataType::Float32 => (as_f64(&number) as f32).to_le_bytes().to_vec(),
        MLOperandDataType::Float16 => half::f16::from_f64(as_f64(&number)).to_le_bytes().to_vec(),
        MLOperandDataType::Int32 => (as_f64(&number) as i32).to_le_bytes().to_vec(),
        MLOperandDataType::Uint32 => (as_f64(&number) as u32).to_le_bytes().to_vec(),
        MLOperandDataType::Int8 => (as_f64(&number) as i8).to_le_bytes().to_vec(),
        MLOperandDataType::Uint8 => (as_f64(&number) as u8).to_le_bytes().to_vec(),
        MLOperandDataType::Int64 => match number {
            Number::Int64(number) => number,
            Number::Uint64(number) => number as i64,
            Number::Double(number) => number as i64,
        }
        .to_le_bytes()
        .to_vec(),
        MLOperandDataType::Uint64 => match number {
            Number::Uint64(number) => number,
            Number::Int64(number) => number as u64,
            Number::Double(number) => number as u64,
        }
        .to_le_bytes()
        .to_vec(),
    })
}

/// Generates `MLGraphBuilder` methods for operations that take two operands and
/// `MLOperatorOptions`, from rustnn's `*_with_options` methods.
macro_rules! binary_operations {
    ($($name:ident => $method:ident),* $(,)?) => {
        $(
            fn $name(
                &self,
                cx: &mut JSContext,
                a: &MLOperand,
                b: &MLOperand,
                options: &MLOperatorOptions,
            ) -> Fallible<DomRoot<MLOperand>> {
                let a = self.operand(a)?;
                let b = self.operand(b)?;
                let options = operator_options(options);
                self.operation(cx, &options.label.clone(), |builder| builder.$method(a, b, options))
            }
        )*
    };
}

/// Generates `MLGraphBuilder` methods for operations that take one operand and
/// `MLOperatorOptions`, from rustnn's `*_with_options` methods.
macro_rules! unary_operations {
    ($($name:ident => $method:ident),* $(,)?) => {
        $(
            fn $name(
                &self,
                cx: &mut JSContext,
                input: &MLOperand,
                options: &MLOperatorOptions,
            ) -> Fallible<DomRoot<MLOperand>> {
                let input = self.operand(input)?;
                let options = operator_options(options);
                self.operation(cx, &options.label.clone(), |builder| builder.$method(input, options))
            }
        )*
    };
}

/// Generates `MLGraphBuilder` methods for operations that take one operand and an options
/// dictionary converted by a function of [`options`].
macro_rules! unary_operations_with_options {
    ($($name:ident($options:ty) => $method:ident, $convert:path);* $(;)?) => {
        $(
            fn $name(
                &self,
                cx: &mut JSContext,
                input: &MLOperand,
                options: &$options,
            ) -> Fallible<DomRoot<MLOperand>> {
                let input = self.operand(input)?;
                let options = $convert(options);
                self.operation(cx, &options.label.clone(), |builder| builder.$method(input, options))
            }
        )*
    };
}

impl MLGraphBuilderMethods<crate::DomTypeHolder> for MLGraphBuilder {
    /// <https://webmachinelearning.github.io/webnn/#dom-mlgraphbuilder-constructor>
    fn Constructor(
        cx: &mut JSContext,
        global: &GlobalScope,
        proto: Option<HandleObject>,
        context: &MLContext,
    ) -> Fallible<DomRoot<MLGraphBuilder>> {
        context.check_not_lost()?;
        Ok(reflect_dom_object_with_proto(
            cx,
            Box::new(MLGraphBuilder::new_inherited(context)),
            global,
            proto,
        ))
    }

    /// <https://webmachinelearning.github.io/webnn/#dom-mlgraphbuilder-input>
    fn Input(
        &self,
        cx: &mut JSContext,
        name: USVString,
        descriptor: &MLOperandDescriptor,
    ) -> Fallible<DomRoot<MLOperand>> {
        self.check_can_build()?;
        if name.0.is_empty() {
            return Err(type_error("The input name must not be empty"));
        }
        let rustnn_descriptor = to_rustnn_descriptor(descriptor)?;
        let operand = self.with_builder(|builder| builder.input(&name.0, &rustnn_descriptor))?;
        self.inputs.borrow_mut().push(GraphOperand {
            index: operand.rustnn_index(),
            name: name.0.clone(),
            data_type: descriptor.dataType,
            shape: descriptor.shape.clone(),
        });
        self.new_operand(cx, operand, OperandKind::Input)
    }

    /// <https://webmachinelearning.github.io/webnn/#dom-mlgraphbuilder-constant>
    fn Constant(
        &self,
        cx: &mut JSContext,
        descriptor: &MLOperandDescriptor,
        buffer: ArrayBufferViewOrArrayBuffer,
    ) -> Fallible<DomRoot<MLOperand>> {
        let data = get_buffer_source_copy((&buffer).into());
        self.constant(cx, descriptor, data)
    }

    /// <https://webmachinelearning.github.io/webnn/#dom-mlgraphbuilder-constant-type-value>
    fn Constant_(
        &self,
        cx: &mut JSContext,
        data_type: MLOperandDataType,
        value: HandleValue,
    ) -> Fallible<DomRoot<MLOperand>> {
        let data = number_bytes(cx, data_type, value)?;
        let descriptor = MLOperandDescriptor {
            dataType: data_type,
            shape: vec![],
        };
        self.constant(cx, &descriptor, data)
    }

    /// <https://webmachinelearning.github.io/webnn/#dom-mlgraphbuilder-constant-tensor>
    fn Constant__(&self, cx: &mut JSContext, tensor: &MLTensor) -> Fallible<DomRoot<MLOperand>> {
        if tensor.context().backend_id() != self.context.backend_id() {
            return Err(type_error("The MLTensor belongs to another MLContext"));
        }
        if tensor.is_destroyed() {
            return Err(type_error("The MLTensor has been destroyed"));
        }
        let Some(data) = tensor.constant_data() else {
            return Err(type_error("The MLTensor is not a constant tensor"));
        };
        let descriptor = MLOperandDescriptor {
            dataType: tensor.data_type(),
            shape: tensor.shape().clone(),
        };
        self.constant(cx, &descriptor, data.to_vec())
    }

    /// <https://webmachinelearning.github.io/webnn/#dom-mlgraphbuilder-build>
    fn Build(
        &self,
        realm: &mut CurrentRealm,
        outputs: Record<USVString, DomRoot<MLOperand>>,
    ) -> RootedPromise {
        let promise = Promise::new_in_realm(realm);
        match self.start_build(&promise, &outputs) {
            Ok(()) => {},
            Err(error) => promise.reject_error(realm, error),
        }
        promise
    }

    binary_operations! {
        Add => add_with_options,
        Sub => sub_with_options,
        Mul => mul_with_options,
        Div => div_with_options,
        Max => max_with_options,
        Min => min_with_options,
        Pow => pow_with_options,
        Equal => equal_with_options,
        NotEqual => not_equal_with_options,
        Greater => greater_with_options,
        GreaterOrEqual => greater_or_equal_with_options,
        Lesser => lesser_with_options,
        LesserOrEqual => lesser_or_equal_with_options,
        LogicalAnd => logical_and_with_options,
        LogicalOr => logical_or_with_options,
        LogicalXor => logical_xor_with_options,
        Matmul => matmul_with_options,
    }

    unary_operations! {
        Abs => abs_with_options,
        Ceil => ceil_with_options,
        Cos => cos_with_options,
        Erf => erf_with_options,
        Exp => exp_with_options,
        Floor => floor_with_options,
        Identity => identity_with_options,
        Log => log_with_options,
        Neg => neg_with_options,
        Reciprocal => reciprocal_with_options,
        RoundEven => round_even_with_options,
        Sin => sin_with_options,
        Sign => sign_with_options,
        Sqrt => sqrt_with_options,
        Tan => tan_with_options,
        LogicalNot => logical_not_with_options,
        IsNaN => is_nan_with_options,
        IsInfinite => is_infinite_with_options,
        Relu => relu_with_options,
        Sigmoid => sigmoid_with_options,
        Tanh => tanh_with_options,
        Softplus => softplus_with_options,
        Softsign => softsign_with_options,
        Gelu => gelu_with_options,
        HardSwish => hard_swish_with_options,
    }

    /// <https://webmachinelearning.github.io/webnn/#api-mlgraphbuilder-where>
    fn Where(
        &self,
        cx: &mut JSContext,
        condition: &MLOperand,
        true_value: &MLOperand,
        false_value: &MLOperand,
        options: &MLOperatorOptions,
    ) -> Fallible<DomRoot<MLOperand>> {
        let condition = self.operand(condition)?;
        let true_value = self.operand(true_value)?;
        let false_value = self.operand(false_value)?;
        let options = operator_options(options);
        self.operation(cx, &options.label.clone(), |builder| {
            builder.where_with_options(condition, true_value, false_value, options)
        })
    }

    /// <https://webmachinelearning.github.io/webnn/#api-mlgraphbuilder-cast>
    fn Cast(
        &self,
        cx: &mut JSContext,
        input: &MLOperand,
        data_type: MLOperandDataType,
        options: &MLOperatorOptions,
    ) -> Fallible<DomRoot<MLOperand>> {
        let input = self.operand(input)?;
        let data_type = to_rustnn_data_type(data_type);
        let options = operator_options(options);
        self.operation(cx, &options.label.clone(), |builder| {
            builder.cast_with_options(input, data_type, options)
        })
    }

    /// <https://webmachinelearning.github.io/webnn/#api-mlgraphbuilder-softmax-method>
    fn Softmax(
        &self,
        cx: &mut JSContext,
        input: &MLOperand,
        axis: u32,
        options: &MLOperatorOptions,
    ) -> Fallible<DomRoot<MLOperand>> {
        let input = self.operand(input)?;
        let options = operator_options(options);
        self.operation(cx, &options.label.clone(), |builder| {
            builder.softmax_with_options(input, axis, options)
        })
    }
    unary_operations_with_options! {
        Elu(MLEluOptions) => elu_with_options, options::elu;
        HardSigmoid(MLHardSigmoidOptions) => hard_sigmoid_with_options, options::hard_sigmoid;
        LeakyRelu(MLLeakyReluOptions) => leaky_relu_with_options, options::leaky_relu;
        Linear(MLLinearOptions) => linear_with_options, options::linear;
        AveragePool2d(MLPool2dOptions) => average_pool2d_with_options, options::pool2d;
        L2Pool2d(MLPool2dOptions) => l2_pool2d_with_options, options::pool2d;
        MaxPool2d(MLPool2dOptions) => max_pool2d_with_options, options::pool2d;
        ReduceL1(MLReduceOptions) => reduce_l1_with_options, options::reduce;
        ReduceL2(MLReduceOptions) => reduce_l2_with_options, options::reduce;
        ReduceLogSum(MLReduceOptions) => reduce_log_sum_with_options, options::reduce;
        ReduceLogSumExp(MLReduceOptions) => reduce_log_sum_exp_with_options, options::reduce;
        ReduceMax(MLReduceOptions) => reduce_max_with_options, options::reduce;
        ReduceMean(MLReduceOptions) => reduce_mean_with_options, options::reduce;
        ReduceMin(MLReduceOptions) => reduce_min_with_options, options::reduce;
        ReduceProduct(MLReduceOptions) => reduce_product_with_options, options::reduce;
        ReduceSum(MLReduceOptions) => reduce_sum_with_options, options::reduce;
        ReduceSumSquare(MLReduceOptions) => reduce_sum_square_with_options, options::reduce;
        Resample2d(MLResample2dOptions) => resample2d_with_options, options::resample2d;
        Reverse(MLReverseOptions) => reverse_with_options, options::reverse;
        Transpose(MLTransposeOptions) => transpose_with_options, options::transpose;
        Triangular(MLTriangularOptions) => triangular_with_options, options::triangular;
    }

    /// <https://webmachinelearning.github.io/webnn/#api-mlgraphbuilder-argminmax>
    fn ArgMin(
        &self,
        cx: &mut JSContext,
        input: &MLOperand,
        axis: u32,
        options: &MLArgMinMaxOptions,
    ) -> Fallible<DomRoot<MLOperand>> {
        let input = self.operand(input)?;
        let options = options::arg_min_max(options);
        self.operation(cx, &options.label.clone(), |builder| {
            builder.arg_min_with_options(input, axis, options)
        })
    }

    /// <https://webmachinelearning.github.io/webnn/#api-mlgraphbuilder-argminmax>
    fn ArgMax(
        &self,
        cx: &mut JSContext,
        input: &MLOperand,
        axis: u32,
        options: &MLArgMinMaxOptions,
    ) -> Fallible<DomRoot<MLOperand>> {
        let input = self.operand(input)?;
        let options = options::arg_min_max(options);
        self.operation(cx, &options.label.clone(), |builder| {
            builder.arg_max_with_options(input, axis, options)
        })
    }

    /// <https://webmachinelearning.github.io/webnn/#api-mlgraphbuilder-batchnorm>
    fn BatchNormalization(
        &self,
        cx: &mut JSContext,
        input: &MLOperand,
        mean: &MLOperand,
        variance: &MLOperand,
        options: &MLBatchNormalizationOptions,
    ) -> Fallible<DomRoot<MLOperand>> {
        let input = self.operand(input)?;
        let mean = self.operand(mean)?;
        let variance = self.operand(variance)?;
        let options = options::batch_normalization(self, options)?;
        self.operation(cx, &options.label.clone(), |builder| {
            builder.batch_normalization_with_options(input, mean, variance, options)
        })
    }

    /// <https://webmachinelearning.github.io/webnn/#api-mlgraphbuilder-clamp>
    fn Clamp(
        &self,
        cx: &mut JSContext,
        input: &MLOperand,
        options: &MLClampOptions,
    ) -> Fallible<DomRoot<MLOperand>> {
        let input = self.operand(input)?;
        let options = options::clamp(cx, options)?;
        self.operation(cx, &options.label.clone(), |builder| {
            builder.clamp_with_options(input, options)
        })
    }

    /// <https://webmachinelearning.github.io/webnn/#api-mlgraphbuilder-concat>
    fn Concat(
        &self,
        cx: &mut JSContext,
        inputs: Vec<DomRoot<MLOperand>>,
        axis: u32,
        options: &MLOperatorOptions,
    ) -> Fallible<DomRoot<MLOperand>> {
        if inputs.is_empty() {
            return Err(type_error("concat() needs at least one input"));
        }
        let inputs = inputs
            .iter()
            .map(|input| self.operand(input))
            .collect::<Fallible<Vec<_>>>()?;
        let options = operator_options(options);
        self.operation(cx, &options.label.clone(), |builder| {
            builder.concat_with_options(&inputs, axis, options)
        })
    }

    /// <https://webmachinelearning.github.io/webnn/#api-mlgraphbuilder-conv2d>
    fn Conv2d(
        &self,
        cx: &mut JSContext,
        input: &MLOperand,
        filter: &MLOperand,
        options: &MLConv2dOptions,
    ) -> Fallible<DomRoot<MLOperand>> {
        let input = self.operand(input)?;
        let filter = self.operand(filter)?;
        let options = options::conv2d(self, options)?;
        self.operation(cx, &options.label.clone(), |builder| {
            builder.conv2d_with_options(input, filter, options)
        })
    }

    /// <https://webmachinelearning.github.io/webnn/#api-mlgraphbuilder-convtranspose2d>
    fn ConvTranspose2d(
        &self,
        cx: &mut JSContext,
        input: &MLOperand,
        filter: &MLOperand,
        options: &MLConvTranspose2dOptions,
    ) -> Fallible<DomRoot<MLOperand>> {
        let input = self.operand(input)?;
        let filter = self.operand(filter)?;
        let options = options::conv_transpose2d(self, options)?;
        self.operation(cx, &options.label.clone(), |builder| {
            builder.conv_transpose2d_with_options(input, filter, options)
        })
    }

    /// <https://webmachinelearning.github.io/webnn/#api-mlgraphbuilder-cumulativesum>
    fn CumulativeSum(
        &self,
        cx: &mut JSContext,
        input: &MLOperand,
        axis: u32,
        options: &MLCumulativeSumOptions,
    ) -> Fallible<DomRoot<MLOperand>> {
        let input = self.operand(input)?;
        let options = options::cumulative_sum(options);
        self.operation(cx, &options.label.clone(), |builder| {
            builder.cumulative_sum_with_options(input, axis, options)
        })
    }

    /// <https://webmachinelearning.github.io/webnn/#api-mlgraphbuilder-dequantizelinear>
    fn DequantizeLinear(
        &self,
        cx: &mut JSContext,
        input: &MLOperand,
        scale: &MLOperand,
        zero_point: &MLOperand,
        options: &MLOperatorOptions,
    ) -> Fallible<DomRoot<MLOperand>> {
        let input = self.operand(input)?;
        let scale = self.operand(scale)?;
        let zero_point = self.operand(zero_point)?;
        let options = operator_options(options);
        self.operation(cx, &options.label.clone(), |builder| {
            builder.dequantize_linear_with_options(input, scale, Some(zero_point), options)
        })
    }

    /// <https://webmachinelearning.github.io/webnn/#api-mlgraphbuilder-quantizelinear>
    fn QuantizeLinear(
        &self,
        cx: &mut JSContext,
        input: &MLOperand,
        scale: &MLOperand,
        zero_point: &MLOperand,
        options: &MLOperatorOptions,
    ) -> Fallible<DomRoot<MLOperand>> {
        let input = self.operand(input)?;
        let scale = self.operand(scale)?;
        let zero_point = self.operand(zero_point)?;
        let options = operator_options(options);
        self.operation(cx, &options.label.clone(), |builder| {
            builder.quantize_linear_with_options(input, scale, Some(zero_point), options)
        })
    }

    /// <https://webmachinelearning.github.io/webnn/#api-mlgraphbuilder-expand>
    fn Expand(
        &self,
        cx: &mut JSContext,
        input: &MLOperand,
        new_shape: Vec<u32>,
        options: &MLOperatorOptions,
    ) -> Fallible<DomRoot<MLOperand>> {
        let input = self.operand(input)?;
        let new_shape = new_shape.into_iter().map(MLDimension::Static).collect();
        let options = operator_options(options);
        self.operation(cx, &options.label.clone(), |builder| {
            builder.expand_with_options(input, new_shape, options)
        })
    }

    /// <https://webmachinelearning.github.io/webnn/#api-mlgraphbuilder-gather>
    fn Gather(
        &self,
        cx: &mut JSContext,
        input: &MLOperand,
        indices: &MLOperand,
        options: &MLGatherOptions,
    ) -> Fallible<DomRoot<MLOperand>> {
        let input = self.operand(input)?;
        let indices = self.operand(indices)?;
        let options = options::gather(options);
        self.operation(cx, &options.label.clone(), |builder| {
            builder.gather_with_options(input, indices, options)
        })
    }

    /// <https://webmachinelearning.github.io/webnn/#api-mlgraphbuilder-gatherelements>
    fn GatherElements(
        &self,
        cx: &mut JSContext,
        input: &MLOperand,
        indices: &MLOperand,
        options: &MLGatherOptions,
    ) -> Fallible<DomRoot<MLOperand>> {
        let input = self.operand(input)?;
        let indices = self.operand(indices)?;
        let options = options::gather(options);
        self.operation(cx, &options.label.clone(), |builder| {
            builder.gather_elements_with_options(input, indices, options)
        })
    }

    /// <https://webmachinelearning.github.io/webnn/#api-mlgraphbuilder-gathernd>
    fn GatherND(
        &self,
        cx: &mut JSContext,
        input: &MLOperand,
        indices: &MLOperand,
        options: &MLOperatorOptions,
    ) -> Fallible<DomRoot<MLOperand>> {
        let input = self.operand(input)?;
        let indices = self.operand(indices)?;
        let options = operator_options(options);
        self.operation(cx, &options.label.clone(), |builder| {
            builder.gather_nd_with_options(input, indices, options)
        })
    }

    /// <https://webmachinelearning.github.io/webnn/#api-mlgraphbuilder-gemm>
    fn Gemm(
        &self,
        cx: &mut JSContext,
        a: &MLOperand,
        b: &MLOperand,
        options: &MLGemmOptions,
    ) -> Fallible<DomRoot<MLOperand>> {
        let a = self.operand(a)?;
        let b = self.operand(b)?;
        let options = options::gemm(self, options)?;
        self.operation(cx, &options.label.clone(), |builder| {
            builder.gemm_with_options(a, b, options)
        })
    }

    /// <https://webmachinelearning.github.io/webnn/#api-mlgraphbuilder-gru>
    fn Gru(
        &self,
        cx: &mut JSContext,
        input: &MLOperand,
        weight: &MLOperand,
        recurrent_weight: &MLOperand,
        steps: u32,
        hidden_size: u32,
        options: &MLGruOptions,
    ) -> Fallible<Vec<DomRoot<MLOperand>>> {
        let input = self.operand(input)?;
        let weight = self.operand(weight)?;
        let recurrent_weight = self.operand(recurrent_weight)?;
        let options = options::gru(self, options)?;
        self.operation_with_outputs(cx, &options.label.clone(), |builder| {
            builder.gru_with_options(input, weight, recurrent_weight, steps, hidden_size, options)
        })
    }

    /// <https://webmachinelearning.github.io/webnn/#api-mlgraphbuilder-grucell>
    fn GruCell(
        &self,
        cx: &mut JSContext,
        input: &MLOperand,
        weight: &MLOperand,
        recurrent_weight: &MLOperand,
        hidden_state: &MLOperand,
        hidden_size: u32,
        options: &MLGruCellOptions,
    ) -> Fallible<DomRoot<MLOperand>> {
        let input = self.operand(input)?;
        let weight = self.operand(weight)?;
        let recurrent_weight = self.operand(recurrent_weight)?;
        let hidden_state = self.operand(hidden_state)?;
        let options = options::gru_cell(self, options)?;
        self.operation(cx, &options.label.clone(), |builder| {
            builder.gru_cell_with_options(
                input,
                weight,
                recurrent_weight,
                hidden_state,
                hidden_size,
                options,
            )
        })
    }

    /// <https://webmachinelearning.github.io/webnn/#api-mlgraphbuilder-instancenorm>
    fn InstanceNormalization(
        &self,
        cx: &mut JSContext,
        input: &MLOperand,
        options: &MLInstanceNormalizationOptions,
    ) -> Fallible<DomRoot<MLOperand>> {
        let input = self.operand(input)?;
        let options = options::instance_normalization(self, options)?;
        self.operation(cx, &options.label.clone(), |builder| {
            builder.instance_normalization_with_options(input, options)
        })
    }

    /// <https://webmachinelearning.github.io/webnn/#api-mlgraphbuilder-layernorm>
    fn LayerNormalization(
        &self,
        cx: &mut JSContext,
        input: &MLOperand,
        options: &MLLayerNormalizationOptions,
    ) -> Fallible<DomRoot<MLOperand>> {
        let input = self.operand(input)?;
        let options = options::layer_normalization(self, options)?;
        self.operation(cx, &options.label.clone(), |builder| {
            builder.layer_normalization_with_options(input, options)
        })
    }

    /// <https://webmachinelearning.github.io/webnn/#api-mlgraphbuilder-lstm>
    fn Lstm(
        &self,
        cx: &mut JSContext,
        input: &MLOperand,
        weight: &MLOperand,
        recurrent_weight: &MLOperand,
        steps: u32,
        hidden_size: u32,
        options: &MLLstmOptions,
    ) -> Fallible<Vec<DomRoot<MLOperand>>> {
        let input = self.operand(input)?;
        let weight = self.operand(weight)?;
        let recurrent_weight = self.operand(recurrent_weight)?;
        let options = options::lstm(self, options)?;
        self.operation_with_outputs(cx, &options.label.clone(), |builder| {
            builder.lstm_with_options(input, weight, recurrent_weight, steps, hidden_size, options)
        })
    }

    /// <https://webmachinelearning.github.io/webnn/#api-mlgraphbuilder-lstmcell>
    #[expect(clippy::too_many_arguments)]
    fn LstmCell(
        &self,
        cx: &mut JSContext,
        input: &MLOperand,
        weight: &MLOperand,
        recurrent_weight: &MLOperand,
        hidden_state: &MLOperand,
        cell_state: &MLOperand,
        hidden_size: u32,
        options: &MLLstmCellOptions,
    ) -> Fallible<Vec<DomRoot<MLOperand>>> {
        let input = self.operand(input)?;
        let weight = self.operand(weight)?;
        let recurrent_weight = self.operand(recurrent_weight)?;
        let hidden_state = self.operand(hidden_state)?;
        let cell_state = self.operand(cell_state)?;
        let options = options::lstm_cell(self, options)?;
        self.operation_with_outputs(cx, &options.label.clone(), |builder| {
            builder.lstm_cell_with_options(
                input,
                weight,
                recurrent_weight,
                hidden_state,
                cell_state,
                hidden_size,
                options,
            )
        })
    }

    /// <https://webmachinelearning.github.io/webnn/#api-mlgraphbuilder-pad>
    fn Pad(
        &self,
        cx: &mut JSContext,
        input: &MLOperand,
        beginning_padding: Vec<u32>,
        ending_padding: Vec<u32>,
        options: &MLPadOptions,
    ) -> Fallible<DomRoot<MLOperand>> {
        let input = self.operand(input)?;
        let options = options::pad(cx, options)?;
        self.operation(cx, &options.label.clone(), |builder| {
            builder.pad_with_options(input, beginning_padding, ending_padding, options)
        })
    }

    /// <https://webmachinelearning.github.io/webnn/#api-mlgraphbuilder-prelu>
    fn Prelu(
        &self,
        cx: &mut JSContext,
        input: &MLOperand,
        slope: &MLOperand,
        options: &MLOperatorOptions,
    ) -> Fallible<DomRoot<MLOperand>> {
        let input = self.operand(input)?;
        let slope = self.operand(slope)?;
        let options = operator_options(options);
        self.operation(cx, &options.label.clone(), |builder| {
            builder.prelu_with_options(input, slope, options)
        })
    }

    /// <https://webmachinelearning.github.io/webnn/#api-mlgraphbuilder-reshape-method>
    fn Reshape(
        &self,
        cx: &mut JSContext,
        input: &MLOperand,
        new_shape: Vec<u32>,
        options: &MLOperatorOptions,
    ) -> Fallible<DomRoot<MLOperand>> {
        let input = self.operand(input)?;
        let new_shape = new_shape.into_iter().map(MLDimension::Static).collect();
        let options = operator_options(options);
        self.operation(cx, &options.label.clone(), |builder| {
            builder.reshape_with_options(input, new_shape, options)
        })
    }

    /// <https://webmachinelearning.github.io/webnn/#api-mlgraphbuilder-scatterelements>
    fn ScatterElements(
        &self,
        cx: &mut JSContext,
        input: &MLOperand,
        indices: &MLOperand,
        updates: &MLOperand,
        options: &MLScatterOptions,
    ) -> Fallible<DomRoot<MLOperand>> {
        let input = self.operand(input)?;
        let indices = self.operand(indices)?;
        let updates = self.operand(updates)?;
        let options = options::scatter(options);
        self.operation(cx, &options.label.clone(), |builder| {
            builder.scatter_elements_with_options(input, indices, updates, options)
        })
    }

    /// <https://webmachinelearning.github.io/webnn/#api-mlgraphbuilder-scatternd>
    fn ScatterND(
        &self,
        cx: &mut JSContext,
        input: &MLOperand,
        indices: &MLOperand,
        updates: &MLOperand,
        options: &MLOperatorOptions,
    ) -> Fallible<DomRoot<MLOperand>> {
        let input = self.operand(input)?;
        let indices = self.operand(indices)?;
        let updates = self.operand(updates)?;
        let options = operator_options(options);
        self.operation(cx, &options.label.clone(), |builder| {
            builder.scatter_nd_with_options(input, indices, updates, options)
        })
    }

    /// <https://webmachinelearning.github.io/webnn/#api-mlgraphbuilder-slice>
    fn Slice(
        &self,
        cx: &mut JSContext,
        input: &MLOperand,
        starts: Vec<u32>,
        sizes: Vec<u32>,
        options: &MLSliceOptions,
    ) -> Fallible<DomRoot<MLOperand>> {
        let input = self.operand(input)?;
        let sizes: Vec<MLDimension> = sizes.into_iter().map(MLDimension::Static).collect();
        let options = options::slice(options);
        self.operation(cx, &options.label.clone(), |builder| {
            builder.slice_with_options(input, &starts, &sizes, options)
        })
    }

    /// <https://webmachinelearning.github.io/webnn/#api-mlgraphbuilder-split>
    fn Split(
        &self,
        cx: &mut JSContext,
        input: &MLOperand,
        splits: RangeEnforcedUnsignedLongOrRangeEnforcedUnsignedLongSequence,
        options: &MLSplitOptions,
    ) -> Fallible<Vec<DomRoot<MLOperand>>> {
        let input = self.operand(input)?;
        let options = options::split(options);
        self.operation_with_outputs(cx, &options.label.clone(), |builder| match splits {
            RangeEnforcedUnsignedLongOrRangeEnforcedUnsignedLongSequence::RangeEnforcedUnsignedLong(
                count,
            ) => builder.split_equal_with_options(input, count, options),
            RangeEnforcedUnsignedLongOrRangeEnforcedUnsignedLongSequence::RangeEnforcedUnsignedLongSequence(
                splits,
            ) => builder.split_with_options(input, &splits, options),
        })
    }

    /// <https://webmachinelearning.github.io/webnn/#api-mlgraphbuilder-tile>
    fn Tile(
        &self,
        cx: &mut JSContext,
        input: &MLOperand,
        repetitions: Vec<u32>,
        options: &MLOperatorOptions,
    ) -> Fallible<DomRoot<MLOperand>> {
        let input = self.operand(input)?;
        let options = operator_options(options);
        self.operation(cx, &options.label.clone(), |builder| {
            builder.tile_with_options(input, repetitions, options)
        })
    }
}

impl MLGraphBuilder {
    /// <https://webmachinelearning.github.io/webnn/#dom-mlgraphbuilder-build>
    fn start_build(
        &self,
        promise: &RootedPromise,
        outputs: &Record<USVString, DomRoot<MLOperand>>,
    ) -> Fallible<()> {
        self.check_can_build()?;
        if outputs.is_empty() {
            return Err(type_error("build() needs at least one output"));
        }
        let mut named_outputs = BTreeMap::new();
        let mut graph_outputs = Vec::new();
        for (name, operand) in outputs.iter() {
            if name.0.is_empty() {
                return Err(type_error("Output names must not be empty"));
            }
            let rustnn_operand = self.operand(operand)?;
            if operand.kind() != OperandKind::Operation {
                return Err(type_error(
                    "Graph outputs must be the results of operations, not inputs or constants",
                ));
            }
            named_outputs.insert(name.0.as_str(), rustnn_operand);
            graph_outputs.push(GraphOperand {
                index: operand.index(),
                name: name.0.clone(),
                data_type: operand.data_type(),
                shape: operand.shape().clone(),
            });
        }

        let graph = self.with_builder(|builder| builder.finish_graph_info(&named_outputs))?;
        self.has_built.set(true);

        // Only the inputs the outputs depend on are inputs of the graph.
        let graph_inputs: Vec<GraphOperand> = self
            .inputs
            .borrow()
            .iter()
            .filter(|input| graph.input_operands.contains(&input.index))
            .map(|input| GraphOperand {
                index: input.index,
                name: input.name.clone(),
                data_type: input.data_type,
                shape: input.shape.clone(),
            })
            .collect();
        for (index, input) in graph_inputs.iter().enumerate() {
            if graph_inputs[..index]
                .iter()
                .any(|other| other.name == input.name)
            {
                return Err(type_error(format!(
                    "The graph has more than one input named {:?}",
                    input.name
                )));
            }
        }

        let context = &self.context;
        let id = backend::next_id();
        let trusted_promise = TrustedPromise::new(promise);
        let trusted_context = Trusted::new(&**context);
        let task_source = self
            .global()
            .task_manager()
            .dom_manipulation_task_source()
            .to_sendable();
        backend::send(Request::BuildGraph {
            context: context.backend_id(),
            id,
            graph: Box::new(graph),
            reply: Box::new(move |result| {
                task_source.queue(task!(webnn_graph_built: move |cx| {
                    let promise = trusted_promise.root(cx);
                    let context = trusted_context.root();
                    match result {
                        Ok(()) => {
                            let graph = MLGraph::new(
                                cx,
                                &context.global(),
                                &context,
                                id,
                                graph_inputs,
                                graph_outputs,
                            );
                            promise.resolve_native(cx, &graph);
                        },
                        Err(error) => promise.reject_error(cx, backend_error(error)),
                    }
                }));
            }),
        });
        Ok(())
    }
}
