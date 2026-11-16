# Armature: Other directories

Detail behind the Other directories row of the [README](../../README.md) conformance table: what Armature changed and what is still missing. Other areas are listed in [the index](README.md).

## What has been done

- `./mach fmt` and the crown GC-rooting lint pass, and the tested directories have no unexpected results other than a
  few tests that also fail or are flaky on the unmodified tree.
- **WebAssembly, WebSockets, MathML, XMLHttpRequest, Shadow DOM and Web Audio**: about 700 recorded expected failures
  are cleared (WebAssembly 828 to 380 failing subtests, XMLHttpRequest 160 to 77, Web Audio 787 to 661, Shadow DOM
  344 to 316).
  - WebAssembly: shared memory and atomics are enabled in SpiderMonkey, typed-array conversion rejects shared buffers
    instead of crashing, and `WebAssembly.compileStreaming()` and `instantiateStreaming()` consume fetch responses.
  - XMLHttpRequest: Blob URL range requests, header-value validation, charset rewriting of the content type, clearing
    `responseURL` on abort and error, and a final `progress` event with a zero length that is not computable.
  - Shadow DOM: manual slot assignment, reassignment in the previous slot's tree, `autofocus` processing, and
    mutation observer callbacks in creation order.
  - Web Audio: `WaveShaperNode`, `BiquadFilterNode.getFrequencyResponse()`, `AudioParam` automation event tracking and
    validation, `AudioNode.connect()` and `disconnect()` validation, and `OfflineAudioContext.suspend()`.

## What has not been done

- WebAssembly, WebSockets, MathML, XMLHttpRequest, Shadow DOM and Web Audio still have failures, each needing a whole
  feature: `WebSocketStream` (and WebSocket over HTTP/2), MathML layout, WebAssembly type reflection (a SpiderMonkey
  build option), the Shadow DOM reference-target feature, and a configurable audio render quantum
  (`renderSizeHint`). XMLHttpRequest also lacks request header case preservation and `.asis` responses without a body.
