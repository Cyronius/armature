/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Conversions from the operators' option dictionaries to rustnn's option structs.

use js::context::JSContext;
use js::conversions::{ConversionResult, FromJSValConvertible};
use js::rust::HandleValue;
use rustnn::operator_enums::{
    MLConv2dFilterOperandLayout as RustnnConv2dFilterLayout,
    MLConvTranspose2dFilterOperandLayout as RustnnConvTranspose2dFilterLayout,
    MLInputOperandLayout as RustnnInputLayout,
};
use rustnn::operator_options::{self as rustnn_options, OperandIndex};

use crate::dom::bindings::codegen::Bindings::WebNNBinding::{
    MLArgMinMaxOptions, MLBatchNormalizationOptions, MLClampOptions, MLConv2dFilterOperandLayout,
    MLConv2dOptions, MLConvTranspose2dFilterOperandLayout, MLConvTranspose2dOptions,
    MLCumulativeSumOptions, MLEluOptions, MLGatherOptions, MLGemmOptions, MLGruCellOptions,
    MLGruOptions, MLHardSigmoidOptions, MLInputOperandLayout, MLInstanceNormalizationOptions,
    MLLayerNormalizationOptions, MLLeakyReluOptions, MLLinearOptions, MLLstmCellOptions,
    MLLstmOptions, MLPadOptions, MLPool2dOptions, MLRecurrentNetworkActivation, MLReduceOptions,
    MLResample2dOptions, MLReverseOptions, MLScatterOptions, MLSliceOptions, MLSplitOptions,
    MLTransposeOptions, MLTriangularOptions,
};
use crate::dom::bindings::error::{Error, Fallible};
use crate::dom::bindings::root::DomRoot;
use crate::dom::webnn::convert::to_rustnn_data_type;
use crate::dom::webnn::mlgraphbuilder::MLGraphBuilder;
use crate::dom::webnn::mloperand::MLOperand;

/// The rustnn index of an optional operand option, which must come from `builder`.
fn optional_operand(
    builder: &MLGraphBuilder,
    operand: &Option<DomRoot<MLOperand>>,
) -> Fallible<Option<OperandIndex>> {
    operand
        .as_ref()
        .map(|operand| Ok(builder.operand(operand)?.rustnn_index()))
        .transpose()
}

fn input_layout(layout: MLInputOperandLayout) -> RustnnInputLayout {
    match layout {
        MLInputOperandLayout::Nchw => RustnnInputLayout::Nchw,
        MLInputOperandLayout::Nhwc => RustnnInputLayout::Nhwc,
    }
}

fn activations(activations: &Option<Vec<MLRecurrentNetworkActivation>>) -> Option<Vec<String>> {
    activations.as_ref().map(|activations| {
        activations
            .iter()
            .map(|activation| activation.as_str().to_owned())
            .collect()
    })
}

/// Convert an MLNumber option (a bigint or a double) to the JSON number rustnn takes, or `None`
/// if it's undefined.
#[expect(unsafe_code)]
pub(crate) fn json_number(
    cx: &mut JSContext,
    value: HandleValue,
) -> Fallible<Option<serde_json::Value>> {
    if value.get().is_undefined() {
        return Ok(None);
    }
    if value.get().is_bigint() {
        let bigint = value.get().to_bigint();
        let number = unsafe { js::jsapi::BigIntToNumber(bigint) };
        let json = if number < 0.0 {
            serde_json::Value::from(unsafe { js::jsapi::ToBigInt64(bigint) })
        } else {
            serde_json::Value::from(unsafe { js::jsapi::ToBigUint64(bigint) })
        };
        return Ok(Some(json));
    }
    match f64::from_jsval(cx, value, ()) {
        // JSON has no NaN or infinities; rustnn treats null as absent.
        Ok(ConversionResult::Success(number)) => Ok(Some(serde_json::Value::from(number))),
        _ => Err(Error::JSFailed),
    }
}

pub(crate) fn arg_min_max(options: &MLArgMinMaxOptions) -> rustnn_options::MLArgMinMaxOptions {
    rustnn_options::MLArgMinMaxOptions {
        label: options.parent.label.0.clone(),
        keep_dimensions: options.keepDimensions,
        output_data_type: to_rustnn_data_type(options.outputDataType),
    }
}

