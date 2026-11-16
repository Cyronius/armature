# Armature: DOM and web APIs

Detail behind the DOM and web APIs row of the [README](../../README.md) conformance table: what Armature changed and what is still missing. Other areas are listed in [the index](README.md).

## What has been done

- **WICG [Observable API](https://wicg.github.io/observable/)**, behind the `dom_observable_enabled` preference
  (off by default; enabled for the tests by `tests/wpt/meta/dom/observable/__dir__.ini`): `Observable`, `Subscriber`,
  `EventTarget.when()`, every operator, and `Observable.from()` for Observables, iterables, async iterables and
  promises. `tests/wpt/tests/dom/observable` went from 523 failing subtests to 1.
- **`ProcessingInstruction` attributes** (`getAttribute()`, `setAttribute()` and friends), behind the
  `dom_processing_instruction_attributes_enabled` preference: all 140 focused subtests pass. An
  `xml-stylesheet` processing instruction now updates its style sheet when inserting or removing the document
  element changes whether the instruction is in the prolog.
- **Events**: `DragEvent`, `DeviceMotionEvent` and `DeviceOrientationEvent` and their `createEvent()` aliases; the
  `<body>`/`<frameset>` window event-handler attributes now forward `onblur` and the frameset handlers.
- **MIME types**: Request and Response `blob()` preserve MIME parameter values, including their case and non-ASCII
  characters, and navigation quotes parameters it re-parses (so `charset` is no longer lost). DOM `Headers` keeps
  the control bytes permitted by Fetch even when the HTTP library cannot store them; this makes 186 more MIME parsing
  subtests and nine header normalization subtests pass. `mimesniff/mime-types/parsing` went from 1,916 failing
  subtests to 1,208.
- **WebCrypto hybrid KEMs**: MLKEM768-P256 and MLKEM1024-P384, next to the existing MLKEM768-X25519. The
  `WebCryptoAPI/modern-algos` tests have no failures left (2,036 fixed).
- **Streams**: async iteration of `ReadableStream` (`for await`, `values()`) and `ReadableStream.from()`, built on new
  generic support for `async_iterable` declarations in the WebIDL code generator: 320 failing subtests down to 0.
- DOM event dispatch now skips listeners when either the event target's node document or the listener callback's
  document is no longer fully active. This also follows a node's current document after adoption. Seventeen
  previously failing subtests across the removed-frame, navigated-frame, and adopted-node event tests now pass,
  as does the detached-document `EventTarget.when()` subtest.
- Real pointer clicks on disabled form controls are suppressed even when hit testing lands inside the control's
  shadow tree. Servoshell no longer treats an open `<select>` menu as a WebDriver user prompt. All nine subtests in
  the disabled-element event dispatch test now pass, including clicks on disabled and re-enabled controls.
- WebDriver clicks on options in a single-select control now set the clicked option's selectedness. The full
  option-click test file passes all 11 subtests, including five that previously failed for single-select controls.
- `AbortController.abort()` now rethrows an exception from an abort algorithm, and `Document.contentType` returns only
  the essence.

## What has not been done

- About 1,200 `mimesniff` Blob/File subtests expect `new Blob([], {type})` to parse and re-serialise the type, which
  Servo does not do. Four Request/Response cases involving commas in `Content-Type` still fail. The network HTTP
  library cannot send control bytes that Fetch accepts in DOM headers, so the header-value round-trip tests still fail.
  One `response-consume` subtest expects the generated `URLSearchParams` charset to be `utf-8`, while Fetch specifies
  `UTF-8` and MIME serialization preserves parameter case.
- `TextEncoder.encodeInto()` and `TextDecoder` streaming into a `SharedArrayBuffer` (about 210 subtests) fail because
  Servo has no cross-origin isolation, so shared memory is off in every global.
- Fetch priority hints are reflected in the DOM but do not yet affect network request scheduling.
