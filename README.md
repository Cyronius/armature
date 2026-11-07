# Armature: Servo, accelerated with AI

Armature is an unofficial fork of [Servo](https://github.com/servo/servo) that uses AI to accelerate development of
the engine. It is built with an AI coding assistant (Claude Code), and progress is measured by
[web-platform-tests](https://web-platform-tests.org/) conformance. It is **not** affiliated with or endorsed by the
Servo project.

Servo's [AI usage policy](https://book.servo.org/policy/ai-usage.html) does not accept AI-generated contributions,
so this work is not intended for, and has not been sent to, upstream Servo.

## Conformance

Measured on 6 November 2026 by running the web-platform-tests that Armature runs (the 61,100 tests selected by
`tests/wpt/include.ini`) with two builds on the same machine: upstream Servo at the commit Armature forked from
(`90c3f623175`, 30 October 2026) and Armature at `e2d5d03b104`. As on [wpt.fyi](https://wpt.fyi/), each test counts
once, with partial credit for the share of its subtests that pass; the last column counts individual subtests. In
total Armature passes 56,623 more subtests than upstream Servo (2,098,086 against 2,041,463).

| Area | Tests | Upstream Servo | Armature | Change (points) | Subtests newly passing |
|---|--:|--:|--:|--:|--:|
| CSS | 28,873 | 67.1% | 70.2% | +3.1 | +22,193 |
| HTML | 10,253 | 66.8% | 68.1% | +1.3 | +18,043 |
| WebGL conformance | 2,939 | 40.5% | 40.4% | -0.0 | -18 |
| Referrer Policy | 1,483 | 86.3% | 87.0% | +0.7 | +11 |
| SVG | 1,411 | 24.9% | 36.5% | +11.5 | +1,187 |
| Encoding | 1,345 | 98.3% | 98.3% | +0.0 | +9 |
| Servo-specific tests | 1,229 | 75.6% | 75.4% | -0.2 | +1 |
| Fetch | 957 | 66.6% | 66.9% | +0.4 | +29 |
| Content Security Policy | 918 | 79.6% | 79.9% | +0.2 | +6 |
| Editing | 860 | 49.8% | 51.4% | +1.6 | +8,857 |
| DOM | 845 | 76.4% | 85.8% | +9.4 | +2,107 |
| IndexedDB | 820 | 57.4% | 57.3% | -0.1 | -1 |
| WebSockets | 799 | 68.1% | 67.9% | -0.1 | -1 |
| MathML | 624 | 45.8% | 46.4% | +0.6 | +46 |
| WebAssembly | 615 | 65.4% | 65.4% | +0.0 | +0 |
| XMLHttpRequest | 440 | 92.1% | 91.7% | -0.5 | -10 |
| Mixed Content | 388 | 87.6% | 87.4% | -0.1 | -1 |
| Shadow DOM | 366 | 67.3% | 67.3% | +0.0 | +0 |
| Web Audio | 355 | 58.9% | 58.9% | +0.0 | +0 |
| Workers | 336 | 92.1% | 91.8% | -0.3 | -1 |
| Web Crypto | 334 | 97.5% | 99.4% | +1.9 | +2,036 |
| Streams | 317 | 91.3% | 94.1% | +2.7 | +421 |
| Trusted Types | 249 | 86.3% | 89.1% | +2.8 | +79 |
| Custom Elements | 203 | 76.1% | 79.0% | +2.9 | +153 |
| Upgrade Insecure Requests | 197 | 74.1% | 74.1% | +0.0 | +0 |
| DOM Parsing | 94 | 43.5% | 48.8% | +5.2 | +51 |
| Compression Streams | 78 | 70.2% | 73.7% | +3.5 | +63 |
| Clipboard | 61 | 25.8% | 27.4% | +1.6 | +1 |
| Web Animations | 47 | 14.9% | 64.6% | +49.8 | +472 |
| Focus | 43 | 63.2% | 60.9% | -2.3 | -4 |
| Scroll-driven Animations | 17 | 21.6% | 100.0% | +78.4 | +372 |
| Everything else | 3,604 | 69.5% | 69.6% | +0.1 | +522 |
| **Total** | **61,100** | **66.7%** | **68.9%** | **+2.3** | **+56,623** |

The CSS gains come mostly from Stylo features that Armature enables and from the Web Animations API, which the
interpolation tests throughout `css/` use, and whose `CSSAnimation` and `CSSTransition` objects most of
`css-animations` and `css-transitions` test. These are the CSS modules that gained the most:

| CSS module | Tests | Upstream Servo | Armature | Change (points) | Subtests newly passing |
|---|--:|--:|--:|--:|--:|
| `css-backgrounds` | 875 | 65.3% | 83.2% | +17.9 | +1,440 |
| `css-transforms` | 933 | 70.4% | 80.0% | +9.6 | +1,449 |
| `css-viewport` | 143 | 25.5% | 82.8% | +57.3 | +387 |
| `selectors` | 833 | 70.0% | 78.9% | +8.9 | +769 |
| `css-animations` | 209 | 55.7% | 84.7% | +29.0 | +628 |
| `motion` | 46 | 20.6% | 97.4% | +76.8 | +4,844 |
| `css-transitions` | 144 | 59.7% | 83.2% | +23.4 | +331 |
| `css-fonts` | 559 | 66.0% | 71.7% | +5.7 | +1,564 |
| `css-ui` | 1,077 | 89.6% | 91.7% | +2.1 | +848 |
| `css-text` | 1,983 | 72.3% | 73.4% | +1.1 | +755 |
| `css-scroll-snap` | 28 | 25.0% | 92.9% | +67.9 | +350 |
| `CSS2` | 6,322 | 93.1% | 93.4% | +0.3 | +231 |
| `filter-effects` | 397 | 51.9% | 56.4% | +4.4 | +628 |
| `css-masking` | 222 | 53.8% | 61.6% | +7.9 | +263 |
| `css-text-decor` | 366 | 40.7% | 45.4% | +4.7 | +667 |
| `css-writing-modes` | 1,247 | 30.1% | 31.4% | +1.3 | +77 |
| `css-properties-values-api` | 124 | 35.9% | 48.8% | +12.9 | +66 |

Notes on the numbers:

- Both builds ran the same tests, including the CSS directories that Armature added to `include.ini` and upstream
  Servo skips. Tests that set a preference only Armature has (Observable, popovers, `ProcessingInstruction`
  attributes, ARIA reflection) error on the upstream build, whose engine lacks those features anyway.
- The Armature run matched the expectations recorded in `tests/wpt/meta` for all but about 200 tests, so the scores
  should be close to what Servo's CI would measure.
- Areas that went down: 7 subgrid reftests that passed only because upstream Servo rejects `subgrid` (Armature parses
  it, but layout does not support it), two `skew()` reftests whose edges differ by one colour step since SVG images
  are sized differently, and three `css-viewport` tests that upstream Servo passes only because it ignores `zoom` in
  both the test and the reference. The other small drops in the table (WebGL, XMLHttpRequest, Focus and a few
  single tests) come from tests that crash, time out or fail intermittently when the whole suite runs. Three
  Servo-specific animation tests now fail because they expect an infinitely repeating animation to show its last
  frame, not its first, exactly at the end of an iteration, which the Web Animations timing model does not do.

## What has been done

- **WICG [Observable API](https://wicg.github.io/observable/)**, behind the `dom_observable_enabled` preference
  (off by default; enabled for the tests by `tests/wpt/meta/dom/observable/__dir__.ini`): `Observable`, `Subscriber`,
  `EventTarget.when()`, every operator, and `Observable.from()` for Observables, iterables, async iterables and
  promises. `tests/wpt/tests/dom/observable` went from 523 failing subtests to 1.
- **HTML reflecting attributes**: about 130 missing IDL attributes (table, table cell, row and column, heading and
  `body` colors, `ol`/`li`/`dl`, `embed`/`object`/`param`/`area`, frames, `marquee`, form controls, `meta.media`,
  `a.download`/`ping`, `enterKeyHint`, `inputMode` and more). The `html/dom/reflection-*` tests went from about
  5,300 failing subtests to about 85. The enumerated ARIA attributes (`ariaBusy`, `ariaLive`, ...) are behind the
  `dom_aria_enumerated_reflection_enabled` preference.
- **`ProcessingInstruction` attributes** (`getAttribute()`, `setAttribute()` and friends), behind the
  `dom_processing_instruction_attributes_enabled` preference: 140 failing subtests down to 26.
- **Events**: `DragEvent`, `DeviceMotionEvent` and `DeviceOrientationEvent` and their `createEvent()` aliases; the
  `<body>`/`<frameset>` window event-handler attributes now forward `onblur` and the frameset handlers.
- **MIME types**: Request and Response `blob()` keep non-ASCII MIME parameters, and navigation quotes parameters it
  re-parses (so `charset` is no longer lost). `mimesniff/mime-types/parsing` went from 1,916 failing subtests to 1,402.
- **[Popover API](https://html.spec.whatwg.org/multipage/popover.html)**, behind the `dom_popover_enabled`
  preference (off by default; enabled for the tests): the `popover` attribute (`auto`, `manual` and `hint`),
  `showPopover()`/`hidePopover()`/`togglePopover()`, `beforetoggle`/`toggle` events, `popovertarget` on buttons and
  inputs, light dismiss, nested popovers, and dialog and fullscreen integration. `html/semantics/popovers` went from
  3,902 failing subtests to 79. Pressing Escape now also closes dialogs and close watchers.
- **WebCrypto hybrid KEMs**: MLKEM768-P256 and MLKEM1024-P384, next to the existing MLKEM768-X25519. The
  `WebCryptoAPI/modern-algos` tests have no failures left (2,036 fixed).
- **Streams**: async iteration of `ReadableStream` (`for await`, `values()`) and `ReadableStream.from()`, built on new
  generic support for `async_iterable` declarations in the WebIDL code generator: 320 failing subtests down to 0.
- **`HTMLElement.draggable`** (323 subtests), **fractional `MouseEvent` coordinates** as CSSOM View specifies (433),
  and the current HTML rules for **valid custom element names** (113). Element names with code points above U+10FFF,
  such as most emoji, are no longer rejected.
- **Editing commands** (`document.execCommand`, behind Servo's `dom_exec_command_enabled` preference):
  `justifyCenter`/`justifyFull`/`justifyLeft`/`justifyRight`, `insertOrderedList`/`insertUnorderedList`, `outdent`,
  `formatBlock` and `insertHTML`, plus fixes to the shared editing algorithms (line-break visibility, splitting a
  parent, sublist normalization, the backColor/hiliteColor value override) and the `align` attribute of table sections,
  rows and cells. The `editing` tests went from 23,799 failing subtests to 15,304.
- **SVG image sizing**: SVG images get their natural width, height and ratio from the outermost `svg` element as SVG 2
  specifies, and `img.naturalWidth`/`naturalHeight` follow the default sizing rules. `naturalWidth-naturalHeight-width-height`
  went from 182 failing subtests to 0, and about 340 more pass in the image, background, sizing and SVG tests.
- **Vendored Stylo**: Stylo, Servo's CSS engine, is vendored in `third_party/stylo/` as an unmodified import plus a
  log of every Armature change (`third_party/stylo.md`), so changes can be ported to and from upstream Stylo.
  `./mach test-tidy` checks that the log matches the actual diff.
- **CSS features behind Stylo preferences**: Servo now exposes 65 of Stylo's own `layout.css.*` preferences (51 are on
  in experimental mode; the rest would reach unimplemented layout code). About 50 standard properties that Stylo
  built only for Firefox are enabled behind new preferences: scroll snapping, `scroll-margin`/`scroll-padding`,
  `scrollbar-gutter`, `overflow-anchor`, scroll and view timelines, the SVG presentation properties, `accent-color`,
  `print-color-adjust`, `image-orientation`, `contain-intrinsic-size`, `content-visibility`, `break-*`, column rules,
  `counter-set`, `margin-trim`, `resize`, `field-sizing`, `shape-*`, `offset-*` (motion path), `transform-box` and
  `text-decoration-skip-ink`. In layout, `scrollIntoView()` honours scroll margin and padding, `transform-box` works,
  the SVG properties are mapped from presentation attributes, and lists use the `list-item` counter (including
  `<ol start reversed>` and `<li value>`). Most of the others are parsed, computed, serialised and animated but not
  laid out. The previously run CSS tests have about 5,300 fewer failing subtests, and newly enabled parsing and
  animation directories add 6,807 passing subtests.
- **SVG documents and SVG scripts**: navigating to an `image/svg+xml` resource creates an SVG document (it used to
  be shown as an image inside an HTML page, with no DOM or scripts), and SVG `<script>` elements run, sharing the HTML
  script processing model (`SVGScriptElement`, `document.currentScript`). Of the 256 testharness tests written as
  `.svg` files, which all errored, 249 now complete, with about 1,300 passing subtests. SVG scripts inside inline
  `<svg>` in HTML documents run too. `svg` elements fire `load` once parsed (`<svg onload>`), and SVG `<image>`
  elements fetch their image and fire `load` or `error`.
- **Vendored html5ever**: the HTML parser is vendored in `third_party/html5ever/` with a change log
  (`third_party/html5ever.md`), checked by `./mach test-tidy` like the Stylo one. Its changes implement the
  parsing rules for SVG `script` end tags and self-closing `<script/>` in SVG, which upstream had not implemented,
  tell Servo about every element the parser closes, and give the `xmlns` attribute of inline SVG and MathML no
  prefix (it had the empty prefix, so its `name` was `":xmlns"`).
- **Processing instructions in HTML**: the HTML parser now parses `<?target data?>` into a `ProcessingInstruction`
  node, following the HTML Standard's new tokenizer states (it used to make a bogus comment; `<?xml ...>`,
  `<?xml-stylesheet ...>` and invalid targets still do). This fixes all 83 `processing-instructions` html5lib tree
  construction subtests in each of the three html5lib test files, 12 more html5lib subtests, the 20 parser subtests of
  the `ProcessingInstruction` attributes test, and 52 Sanitizer API subtests. The XML parser no longer creates a
  `ProcessingInstruction` node for the XML declaration (`<?xml version="1.0"?>`).
- **`xml-stylesheet` processing instructions**: `<?xml-stylesheet href="..."?>` in the prolog of a document now loads
  and applies its style sheet, as CSSOM requires: its pseudo-attributes (`href`, `type`, `media`, `title`,
  `alternate`, with character references) are parsed, `ProcessingInstruction.sheet` returns the sheet,
  `StyleSheet.ownerNode` can be the processing instruction, and the sheet blocks scripts in documents parsed by the
  XML parser. Changing the data reloads the sheet and removing the node removes it. This fixes 4 subtests,
  among them `ProcessingInstruction-escapes-1.xhtml` and `xml-stylesheet-blocking.xhtml`.
- **Referrer of style sheet resources**: images, fonts and imported style sheets that an external style sheet
  refers to are fetched with the style sheet's URL as the referrer and the referrer policy of its response, as the
  Referrer Policy spec requires (they used the document's). Stylo's URL data now carries that referrer (Stylo change
  AS-501). This fixes 11 subtests in `referrer-policy/css-integration`; the SVG ones there still fail because Servo
  does not fetch external SVG resources such as `fill: url(file.svg#paint)`.
- **XML serialization**: `XMLSerializer`, `innerHTML` and `outerHTML` in XML documents, and the drawing of inline SVG
  now use an implementation of the [DOM Parsing](https://w3c.github.io/DOM-Parsing/#xml-serialization) XML
  serialization algorithm instead of xml5ever's serializer. It keeps namespaces with prefixes and `xmlns`
  declarations as the spec says, escapes attribute values (`<`, `>`, `"`, tabs and newlines), writes doctypes with
  their public and system IDs, and makes `innerHTML`/`outerHTML` throw `InvalidStateError` for markup that would not
  be well-formed. 48 failing subtests and 14 reftests now pass in `domparsing`, `html/syntax`, the
  `ProcessingInstruction` tests and the SVG tests, among them inline SVG `<image>` elements with `data:` URLs that
  contain `<`. Inline SVG `<use>` also follows `xlink:href`. `XMLHttpRequest.send()` with an XML document uses it
  too (it used the HTML serializer), which fixes 4 `xhr` subtests.
- **Vendored xml5ever**: the XML parser is vendored in `third_party/xml5ever/` with a change log
  (`third_party/xml5ever.md`), checked by `./mach test-tidy` like the other vendored copies. Parsed XML documents now
  keep their namespace declarations (`xmlns`, `xmlns:p`) as attributes, in source order, which fixes 10 more
  `XMLSerializer` subtests and 7 `Node.lookupPrefix()` subtests.
- **Vendored markup5ever and CDATA sections**: markup5ever, which holds the interface (`TreeSink`) through which
  both parsers build the DOM, is vendored in `third_party/markup5ever/` with its own change log
  (`third_party/markup5ever.md`). A new `TreeSink` method lets the XML parser create CDATA sections, which it used to
  turn into text: `DOMParser` now produces `CDATASection` nodes (also empty ones), and CDATA sections survive a parse
  and serialize round trip, which fixes 14 `domparsing` subtests.
- **CSS `zoom`**: the `zoom` property, which Stylo supports and Armature enables, now works with the CSSOM.
  `element.style.zoom` and `getComputedStyle(element).zoom` exist (Stylo change AS-503); the client, offset and scroll
  geometry of an element (`clientWidth`, `offsetTop`, `scrollWidth`, `scrollTop`, `scrollTo()` and the rest) and the
  lengths that `getComputedStyle()` takes from layout are in unzoomed CSS pixels, as the CSS Viewport spec requires;
  and Stylo now zooms the `thin`/`medium`/`thick` line widths, the initial values of the line widths, `font-size` and
  `stroke-width`, and explicitly inherited lengths of many more properties (AS-502, AS-504 and AS-505). The `css-
  viewport` tests, which are mostly about zoom, now run: 143 tests, of which 83% pass.
- **Style sheet encoding**: an external style sheet without a byte order mark, an `@charset` rule or a charset in its
  `Content-Type` is decoded with its environment encoding (the `charset` attribute of its `<link>` or pseudo-attribute
  of its `xml-stylesheet` processing instruction, and otherwise the document's encoding) instead of UTF-8. This fixes
  all 11 failing `css-syntax/charset` tests and 4 `CSS2/syntax` tests.
- **Keyframe offsets**: in keyframes given to `element.animate()` and `KeyframeEffect`, `offset` is the keyframe
  offset again and the CSS `offset` property is `cssOffset`, as the Web Animations spec says (enabling the motion path
  properties had made `offset` name the CSS property, which broke about 50 subtests).
- **Web Animations API**, on in experimental mode (`dom_web_animations_enabled`): `Animation` has the playback model of
  the spec (`play()`, `pause()`, `finish()`, `cancel()`, `reverse()`, `updatePlaybackRate()`, `currentTime`,
  `startTime`, `playbackRate`, `playState`, `pending`, the `ready` and `finished` promises, and `finish` and `cancel`
  events), effects compute their timing (`getComputedTiming()`), `KeyframeEffect` takes timing options, property-indexed
  keyframes and a composite operation, and `element.animate()` plays the animation. Animations created by script now
  change the style of their targets: Stylo resolves their keyframes and composites them (`replace`, `add` and
  `accumulate`) over CSS animations (AS-506). The "Web Animations" and "Compositing Web Animations" subtests of the
  interpolation tests throughout `css/` went from about 12,100 failing to about 2,300, and `web-animations/interfaces`
  from 625 failing subtests to 211.
- **`CSSAnimation`, `CSSTransition` and `getAnimations()`**: CSS animations and transitions are now `CSSAnimation`
  and `CSSTransition` objects that `Document`, `ShadowRoot` and `Element.getAnimations()` return in composite order,
  so script can pause, seek, reverse, finish or cancel them and read their timing and keyframes
  (`KeyframeEffect.getKeyframes()`). Script times them with the same playback model as other animations; Stylo
  still generates them from the computed values and samples them at the progress script gives it (AS-507).
  `animation-play-state` and the `animation-*` properties keep applying until script overrides them with `play()`,
  `pause()` or `updateTiming()`, and their events (`animationstart`, `animationiteration`, `animationend`,
  `animationcancel`, `transitionrun`, `transitionstart`, `transitionend` and `transitioncancel`) follow the
  phase changes of CSS Animations 2 and CSS Transitions 2, sorted by their scheduled time, and carry the animation
  in `animation`. Animations with a zero duration now exist and fire their events. `css-animations` went from 63.5%
  to 84.7% and `css-transitions` from 60.9% to 83.2%, and the animated reftests that wait for
  `document.getAnimations()[0].ready` now run instead of timing out.
- `AbortController.abort()` now rethrows an exception from an abort algorithm, and `Document.contentType` returns only
  the essence.
- `./mach fmt` and the crown GC-rooting lint pass, and the tested directories have no unexpected results other than a
  few tests that also fail or are flaky on the unmodified tree.

## What has not been done

- One Observable subtest still fails: `when()` in a detached document, because ordinary event listeners still run in
  detached documents.
- `xml-stylesheet` processing instructions do not yet react when inserting or removing the document element moves them
  into or out of the prolog, and a style sheet loaded by `@import` falls back to the document's encoding instead of
  the encoding of the style sheet that imports it.
- Four subtests (two in `XMLSerializer-serializeToString.html`, two doctype subtests in `xml-serialization.xhtml`)
  expect behaviour that the DOM Parsing spec changed in August 2026 and that Armature follows: a doctype ID that
  contains `"` is quoted with `'`, and an attribute keeps its own prefix when the element does not declare that
  prefix itself (an open web-platform-tests change updates the tests). One more expects a redundant `xmlns=""` to be
  kept, which the spec drops.
- About 1,200 `mimesniff` Blob/File subtests expect `new Blob([], {type})` to parse and re-serialise the type, which
  Servo does not do; about 190 more need header values with control characters, which Servo's header storage rejects.
- 79 popover subtests still fail: layout has no real top layer or `::backdrop` (open popovers are drawn as
  `position: fixed`), focus navigation into popovers is not implemented, and 6 tests contradict the spec.
- `TextEncoder.encodeInto()` and `TextDecoder` streaming into a `SharedArrayBuffer` (about 210 subtests) fail because
  Servo has no cross-origin isolation, so shared memory is off in every global.
- Not enabled from Stylo: anchor positioning, `@scope`, `@starting-style`, `@page` margin rules, custom media,
  `background-clip: text`/`border-area`, grid lanes, `random()`, tree-counting functions and a few more, because
  Servo's layout or CSSOM would panic on them; `size`, `page-orientation` and `page`, which need `@page` support.
  `resize` has no resize handles, and `subgrid` is parsed but not laid out.
- Web Animations: animations of pseudo-elements created by script have no effect, and `iterationComposite`,
  replacing and removing animations (`persist()`, `onremove`) and `commitStyles()` are not implemented, nor the
  `pseudoElement` option of `getAnimations()`. A `CSSAnimation` per repeated name in `animation-name` is missing
  (Stylo matches animations by name), `getKeyframes()` does not keep keyframes that share an offset but not an easing
  apart and shows additive keyframe values already added, and a finished `CSSTransition` cannot be replayed once
  style has dropped it. Container query units in keyframes are not supported, as in style sheets.
- SVG documents are drawn by the same code as inline SVG, which handles `<use>` and nested images
  less well than drawing the file as an image did, so a few `.svg` reftests that used to pass now fail. It only
  draws `<image>` elements whose `href` is a `data:` URL. SVG `<style>` in SVG documents and SMIL animation are also
  missing.
- Many editing tests still fail: deleting across differently styled blocks, whitespace handling, and cases where the
  tests expect browser behaviour that differs from the editing spec (Armature follows the spec). `formatBlock` and
  `insertHTML` are disabled in `contenteditable=plaintext-only` hosts.
- Some reflection subtests still fail (`col.span`, `progress.max`, `meter`, `textarea.rows`/`cols`, `pre.width`), as
  do 30 in the tentative ARIA test.
- Conformance outside the directories above has not been measured or improved, and other missing features have not
  been implemented.
- Nothing here has been reviewed by the upstream Servo maintainers.

For Servo's documentation, contribution guidelines and community channels, see the
[Servo project](https://github.com/servo/servo). The build instructions below are Servo's own.

## Getting started

For more detailed build instructions, see the Servo Book under [Getting the Code] and [Building Servo].

[Getting the Code]: https://book.servo.org/building/getting-the-code.html
[Building Servo]: https://book.servo.org/building/building.html

### macOS

- Download and install [Xcode](https://developer.apple.com/xcode/) and [`brew`](https://brew.sh/).
- Install `uv`: `curl -LsSf https://astral.sh/uv/install.sh | sh` 
- Install `rustup`: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- Restart your shell to make sure `cargo` is available
- Install the other dependencies: `./mach bootstrap`
- Build servoshell: `./mach build`

### Linux

- Install `curl`:
  - Arch: `sudo pacman -S --needed curl`
  - Debian, Ubuntu: `sudo apt install curl`
  - Fedora: `sudo dnf install curl`
  - Gentoo: `sudo emerge net-misc/curl`
- Install `uv`: `curl -LsSf https://astral.sh/uv/install.sh | sh` 
- Install `rustup`: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- Restart your shell to make sure `cargo` is available
- Install the other dependencies: `./mach bootstrap`
- Build servoshell: `./mach build`

### Windows

- Download [`uv`](https://docs.astral.sh/uv/getting-started/installation/#standalone-installer), and [`rustup`](https://win.rustup.rs/)
  - Be sure to select *Quick install via the Visual Studio Community installer*
- Ensure that [`winget`](https://learn.microsoft.com/en-us/windows/package-manager/winget/) is available. It should be preinstalled on Windows 10 1809+ and Windows 11, otherwise can be [`manually installed`](https://github.com/microsoft/winget-cli#installing-the-client).
- In the Visual Studio Installer, ensure the following components are installed:
  - **Windows 10/11 SDK (anything >= 10.0.19041.0)** (`Microsoft.VisualStudio.Component.Windows{10, 11}SDK.{>=19041}`)
  - **MSVC v143 - VS 2022 C++ x64/x86 build tools (Latest)** (`Microsoft.VisualStudio.Component.VC.Tools.x86.x64`)
  - **C++ ATL for latest v143 build tools (x86 & x64)** (`Microsoft.VisualStudio.Component.VC.ATL`)
- Restart your shell to make sure `cargo` is available
- Install the other dependencies: `.\mach bootstrap`
- Build servoshell: `.\mach build`

### Android

- Ensure that the following environment variables are set:
  - `ANDROID_SDK_ROOT`
  - `ANDROID_NDK_ROOT`: `$ANDROID_SDK_ROOT/ndk/28.2.13676358/`
 `ANDROID_SDK_ROOT` can be any directory (such as `~/android-sdk`).
  All of the Android build dependencies will be installed there.
- Install the latest version of the [Android command-line
  tools](https://developer.android.com/studio#command-tools) to
  `$ANDROID_SDK_ROOT/cmdline-tools/latest`.
- Run the following command to install the necessary components:
  ```shell
  sudo $ANDROID_SDK_ROOT/cmdline-tools/latest/bin/sdkmanager --install \
   "build-tools;36.0.0" \
   "emulator" \
   "ndk;29.0.14206865" \
   "platform-tools" \
   "platforms;android-37" \
   "system-images;android-37;google_apis;x86_64"
  ```
- Follow the instructions above for the platform you are building on