pub(crate) fn batch_normalization(
    builder: &MLGraphBuilder,
    options: &MLBatchNormalizationOptions,
) -> Fallible<rustnn_options::MLBatchNormalizationOptions> {
    Ok(rustnn_options::MLBatchNormalizationOptions {
        label: options.parent.label.0.clone(),
        scale: optional_operand(builder, &options.scale)?,
        bias: optional_operand(builder, &options.bias)?,
        axis: options.axis,
        epsilon: *options.epsilon,
    })
}

pub(crate) fn clamp(
    cx: &mut JSContext,
    options: &MLClampOptions,
) -> Fallible<rustnn_options::MLClampOptions> {
    rooted!(&in(cx) let min_value = options.minValue.get());
    rooted!(&in(cx) let max_value = options.maxValue.get());
    Ok(rustnn_options::MLClampOptions {
        label: options.parent.label.0.clone(),
        min_value: json_number(cx, min_value.handle().into())?,
        max_value: json_number(cx, max_value.handle().into())?,
    })
}

pub(crate) fn conv2d(
    builder: &MLGraphBuilder,
    options: &MLConv2dOptions,
) -> Fallible<rustnn_options::MLConv2dOptions> {
    Ok(rustnn_options::MLConv2dOptions {
        label: options.parent.label.0.clone(),
        padding: options.padding.clone().unwrap_or_default(),
        strides: options.strides.clone().unwrap_or_default(),
        dilations: options.dilations.clone().unwrap_or_default(),
        groups: options.groups,
        input_layout: input_layout(options.inputLayout),
        filter_layout: match options.filterLayout {
            MLConv2dFilterOperandLayout::Oihw => RustnnConv2dFilterLayout::Oihw,
            MLConv2dFilterOperandLayout::Hwio => RustnnConv2dFilterLayout::Hwio,
            MLConv2dFilterOperandLayout::Ohwi => RustnnConv2dFilterLayout::Ohwi,
            MLConv2dFilterOperandLayout::Ihwo => RustnnConv2dFilterLayout::Ihwo,
        },
        bias: optional_operand(builder, &options.bias)?,
    })
}

pub(crate) fn conv_transpose2d(
    builder: &MLGraphBuilder,
    options: &MLConvTranspose2dOptions,
) -> Fallible<rustnn_options::MLConvTranspose2dOptions> {
    Ok(rustnn_options::MLConvTranspose2dOptions {
        label: options.parent.label.0.clone(),
        padding: options.padding.clone().unwrap_or_default(),
        strides: options.strides.clone().unwrap_or_default(),
        dilations: options.dilations.clone().unwrap_or_default(),
        output_padding: options.outputPadding.clone().unwrap_or_default(),
        output_sizes: options.outputSizes.clone(),
        groups: options.groups,
        input_layout: input_layout(options.inputLayout),
        filter_layout: match options.filterLayout {
            MLConvTranspose2dFilterOperandLayout::Iohw => RustnnConvTranspose2dFilterLayout::Iohw,
            MLConvTranspose2dFilterOperandLayout::Hwoi => RustnnConvTranspose2dFilterLayout::Hwoi,
            MLConvTranspose2dFilterOperandLayout::Ohwi => RustnnConvTranspose2dFilterLayout::Ohwi,
        },
        bias: optional_operand(builder, &options.bias)?,
    })
}

pub(crate) fn cumulative_sum(
    options: &MLCumulativeSumOptions,
) -> rustnn_options::MLCumulativeSumOptions {
    rustnn_options::MLCumulativeSumOptions {
        label: options.parent.label.0.clone(),
        exclusive: options.exclusive,
        reversed: options.reversed,
    }
}

pub(crate) fn elu(options: &MLEluOptions) -> rustnn_options::MLEluOptions {
    rustnn_options::MLEluOptions {
        label: options.parent.label.0.clone(),
        alpha: *options.alpha,
    }
}

pub(crate) fn gather(options: &MLGatherOptions) -> rustnn_options::MLGatherOptions {
    rustnn_options::MLGatherOptions {
        label: options.parent.label.0.clone(),
        axis: options.axis,
    }
}

