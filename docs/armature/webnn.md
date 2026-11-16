# Armature: WebNN

Detail behind the WebNN row of the [README](../../README.md) conformance table: what Armature changed and what is still missing. Other areas are listed in [the index](README.md).

## What has been done

- **[Web Neural Network API](https://webmachinelearning.github.io/webnn/)** (`navigator.ml`), behind the `webnn`
  Cargo feature (on in servoshell's default build) and the `dom_webnn_enabled` preference (off by default; on for
  WPT). All of `ML`, `MLContext`, `MLGraphBuilder` (every operator), `MLGraph`, `MLOperand` and `MLTensor` are
  implemented except the `GPUDevice` overloads. Graphs are validated and recorded on the script thread with the
  [rustnn](https://github.com/rustnn/rustnn) crate and compiled and run with ONNX Runtime on a WebNN thread, which
  shares one small ONNX Runtime thread pool between all graphs. ONNX Runtime is loaded when the first context is
  created (`libonnxruntime.so`, or `ORT_DYLIB_PATH`); without it `createContext()` rejects with
  `NotSupportedError`. `webnn/` is now enabled and passes 12,772 of 13,755 subtests (89.9% of tests).

## What has not been done

- WebNN: rustnn accepts some operator arguments the specification rejects, which accounts for 765 of the 983
  failing `webnn/` subtests (mostly `conv2d`, `convTranspose2d`, pooling, recurrent and normalization validation).
  About 90 subtests differ from the expected values by more than the allowed ULP distance. `opSupportLimits()`
  reports rustnn's generic limits rather than what ONNX Runtime supports. Only ONNX Runtime's CPU execution provider
  is used, so `accelerated` is always false; there is no NPU or GPU backend yet. `createContext(GPUDevice)`,
  `createExportableTensor()`, `exportToGPU()` and `MLGraph.devices` are missing, and `MLNumber` arguments are typed
  `any` because the bindings don't support `bigint`.
