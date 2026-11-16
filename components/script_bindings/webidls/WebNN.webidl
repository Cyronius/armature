/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// skip-unless CARGO_FEATURE_WEBNN

// https://webmachinelearning.github.io/webnn/
//
// Differences from the specification:
// - The GPUDevice overloads (createContext(GPUDevice), createExportableTensor, exportToGPU) are
//   not implemented.
// - AllowSharedBufferSource is BufferSource, MLNumber is `any` (it is (bigint or unrestricted
//   double), and the bindings do not support bigint), and FrozenArray attributes are `any`.
// - opSupportLimits() returns `any`: the object is built from rustnn's serialized limits instead
//   of the MLOpSupportLimits dictionary and its many partial dictionaries.

partial interface Navigator {
  [SecureContext, SameObject, Pref="dom_webnn_enabled"] readonly attribute ML ml;
};

partial interface WorkerNavigator {
  [SecureContext, SameObject, Pref="dom_webnn_enabled"] readonly attribute ML ml;
};

enum MLPowerPreference {
  "default",
  "high-performance",
  "low-power"
};

dictionary MLContextOptions {
  MLPowerPreference powerPreference = "default";
  boolean accelerated = true;
};

[SecureContext, Exposed=(Window, Worker), Pref="dom_webnn_enabled"]
interface ML {
  Promise<MLContext> createContext(optional MLContextOptions options = {});
};

typedef record<USVString, MLTensor> MLNamedTensors;

dictionary MLContextLostInfo {
  DOMString message;
};

[SecureContext, Exposed=(Window, Worker), Pref="dom_webnn_enabled"]
interface MLContext {
  [Throws] undefined dispatch(MLGraph graph, MLNamedTensors inputs, MLNamedTensors outputs);

  Promise<MLTensor> createTensor(MLTensorDescriptor descriptor);
  Promise<MLTensor> createConstantTensor(
    MLOperandDescriptor descriptor, BufferSource inputData);

  Promise<ArrayBuffer> readTensor(MLTensor tensor);
  Promise<undefined> readTensor(MLTensor tensor, BufferSource outputData);

  [Throws] undefined writeTensor(MLTensor tensor, BufferSource inputData);

  // Returns an MLOpSupportLimits dictionary.
  any opSupportLimits();

  undefined destroy();

  readonly attribute boolean accelerated;
  readonly attribute Promise<MLContextLostInfo> lost;
};

[SecureContext, Exposed=(Window, Worker), Pref="dom_webnn_enabled"]
interface MLGraph {
  undefined destroy();
};

enum MLInputOperandLayout {
  "nchw",
  "nhwc"
};

enum MLOperandDataType {
  "float32",
  "float16",
  "int32",
  "uint32",
  "int64",
  "uint64",
  "int8",
  "uint8"
};

dictionary MLOperandDescriptor {
  required MLOperandDataType dataType;
  required sequence<[EnforceRange] unsigned long> shape;
};

[SecureContext, Exposed=(Window, Worker), Pref="dom_webnn_enabled"]
interface MLOperand {
  readonly attribute MLOperandDataType dataType;
  readonly attribute /* FrozenArray<unsigned long> */ any shape;
};

dictionary MLOperatorOptions {
  USVString label = "";
};

typedef /* (bigint or unrestricted double) */ any MLNumber;

dictionary MLTensorDescriptor : MLOperandDescriptor {
  boolean readable = false;
  boolean writable = false;
};

[SecureContext, Exposed=(Window, Worker), Pref="dom_webnn_enabled"]
interface MLTensor {
  readonly attribute MLOperandDataType dataType;
  readonly attribute /* FrozenArray<unsigned long> */ any shape;
  readonly attribute boolean readable;
  readonly attribute boolean writable;
  readonly attribute boolean constant;

  undefined destroy();
};

typedef record<USVString, MLOperand> MLNamedOperands;

[SecureContext, Exposed=(Window, Worker), Pref="dom_webnn_enabled"]
interface MLGraphBuilder {
  [Throws] constructor(MLContext context);

  [Throws] MLOperand input(USVString name, MLOperandDescriptor descriptor);

  [Throws] MLOperand constant(MLOperandDescriptor descriptor, BufferSource buffer);
  [Throws] MLOperand constant(MLOperandDataType dataType, MLNumber value);
  [Throws] MLOperand constant(MLTensor tensor);