pub(crate) fn gemm(
    builder: &MLGraphBuilder,
    options: &MLGemmOptions,
) -> Fallible<rustnn_options::MLGemmOptions> {
    Ok(rustnn_options::MLGemmOptions {
        label: options.parent.label.0.clone(),
        c: optional_operand(builder, &options.c)?,
        alpha: *options.alpha,
        beta: *options.beta,
        a_transpose: options.aTranspose,
        b_transpose: options.bTranspose,
    })
}

pub(crate) fn gru(
    builder: &MLGraphBuilder,
    options: &MLGruOptions,
) -> Fallible<rustnn_options::MLGruOptions> {
    Ok(rustnn_options::MLGruOptions {
        label: options.parent.label.0.clone(),
        bias: optional_operand(builder, &options.bias)?,
        recurrent_bias: optional_operand(builder, &options.recurrentBias)?,
        initial_hidden_state: optional_operand(builder, &options.initialHiddenState)?,
        reset_after: options.resetAfter,
        return_sequence: options.returnSequence,
        direction: options.direction.as_str().to_owned(),
        layout: options.layout.as_str().to_owned(),
        activations: activations(&options.activations),
    })
}

pub(crate) fn gru_cell(
    builder: &MLGraphBuilder,
    options: &MLGruCellOptions,
) -> Fallible<rustnn_options::MLGruCellOptions> {
    Ok(rustnn_options::MLGruCellOptions {
        label: options.parent.label.0.clone(),
        bias: optional_operand(builder, &options.bias)?,
        recurrent_bias: optional_operand(builder, &options.recurrentBias)?,
        reset_after: options.resetAfter,
        layout: options.layout.as_str().to_owned(),
        activations: activations(&options.activations),
    })
}

pub(crate) fn hard_sigmoid(options: &MLHardSigmoidOptions) -> rustnn_options::MLHardSigmoidOptions {
    rustnn_options::MLHardSigmoidOptions {
        label: options.parent.label.0.clone(),
        alpha: *options.alpha,
        beta: *options.beta,
    }
}

pub(crate) fn instance_normalization(
    builder: &MLGraphBuilder,
    options: &MLInstanceNormalizationOptions,
) -> Fallible<rustnn_options::MLInstanceNormalizationOptions> {
    Ok(rustnn_options::MLInstanceNormalizationOptions {
        label: options.parent.label.0.clone(),
        scale: optional_operand(builder, &options.scale)?,
        bias: optional_operand(builder, &options.bias)?,
        epsilon: *options.epsilon,
        layout: options.layout.as_str().to_owned(),
    })
}

pub(crate) fn layer_normalization(
    builder: &MLGraphBuilder,
    options: &MLLayerNormalizationOptions,
) -> Fallible<rustnn_options::MLLayerNormalizationOptions> {
    Ok(rustnn_options::MLLayerNormalizationOptions {
        label: options.parent.label.0.clone(),
        scale: optional_operand(builder, &options.scale)?,
        bias: optional_operand(builder, &options.bias)?,
        axes: options.axes.clone(),
        epsilon: *options.epsilon,
    })
}

pub(crate) fn leaky_relu(options: &MLLeakyReluOptions) -> rustnn_options::MLLeakyReluOptions {
    rustnn_options::MLLeakyReluOptions {
        label: options.parent.label.0.clone(),
        alpha: *options.alpha,
    }
}

pub(crate) fn linear(options: &MLLinearOptions) -> rustnn_options::MLLinearOptions {
    rustnn_options::MLLinearOptions {
        label: options.parent.label.0.clone(),
        alpha: *options.alpha,
        beta: *options.beta,
    }
}

pub(crate) fn lstm(
    builder: &MLGraphBuilder,
    options: &MLLstmOptions,
) -> Fallible<rustnn_options::MLLstmOptions> {
    Ok(rustnn_options::MLLstmOptions {
        label: options.parent.label.0.clone(),
        bias: optional_operand(builder, &options.bias)?,
        recurrent_bias: optional_operand(builder, &options.recurrentBias)?,
        peephole_weight: optional_operand(builder, &options.peepholeWeight)?,
        initial_hidden_state: optional_operand(builder, &options.initialHiddenState)?,
        initial_cell_state: optional_operand(builder, &options.initialCellState)?,
        return_sequence: options.returnSequence,
        direction: options.direction.as_str().to_owned(),
        layout: options.layout.as_str().to_owned(),
        activations: activations(&options.activations),
    })
}

