# Armature: Servo, accelerated with AI

Armature is an unofficial fork of [Servo](https://github.com/servo/servo) that uses AI to accelerate development of
the engine. It is built with an AI coding assistant (Claude Code), and progress is measured by
[web-platform-tests](https://web-platform-tests.org/) conformance. It is **not** affiliated with or endorsed by the
Servo project.

Servo's [AI usage policy](https://book.servo.org/policy/ai-usage.html) does not accept AI-generated contributions,
so this work is not intended for, and has not been sent to, upstream Servo.

## Conformance

Measured on 7 November 2026 by running the web-platform-tests that Armature runs (the 61,548 tests selected by
`tests/wpt/include.ini`) with two builds on the same machine: upstream Servo at the commit Armature forked from
(`90c3f623175`, 30 October 2026) and Armature at `568f69bdeac`. As on [wpt.fyi](https://wpt.fyi/), each test counts
once, with partial credit for the share of its subtests that pass; the last column counts individual subtests. In
total Armature passes 63,085 more subtests than upstream Servo (2,087,693 against 2,024,608).

| Area | Tests | Upstream Servo | Armature | Change (points) | Subtests newly passing |
|---|--:|--:|--:|--:|--:|
| CSS | 29,164 | 67.0% | 70.5% | +3.4 | +23,261 |
| HTML | 10,253 | 66.7% | 68.1% | +1.4 | +18,030 |
| WebGL conformance | 2,939 | 40.5% | 40.5% | +0.0 | -118 |
| Referrer Policy | 1,483 | 86.3% | 87.0% | +0.7 | +11 |
| SVG | 1,411 | 24.9% | 46.7% | +21.8 | +3,658 |
| Encoding | 1,345 | 98.3% | 98.3% | +0.0 | +9 |
| Servo-specific tests | 1,229 | 75.6% | 75.6% | -0.0 | +4 |
| Fetch | 957 | 66.6% | 66.8% | +0.2 | +47 |
| Content Security Policy | 918 | 79.5% | 80.0% | +0.5 | +10 |
| Editing | 860 | 49.8% | 51.4% | +1.6 | +8,857 |
| DOM | 845 | 76.3% | 85.9% | +9.6 | +2,107 |
| IndexedDB | 820 | 57.4% | 57.4% | +0.0 | +0 |
| WebSockets | 799 | 68.1% | 68.1% | +0.0 | +0 |
| MathML | 624 | 45.6% | 46.8% | +1.2 | +40 |
| WebAssembly | 615 | 65.3% | 65.4% | +0.0 | +1 |
| XMLHttpRequest | 440 | 92.1% | 92.3% | +0.3 | +5 |
| Mixed Content | 388 | 87.6% | 87.4% | -0.1 | -1 |
| Shadow DOM | 366 | 67.3% | 67.3% | +0.0 | +0 |
| Web Audio | 355 | 58.9% | 58.9% | +0.0 | +0 |
| Workers | 336 | 91.8% | 92.1% | +0.3 | +1 |
| Web Crypto | 334 | 97.5% | 99.4% | +1.9 | +2,036 |
| Streams | 317 | 91.6% | 94.1% | +2.4 | +320 |
| Trusted Types | 249 | 86.7% | 89.2% | +2.6 | +85 |
| Web Animations | 204 | 27.6% | 86.7% | +59.1 | +3,479 |
| Custom Elements | 203 | 76.1% | 79.5% | +3.4 | +156 |
| Upgrade Insecure Requests | 197 | 73.6% | 73.1% | -0.5 | -6 |
| DOM Parsing | 94 | 43.5% | 48.8% | +5.2 | +51 |
| Compression Streams | 78 | 70.2% | 73.7% | +3.5 | +63 |
| Clipboard | 61 | 25.8% | 27.4% | +1.6 | +1 |
| Focus | 43 | 60.9% | 60.9% | +0.0 | +0 |
| Scroll-driven Animations | 17 | 21.6% | 100.0% | +78.4 | +372 |
| Everything else | 3,604 | 69.5% | 69.6% | +0.1 | +606 |
| **Total** | **61,548** | **66.5%** | **69.4%** | **+2.8** | **+63,085** |

The CSS gains come mostly from Stylo features that Armature enables and from the Web Animations API, which the
interpolation tests throughout `css/` use, and whose `CSSAnimation` and `CSSTransition` objects most of
`css-animations` and `css-transitions` test. These are the CSS modules with the most subtests newly passing:

| CSS module | Tests | Upstream Servo | Armature | Change (points) | Subtests newly passing |
|---|--:|--:|--:|--:|--:|
| `motion` | 46 | 20.6% | 97.4% | +76.8 | +4,844 |
| `css-fonts` | 559 | 66.0% | 71.7% | +5.7 | +1,551 |
| `css-transforms` | 933 | 70.4% | 80.2% | +9.8 | +1,410 |
| `css-shapes` | 70 | 33.1% | 89.3% | +56.1 | +1,357 |
| `css-backgrounds` | 875 | 65.3% | 83.2% | +17.9 | +1,294 |
| `css-sizing` | 760 | 67.7% | 69.3% | +1.6 | +1,205 |
| `filter-effects` | 397 | 51.7% | 56.8% | +5.2 | +1,116 |
| `css-grid` | 2,516 | 49.1% | 49.6% | +0.5 | +887 |
| `css-ui` | 1,077 | 89.6% | 91.7% | +2.1 | +848 |
| `css-borders` | 296 | 21.9% | 23.3% | +1.4 | +770 |
| `css-text` | 1,983 | 72.3% | 73.4% | +1.1 | +750 |
| `selectors` | 833 | 70.0% | 78.9% | +8.9 | +710 |
| `css-text-decor` | 366 | 40.7% | 45.4% | +4.7 | +682 |
| `css-animations` | 209 | 55.7% | 84.7% | +29.0 | +614 |
| `css-multicol` | 171 | 88.7% | 95.8% | +7.0 | +554 |
| `css-lists` | 225 | 55.2% | 59.5% | +4.3 | +511 |
| `css-scroll-snap` | 28 | 25.0% | 92.9% | +67.9 | +350 |

Notes on the numbers:

- Both builds ran the same tests, including the directories that Armature added to `include.ini` and upstream
  Servo skips (all of `web-animations`, and CSS directories whose properties Armature enables). Tests that set a
  preference only Armature has (Observable, popovers, `ProcessingInstruction` attributes, ARIA reflection) error on
  the upstream build, whose engine lacks those features anyway.
- The Armature run matched the expectations recorded in `tests/wpt/meta` for all but about 450 tests, most of them
  WebGL conformance tests and tests that depend on the fonts of the machine, so the scores should be close to what
  Servo's CI would measure.
- The table predates the SVG hit-testing, path DOM, image sizing, canvas image, focus, conditional processing and
  SMIL work, as well as the final HTML reflection fixes described below; their gains are not included in these scores.
- Areas that went down: 7 subgrid reftests that passed only because upstream Servo rejects `subgrid` (Armature parses
  it, but layout does not support it), two `skew()` reftests whose edges differ by one colour step since SVG images
  are sized differently, and three `css-viewport` tests that upstream Servo passes only because it ignores `zoom` in
  both the test and the reference. The other small drops in the table (WebGL, where one test with 122 subtests
  crashed in this run, Upgrade Insecure Requests, Mixed Content and a few single tests) come from tests that crash,
  time out or fail intermittently when the whole suite runs.

## What has been done

- **WICG [Observable API](https://wicg.github.io/observable/)**, behind the `dom_observable_enabled` preference
  (off by default; enabled for the tests by `tests/wpt/meta/dom/observable/__dir__.ini`): `Observable`, `Subscriber`,
  `EventTarget.when()`, every operator, and `Observable.from()` for Observables, iterables, async iterables and
  promises. `tests/wpt/tests/dom/observable` went from 523 failing subtests to 1.
- **HTML reflecting attributes**: about 130 missing IDL attributes (table, table cell, row and column, heading and
  `body` colors, `ol`/`li`/`dl`, `embed`/`object`/`param`/`area`, frames, `marquee`, form controls, `meta.media`,
  `a.download`/`ping`, `enterKeyHint`, `inputMode` and more). The 11 automated `html/dom/reflection-*` files now
  pass all 61,302 subtests, down from about 5,300 failures. Table `span`, `colSpan` and `rowSpan` keep the assigned
  content attribute while clamping the reflected value, fixing 38 subtests. HTML floating-point reflection now
  parses numeric prefixes after ASCII whitespace and serializes exponent values as JavaScript does; `textarea`
  positive-value fallback, negative `pre.width`, and empty `track.kind` reflection close the remaining 47 failures
  in the seven-file baseline. The enumerated ARIA attributes (`ariaBusy`, `ariaLive`, ...) are behind the
  `dom_aria_enumerated_reflection_enabled` preference.
- **Fetch priority reflection**: `fetchPriority` reflects `fetchpriority` on HTML `img`, `link` and `script`, and SVG
  `image` and `script` elements. All 10 focused subtests now pass across five files.
- **`ProcessingInstruction` attributes** (`getAttribute()`, `setAttribute()` and friends), behind the
  `dom_processing_instruction_attributes_enabled` preference: all 140 focused subtests pass. An
  `xml-stylesheet` processing instruction now updates its style sheet when inserting or removing the document
  element changes whether the instruction is in the prolog.
- **Events**: `DragEvent`, `DeviceMotionEvent` and `DeviceOrientationEvent` and their `createEvent()` aliases; the
  `<body>`/`<frameset>` window event-handler attributes now forward `onblur` and the frameset handlers.
- **MIME types**: Request and Response `blob()` keep non-ASCII MIME parameters, and navigation quotes parameters it
  re-parses (so `charset` is no longer lost). `mimesniff/mime-types/parsing` went from 1,916 failing subtests to 1,402.
- **[Popover API](https://html.spec.whatwg.org/multipage/popover.html)**, behind the `dom_popover_enabled`
  preference (off by default; enabled for the tests): the `popover` attribute (`auto`, `manual` and `hint`),
  `showPopover()`/`hidePopover()`/`togglePopover()`, `beforetoggle`/`toggle` events, `popovertarget` on buttons and
  inputs, button `commandForElement` reflection and activation, light dismiss, nested popovers, and dialog and
  fullscreen integration. Open popovers now join sequential focus navigation at their invoker, including across
  slots and with explicit tabindex values. The fallback `<summary>` in a `<details>` without an explicit summary
  now participates in focus navigation, and WebDriver keys sent to an active shadow host reach its focused
  descendant. Twelve more focus subtests pass. An open popover used as the `source` of another popover now remains its
  ancestor, preserving auto and hint stacks; six more subtests pass. Canceled form resets no longer block later
  resets, empty labels inside invoker buttons allow button activation, and pointer events on disabled controls now
  light-dismiss open popovers; six more subtests pass. Light dismiss now ignores right clicks and touch scrolls, and
  leaves a popover open if its press target is removed; three more subtests pass. Activation now uses the original
  click target across shadow boundaries, so a nested popover stays open when clicked; one more subtest passes.
  A displayed popover with an otherwise generic accessibility role now maps to `group`, and its role updates when
  display or the `popover` attribute changes; two more subtests pass. `html/semantics/popovers` has 25 failures in
  the latest run, down from 3,902. Pressing Escape now also closes
  dialogs and close watchers.
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
- **CSS import encoding**: `@import` now uses the encoding actually selected for its parent style sheet as its
  fallback, including through nested imports. Four focused subtests cover HTTP `charset`, `@charset`, UTF-16 BOMs,
  and a second import level; all 21 nearby CSS charset tests still run as expected.
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
- **CSSOM border-box sizes**: `getComputedStyle()` now reports the used border-box width and height when
  `box-sizing: border-box`, including when padding and borders exceed the specified size. This fixes the
  `css/css-values/lh-unit-004.html` subtest for a `<select>` whose width uses `lh`.
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
- **Replaced animations, `persist()` and `commitStyles()`**: every update of the animations of a document removes the
  finished animations whose effects later animations fully cover (comparing physical longhands, so logical properties
  and shorthands count), which sets their `replaceState` to `removed`, takes them out of style and
  `getAnimations()`, and fires `remove` events after the `finish` events. `persist()` brings one back, and
  `commitStyles()` writes the result of the effect stack up to the animation into the `style` attribute as one
  change, counting animations exactly at the end of their active interval as filling, as other engines do. All of
  `web-animations/` now runs (it was mostly skipped), and the Web Animations area went from 64.6% to 86.7%.
- **The SVG DOM**, which had almost no members: the animated value types (`SVGAnimatedLength`, `-Number`,
  `-Integer`, `-Boolean`, `-Enumeration`, `-String`, `-Angle`, `-Rect`, `-PreserveAspectRatio`, `-LengthList`,
  `-NumberList` and `-TransformList`) with `SVGLength`, `SVGAngle`, `SVGNumber`, `SVGPreserveAspectRatio`,
  `SVGTransform` and the list interfaces, which reflect their attributes and synchronize their items with them as
  SVG 2 specifies; the attributes of every element interface (shapes, gradients, patterns, markers, filters, text,
  `href`, `viewBox`, `preserveAspectRatio`, `points`, `transform`, `requiredExtensions`, `systemLanguage` and the
  hyperlink members of `<a>`); 39 element interfaces that were missing (filters and their primitives, `clipPath`,
  `mask`, `marker`, `foreignObject`, `textPath`, `style`, `view`, `switch`, `title`, `desc`, `metadata`, `mpath`);
  and `getBBox()`, `getCTM()`, `getScreenCTM()`, `getTotalLength()`, `getPointAtLength()`, `isPointInFill()`,
  `isPointInStroke()` and the character methods of text elements (`getNumberOfChars()`, `getExtentOfChar()` and
  so on). Servo draws SVG with resvg, so layout knows nothing about SVG geometry; these compute it from the attributes
  (with kurbo) and, for text, from the fonts. Lengths convert between all the units of CSS, resolving font-relative
  ones with the metrics of the element's font. `<style>` elements in SVG now apply their style sheets, every
  presentation attribute of SVG 2 maps to its property, and hit testing (`elementFromPoint()` and the targets of
  mouse events) finds the shape under the point inside an `svg` element, which layout treats as one box. The SVG
  area went from 36.5% to 49.2% before hit testing (the table above was measured earlier still, at 46.7%), and hit
  testing, with the SVG values of `pointer-events` that Stylo now builds for Servo (AS-508), makes about 4,700 more
  subtests pass. SVG 2 `pointer-events: bounding-box` (AS-511) now parses and hit-tests group, shape, text and image
  bounds, including their edges. Eighteen pointer-events subtests and three text hit-test subtests now pass. A
  subsequent fix makes `getPointAtLength()` return the move point of a move-only path and throw
  `InvalidStateError` for an empty path, bringing 22 more SVG subtests into conformance. The path DOM APIs
  (`getPathData()`, `setPathData()` and `getPathSegmentAtLength()`) now pass 79 more subtests across 19 SVG path
  interface files, including normalization, malformed data, mutation records and length boundaries. SVG `<image>`
  now uses resource natural dimensions for `auto` sizing in `getBBox()`, and waits for image loads before the document
  load event; all 33 focused subtests pass across two geometry files. SVG `<image>` also loads when detached and
  reflects `crossOrigin`; canvas drawing and `createImageBitmap()` use its decoded raster pixels and CORS status.
  Four SVG image CORS subtests and three image bitmap origin subtests now pass. Loaded SVG images can now be rasterized
  for canvas drawing and `createImageBitmap()` at its requested output size. Nested data URL SVGs inherit their
  embedding image viewport instead of their painted bounds. Three nested canvas reftests, two other SVG image
  reftests, the SVG bitmap resize reftest and one `drawImage()` subtest now pass. SVG elements now expose `dataset`,
  SVG links participate in focus navigation, and unrendered SVG definition elements stay out of it. Invalid
  `tabindex` values no longer create focusable areas where the element has no default focus behavior. The targeted SVG
  focus and dataset tests pass, alongside eight previously failing HTML focus subtests.
  `getScreenCTM()` now includes CSS transforms of the outermost `<svg>` box and its HTML ancestors, plus
  CSS transforms and padding on nested SVG viewports. Eight more SVG scripted subtests pass. Invalid tokens in SVG
  `points` attributes now clear the reflected `SVGPointList`, while an unmatched final coordinate still leaves
  complete points in place. SVG-created points and rectangles now reject non-finite coordinate assignments, while
  ordinary DOM geometry objects still accept them; animated SVG point-list items throw when modified. Eight more
  subtests pass since the CTM work. SVG `lh` and `rlh` lengths now resolve against the element and root line heights,
  including `normal` from the first available font. Four more SVG scripted subtests and one CSS line-height subtest
  pass. `isPointInFill()` now includes points on circle, ellipse, path and line boundaries, so another SVG scripted
  subtest passes. `SVGFECompositeElement` now reflects the `lighter` operator through its animated enumeration, and
  SVG text geometry removes CSS zoom from font metrics when answering `getBBox()`. SVG path geometry now uses the
  computed CSS `d` coordinates directly, preserving path length precision after `setPathData()` without changing
  CSSOM `d` serialization. Three more scripted subtests pass from these fixes; the 97-file cluster now has 11
  failures, down from 19.
  SVG `systemLanguage` now preserves empty comma-separated tokens in its DOM list, excludes nonmatching graphics from
  geometry, hit testing and focus, and selects the first matching `<switch>` child. The SVG rasterizer uses the
  browser's language preference, so image pixels follow the same conditions. All 27 focused parsing and matching
  subtests and the new raster and focus tests pass.
  Allowing SVG descendants to participate
  in style traversal makes CSS `d` interpolation work in dynamically styled SVG subtrees, bringing 180 more subtests into
  conformance across three path property files, plus one geometry subtest. SVG roots now have independent animation
  clocks with pause, resume and seek controls, and a simple `<animate>` child interpolates `SVGAnimatedLength.animVal`.
  The pause/seek and SVG reinsertion timing tests now pass three subtests across two files. SVG animation elements
  expose their DOM interfaces and timing methods, including scheduled begin and end instance times, interval queries,
  and SMIL clock parsing; 23 more subtests pass across four timing and clock files. A local WPT covers DOM instance
  times, frozen values after an early end, and nested SVG fragments sharing the outermost clock. Length animations
  now compose `by`, `from`/`by`, `from`/`to`, `to`, and `values` effects across SVG length units, with frozen effects
  and `additive="sum"`; 31 SVG length animation subtests pass while `baseVal` remains unchanged. Enumerated SVG
  attributes now sample discrete `from`/`to` and `values` animations, making 13 more subtests pass. Numeric and integer
  SVG DOM attributes now sample interpolated values, including optional number pairs, making eight more subtests pass;
  three invalid-value tests already passed but still had failure expectations. Scheduled SMIL begin/end events,
  `onbegin`/`onend` handlers, eventbase timing dependencies, and `<set>` length values now pass the event listener,
  begin attribute mutation, begin event, and end event tests. Basic syncbase dependencies now resolve during playback
  and paused timeline seeks, including cyclic begin/end chains and escaped dots in referenced IDs; six more timing
  tests pass. Seeking queues begin and end events when the active interval changes and skips events for intervals or
  repeats crossed entirely by the seek; eight seeking event tests now pass, and the large-time seek test also passes.
  Repeat boundaries now dispatch `repeatEvent` and resolve `repeat(n)` dependencies, making six more repeat tests pass.
  Seeking during a repeat event preserves dependent begin events, and SVG animations honor ancestor `systemLanguage`
  conditions. SMIL now sends CSS presentation property effects, including `fill` and `d`, through the style animation
  compositor, and dispatches timing boundaries across animations in clock order. The animated `d` style leaves
  `getPathData()` on the base attribute. Discrete sampling, `keyTimes` and spline easing now share one timing
  calculation across SVG DOM values and animated style. `repeatDur` and `repeatCount` now determine the active
  interval, and repeated length and numeric animations sample each iteration with `accumulate="sum"` where requested.
  SVG string and boolean `animVal` values now follow discrete animations; a single `values` entry and an indefinite
  simple duration keep the first specified value, and CSS `visibility` uses discrete SMIL timing.
  Animated marker `orient` angles interpolate across degree, radian and gradian units; `orientAngle.animVal` and
  `orientType.animVal` also follow transitions between an angle and `auto`.
  Animated SVG length and number lists now sample `values`, `from`/`to`, `from`/`by`, additive, and repeated effects
  while keeping `baseVal` independent. Animated polygon and polyline point lists also sample these effects while
  keeping malformed animation values from replacing the base list. All eleven targeted list animation failures pass.
  SVG motion now contributes its sampled path or coordinate-pair translation and rotation to DOM geometry, including
  referenced shapes, discrete timing, key points, repeats, and zero-length paths. The 39 previously failing motion
  and related timing subtests in this selection now pass. SVG 2's `auto` ellipse radius is reflected in geometry.
  `animateTransform` now updates transform-list `animVal` and CTM from the same sampled effect, including discrete
  timing, accumulation, additive composition, and the default `translate` type. Seven more animation subtests pass.
  SVG path `d` animations now use path addition for `by` effects and accumulate compatible paths across repeats,
  while invalid or incompatible path additions leave the base value intact. Five more animation subtests pass.
  SMIL eventbase timing now observes DOM events, including bubbling and events on non-SVG elements. Effects that begin
  later take priority, and unitless SVG lengths reach the CSS compositor as lengths. Discrete `display` effects now
  resolve in Stylo. Five more selected SVG animation subtests pass and the four timeouts are gone. In the focused CSS
  display animation checks, 26 formerly expected failures now pass; other display animation failures remain.
  SVG length effects now compose by interval start time, so later frozen effects take priority even when their
  elements appear earlier in the DOM. The rewind and multiple-animation ending tests now pass.
  An immediate `beginElement()` also consumes past DOM end instance times, so an `endElement()` call on an inactive
  animation no longer prevents it from starting at the same clock instant.
  SVG geometry now samples the same animated lengths as `animVal`, so `getBBox()` reflects active length animations;
  invalid `keySplines` control points on either axis leave the animation unapplied. Both spline-limit tests pass.
  SVG length animations now evaluate `calc()` values with arithmetic precedence and SVG unit conversion, including
  viewport-relative percentages. Two additive length tests now pass, with local coverage for nested calculations,
  invalid types and changing viewport size.
  Animated SVG `viewBox` values now reach `animVal` and the geometry used by `getCTM()`; both SVGRect animation tests
  pass, along with a local transform regression test.
  SMIL length values now follow animations that target an element through `href` or `xlink:href`, including when the
  target changes; `points` animations also stop affecting the XML DOM value when `attributeType` switches to CSS.
  Other animated SVG DOM value classes now use the same external target lookup, and signed `by` changes work for
  `viewBox` dimensions. Absolute color `by` animations sample channels in sRGB, including transparent colors and
  `currentColor` effects. The multi-type additive test and two color tests now pass.
  SMIL `class` effects now participate in CSS selector matching while leaving the DOM attribute unchanged; styles
  return to the base class when the effect ends. Both affected upstream tests and a local regression test pass.
  SVG graphics elements now expose transformed client rectangles, including text ranges, while respecting hidden
  ancestors and stylesheet `display: none`. Event clicks on SVG children can reach SMIL timing, and `restart="never"`
  keeps an animation to its first interval.
  Paced numeric SMIL animations place keyframes by value distance, ignoring `keyTimes`. The SVG 2 `path-length` CSS
  property now parses, computes, and interpolates; `pathLength` presentation attributes and SMIL animations feed it.
  The focused parsing and interpolation tests gained 47 passing subtests. The SVG rasterizer now calibrates stroke
  dashes and text path start offsets with CSS `path-length` or the `pathLength` attribute. All 15 SVG path distance
  reftests now pass, including 12 that previously had failure expectations and all four CSS rendering cases.
  Computed `d` values now serialize with absolute path commands, including inherited and animated values. The five
  path serialization subtests and the last two SVG animation subtests in this selection now pass.
  CSS `d: path(...)` and `d: none` now determine rasterized path geometry; SVG DOM geometry queries also use the
  computed `d` value. XML path data is escaped when parsed as a CSS presentation hint, preserving SVG whitespace;
  malformed XML data still renders its valid prefix. Three rendering reftests, four DOM geometry subtests and one
  path number parsing subtest now pass. The `mpath.svg` reftest now renders motion along a path
  supplied by CSS `d`: the serializer writes the sampled motion transform to its rendering copy
  without changing the live element's transform attribute.
  SVG path geometry now keeps complete segments before malformed parameters, rejects trailing decimal points, and
  returns positive zero for empty paths. Empty shapes no longer enlarge their parent group's bounding box. Thirteen
  more path error-handling subtests pass across four files.
  SVG bearing commands now orient relative path segments for both rendering and DOM geometry queries. The absolute,
  relative and zero-bearing reftests pass; the 72-test `svg/path` run matches expectations.
  Seeking an SVG animation clock refreshes its cached raster image. The serializer also applies active `<animate>`
  and `<set>` presentation property values to the rendering copy, so the scripted motion and rewind/freeze reftests
  now pass.
  SVG descendant attribute and child mutations now refresh the outermost raster. SMIL presentation-property effects
  also refresh it when they start, end, or cross a discrete keyframe; four more SVG animation reftests now pass.
  Active continuous `<animate>` presentation-property and `<animateMotion>` effects now refresh the raster during
  playback. Redraw waits for the document load and initial font readiness, and screenshot capture lets the current
  frame settle. Evicting an in-flight SVG image wakes its pending listeners so canceled frames cannot block a
  screenshot. A new mid-animation fill reftest passes alongside the existing indefinite fill and motion reftests.
  All 86 path interface subtests and 33 SVG image sizing subtests pass. Across the same 343 SVG animation subtests,
  the broader run moved from 145 PASS / 151 FAIL / 47 TIMEOUT to 343 PASS / 0 FAIL / 0 TIMEOUT, with no previously
  passing subtest regressing. Animations excluded by `systemLanguage` or an unselected `<switch>` branch no longer
  affect their targets. The rasterized SVG copy receives sampled geometry lengths as attributes, so both `<switch>`
  animation reftests now pass. On-demand style queries resolve animation keyframes in hidden SVG
  subtrees, and the rendering copy samples effects on definitions imported through `<use>`.
  Imported IDs are tracked so changes in their source subtree refresh the visible SVG raster. The
  two hidden-symbol animation reftests now pass; the media-fragment override is the one remaining
  expected-fail reftest in the SVG animation run.
  Cyclic SVG pattern paint references now use their declared fallback color instead of overflowing
  the image parser's stack. All 23 SVG crashtests pass, including the pattern cycle test that
  previously reported PASS immediately before the browser crashed.
- **SVG image URL fragments**: External SVG images crop `xywh` fragments in pixel or percent units, clamp the
  selection to the source image, and expose the cropped intrinsic size for CSS backgrounds and `<img>`. All 19
  spatial fragment reftests pass, including 12 that previously failed. `svgView(...)` fragments now override root
  view attributes, including `viewBox`, `preserveAspectRatio`, and `transform`; named `<view>` elements work in SVG
  data URLs loaded by `<object>`.
  The renderer resolves selected views against the image viewport, preserving aspect ratio behavior. Eleven more
  linking reftests pass, leaving 7 expected failures in the 55-test run, down from 30 before fragment support.
  Inline SVG rendering now carries resolved CSS `fill` into its rasterized copy, including animated values, and
  SVG anchors with either `href` or `xlink:href` match `:link`. Three more linking reftests pass, leaving four
  expected failures in the latest 55-test run.
- **SVG geometry properties**: Negative `r`, `rx`, `ry`, `width` and `height` presentation attributes are ignored, and
  `<foreignObject>` maps its geometry attributes to CSS properties. `getComputedStyle()` resolves the used width and
  height of SVG rectangles, images and foreign objects against their SVG viewport, including percentages and
  `calc()`. A nested `<svg>` keeps its width and height attributes for viewport sizing without treating them as CSS
  sizing declarations. All 308 subtests in `svg/geometry/parsing` now pass, including 60 that previously failed; the
  surrounding 86-file `svg/geometry` run matches expectations.
- **SVG bounding boxes**: `getBBox()` now includes stroked path and line outlines when requested, preserves the
  position of move-only paths and single-point polygons, measures foreign objects, and excludes images with no
  source or nonpositive dimensions. In the 97-file `svg/types/scripted` run, 22 previously failing subtests now pass
  with no passing subtest regressing. Its optional marker and clipping results now include marker content at path
  vertices, SVG clip paths (including nested paths and `objectBoundingBox` units), and legacy CSS `clip` rectangles.
  All 15 subtests in `SVGGraphicsElement.getBBox-05.html` pass; the same 97-file suite gained seven more passes,
  reaching 987 passing and 41 failing subtests with no passing subtest regressing.
- **SVG stroke hit testing**: `isPointInStroke()` now uses the stroked outline, including caps, joins, dash patterns
  and offsets, `pathLength` calibration, and non-scaling strokes. Both focused stroke files pass all subtests. The
  97-file `svg/types/scripted` suite gained six passes, reaching 993 passing and 35 failing subtests without a
  previously passing subtest regressing.
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
- `./mach fmt` and the crown GC-rooting lint pass, and the tested directories have no unexpected results other than a
  few tests that also fail or are flaky on the unmodified tree.

## What has not been done

- Four subtests (two in `XMLSerializer-serializeToString.html`, two doctype subtests in `xml-serialization.xhtml`)
  expect behaviour that the DOM Parsing spec changed in August 2026 and that Armature follows: a doctype ID that
  contains `"` is quoted with `'`, and an attribute keeps its own prefix when the element does not declare that
  prefix itself (an open web-platform-tests change updates the tests). One more expects a redundant `xmlns=""` to be
  kept, which the spec drops.
- About 1,200 `mimesniff` Blob/File subtests expect `new Blob([], {type})` to parse and re-serialise the type, which
  Servo does not do; about 190 more need header values with control characters, which Servo's header storage rejects.
- 25 popover subtests still fail in the latest run: layout has no real top layer or `::backdrop` (open popovers are
  drawn as `position: fixed`), some focus navigation cases remain incomplete, four inactive-document cases expect
  listeners to run despite the DOM Standard's fully active document check, and 6 other tests contradict the spec.
- `TextEncoder.encodeInto()` and `TextDecoder` streaming into a `SharedArrayBuffer` (about 210 subtests) fail because
  Servo has no cross-origin isolation, so shared memory is off in every global.
- Fetch priority hints are reflected in the DOM but do not yet affect network request scheduling.
- Not enabled from Stylo: anchor positioning, `@scope`, `@starting-style`, `@page` margin rules, custom media,
  `background-clip: text`/`border-area`, grid lanes, `random()`, tree-counting functions and a few more, because
  Servo's layout or CSSOM would panic on them; `size`, `page-orientation` and `page`, which need `@page` support.
  `resize` has no resize handles, and `subgrid` is parsed but not laid out.
- Web Animations: animations of pseudo-elements created by script have no effect, and `iterationComposite` is not
  implemented, nor the `pseudoElement` option of `getAnimations()`. `commitStyles()` of an animation below others
  briefly recomputes the style of the target without them, which can start transitions. A `CSSAnimation` per repeated name in `animation-name` is missing
  (Stylo matches animations by name), `getKeyframes()` does not keep keyframes that share an offset but not an easing
  apart and shows additive keyframe values already added, and a finished `CSSTransition` cannot be replayed once
  style has dropped it. Container query units in keyframes are not supported, as in style sheets.
- SVG documents are drawn by the same code as inline SVG, which handles `<use>` and some nested images
  less well than drawing the file as an image did, so a few `.svg` reftests that used to pass now fail. It only
  draws `<image>` elements whose `href` is a `data:` URL. SMIL effects cover basic lengths, enumerations, numbers,
  presentation properties and motion. Full repeat and syncbase
  timing, `TimeEvent` details, and many animated attributes are still missing. Most geometry
  methods still use attributes rather than CSS geometry properties, though paths now use computed `d`. Script's text
  geometry is measured glyph by glyph without shaping, `textPath`, `dominant-baseline` or vertical writing.
- SVG `getBBox()` intersects clipping bounds as rectangles rather than calculating the exact clipped silhouette, and
  marker sizing does not cover every `preserveAspectRatio` and orientation case. Five WPT cases expect zero for
  graphics inside definitions, clip paths and masks; Armature returns their own geometry, as the current
  [SVG 2 bounding-box text](https://www.w3.org/TR/SVG2/coords.html#BoundingBoxes) describes.
- SVG DOM stroke hit testing does not yet resolve percentage dash lengths or CSS transforms for non-scaling strokes.
- Many editing tests still fail: deleting across differently styled blocks, whitespace handling, and cases where the
  tests expect browser behaviour that differs from the editing spec (Armature follows the spec). `formatBlock` and
  `insertHTML` are disabled in `contenteditable=plaintext-only` hosts.
- The tentative ARIA reflection test still has 30 failing subtests.
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