  Promise<MLGraph> build(MLNamedOperands outputs);
};

// The operators, as in the specification.

dictionary MLArgMinMaxOptions : MLOperatorOptions {
  boolean keepDimensions = false;
  MLOperandDataType outputDataType = "int32";
};

partial interface MLGraphBuilder {
  [Throws] MLOperand argMin(MLOperand input, [EnforceRange] unsigned long axis,
                   optional MLArgMinMaxOptions options = {});
  [Throws] MLOperand argMax(MLOperand input, [EnforceRange] unsigned long axis,
                   optional MLArgMinMaxOptions options = {});
};

dictionary MLBatchNormalizationOptions : MLOperatorOptions {
  MLOperand scale;
  MLOperand bias;
  [EnforceRange] unsigned long axis = 1;
  double epsilon = 1e-5;
};

partial interface MLGraphBuilder {
  [Throws] MLOperand batchNormalization(MLOperand input, MLOperand mean, MLOperand variance,
                               optional MLBatchNormalizationOptions options = {});
};

partial interface MLGraphBuilder {
  [Throws] MLOperand cast(MLOperand input,
                 MLOperandDataType dataType,
                 optional MLOperatorOptions options = {});
};

dictionary MLClampOptions : MLOperatorOptions {
  MLNumber minValue;
  MLNumber maxValue;
};

partial interface MLGraphBuilder {
  [Throws] MLOperand clamp(MLOperand input, optional MLClampOptions options = {});
};

partial interface MLGraphBuilder {
  [Throws] MLOperand concat(sequence<MLOperand> inputs,
                   [EnforceRange] unsigned long axis,
                   optional MLOperatorOptions options = {});
};

enum MLConv2dFilterOperandLayout {
  "oihw",
  "hwio",
  "ohwi",
  "ihwo"
};

dictionary MLConv2dOptions : MLOperatorOptions {
  sequence<[EnforceRange] unsigned long> padding;
  sequence<[EnforceRange] unsigned long> strides;
  sequence<[EnforceRange] unsigned long> dilations;
  [EnforceRange] unsigned long groups = 1;
  MLInputOperandLayout inputLayout = "nchw";
  MLConv2dFilterOperandLayout filterLayout = "oihw";
  MLOperand bias;
};

partial interface MLGraphBuilder {
  [Throws] MLOperand conv2d(MLOperand input,
                   MLOperand filter,
                   optional MLConv2dOptions options = {});
};

enum MLConvTranspose2dFilterOperandLayout {
  "iohw",
  "hwoi",
  "ohwi"
};

dictionary MLConvTranspose2dOptions : MLOperatorOptions {
  sequence<[EnforceRange] unsigned long> padding;
  sequence<[EnforceRange] unsigned long> strides;
  sequence<[EnforceRange] unsigned long> dilations;
  sequence<[EnforceRange] unsigned long> outputPadding;
  sequence<[EnforceRange] unsigned long> outputSizes;
  [EnforceRange] unsigned long groups = 1;
  MLInputOperandLayout inputLayout = "nchw";
  MLConvTranspose2dFilterOperandLayout filterLayout = "iohw";
  MLOperand bias;
};

partial interface MLGraphBuilder {
  [Throws] MLOperand convTranspose2d(MLOperand input, MLOperand filter,
                            optional MLConvTranspose2dOptions options = {});
};

dictionary MLCumulativeSumOptions : MLOperatorOptions {
  boolean exclusive = false;
  boolean reversed = false;
};

partial interface MLGraphBuilder {
  [Throws] MLOperand cumulativeSum(MLOperand input,
                          unsigned long axis,
                          optional MLCumulativeSumOptions options = {});
};

partial interface MLGraphBuilder {
  [Throws] MLOperand add(MLOperand a, MLOperand b, optional MLOperatorOptions options = {});
  [Throws] MLOperand sub(MLOperand a, MLOperand b, optional MLOperatorOptions options = {});
  [Throws] MLOperand mul(MLOperand a, MLOperand b, optional MLOperatorOptions options = {});
  [Throws] MLOperand div(MLOperand a, MLOperand b, optional MLOperatorOptions options = {});
  [Throws] MLOperand max(MLOperand a, MLOperand b, optional MLOperatorOptions options = {});
  [Throws] MLOperand min(MLOperand a, MLOperand b, optional MLOperatorOptions options = {});
  [Throws] MLOperand pow(MLOperand a, MLOperand b, optional MLOperatorOptions options = {});
};

