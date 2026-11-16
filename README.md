# Armature: Servo, accelerated with AI

Armature is an unofficial fork of [Servo](https://github.com/servo/servo) that uses AI to accelerate development of
the engine. It is built with an AI coding assistant (Claude Code), and progress is measured by
[web-platform-tests](https://web-platform-tests.org/) conformance. It is **not** affiliated with or endorsed by the
Servo project.

Servo's [AI usage policy](https://book.servo.org/policy/ai-usage.html) does not accept AI-generated contributions,
so this work is not intended for, and has not been sent to, upstream Servo.

## Added features

Capabilities Armature has that upstream Servo does not, beyond fixes that raise conformance.

### WebNN: neural networks in the browser

Armature implements the W3C [Web Neural Network API](https://webmachinelearning.github.io/webnn/) (WebNN), which
lets web pages build and run machine-learning models on the device's own hardware. Upstream Servo has no WebNN at
all; Chromium ships it behind a flag and Firefox has only an experimental prototype.

- **The whole API:** `navigator.ml`, `MLContext`, `MLGraphBuilder` with every operator in the specification (about 95,
  from `add` to `lstm`), `MLGraph`, `MLOperand` and `MLTensor`, in windows and workers.
- **Real execution:** graphs are compiled and run by [ONNX Runtime](https://onnxruntime.ai/), through the Rust
  [rustnn](https://github.com/rustnn/rustnn) crate, on a dedicated thread so pages stay responsive.
- **Conformance:** `webnn/` in web-platform-tests passes 12,772 of 13,755 subtests (89.9%).

Try it with the `webnn` Cargo feature (on by default) and ONNX Runtime installed (Arch: `onnxruntime-cpu`; or point
`ORT_DYLIB_PATH` at `libonnxruntime.so`):

```
./target/release/servoshell --pref dom_webnn_enabled page.html
```

```js
const context = await navigator.ml.createContext();
const builder = new MLGraphBuilder(context);
const desc = {dataType: "float32", shape: [2, 2]};
const x = builder.input("x", desc);
const y = builder.relu(builder.add(x, builder.constant(desc, new Float32Array([10, 20, 30, 40]))));
const graph = await builder.build({y});
const input = await context.createTensor({...desc, writable: true});
const output = await context.createTensor({...desc, readable: true});
context.writeTensor(input, new Float32Array([1, 2, 3, 4]));
context.dispatch(graph, {x: input}, {y: output});
console.log(new Float32Array(await context.readTensor(output)));  // [11, 22, 33, 44]
```

WebNN currently runs on the CPU. Next: hardware acceleration, starting with AMD's XDNA2 NPU, either through ONNX
Runtime's NPU execution provider or a new rustnn backend. The remaining gaps are listed in
the [WebNN notes](docs/armature/webnn.md).

## Conformance

Except for CSS, the table below was measured on 7 November 2026 using the 61,548 web-platform-tests selected then.
Two builds ran on the same machine: upstream Servo at the commit Armature forked from
(`90c3f623175`, 30 October 2026) and Armature at `568f69bdeac`. As on [wpt.fyi](https://wpt.fyi/), each test counts
once, with partial credit for the share of its subtests that pass; the last column counts individual subtests. The
Total row reports 63,770 more passing subtests than upstream Servo (2,088,378 against 2,024,608) and predates the
WebGL rerun. The CSS row records Armature's 15 November 2026 result on the current test selection; its upstream and
subtest figures are carried forward until the next full-suite rerun.

| Area | Tests | Upstream Servo | Armature | Change (points) | Subtests newly passing |
|---|--:|--:|--:|--:|--:|
| CSS | 29,542 | 67.0% | **78.87%** | +11.9 | +23,261 |
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
| WebNN | 598 | — | **89.9%** | — | +12,772 |
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
Resolving an orthogonal flow block's intrinsic contribution on the parent's inline axis raised the comparable score
to **74.75%** (21,827.1 test-equivalents passing, raw), a further gain of 18.7. Writing modes gained 16.5, including
the shrink-to-fit cases where the earlier sizing path contributed the child's inline size instead of its block size.
The full-run score includes unrelated intermittent results; expectation updates use only the gains confirmed by a
focused repeat.
Resolving `line-clamp: auto` against the used block-size limit raised the comparable score to **74.81%** (21,844.7
test-equivalents passing, raw), a further gain of 17.6. Fifteen previously failing line-clamp reftests passed in a
324-test focused run with no regressions. Other full-run changes were intermittent and were not used for expectation
updates.
Counting only non-phantom lines and rendering custom block ellipsis strings raised the comparable score to
**74.84%** (21,853.6 test-equivalents passing, raw), a further gain of 8.9. Nine line-clamp reftests passed in the
focused and full runs without a focused regression; the small changes elsewhere in the full run were intermittent.
Making room for a block ellipsis at direct-text soft wrap points and trimming trailing whitespace raised the
comparable score to **74.86%** (21,859.7 test-equivalents passing, raw), a further gain of 6.1. Six line-clamp
reftests passed in both the focused and full runs. Two other full-run regressions passed on a focused repeat and were
excluded from expectation updates.
Aligning trailing items in auto-sized column grid lanes raised the comparable score to **74.88%** (21,863.5
test-equivalents passing, raw), a further gain of 3.8. Four grid-lanes reftests passed in the 1,005-test focused run
and the full CSS run, with no grid-lanes status regressions. Unrelated full-run status changes were excluded from
expectation updates; the filter-effects regression passed on a focused repeat.
Parsing `grid-lanes-direction` and honoring its row, column, track-reverse, and fill-reverse values raised the
comparable score to **74.93%** (21,877.6 test-equivalents passing, raw), a further gain of 14.2. The 1,005-test
grid-lanes run gained 19.5 test-equivalents with no regressions in that directory. Seven unrelated full-run losses
returned to passing status on a focused repeat and were excluded from expectation updates.
Mapping vertical Grid tracks and item boxes to their physical axes raised the comparable score to **75.20%**
(21,956.7 test-equivalents passing, raw), a further gain of 79.0. The 2,516-test Grid run gained 74.8
test-equivalents, with 14 reftests newly passing and one Grid Lanes alignment reftest regressing. That remaining
case exposes an intrinsic-size gap for vertical Grid Lanes. Other apparent full-run losses returned to their prior
results on a focused repeat and were excluded from expectation updates.
Using logical block margins in the user-agent stylesheet and synthesizing a central baseline for replaced inline
boxes in vertical text raised the comparable score to **75.42%** (22,020.7 test-equivalents passing, raw), a further
gain of 64.0. The 1,247-test writing-modes run gained 65.0, with 67 reftests newly passing and two text-combine
reftests regressing. Unrelated apparent full-run losses returned to their prior results on a focused repeat and were
excluded from expectation updates.
Keeping a leading out-of-flow block in a clamped block's direct inline formatting context makes four abspos and
fixed-position reftests pass in the 324-test `line-clamp` run, with no regressions there. The next full CSS run
measured **75.44%** (22,029.0 test-equivalents passing, raw). Its 8.3 increase includes 4.3 test-equivalents of
changes in unrelated tests; only the four repeatable line-clamp passes are attributed to this change.
When `line-clamp` specifies both a line count and `auto`, layout now uses the earlier of the line-count and block-size
limits. The 324-test `line-clamp` rerun gained one passing reftest (`line-clamp-041.html`) with no regressions. This
focused gain has not been included in a full CSS rerun.
Experimental `flex-wrap: balance` distributes items across lines by minimizing squared unused space, with
`flex-line-count` setting a minimum. Intrinsic main and cross sizes account for balanced lines, and flex items use
the space available on each line when measuring their content. The 1,419-test `css-flexbox` run gained 41.0 passing
test-equivalents against the last full CSS run, including 35 newly passing reftests, with no status regressions.
Together with the preceding line-clamp gain, the projected comparable CSS score is **75.59%** (22,071.0 of 29,199
test-equivalents); these focused gains have not yet been measured in a full CSS rerun.
Implementing predefined Greek, kana, CJK, Armenian, Georgian, and Hebrew counter styles, plus custom additive
systems and named fallbacks, raised the 259-test `css-counter-styles` run from 98.59 to 148.59 passing
test-equivalents. The 225-test `css-lists` run gained another 4.0; neither directory had a status regression.
Including these 54.0 focused gains, the projected comparable CSS score is **75.77%** (22,125.0 of 29,199
test-equivalents); these gains have not yet been measured in a full CSS rerun.
The required limited-range Japanese, Korean, and Chinese longhand counter styles added 48.0 more passing
test-equivalents in a 484-test `css-counter-styles` and `css-lists` rerun, with no status regressions. The projected
comparable CSS score is **75.94%** (22,173.0 of 29,199 test-equivalents); these focused gains also await a full CSS
rerun.
Applying custom `@counter-style` marker prefixes and suffixes raised the 484-test `css-counter-styles` and `css-lists`
run by another 16.0 passing test-equivalents with no status regressions. A focused 382-test CSS2 Lists and Generated
Content run after the counter-style changes gained 9.0 more test-equivalents against the last full CSS run, also with
no regressions. These disjoint gains project a comparable CSS score of **76.02%** (22,198.0 of 29,199
test-equivalents); a full CSS rerun has not yet measured them.
Custom `@counter-style` negative, range, and pad descriptors, including overrides on `extends` styles, raised the
same 484-test counter-style and list run by 11.0 more test-equivalents with no status regressions. The projected
comparable CSS score is **76.06%** (22,209.0 of 29,199 test-equivalents); a full CSS rerun is still pending.
Grid Lanes now limits automatic placement to the template's lanes, chooses the next lane when heights tie, and
stretches auto-sized items across their row span. Auto-sized row-lane containers include their lane tracks. The
1,005-test `grid-lanes` run gained 25.97 passing test-equivalents against the last full CSS run, with 24 tests and 23
subtests newly passing and no status regressions. A subsequent full `css` run measured **75.73%** (22,121.0 of 29,211
test-equivalents) for the tests selected by the current `include.ini`, and **68.56%** (23,889.5 of 34,847) across all
explicit CSS tests. Earlier focused projections counted gains in `css-counter-styles`, which `include.ini` skips.
Clamping a negative `calc()` weight in additive counter symbols to zero restores the passing
`css-values/negative-calc-to-non-negative-integer.html` reftest. The 484 counter-style and list tests retain their
previous results; a full CSS rerun after this final fix is pending.

On 15 November 2026, on a new machine, a full `css` run selected 29,542 tests through the current `include.ini` and
measured **75.66%** (22,351.3 test-equivalents) before further work. After the changes below, the same selection
measured **77.75%** (22,970.2), a gain of 618.9. The largest contributions were gap decorations (about 270 across
`css-gaps` and Grid Lanes), `corner-shape` and polygon `clip-path` (about 100), visible control characters (61), SVG
pattern sampling (36), `mask-image` (36), gap decoration interpolation (22), SVG image root backgrounds (21) and
`:focus-within` (18). The few tests that changed from passing to failing in the full run are listed with the
remaining work below.

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

## Details and limits

Per-area detail (what changed, what is still missing) is in [docs/armature](docs/armature/README.md), along with how the
numbers above were measured.

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
  - On Arch, which `./mach bootstrap` doesn't support, also install `protobuf` (the `webnn` feature needs `protoc`
    to build) and, to run WebNN, `onnxruntime-cpu`.
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
