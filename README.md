# Armature: Servo, accelerated with AI

Armature is an unofficial fork of [Servo](https://github.com/servo/servo) that uses AI to accelerate development of
the engine. It is built with an AI coding assistant (Claude Code), and progress is measured by
[web-platform-tests](https://web-platform-tests.org/) conformance. It is **not** affiliated with or endorsed by the
Servo project.

Servo's [AI usage policy](https://book.servo.org/policy/ai-usage.html) does not accept AI-generated contributions,
so this work is not intended for, and has not been sent to, upstream Servo.

## Conformance

Unless noted below, scores were measured on 7 November 2026 using the 61,548 web-platform-tests selected by
`tests/wpt/include.ini`. Two builds ran on the same machine: upstream Servo at the commit Armature forked from
(`90c3f623175`, 30 October 2026) and Armature at `568f69bdeac`. As on [wpt.fyi](https://wpt.fyi/), each test counts
once, with partial credit for the share of its subtests that pass; the last column counts individual subtests. The
Total row reports 63,770 more passing subtests than upstream Servo (2,088,378 against 2,024,608) and predates the
WebGL rerun.

| Area | Tests | Upstream Servo | Armature | Change (points) | Subtests newly passing |
|---|--:|--:|--:|--:|--:|
| CSS | 29,164 | 67.0% | 70.5% | +3.4 | +23,261 |
| HTML | 10,253 | 66.7% | 68.1% | +1.4 | +18,030 |
| WebGL conformance | 2,939 | 40.5% | **82.2%** | +41.7 | — |
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
| WebAssembly | 615 | 65.3% | 68.9% | +3.6 | +449 |
| XMLHttpRequest | 440 | 92.1% | 93.8% | +1.7 | +88 |
| Mixed Content | 388 | 87.6% | 87.4% | -0.1 | -1 |
| Shadow DOM | 366 | 67.3% | 68.2% | +0.9 | +28 |
| Web Audio | 355 | 58.9% | 62.0% | +3.1 | +126 |
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
| **Total** | **61,548** | **66.5%** | **69.5%** | **+2.9** | **+63,770** |

A CSS-only rerun on 12 November 2026 at `cd542dc6879` selected 29,199 tests and scored **71.3%** (20,829.7
test-equivalents passing, with eight skipped tests counted as nonpassing). This is one Armature run; the upstream
comparison above remains the 7 November measurement.

Work on non-replaced `aspect-ratio` raised the `css-sizing/aspect-ratio` directory from 150.75 to 211.594 passing
test-equivalents out of 295 tests. A CSS rerun on 13 November selected 34,847 tests; restricted to the same 29,199
test IDs as the 12 November run, it scored **71.6%** (20,914.5 test-equivalents passing, up 84.8). The expanded
selection includes directories skipped by `include.ini` and is not used for this comparison.
After the block-flow absolute alignment change, a targeted `css-position` and `css-align` rerun gained two passing
reftests across 687 common tests (499.122 to 501.122 test-equivalents), with no measured regressions in that set.
After that change and the SVG `transform-box: fill-box` fix, another CSS rerun selected the same 34,847 tests. On
the original 29,199 test IDs, it scored **71.8%** (20,965.5 test-equivalents passing, up 51.0 from the preceding
rerun), with no tests regressing. The two fixes account for 49 newly passing reftests; two unrelated reftests also
passed in this run and on a focused repeat, but may vary between runs.

After counter resolution and named counter-style rendering, a four-chunk CSS rerun covered the same 34,847 test
IDs. On the original 29,199 IDs, it scored **72.3%** (21,107.8 test-equivalents passing, up 142.3 from the preceding
rerun). A later four-chunk CSS rerun, after the shadow-tree counter fixes, quote-depth resolution, and emergency
wrapping at grapheme boundaries, covered the same 34,847 test IDs. On the original 29,199 IDs, it scored **72.7%**
(21,213.5 test-equivalents passing, up 105.7). Across the expanded selection, 113 previously failing reftests passed;
the largest groups were `css-text` (54), `css-content` (28), and `CSS2` (17). Three previously passing reftests failed in
the full run but all passed on a focused rerun; their full-run failures appear intermittent.
A subsequent block-container `margin-trim` change made 32 of 106 focused `css-box/margin-trim` tests newly pass, with no
status regressions. That directory is excluded by `include.ini`, so these gains are outside the 29,199-test score.
After principal writing-mode propagation and vertical absolute-position fixes, a four-chunk CSS rerun again covered
34,847 IDs. On the original 29,199 IDs, its raw score was **72.76%** (21,245.9 test-equivalents passing, up 32.3).
Thirty-three previously failing `css-writing-modes` reftests passed. Twelve previously passing tests that crashed or
timed out in the full run passed on an isolated rerun; substituting only those verified results gives **72.80%**
(21,257.9 test-equivalents, up 44.3). Four `css-inline` baseline variants and one vertical relative-position reftest
failed again on a focused rerun. The expanded selection also includes the 32 `margin-trim` gains noted above.
An additional focused run of all 934 `css-overflow` tests after direct-line clamping and ellipsis support gained
22.4 passing test-equivalents, including 21 previously failing `line-clamp` reftests and legacy shorthand parsing
subtests, with no status regressions. These gains are not yet included in the full CSS score above.
A later 324-test `line-clamp` rerun gained one more passing reftest after exporting the baseline of the last retained
line from an inline block. The popover case changed from TIMEOUT to FAIL when experimental WPT runs enabled popovers;
no previously passing test regressed. This gain is also outside the full CSS score above.
A focused run of all 320 `css-conditional/container-queries` tests after enabling `@container` rules and post-layout
query evaluation rose from 111.98 to 208.33 passing test-equivalents, up 96.35. It gained 449 passing subtests and
lost six, while the same three tests timed out in both runs. These gains are also not yet included in the full CSS
score above.
A focused CSS Cascade rerun after enabling `@scope` rose from 114.02 to 135.99 passing test-equivalents across
157 tests, up 21.97. Eight related tests in CSS Nesting, CSSOM, Selectors and CSS Mixins rose from 5.25 to 5.93,
up another 0.68. Across both sets, 206 subtests newly passed and three previously passing subtests failed. These
gains are not yet included in the full CSS score above.
CSS Typed OM transform values made the 244 explicitly selected style property tests rise from 0 to 164.07 passing
test-equivalents on the same test IDs as the expanded CSS baseline. The 12 direct transform suites rose from 0 to 12,
with every subtest passing. The property directory remains excluded by `include.ini`; the 12 direct suites are now
included. These focused gains are outside the full CSS score above. Five property files still error while
constructing unsupported `<position>` values; many other property subtests need `computedStyleMap()`.
Enabling the existing popover API for experimental WPT runs and using the initial containing block as a top-layer
box's static-position rectangle raised a focused 278-test CSS Position and CSS Containment set from 150.54 to
158.54 passing test-equivalents. Four of the eight newly passing tests are in the default CSS selection; the four
content-visibility tests are excluded by `include.ini`. These gains are outside the full CSS score above.
A later 1,737-test writing-mode, static-position and grid-alignment run found 34 FAIL-to-PASS and four
TIMEOUT-to-PASS changes after correcting the bidi order and vertical-rl static position of absolutely positioned
boxes. All 20 newly passing reftests with stale failure expectations passed on a focused repeat, and their
expectations were removed. Two tests that passed in the earlier full CSS run failed in this sweep: the orthogonal
float case fails identically on the unmodified build, while the vertical relative-position case passes on an
isolated repeat. These focused gains are outside the full CSS score above.
A full CSS sweep later on 13 November selected 34,847 tests. Restricted to the original 29,199 IDs, it scored
**73.35%** (21,418.5 test-equivalents passing, raw). Three tests that passed in the preceding full run and failed in
this sweep passed on an isolated repeat. Five other apparent passing-to-failing cases failed again on repeat; the
orthogonal float case and four vertical baseline cases also fail identically with the pre-patch layout build. The
expanded selection is not used for the comparable score; the new sweep includes the default-selected tests from the
focused batches above.
A full CSS sweep after enabling grid lanes again selected 34,847 tests. Restricted to the original 29,199 IDs, it
scored **74.12%** (21,642.2 test-equivalents passing, raw), up 223.7 from the preceding full run. Grid-lanes tests
account for 222.8 of that gain. Nine tests outside the grid-lanes directory had different top-level statuses, with
almost no net weighted change. A focused repeat confirmed two new failures and one new pass in grid-lanes gap
decorations; the other status changes did not reveal a new stable failure.
After correcting vertical logical-to-physical geometry, the same 34,847-test CSS selection scored **74.36%** on the
original 29,199 IDs (21,713.8 test-equivalents passing, raw), a gain of 71.6 over the grid-lanes run. The largest
weighted gain was 65.7 in `css-writing-modes`; `css-align` gained 13.0, while several vertical text and containment
cases regressed. Focused reruns confirmed the changed results used for expectation updates.
Sizing orthogonal in-flow blocks in their own writing mode raised the comparable score to **74.69%** (21,808.4
test-equivalents passing, raw), a gain of 94.6. The largest gain was 84.8 in `css-writing-modes`, including 99
reftests changing from failure to pass. A focused repeat confirmed the new shrink-to-fit, border, padding, resize,
grid, and text failures. The final full sweep also had intermittent crashes in unrelated modules; crash results
were excluded from expectation updates.

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

- The Armature WebGL score comes from a full `_webgl` run on 11 November 2026 at `5b9c25bdcbb`: 2,415.7
  passing-test equivalents across 2,939 tests. Its upstream score comes from the 7 November run; the two scores
  were not measured as a pair. The subtest difference was not recalculated. The Total row still excludes the WebGL
  rerun because a new whole-suite run has not been made.
- Both builds ran the same tests, including the directories that Armature added to `include.ini` and upstream
  Servo skips (all of `web-animations`, and CSS directories whose properties Armature enables). Tests that set a
  preference only Armature has (Observable, popovers, `ProcessingInstruction` attributes, ARIA reflection) error on
  the upstream build, whose engine lacks those features anyway.
- The Armature run matched the expectations recorded in `tests/wpt/meta` for all but about 450 tests, most of them
  WebGL conformance tests and tests that depend on the fonts of the machine, so the scores should be close to what
  Servo's CI would measure.
- The WebAssembly, XMLHttpRequest, Shadow DOM and Web Audio rows were re-measured on 11 November 2026 at
  `f41ea979ea2`, running only those directories (and WebSockets and MathML). Their upstream columns come from the
  7 November run, so their "newly passing" counts are the earlier counts plus the expectations cleared since, and the
  total row was adjusted from those rows rather than from a new full run. The WebSockets and MathML rows are
  unchanged: no work went into them, and a MathML rerun differs from the recorded expectations in about 140
  subtests that depend on the machine rather than on Armature's changes.
- Apart from the updated WebGL row, the table predates the SVG hit-testing, path DOM, image sizing, non-scaling stroke,
  canvas image, focus, conditional processing, SMIL, image rendering keywords and filtered child selector invalidation
  work, as well as the final HTML reflection fixes; their gains are not included in those historical scores.
- Areas that went down: 7 subgrid reftests that passed only because upstream Servo rejects `subgrid` (Armature parses
  it, but layout does not support it), two `skew()` reftests whose edges differ by one colour step since SVG images
  are sized differently, and three `css-viewport` tests that upstream Servo passes only because it ignores `zoom` in
  both the test and the reference. The other small drops in the table (Upgrade Insecure Requests, Mixed Content and a
  few single tests) come from tests that crash,
  time out or fail intermittently when the whole suite runs.

## What has been done

- **WebGL2 texture sources and dEQP coverage**: WebGL2 image and subimage uploads now accept canvas, image, SVG,
  video and `ImageData` sources in 2D and 3D. Typed-array 3D subimages and pixel unpack buffer 2D subimages are
  supported. Twelve WebGL2 texture source directories and eight dEQP groups (456 tests) are enabled, bringing the
  full WebGL conformance score to 82.2%.
- **WebGL canvas pixels and texture uploads**: WebGL canvas snapshots now put the top row first for `toDataURL`,
  canvas consumers and texture uploads. `OffscreenCanvas` is accepted as a `TexImageSource`. Seventeen WebGL canvas
  conformance tests that previously failed or ended in error now complete successfully, including 200 subtests
  previously expected to fail.
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
- **MIME types**: Request and Response `blob()` preserve MIME parameter values, including their case and non-ASCII
  characters, and navigation quotes parameters it re-parses (so `charset` is no longer lost). DOM `Headers` keeps
  the control bytes permitted by Fetch even when the HTTP library cannot store them; this makes 186 more MIME parsing
  subtests and nine header normalization subtests pass. `mimesniff/mime-types/parsing` went from 1,916 failing
  subtests to 1,208.
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
- **CSS aspect-ratio sizing**: Non-replaced blocks, atomic inline boxes, and absolutely positioned boxes transfer
  definite width or height through their preferred aspect ratio. The ratio-dependent size retains its content-based
  automatic minimum where applicable, including for flex containers and items. Maximum block sizes also constrain
  automatic inline sizes through the ratio in absolute positioning. The targeted `css-sizing/aspect-ratio` run gains
  60.844 passing test-equivalents across 295 tests against the 12 November CSS baseline.
- **Absolute positioning in block flow**: The static-position rectangle now spans the parent block's inline size,
  and its child uses `justify-self` and `align-self` when both insets in an axis are `auto`. This makes two
  `css-align/abspos` reftests pass.
- **Vertical absolute positioning**: Out-of-flow placeholders resolve to the paragraph bidi level, and vertical-rl
  static-position rectangles measure their block offset from the containing block's right edge. The focused
  writing-mode, static-position and grid-alignment sweep found 38 newly passing tests against the previous CSS run.
- **Top-layer static position**: Open popovers use the initial containing block as their static-position rectangle.
  Experimental WPT runs enable the existing popover API. Four CSS Position reftests now pass; vertical writing-mode
  variants still expose a separate root-flow placement issue.
- **SVG fill-box transform origins**: Inline SVG rasterization resolves percentage `transform-origin` values against
  the source element's fill box when `transform-box: fill-box` is selected. All 46
  `svg-origin-relative-length` reftests and one `transform-box` reftest now pass.
- **CSS counters and counter styles**: Layout resolves `counter-reset`, `counter-increment`, `counter-set`, implicit
  `list-item` increments and generated counter content in tree order before parallel box construction. It renders
  common built-in styles, `symbols()` and basic named `@counter-style` systems, including styles scoped to shadow
  roots and declarations from `::part` rules. The 866-test counter, list and generated-content rerun gained 195.6
  passing test-equivalents against the preceding full CSS run; a subsequent focused run fixed its one shadow-scoping
  regression and all four focused shadow-reference tests pass.
- **Generated quote nesting**: Quote depth now follows generated content through the flattened element tree,
  including `no-open-quote` and `no-close-quote`. Style-contained subtrees keep their quote changes inside the
  subtree. The focused `CSS2/generated-content` and `css-content` runs gain 43 passing reftests, and all nine focused
  quote-containment cases pass.
- **Emergency text wrapping**: `overflow-wrap: anywhere`, `overflow-wrap: break-word`, and `word-break: break-word`
  can break overflowing text at grapheme boundaries while retaining the original shaping and normal break priorities.
  `anywhere` and `word-break: break-word` also contribute those opportunities to min-content sizing. The 2,394-test
  `css-text` and `CSS2/text` rerun gains 54 passing test IDs against the original CSS baseline without regressions.
- **Block-container margin trimming**: `margin-trim: block-start`, `block-end`, and `block` discard adjacent child
  margins, including margins collapsed through empty blocks. The focused 106-test directory gains 32 passing reftests.
- **Principal writing mode and vertical absolute positioning**: an eligible HTML body supplies the root's used writing
  mode and the initial containing block's writing mode. Containment and `display: none` prevent body propagation.
  Vertical-rl absolute boxes now resolve physical horizontal insets from the right block-start edge. A 3,003-test
  focused run gained 34 passing reftests against the preceding full CSS run, with one reproducible regression.
- **Vertical logical geometry**: layout converts right-to-left block and bottom-to-top inline coordinates using their
  physical start edges, including inverse point and rectangle conversions and signed vectors. This replaces special
  handling in absolute positioning and gains 71.6 passing test-equivalents in the comparable CSS run, chiefly in
  vertical writing modes and alignment.
- **Orthogonal in-flow sizing**: blocks whose writing mode crosses the parent's axes establish an independent
  formatting context. Layout sizes their content in their own writing mode, then positions the result in the
  parent's mode. A focused writing-mode run gained 99 passing reftests, and the comparable full CSS run gained 94.6
  passing test-equivalents after regressions. Row flex intrinsic sizing now supplies a containing block when an
  orthogonal child needs a block-size layout.
- **CSS grid lanes**: Experimental WPT runs enable `display: grid-lanes`. Layout uses grid track sizing in the lane
  axis, then places each in-flow item after the previous item in its shortest eligible lane. Both row and column
  lanes, gaps, spans, explicit lane placement, and natural item sizing in the stacking axis work. The 1,005-test
  grid-lanes suite rose from 133.93 to 356.69 passing test-equivalents against the preceding full CSS run; the
  surrounding grid and display suites had no status changes outside grid lanes. Subgrid, intrinsic auto-repeat,
  dense packing, and older grid-lanes properties still need work.
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
- **CSS Box Alignment flow aliases**: `flow-start` and `flow-end` parse for the six alignment longhands and the
  `place-*` shorthands. Specified styles keep their spelling, while computed styles serialize both these values and
  `flex-start`/`flex-end` as `flow-start`/`flow-end` (AS-517). In the focused 54-file WPT run, 112 subtests newly pass,
  including alignment parsing, computed serialization and discrete Web Animations comparisons.
- **Animated opacity clamping**: keyframe opacity values remain out of range through interpolation and effect
  composition, then the final CSS and SVG opacity values are clamped to `[0, 1]` (AS-518, AS-519). All 10 previously
  failing subtests in the two focused Web Animations clamping tests pass, and the SVG `stop-opacity` and
  `stroke-opacity` accumulation cases still clamp.
- **Precise animated color extrapolation**: color mixing retains double-precision weights for absolute colors
  (AS-522), so large extrapolation progress no longer loses the difference between opposite weights. All four
  previously failing `filter-interpolation-002.html` drop-shadow subtests now pass; the focused CSS color animation
  interpolation and composition tests still match their expectations.
- **Images in embedded elements**: `<embed src>` and `<object data>` resolve image URLs and use the image cache's
  natural dimensions for `object-fit` and `object-position`. In the 85-file CSS Images reftest run, 81 tests newly
  pass; eight related `object-position` reftests also pass. Four tentative `contain-intrinsic-size` cases still fail.
- **SVG video posters**: `<video poster>` keeps an SVG image as a vector until layout knows its `object-fit` size,
  then rasterizes it at that size. All 30 previously failing SVG poster object-fit reftests and two related
  `object-position` reftests now pass, while the focused PNG poster regressions remain passing.
- **Grid self alignment**: `self-start` and `self-end` align grid items to the physical sides of their own writing
  mode, including vertical and right-to-left items. The CSS Grid alignment directory gains 1.74 passing
  test-equivalents across five tests in a 454-test rerun, with no regressions against the earlier CSS-only run.
- **Direct-line `line-clamp`**: layout suppresses lines after the requested count, sizes the block to the retained
  lines and appends a default ellipsis when it fits after simple text. Stylo now parses `-webkit-line-clamp` in Servo
  so it participates correctly in the cascade (AS-524). A 934-test CSS Overflow run gains 22.4 passing
  test-equivalents, including 21 reftests. Clamped inline blocks now export the last visible line's baseline, making
  one more reftest pass in a 324-test `line-clamp` rerun.
- **CSS container queries**: Stylo parses `@container` rules (AS-525), layout exposes the query container's content-box
  size and repeats style and layout when query results change, and script exposes `CSSContainerRule` with its
  conditions. The 320-test CSS Conditional container-query directory gains 96.35 passing test-equivalents.
- **Scoped CSS rules**: `@scope` is enabled in the WPT experimental mode, and `CSSScopeRule` exposes the parsed
  `start`, `end` and nested rules. All 43 `at-scope-parsing.html` subtests and all 13 `scope-cssom.html` subtests pass;
  the 45 direct scope tests in CSS Cascade gain 21.44 passing test-equivalents.
- **Resolved transform serialization**: `getComputedStyle()` chooses `matrix()` or `matrix3d()` from the resulting
  matrix, including for the legacy `-webkit-transform` alias. In the focused seven-file WPT run, 15 transform-related
  subtests newly pass across CSS Transforms and Web Animations.
- **Web Animations API**, on in experimental mode (`dom_web_animations_enabled`): `Animation` has the playback model of
  the spec (`play()`, `pause()`, `finish()`, `cancel()`, `reverse()`, `updatePlaybackRate()`, `currentTime`,
  `startTime`, `overallProgress`, `playbackRate`, `playState`, `pending`, the `ready` and `finished` promises, and
  `finish` and `cancel` events), effects compute their timing (`getComputedTiming()`, including effect `startTime`),
  `KeyframeEffect` takes timing options, property-indexed
  keyframes (including nullable offset arrays, number-valued `calc()` offsets and CSS custom properties) and a
  composite operation, and
  its copy constructor preserves the target, keyframes, composition and timing, including a snapshot of CSS animation
  keyframes. `iterationComposite: accumulate` builds on the final keyframe value across script animation iterations.
  Easing values are fully parsed and serialized, and exceptions from keyframe property getters propagate.
  `element.animate()` plays the animation. Custom properties also affect computed style and `commitStyles()`.
  Animations created by script now change the style of their targets, including `::before` and `::after` even when
  they generate no box. Retargeting an effect moves it between elements and pseudo-elements without leaving an old
  effect behind (AS-515). Stylo resolves their keyframes and composites them (`replace`, `add` and `accumulate`)
  over CSS animations (AS-506). The "Web Animations" and
  "Compositing Web Animations" subtests of the interpolation tests throughout `css/` went from about 12,100 failing
  to about 2,300, and `web-animations/interfaces`
  from 625 failing subtests to 49 in the 47-file interface run (822 passing subtests).
  Constructed `DocumentTimeline` objects now use the document's sampled animation clock and their origin offset,
  while `AnimationTimeline.duration` is exposed as null for document timelines. Seven previously expected failures
  pass across the constructor, duration and IDL tests; the full 207-test `web-animations` run has no new failures.
  Keyframe sequences now read every keyframe and property before reporting an invalid easing, so both ordering
  subtests in the focused WPT pass. Playback events now sort by scheduled time and CSS composite order, and the
  event queues are snapshotted before callbacks run. Four event-ordering subtests now pass, the focused 19-test timing
  and CSS event run matches expectations, and the full 207-test `web-animations` run has no new failures. Animation
  ticks now wait for the 120 Hz refresh interval when the previous frame was too recent, so finish events for CSS
  transitions, CSS animations and script animations arrive in composite order. All nine subtests of the focused event
  timing test pass; the 225-test animation and frame-callback run has no new failures.
- **`CSSAnimation`, `CSSTransition` and `getAnimations()`**: CSS animations and transitions are now `CSSAnimation`
  and `CSSTransition` objects that `Document`, `ShadowRoot` and `Element.getAnimations()` return in composite order,
  so script can pause, seek, reverse, finish or cancel them and read their timing and keyframes
  (`KeyframeEffect.getKeyframes()`). `Element.getAnimations({pseudoElement})` selects the matching pseudo-element
  and rejects invalid selectors. Script times animations with the same playback model as other animations; Stylo
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
  ones with the metrics of the element's font. `<style>` elements in SVG now apply their style sheets, SVG
  presentation attributes map to CSS properties, and hit testing (`elementFromPoint()` and the targets of
  mouse events) finds the shape under the point inside an `svg` element, which layout treats as one box. `SVGLength`
  preserves unresolved `var()` text and reports an unknown unit with a zero numeric value until script replaces it.
  Eight more SVG styling subtests pass, and the 97-file scripted SVG types directory matches expectations. The SVG
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
  reftests, the SVG bitmap resize reftest and one `drawImage()` subtest now pass. Inline SVG now paints fetched
  external images and refreshes its raster when one loads or its URL changes. Both new static and dynamic reftests
  pass, and the 82 SVG embedded and linking tests match expectations.
  Standalone SVG documents now embed fetched raster and SVG `<image>` resources in their rendering copy. Two new
  reftests pass, and an existing adopted-image reftest now passes after its stale failure expectation was removed.
  The 109-file embedded and linking run matches expectations. SVG elements now expose `dataset`, SVG links
  participate in focus navigation, and unrendered SVG definition elements stay out of it. Invalid
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
  SVG geometry DOM methods now use computed CSS position and size for circles, ellipse centers and rectangles,
  including percentages and live style changes. Seven focused subtests pass, and a 129-file SVG geometry regression
  run matches expectations.
  SVG `<image>` and `<foreignObject>` bounding boxes now use CSS geometry positions and sizes, while image `auto`
  dimensions still use natural dimensions. Three more focused subtests pass, and the 107-file SVG scripted and image
  sizing run matches expectations.
  Rounded `<rect>` DOM geometry now follows its elliptical corner radii, including CSS overrides, percentages based
  on the rectangle size, auto radii and clamping. All seven focused subtests pass; the 131-file SVG shape, type and
  geometry regression run matches expectations.
  SVG `<script>` now exposes the `SVGURIReference.href` animated string, including `href`/`xlink:href` precedence.
  Four script-linking subtests and two SVG IDL checks now pass. The SVG IDL expectations were also refreshed for 121
  checks fixed by earlier work, and four stale scripted-geometry failure expectations were removed.
  SVG text `getRotationOfChar()` now includes CSS writing mode, Unicode vertical glyph orientation, the `rotate`
  attribute and text-path tangents, including paths supplied through `<textPath path>`. All 25 focused subtests pass,
  including 19 that previously failed. Experimental WPT runs now enable CSS `writing-mode`; the 27-file SVG scripted
  text run matches expectations after removing seven stale failure expectations in related geometry tests. The
  1,247-test CSS writing modes sweep had three unexpected test files; rerunning those files with the pref off gave
  the same results.
  Script's SVG text geometry now places characters on `<textPath>` geometry from inline `path` data or referenced
  paths, including dynamic changes, `startOffset`, `pathLength` calibration and basic SMIL path interpolation.
  `textLength` adjusts character spacing or glyph advances, and character position and hit-testing methods handle
  vertical writing modes. The 27-file scripted text run now passes 35 subtests that previously failed; two expected
  failures remain in that directory.
  SVG DOM hit testing now uses the stroked path outline, including caps, joins and dash gaps, so the six mitered
  rectangle corner cases in `elementFromPoint()` pass. The 24-file scripted interaction run has no new failures.
  SVG `stroke-width`, `stroke-dashoffset` and `stroke-dasharray` now preserve unitless numbers in specified CSS values,
  including number-only `calc()` expressions, and compute them as user-unit lengths (AS-513). Thirteen painting
  parsing subtests now pass; the 61-file painting parsing directory matches expectations.
  HTML and SVG elements now expose a live `attributeStyleMap` backed by their inline style declaration. The map
  supports reading and changing properties, and `CSSKeywordValue` is available in windows and workers. All 61 SVG
  `attributeStyleMap` subtests now pass; broader CSS Typed OM value types and iteration remain unfinished.
  `CSSStyleValue.parse()` and `parseAll()` now validate property names and values, return the first or all iterations
  of supported list-valued properties, and keep direct values associated with their property. All 21 focused parse
  and parseAll subtests pass, along with the keyword serialization case that previously lacked `parse()`.
  `CSSUnparsedValue` and `CSSVariableReferenceValue` now support string and variable fragments, indexed access,
  iteration, fallbacks and serialization in windows and workers. Custom properties in `attributeStyleMap` reify as
  unparsed values; all 11 inline style mutation callback subtests pass.
  `CSSNumericValue` and `CSSUnitValue` now expose validated CSS units, mutable numeric values and stringification,
  along with all 34 `CSS` numeric factory functions. The three focused constructor, value and factory WPT files went
  from 71 failing subtests to all passing. `CSSNumericValue.type()` reports unit base types, `parse()` handles single
  numeric tokens at double precision, and `to()` converts compatible units. Five focused typing, parsing and
  conversion WPT pages went from 74 failing subtests to 43. The seven CSS math DOM subclasses and indexed
  `CSSNumericArray` now expose constructors, operands, operators, numeric types and basic stringification in windows
  and workers. All 63 subtests in five focused window WPT pages pass, as do seven interface and worker type subtests.
  `CSSNumericValue` arithmetic now simplifies compatible unit values, builds math expressions for mixed units,
  rejects incompatible types and division by zero, and compares math values structurally. The `CSS` numeric factories
  are available in workers. The arithmetic and equality pages went from 68 failing subtests to all 78 passing in
  windows; the worker arithmetic page passes all 67 subtests. `to()` now evaluates math expressions through canonical
  unit sums, and `toSum()` converts or sorts their unit terms. The three focused conversion pages went from 22 failing
  subtests to all 49 passing, including worker conversion. `CSSNumericValue.parse()` now builds `calc()`, `min()`,
  `max()` and `clamp()` values with type validation and compatible-unit simplification. All 23 focused parsing
  subtests pass, including 10 previous failures. All 675 subtests across the numeric-objects directory's 35 window,
  worker and service worker variants pass. Directory WPT metadata enables service workers for these tests, and the
  numeric-objects directory is included in the default WPT selection.
  `CSSTransformValue` and its eight transform component classes now expose constructors, mutable values, CSS
  serialization and matrix conversion. All subtests across 12 direct WPT suites pass, including 2D matrix flattening.
  The common Typed OM property test data now loads; 239 of 244 property files reach their assertions.
  SVG anchors now navigate on click, including `xlink:href`, dynamic `target` changes, and links rendered through
  `<use>`. Their `rel` and referrer policy attributes affect navigation. Seventeen more subtests pass across four
  scripted linking files; the full 25-file scripted linking directory matches expectations.
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
  A paused SVG clock now processes due boundaries for animations inserted at its held time without advancing the
  clock. All four SVG animation `href`/`xlink:href` linking subtests pass, and the 322-file `svg/animations` run
  matches expectations.
  Motion paths now retain double precision while sampling iteration progress and eased value intervals. All three
  `<mpath>` `href`/`xlink:href` linking subtests pass, including switching the referenced path; the combined
  394-file SVG animation and path run matches expectations.
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
  SVG markers at closed subpaths now bisect the incoming and outgoing directions, draw middle markers at nonfinal
  closing vertices, and avoid duplicate markers when a segment already ends at its subpath start. Four more
  `svg/painting` marker reftests pass. Arc markers now use the vertices of the original path commands instead of
  placing extra middle markers where an arc was approximated by multiple cubic curves; three more marker reftests pass.
  Markers at exact 180-degree reversals choose a consistent clockwise bisector, and repeated `closepath` commands
  retain their middle-marker vertices even when the painted path collapses them. Two more marker reftests pass.
  Non-scaling strokes are outlined in the outer SVG viewport before being mapped back to path coordinates, including
  under non-uniform transforms. Inline SVG rendering copies now carry computed CSS stroke and `vector-effect` values,
  and SVGs with automatic sizes resolve their layout viewport before outlining. Two more painting reftests pass,
  including the viewBox scaling cases. Embedded data URL SVGs with a `viewBox` now resolve non-scaling strokes in the
  fitted `<image>` viewport, even when `preserveAspectRatio="none"` stretches the image non-uniformly. Solid strokes
  are painted as strokes in that viewport, matching the raster edges of a directly drawn line. Both focused image
  reftests now pass; all 27 `svg/embedded` tests still match expectations.
  Markers sized in stroke-width units now use that same outer viewport for non-scaling strokes, passing the focused
  marker reftest. Computed CSS transforms on SVG descendants are also transferred to the rendering copy when their
  transform does not already come from an SVG attribute or motion animation.
  Seeking an SVG animation clock refreshes its cached raster image. The serializer also applies active `<animate>`
  and `<set>` presentation property values to the rendering copy, so the scripted motion and rewind/freeze reftests
  now pass.
  SVG descendant attribute and child mutations now refresh the outermost raster. SMIL presentation-property effects
  also refresh it when they start, end, or cross a discrete keyframe; four more SVG animation reftests now pass.
  Active continuous `<animate>` presentation-property and `<animateMotion>` effects now refresh the raster during
  playback. Redraw waits for the document load and initial font readiness, and screenshot capture lets the current
  frame settle. Evicting an in-flight SVG image wakes its pending listeners so canceled frames cannot block a
  screenshot. A new mid-animation fill reftest passes alongside the existing indefinite fill and motion reftests.
  Presentation hints now map `transform`, `patternTransform` and `gradientTransform` to CSS `transform`, apply
  `transform-origin` on SVG elements, and leave the SMIL `fill` attribute on animation elements out of CSS paint.
  Fourteen more SVG presentation-attribute subtests pass; the three remaining focused failures require URL-valued CSS
  `filter` support in Stylo's Servo build. All 86 path interface subtests and 33 SVG image sizing subtests pass.
  Across the same 343 SVG animation subtests,
  the broader run moved from 145 PASS / 151 FAIL / 47 TIMEOUT to 343 PASS / 0 FAIL / 0 TIMEOUT, with no previously
  passing subtest regressing. Animations excluded by `systemLanguage` or an unselected `<switch>` branch no longer
  affect their targets. The rasterized SVG copy receives sampled geometry lengths as attributes, so both `<switch>`
  animation reftests now pass. On-demand style queries resolve animation keyframes in hidden SVG
  subtrees, and the rendering copy samples effects on definitions imported through `<use>`.
  Imported IDs are tracked so changes in their source subtree refresh the visible SVG raster. The
  two hidden-symbol animation reftests now pass; the media-fragment override is the one remaining
  expected-fail reftest in the SVG animation run.
  SVG font IDs are now cached with their issuing font database, so reparsing a viewport-dependent SVG retains its
  text. A local reftest comparing viewport-dependent and fixed-size SVG text passes. The existing local `textPath`
  reference now renders; its last mismatch is one antialiased edge column.
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
  surrounding 86-file `svg/geometry` run matches expectations. The SVG image parser now accepts CSS geometry
  declarations on circles, ellipses, rectangles and images, and resolves `vw` and `vh` in nested SVG images against
  the embedding viewport. Eleven geometry reftests and both embedded viewport-unit reftests now pass; the 113-file
  embedded and geometry run matches expectations. The SVG renderer also evaluates `calc()` geometry lengths and
  percentages in the current viewport, including after a container resize. Six more reftests now pass, so all 25
  `svg/geometry/reftests` files pass; the 113-file embedded and geometry run still matches expectations. Standalone
  SVG shape geometry now resolves `vw`, `vh`, `vmin` and `vmax` against the initial viewport, including inside `calc()`.
  The existing rectangle reftest and a new `vmin`/`vmax` visual reftest pass; the 117-file focused run matches
  expectations. SVG geometry presentation attributes now defer `var()` substitution until custom properties have
  cascaded, and the rendering copy receives only those attributes' resolved geometry. Unitless substituted lengths
  remain invalid in standards mode and resolve as SVG user units in quirks mode. Four more styling reftests now pass;
  all seven `css-var-on-length-attributes` cases match expectations. The wider SVG styling, geometry, embedded image,
  path and painting run has no pass-to-fail changes against the saved baseline runs.
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
  previously passing subtest regressing. A fresh run also confirmed that all seven `ellipse-auto-radii.svg` geometry
  subtests and the high-scale dashed-stroke hit test pass; their stale failure expectations have been removed.
  Stroke width, dash lengths and dash offsets now resolve percentages and `calc()` against the current SVG viewport's
  normalized diagonal, including after a `viewBox` change and under CSS zoom. Ellipse geometry now uses computed CSS
  radii, so a radius set to zero in style disables the shape. The new six-subtest percentage stroke test passes, and
  the 102-file SVG scripted regression run matches expectations. `isPointInStroke()` now includes computed CSS
  transforms on the shape and its SVG ancestors for non-scaling strokes, including nonuniform scales, skew, CSS
  overrides of presentation attributes and live transform changes. All six transform subtests pass. One 103-file SVG
  scripted run matched expectations; two font-dependent differences on a repeat run passed when rerun separately.
- **SVG clip-path hit testing**: `elementFromPoint()` now rejects shapes outside a resolved clip
  path's fill silhouette, including clips on ancestor groups, `objectBoundingBox` clips, and
  shapes referenced through `<use>`. Cyclic `<use>` references contribute no hit region. All
  three previously failing clipped-shape subtests and a new five-subtest regression test pass.
  Ten older SVG shape reftests also pass consistently after earlier geometry and stroke work;
  their stale failure expectations are removed. All 36 `svg/shapes` files and the combined
  162-file shapes, interaction and SVG types run match expectations.
- **Filtered child selector invalidation**: Attribute and pseudo-class state changes now restyle
  siblings whose index may change under `:nth-child(... of <selector>)` or
  `:nth-last-child(... of <selector>)`, including direct children of shadow roots and descendants
  of the indexed siblings. SVG rendering copies receive Stylo's resolved paint values for those
  children, since the image parser does not support the selector filter. Twenty-two CSS selector
  invalidation reftests and two SVG styling reftests now pass. The 249-file CSS and SVG regression
  run had no pass-to-fail changes against the preceding run; all 145 tests in the selector
  invalidation directory now run as expected.
- **CSS and SVG image rendering keywords**: Servo now accepts `smooth`, `high-quality`,
  `optimizequality` and `optimizespeed`, preserving their computed values. The quality values use
  WebRender's default image sampling, and `optimizespeed` uses crisp edges. Nine previously failing
  parsing, computed style and inheritance subtests now pass; all 87 nearby CSS Images and SVG
  painting parsing tests run as expected.
- **Inline SVG currentColor repainting**: Changes to an HTML ancestor's class, ID, or style now
  refresh the serialized SVG raster. Resolved fill and stroke colors use legacy sRGB syntax in the
  rendering copy so the rasterizer can paint `color-mix()` results. Both focused repaint reftests
  now pass. The SVG rasterizer also accepts case-insensitive `currentcolor` in fill and stroke,
  including after a paint-server URL. Three more paint-server reftests now pass. Gradient stops
  also accept case-insensitive `currentcolor`, so the dynamic stop-color reftest follows changes
  to the SVG root's inherited color.
- **SVG paint-server transforms**: Gradients and patterns inherit transforms from `href` templates
  when their own transform is absent, empty, or invalid. Supported CSS `transform` declarations
  also affect paint servers. Singular transforms now use the referring paint's fallback color.
  Twelve gradient and pattern reftests now pass, including three that clear a transform list
  through the SVG DOM. CSS pixel lengths in `translate()`, `translateX()` and `translateY()` now
  work on paint servers; the dynamic pattern translation reftest also passes.
- **SVG fragment references**: Percent-encoded fragment IDs now resolve in rasterized fill,
  stroke, filter, clipping, masking and `href` references. The encoded gradient fill and stroke
  reftest passes. Two gradient reftests that resize text also pass consistently after earlier
  font and rendering fixes; their failure expectations have been removed. All 33 paint-server
  reftests now match expectations.
- **SVG filter DOM tests**: The filter-effects IDL harness and `SVGFEBlendElement.mode` test now
  match the implementation. Their 492 stale failure expectations were removed; all three window
  and worker test variants run as expected.
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

- CSS counter support still lacks some predefined and custom numbering systems, counter-style descriptors, and
  `CSSCounterStyleRule` mutation. Four reversed-list counter scope reftests still fail.
- `margin-trim` still needs flex and grid item trimming and some block fragmentation cases; its WPT directory remains
  excluded from the default CSS run.
- Vertical in-flow and orthogonal box placement remain incomplete. Orthogonal children still contribute the wrong
  intrinsic width to some shrink-to-fit parents, and border, padding, dynamic resize, and grid percentage sizing
  cases regress with the new sizing path. In
  `css-writing-modes/relpos-inline-overflowing-block-vrl.html`, the corrected absolute box now exposes the existing
  misplaced inline box, causing a reproducible reftest failure. Vertical coordinate conversion also remains
  provisional when the containing block's block size is indefinite; several containment and vertical text reftests
  still fail.
- `line-clamp` still needs ellipsis placement when the last line is full or has nested inline boxes, counting lines
  inside descendant blocks, vertical and right-to-left text, and the legacy `-webkit-box` display behavior.
- Container queries still have failures involving hidden ancestors, unsupported query forms, and other dynamic
  cases. Six previously passing subtests in `query-evaluation.html` and `display-none.html` now fail, and three
  existing timeout cases remain.
- Implicit `@scope` roots are not passed from stylesheet owners to layout. Scopes without a prelude can therefore
  match outside the intended root or fail to style that root; three previously passing subtests now fail.
- WebGL2 texture overload coverage remains incomplete (`tex-unpack-params.html` still errors), and 368 dEQP files
  remain disabled.
- Four subtests (two in `XMLSerializer-serializeToString.html`, two doctype subtests in `xml-serialization.xhtml`)
  expect behaviour that the DOM Parsing spec changed in August 2026 and that Armature follows: a doctype ID that
  contains `"` is quoted with `'`, and an attribute keeps its own prefix when the element does not declare that
  prefix itself (an open web-platform-tests change updates the tests). One more expects a redundant `xmlns=""` to be
  kept, which the spec drops.
- About 1,200 `mimesniff` Blob/File subtests expect `new Blob([], {type})` to parse and re-serialise the type, which
  Servo does not do. Four Request/Response cases involving commas in `Content-Type` still fail. The network HTTP
  library cannot send control bytes that Fetch accepts in DOM headers, so the header-value round-trip tests still fail.
  One `response-consume` subtest expects the generated `URLSearchParams` charset to be `utf-8`, while Fetch specifies
  `UTF-8` and MIME serialization preserves parameter case.
- 25 popover subtests still fail in the latest run: layout has no real top layer or `::backdrop` (open popovers are
  drawn as `position: fixed`), some focus navigation cases remain incomplete, four inactive-document cases expect
  listeners to run despite the DOM Standard's fully active document check, and 6 other tests contradict the spec.
- `TextEncoder.encodeInto()` and `TextDecoder` streaming into a `SharedArrayBuffer` (about 210 subtests) fail because
  Servo has no cross-origin isolation, so shared memory is off in every global.
- Fetch priority hints are reflected in the DOM but do not yet affect network request scheduling.
- Not enabled from Stylo: anchor positioning, `@starting-style`, `@page` margin rules, custom media,
  `background-clip: text`/`border-area`, `random()`, tree-counting functions and a few more, because
  Servo's layout or CSSOM would panic on them; `size`, `page-orientation` and `page`, which need `@page` support.
  `resize` has no resize handles, and `subgrid` is parsed but not laid out.
- `flex-wrap: balance` is ignored. Two radial-gradient masking reftests also fail after their absolutely positioned
  aspect-ratio boxes gain height. Their WPT expectations record those failures.
- Web Animations: script animations on pseudo-elements beyond `::before` and `::after` are not yet applied; CSS
  functional pseudo-elements such as `::part()` and `::highlight()` are not yet supported; and
  changing `iterationComposite` on a CSS animation does not yet affect its style-generated keyframes.
  Script keyframe offsets do not yet accept percentage or named timeline-range strings from Web Animations 2.
  `commitStyles()` of an animation below others
  briefly recomputes the style of the target without them, which can start transitions. A `CSSAnimation` per repeated name in `animation-name` is missing
  (Stylo matches animations by name), `getKeyframes()` does not keep keyframes that share an offset but not an easing
  apart and shows additive keyframe values already added, and a finished `CSSTransition` cannot be replayed once
  style has dropped it. Container query units in keyframes are not supported, as in style sheets.
- SVG documents are drawn by the same code as inline SVG, which handles `<use>` and some nested images
  less well than drawing the file as an image did, so a few `.svg` reftests that used to pass now fail. `<use>` still
  lacks a DOM shadow tree, including the instance event path and synchronized event listeners; direct references to
  SVG anchors activate through their source element. SMIL effects cover basic lengths, enumerations, numbers,
  presentation properties and motion. Full repeat and syncbase timing, `TimeEvent` details, and many animated
  attributes are still missing. Some geometry methods still reconstruct values from attributes rather than used CSS
  values. Script's text geometry is measured glyph by glyph without shaping or full font baseline metrics. Text-path
  layout is incomplete
  for advanced typography and path animation modes. One text-path `textLength` bounding-box test reads its box before
  the Ahem font loads, so its result depends on font-loading timing.
- SVG `getBBox()` intersects clipping bounds as rectangles rather than calculating the exact clipped silhouette, and
  marker sizing does not cover every `preserveAspectRatio` and orientation case. Eight WPT cases expect zero for
  graphics inside definitions, clip paths, masks or `display:none` containers; Armature returns their own geometry,
  as the current [SVG 2 bounding-box text](https://www.w3.org/TR/SVG2/coords.html#BoundingBoxes) describes. Two
  `lh`/`rlh` tests compare the 32-bit `SVGLength.value` with a layout double using exact equality. One `ex` test
  measures its reference before Ahem loads, so its expected value can change with font-loading timing.
- SVG DOM stroke hit testing does not yet project non-scaling strokes through 3D CSS transforms.
- SVG clip-path hit testing does not yet cover text outlines or the viewBox of a referenced `<symbol>` inside a clip path.
- CSS `filter: url(...)` remains unsupported in computed style, even for an unencoded local fragment.
- Some SVG non-scaling stroke reftests still differ at raster edges or under CSS transforms. The embedded SVG viewport
  correction currently covers data URL images with a `viewBox` and a `vector-effect` attribute.
- Many editing tests still fail: deleting across differently styled blocks, whitespace handling, and cases where the
  tests expect browser behaviour that differs from the editing spec (Armature follows the spec). `formatBlock` and
  `insertHTML` are disabled in `contenteditable=plaintext-only` hosts.
- The tentative ARIA reflection test still has 30 failing subtests.
- WebAssembly, WebSockets, MathML, XMLHttpRequest, Shadow DOM and Web Audio still have failures, each needing a whole
  feature: `WebSocketStream` (and WebSocket over HTTP/2), MathML layout, WebAssembly type reflection (a SpiderMonkey
  build option), the Shadow DOM reference-target feature, and a configurable audio render quantum
  (`renderSizeHint`). XMLHttpRequest also lacks request header case preservation and `.asis` responses without a body.
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