partial interface MLGraphBuilder {
  [Throws] MLOperand equal(MLOperand a,
                  MLOperand b,
                  optional MLOperatorOptions options = {});
  [Throws] MLOperand notEqual(MLOperand a,
                     MLOperand b,
                     optional MLOperatorOptions options = {});
  [Throws] MLOperand greater(MLOperand a,
                    MLOperand b,
                    optional MLOperatorOptions options = {});
  [Throws] MLOperand greaterOrEqual(MLOperand a,
                           MLOperand b,
                           optional MLOperatorOptions options = {});
  [Throws] MLOperand lesser(MLOperand a,
                   MLOperand b,
                   optional MLOperatorOptions options = {});
  [Throws] MLOperand lesserOrEqual(MLOperand a,
                          MLOperand b,
                          optional MLOperatorOptions options = {});
  [Throws] MLOperand logicalNot(MLOperand a, optional MLOperatorOptions options = {});
  [Throws] MLOperand logicalAnd(MLOperand a,
                       MLOperand b,
                       optional MLOperatorOptions options = {});
  [Throws] MLOperand logicalOr(MLOperand a,
                      MLOperand b,
                      optional MLOperatorOptions options = {});
  [Throws] MLOperand logicalXor(MLOperand a,
                       MLOperand b,
                       optional MLOperatorOptions options = {});
  [Throws] MLOperand isNaN(MLOperand a, optional MLOperatorOptions options = {});
  [Throws] MLOperand isInfinite(MLOperand a, optional MLOperatorOptions options = {});
};

partial interface MLGraphBuilder {
  [Throws] MLOperand abs(MLOperand input, optional MLOperatorOptions options = {});
  [Throws] MLOperand ceil(MLOperand input, optional MLOperatorOptions options = {});
  [Throws] MLOperand cos(MLOperand input, optional MLOperatorOptions options = {});
  [Throws] MLOperand erf(MLOperand input, optional MLOperatorOptions options = {});
  [Throws] MLOperand exp(MLOperand input, optional MLOperatorOptions options = {});
  [Throws] MLOperand floor(MLOperand input, optional MLOperatorOptions options = {});
  [Throws] MLOperand identity(MLOperand input, optional MLOperatorOptions options = {});
  [Throws] MLOperand log(MLOperand input, optional MLOperatorOptions options = {});
  [Throws] MLOperand neg(MLOperand input, optional MLOperatorOptions options = {});
  [Throws] MLOperand reciprocal(MLOperand input, optional MLOperatorOptions options = {});
  [Throws] MLOperand roundEven(MLOperand input, optional MLOperatorOptions options = {});
  [Throws] MLOperand sin(MLOperand input, optional MLOperatorOptions options = {});
  [Throws] MLOperand sign(MLOperand input, optional MLOperatorOptions options = {});
  [Throws] MLOperand sqrt(MLOperand input, optional MLOperatorOptions options = {});
  [Throws] MLOperand tan(MLOperand input, optional MLOperatorOptions options = {});
};

partial interface MLGraphBuilder {
  [Throws] MLOperand dequantizeLinear(MLOperand input,
                             MLOperand scale,
                             MLOperand zeroPoint,
                             optional MLOperatorOptions options = {});
};

partial interface MLGraphBuilder {
  [Throws] MLOperand quantizeLinear(MLOperand input,
                           MLOperand scale,
                           MLOperand zeroPoint,
                           optional MLOperatorOptions options = {});
};

dictionary MLEluOptions : MLOperatorOptions {
  double alpha = 1;
};

partial interface MLGraphBuilder {
  [Throws] MLOperand elu(MLOperand input, optional MLEluOptions options = {});
};

partial interface MLGraphBuilder {
  [Throws] MLOperand expand(MLOperand input,
                   sequence<[EnforceRange] unsigned long> newShape,
                   optional MLOperatorOptions options = {});
};

dictionary MLGatherOptions : MLOperatorOptions {
  [EnforceRange] unsigned long axis = 0;
};

partial interface MLGraphBuilder {
  [Throws] MLOperand gather(MLOperand input,
                   MLOperand indices,
                   optional MLGatherOptions options = {});
};

