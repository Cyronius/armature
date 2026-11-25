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
- **Crashes**: 16 tests that recorded a crash now run. Panics are fixed in:
  - the HTML parser dropping text, doctypes and reparented children that script made impossible to insert
  - service worker registration from a detached iframe
  - trusted types violation samples with non-ASCII text
  - intersection observation through an unrendered iframe
  - WebDriver send keys after Shift
  - indirect `clip-path` cycles in SVG

  The other tests on the list had stopped crashing earlier.
- **Service workers in WPT runs**: `dom_serviceworker_enabled` is now one of the experimental prefs that WPT runs turn
  on. About 450 `.any.serviceworker.html` test variants had failed before any subtest ran because
  `navigator.serviceWorker` was undefined; they now run, worth about 314 tests, mostly in IndexedDB (+131) and Fetch
  (+110). Two crashes this exposed are fixed: a service worker has no image cache, so `OffscreenCanvas` 2D contexts
  and notification icons there no longer panic, and a script stopped by a closing worker no longer trips the
  pending-exception assertion. `Worker` is no longer exposed in service workers, as the HTML spec requires.
  About 45 window tests that register a service worker and wait for fetch interception, `clients` or back-forward
  cache support now time out instead of failing early; they scored zero either way.

## What has not been done

- 16 test files still record a crash. Of these, 11 hang until the harness kills them, including:
  - an `<iframe srcdoc>` that is re-inserted from its own `load` handler reloads synchronously, so the loop never
    yields
  - setting `options.length` to 100,000 rescans every option on each insertion
  - a `pattern` attribute with catastrophic backtracking runs without limit
  - several iframe navigation and unload cases

- WebAssembly, WebSockets, MathML, XMLHttpRequest, Shadow DOM and Web Audio still have failures, each needing a whole
  feature: `WebSocketStream` (and WebSocket over HTTP/2), MathML layout, WebAssembly type reflection (a SpiderMonkey
  build option), the Shadow DOM reference-target feature, and a configurable audio render quantum
  (`renderSizeHint`). XMLHttpRequest also lacks request header case preservation and `.asis` responses without a body.