pub(crate) fn lstm_cell(
    builder: &MLGraphBuilder,
    options: &MLLstmCellOptions,
) -> Fallible<rustnn_options::MLLstmCellOptions> {
    Ok(rustnn_options::MLLstmCellOptions {
        label: options.parent.label.0.clone(),
        bias: optional_operand(builder, &options.bias)?,
        recurrent_bias: optional_operand(builder, &options.recurrentBias)?,
        peephole_weight: optional_operand(builder, &options.peepholeWeight)?,
        layout: options.layout.as_str().to_owned(),
        activations: activations(&options.activations),
    })
}

pub(crate) fn pad(
    cx: &mut JSContext,
    options: &MLPadOptions,
) -> Fallible<rustnn_options::MLPadOptions> {
    rooted!(&in(cx) let value = options.value.get());
    Ok(rustnn_options::MLPadOptions {
        label: options.parent.label.0.clone(),
        mode: options.mode.as_str().to_owned(),
        value: json_number(cx, value.handle().into())?,
    })
}

pub(crate) fn pool2d(options: &MLPool2dOptions) -> rustnn_options::MLPool2dOptions {
    rustnn_options::MLPool2dOptions {
        label: options.parent.label.0.clone(),
        window_dimensions: options.windowDimensions.clone(),
        padding: options.padding.clone().unwrap_or_default(),
        strides: options.strides.clone().unwrap_or_default(),
        dilations: options.dilations.clone().unwrap_or_default(),
        layout: options.layout.as_str().to_owned(),
        output_shape_rounding: options.outputShapeRounding.as_str().to_owned(),
        output_sizes: options.outputSizes.clone(),
    }
}

pub(crate) fn reduce(options: &MLReduceOptions) -> rustnn_options::MLReduceOptions {
    rustnn_options::MLReduceOptions {
        label: options.parent.label.0.clone(),
        axes: options.axes.clone(),
        keep_dimensions: options.keepDimensions,
    }
}

pub(crate) fn resample2d(options: &MLResample2dOptions) -> rustnn_options::MLResample2dOptions {
    rustnn_options::MLResample2dOptions {
        label: options.parent.label.0.clone(),
        mode: options.mode.as_str().to_owned(),
        scales: options
            .scales
            .as_ref()
            .map(|scales| scales.iter().map(|scale| **scale).collect())
            .unwrap_or_default(),
        sizes: options.sizes.clone(),
        axes: options.axes.clone().unwrap_or_default(),
    }
}

pub(crate) fn reverse(options: &MLReverseOptions) -> rustnn_options::MLReverseOptions {
    rustnn_options::MLReverseOptions {
        label: options.parent.label.0.clone(),
        axes: options.axes.clone(),
    }
}

pub(crate) fn scatter(options: &MLScatterOptions) -> rustnn_options::MLScatterOptions {
    rustnn_options::MLScatterOptions {
        label: options.parent.label.0.clone(),
        axis: options.axis,
    }
}

pub(crate) fn slice(options: &MLSliceOptions) -> rustnn_options::MLSliceOptions {
    rustnn_options::MLSliceOptions {
        label: options.parent.label.0.clone(),
        strides: options.strides.clone().unwrap_or_default(),
    }
}

pub(crate) fn split(options: &MLSplitOptions) -> rustnn_options::MLSplitOptions {
    rustnn_options::MLSplitOptions {
        label: options.parent.label.0.clone(),
        axis: options.axis,
    }
}

pub(crate) fn transpose(options: &MLTransposeOptions) -> rustnn_options::MLTransposeOptions {
    rustnn_options::MLTransposeOptions {
        label: options.parent.label.0.clone(),
        permutation: options.permutation.clone().unwrap_or_default(),
    }
}

pub(crate) fn triangular(options: &MLTriangularOptions) -> rustnn_options::MLTriangularOptions {
    rustnn_options::MLTriangularOptions {
        label: options.parent.label.0.clone(),
        upper: Some(options.upper),
        diagonal: options.diagonal,
    }
}