partial interface MLGraphBuilder {
  [Throws] MLOperand gatherElements(MLOperand input,
                           MLOperand indices,
                           optional MLGatherOptions options = {});
};

partial interface MLGraphBuilder {
  [Throws] MLOperand gatherND(MLOperand input,
                     MLOperand indices,
                     optional MLOperatorOptions options = {});
};

partial interface MLGraphBuilder {
  [Throws] MLOperand gelu(MLOperand input, optional MLOperatorOptions options = {});
};

dictionary MLGemmOptions : MLOperatorOptions {
  MLOperand c;
  double alpha = 1.0;
  double beta = 1.0;
  boolean aTranspose = false;
  boolean bTranspose = false;
};

partial interface MLGraphBuilder {
  [Throws] MLOperand gemm(MLOperand a, MLOperand b, optional MLGemmOptions options = {});
};

enum MLGruWeightLayout {
  "zrn",  // update-reset-new gate ordering
  "rzn"   // reset-update-new gate ordering
};

enum MLRecurrentNetworkActivation {
  "relu",
  "sigmoid",
  "tanh"
};

enum MLRecurrentNetworkDirection {
  "forward",
  "backward",
  "both"
};

dictionary MLGruOptions : MLOperatorOptions {
  MLOperand bias;
  MLOperand recurrentBias;
  MLOperand initialHiddenState;
  boolean resetAfter = true;
  boolean returnSequence = false;
  MLRecurrentNetworkDirection direction = "forward";
  MLGruWeightLayout layout = "zrn";
  sequence<MLRecurrentNetworkActivation> activations;
};

partial interface MLGraphBuilder {
  [Throws] sequence<MLOperand> gru(MLOperand input,
                          MLOperand weight,
                          MLOperand recurrentWeight,
                          [EnforceRange] unsigned long steps,
                          [EnforceRange] unsigned long hiddenSize,
                          optional MLGruOptions options = {});
};

dictionary MLGruCellOptions : MLOperatorOptions {
  MLOperand bias;
  MLOperand recurrentBias;
  boolean resetAfter = true;
  MLGruWeightLayout layout = "zrn";
  sequence<MLRecurrentNetworkActivation> activations;
};

partial interface MLGraphBuilder {
  [Throws] MLOperand gruCell(MLOperand input,
                    MLOperand weight,
                    MLOperand recurrentWeight,
                    MLOperand hiddenState,
                    [EnforceRange] unsigned long hiddenSize,
                    optional MLGruCellOptions options = {});
};

dictionary MLHardSigmoidOptions : MLOperatorOptions {
  double alpha = 0.2;
  double beta = 0.5;
};

partial interface MLGraphBuilder {
  [Throws] MLOperand hardSigmoid(MLOperand input, optional MLHardSigmoidOptions options = {});
};

partial interface MLGraphBuilder {
  [Throws] MLOperand hardSwish(MLOperand input, optional MLOperatorOptions options = {});
};

dictionary MLInstanceNormalizationOptions : MLOperatorOptions {
  MLOperand scale;
  MLOperand bias;
  double epsilon = 1e-5;
  MLInputOperandLayout layout = "nchw";
};

partial interface MLGraphBuilder {
  [Throws] MLOperand instanceNormalization(
    MLOperand input,
    optional MLInstanceNormalizationOptions options = {});
};

dictionary MLLayerNormalizationOptions : MLOperatorOptions {
  MLOperand scale;
  MLOperand bias;
  sequence<[EnforceRange] unsigned long> axes;
  double epsilon = 1e-5;
};

partial interface MLGraphBuilder {
  [Throws] MLOperand layerNormalization(MLOperand input,
                               optional MLLayerNormalizationOptions options = {});
};

dictionary MLLeakyReluOptions : MLOperatorOptions {
  double alpha = 0.01;
};

partial interface MLGraphBuilder {
  [Throws] MLOperand leakyRelu(MLOperand input, optional MLLeakyReluOptions options = {});
};

dictionary MLLinearOptions : MLOperatorOptions {
  double alpha = 1;
  double beta = 0;
};

partial interface MLGraphBuilder {
  [Throws] MLOperand linear(MLOperand input, optional MLLinearOptions options = {});
};

enum MLLstmWeightLayout {
  "iofg", // input-output-forget-cell gate ordering
  "ifgo"  // input-forget-cell-output gate ordering
};

dictionary MLLstmOptions : MLOperatorOptions {
  MLOperand bias;
  MLOperand recurrentBias;
  MLOperand peepholeWeight;
  MLOperand initialHiddenState;
  MLOperand initialCellState;
  boolean returnSequence = false;
  MLRecurrentNetworkDirection direction = "forward";
  MLLstmWeightLayout layout = "iofg";
  sequence<MLRecurrentNetworkActivation> activations;
};

partial interface MLGraphBuilder {
  [Throws] sequence<MLOperand> lstm(MLOperand input,
                           MLOperand weight,
                           MLOperand recurrentWeight,
                           [EnforceRange] unsigned long steps,
                           [EnforceRange] unsigned long hiddenSize,
                           optional MLLstmOptions options = {});
};

dictionary MLLstmCellOptions : MLOperatorOptions {
  MLOperand bias;
  MLOperand recurrentBias;
  MLOperand peepholeWeight;
  MLLstmWeightLayout layout = "iofg";
  sequence<MLRecurrentNetworkActivation> activations;
};

partial interface MLGraphBuilder {
  [Throws] sequence<MLOperand> lstmCell(MLOperand input,
                               MLOperand weight,
                               MLOperand recurrentWeight,
                               MLOperand hiddenState,
                               MLOperand cellState,
                               [EnforceRange] unsigned long hiddenSize,
                               optional MLLstmCellOptions options = {});
};

partial interface MLGraphBuilder {
  [Throws] MLOperand matmul(MLOperand a, MLOperand b, optional MLOperatorOptions options = {});
};

enum MLPaddingMode {
  "constant",
  "edge",
  "reflection"
};

dictionary MLPadOptions : MLOperatorOptions {
  MLPaddingMode mode = "constant";
  // Defaults to 0; the bindings don't support defaults for `any`.
  MLNumber value;
};

partial interface MLGraphBuilder {
  [Throws] MLOperand pad(MLOperand input,
                sequence<[EnforceRange] unsigned long> beginningPadding,
                sequence<[EnforceRange] unsigned long> endingPadding,
                optional MLPadOptions options = {});
};

enum MLRoundingType {
  "floor",
  "ceil"
};

dictionary MLPool2dOptions : MLOperatorOptions {
  sequence<[EnforceRange] unsigned long> windowDimensions;
  sequence<[EnforceRange] unsigned long> padding;
  sequence<[EnforceRange] unsigned long> strides;
  sequence<[EnforceRange] unsigned long> dilations;
  MLInputOperandLayout layout = "nchw";
  MLRoundingType outputShapeRounding = "floor";
  sequence<[EnforceRange] unsigned long> outputSizes;
};

partial interface MLGraphBuilder {
  [Throws] MLOperand averagePool2d(MLOperand input, optional MLPool2dOptions options = {});
  [Throws] MLOperand l2Pool2d(MLOperand input, optional MLPool2dOptions options = {});
  [Throws] MLOperand maxPool2d(MLOperand input, optional MLPool2dOptions options = {});
};

partial interface MLGraphBuilder {
  [Throws] MLOperand prelu(MLOperand input,
                  MLOperand slope,
                  optional MLOperatorOptions options = {});
};

dictionary MLReduceOptions : MLOperatorOptions {
  sequence<[EnforceRange] unsigned long> axes;
  boolean keepDimensions = false;
};

partial interface MLGraphBuilder {
  [Throws] MLOperand reduceL1(MLOperand input, optional MLReduceOptions options = {});
  [Throws] MLOperand reduceL2(MLOperand input, optional MLReduceOptions options = {});
  [Throws] MLOperand reduceLogSum(MLOperand input, optional MLReduceOptions options = {});
  [Throws] MLOperand reduceLogSumExp(MLOperand input, optional MLReduceOptions options = {});
  [Throws] MLOperand reduceMax(MLOperand input, optional MLReduceOptions options = {});
  [Throws] MLOperand reduceMean(MLOperand input, optional MLReduceOptions options = {});
  [Throws] MLOperand reduceMin(MLOperand input, optional MLReduceOptions options = {});
  [Throws] MLOperand reduceProduct(MLOperand input, optional MLReduceOptions options = {});
  [Throws] MLOperand reduceSum(MLOperand input, optional MLReduceOptions options = {});
  [Throws] MLOperand reduceSumSquare(MLOperand input, optional MLReduceOptions options = {});
};

partial interface MLGraphBuilder {
  [Throws] MLOperand relu(MLOperand input, optional MLOperatorOptions options = {});
};

enum MLInterpolationMode {
  "nearest-neighbor",
  "linear"
};

dictionary MLResample2dOptions : MLOperatorOptions {
  MLInterpolationMode mode = "nearest-neighbor";
  sequence<float> scales;
  sequence<[EnforceRange] unsigned long> sizes;
  sequence<[EnforceRange] unsigned long> axes;
};

partial interface MLGraphBuilder {
  [Throws] MLOperand resample2d(MLOperand input, optional MLResample2dOptions options = {});
};

partial interface MLGraphBuilder {
  [Throws] MLOperand reshape(MLOperand input,
                    sequence<[EnforceRange] unsigned long> newShape,
                    optional MLOperatorOptions options = {});
};

dictionary MLReverseOptions : MLOperatorOptions {
  sequence<[EnforceRange] unsigned long> axes;
};

partial interface MLGraphBuilder {
  [Throws] MLOperand reverse(MLOperand input, optional MLReverseOptions options = {});
};

dictionary MLScatterOptions : MLOperatorOptions {
  [EnforceRange] unsigned long axis = 0;
};

partial interface MLGraphBuilder {
  [Throws] MLOperand scatterElements(MLOperand input,
                            MLOperand indices,
                            MLOperand updates,
                            optional MLScatterOptions options = {});
};

partial interface MLGraphBuilder {
  [Throws] MLOperand scatterND(MLOperand input,
                      MLOperand indices,
                      MLOperand updates,
                      optional MLOperatorOptions options = {});
};

partial interface MLGraphBuilder {
  [Throws] MLOperand sigmoid(MLOperand input, optional MLOperatorOptions options = {});
};

dictionary MLSliceOptions : MLOperatorOptions {
  sequence<[EnforceRange] unsigned long> strides;
};

partial interface MLGraphBuilder {
  [Throws] MLOperand slice(MLOperand input,
                  sequence<[EnforceRange] unsigned long> starts,
                  sequence<[EnforceRange] unsigned long> sizes,
                  optional MLSliceOptions options = {});
};

partial interface MLGraphBuilder {
  [Throws] MLOperand softmax(MLOperand input,
                    [EnforceRange] unsigned long axis,
                    optional MLOperatorOptions options = {});
};

partial interface MLGraphBuilder {
  [Throws] MLOperand softplus(MLOperand input, optional MLOperatorOptions options = {});
};

partial interface MLGraphBuilder {
  [Throws] MLOperand softsign(MLOperand input, optional MLOperatorOptions options = {});
};

dictionary MLSplitOptions : MLOperatorOptions {
  [EnforceRange] unsigned long axis = 0;
};

partial interface MLGraphBuilder {
  [Throws] sequence<MLOperand> split(
      MLOperand input,
      ([EnforceRange] unsigned long or sequence<[EnforceRange] unsigned long>) splits,
      optional MLSplitOptions options = {});
};

partial interface MLGraphBuilder {
  [Throws] MLOperand tanh(MLOperand input, optional MLOperatorOptions options = {});
};

partial interface MLGraphBuilder {
  [Throws] MLOperand tile(MLOperand input,
                 sequence<unsigned long> repetitions,
                 optional MLOperatorOptions options = {});
};

dictionary MLTransposeOptions : MLOperatorOptions {
  sequence<[EnforceRange] unsigned long> permutation;
};

partial interface MLGraphBuilder {
  [Throws] MLOperand transpose(MLOperand input, optional MLTransposeOptions options = {});
};

dictionary MLTriangularOptions : MLOperatorOptions {
  boolean upper = true;
  [EnforceRange] long diagonal = 0;
};

partial interface MLGraphBuilder {
  [Throws] MLOperand triangular(MLOperand input, optional MLTriangularOptions options = {});
};

partial interface MLGraphBuilder {
  [Throws] MLOperand where(MLOperand condition,
                  MLOperand trueValue,
                  MLOperand falseValue,
                  optional MLOperatorOptions options = {});
};
