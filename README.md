# Armature: Servo, accelerated with AI

Armature is an unofficial fork of [Servo](https://github.com/servo/servo) that uses AI to accelerate development of
the engine. It is built with an AI coding assistant (Claude Code), and progress is measured by
[web-platform-tests](https://web-platform-tests.org/) conformance. It is **not** affiliated with or endorsed by the
Servo project.

Servo's [AI usage policy](https://book.servo.org/policy/ai-usage.html) does not accept AI-generated contributions,
so this work is not intended for, and has not been sent to, upstream Servo.

## Conformance

Measured on 5 November 2026 by running the web-platform-tests that Armature runs (the 60,957 tests selected by
`tests/wpt/include.ini`) with two builds on the same machine: upstream Servo at the commit Armature forked from
(`90c3f623175`, 30 October 2026) and Armature at `29904a1afdb` (the Web Animations row was measured again on 6
November, after a fix). As on [wpt.fyi](https://wpt.fyi/), each test counts once, with partial credit for the share of
its subtests that pass; the last column counts individual subtests. In total Armature passes 44,925 more subtests than
upstream Servo (2,086,143 against 2,041,218).

| Area | Tests | Upstream Servo | Armature | Change (points) | Subtests newly passing |
|---|--:|--:|--:|--:|--:|
| CSS | 28,730 | 67.3% | 69.1% | +1.8 | +11,348 |
| HTML | 10,253 | 66.7% | 68.1% | +1.4 | +18,040 |
| WebGL conformance | 2,939 | 40.5% | 40.5% | -0.1 | -24 |
| Referrer Policy | 1,483 | 86.3% | 87.0% | +0.7 | +11 |
| SVG | 1,411 | 24.9% | 35.9% | +11.0 | +968 |
| Encoding | 1,345 | 98.3% | 98.3% | +0.0 | +9 |
| Servo-specific tests | 1,229 | 75.6% | 75.6% | +0.0 | +0 |
| Fetch | 957 | 66.6% | 66.8% | +0.2 | +47 |
| Content Security Policy | 918 | 79.7% | 80.0% | +0.3 | +10 |
| Editing | 860 | 49.8% | 51.4% | +1.6 | +8,857 |
| DOM | 845 | 76.3% | 84.9% | +8.7 | +2,098 |
| IndexedDB | 820 | 57.3% | 57.4% | +0.1 | +1 |
| WebSockets | 799 | 67.9% | 68.1% | +0.1 | +1 |
| MathML | 624 | 46.1% | 46.5% | +0.4 | +41 |
| WebAssembly | 615 | 65.4% | 65.4% | +0.0 | +0 |
| XMLHttpRequest | 440 | 92.1% | 92.3% | +0.2 | +4 |
| Mixed Content | 388 | 87.4% | 87.4% | +0.0 | +0 |
| Shadow DOM | 366 | 67.3% | 67.3% | +0.0 | +0 |
| Web Audio | 355 | 58.6% | 58.9% | +0.3 | +1 |
| Workers | 336 | 91.8% | 92.1% | +0.3 | +1 |
| Web Crypto | 334 | 97.5% | 99.4% | +1.9 | +2,036 |
| Streams | 317 | 91.6% | 94.1% | +2.4 | +320 |
| Trusted Types | 249 | 86.7% | 89.0% | +2.4 | +77 |
| Custom Elements | 203 | 76.1% | 79.0% | +2.9 | +153 |
| Upgrade Insecure Requests | 197 | 73.0% | 72.5% | -0.5 | -6 |
| DOM Parsing | 94 | 43.5% | 48.8% | +5.3 | +51 |
| Compression Streams | 78 | 70.3% | 73.7% | +3.5 | +63 |
| Clipboard | 61 | 25.7% | 27.4% | +1.6 | +1 |
| Web Animations | 47 | 14.9% | 14.9% | +0.0 | +1 |
| Focus | 43 | 63.3% | 60.9% | -2.3 | -4 |
| Scroll-driven Animations | 17 | 21.8% | 100.0% | +78.2 | +372 |
| Everything else | 3,604 | 69.6% | 69.6% | +0.1 | +448 |
| **Total** | **60,957** | **66.8%** | **68.4%** | **+1.6** | **+44,925** |

The CSS gains come mostly from Stylo features that Armature enables. These are the CSS modules that gained the most:

| CSS module | Tests | Upstream Servo | Armature | Change (points) | Subtests newly passing |
|---|--:|--:|--:|--:|--:|
| `css-backgrounds` | 875 | 65.2% | 78.1% | +13.0 | +119 |
| `selectors` | 833 | 70.0% | 78.9% | +8.9 | +769 |
| `css-transforms` | 933 | 70.4% | 76.0% | +5.6 | +62 |
| `motion` | 46 | 20.7% | 83.3% | +62.6 | +3,761 |
| `css-fonts` | 559 | 66.0% | 70.9% | +4.9 | +859 |
| `css-scroll-snap` | 28 | 25.0% | 92.9% | +67.9 | +350 |
| `css-text-decor` | 366 | 40.7% | 45.2% | +4.5 | +459 |
| `css-writing-modes` | 1,247 | 30.1% | 31.4% | +1.3 | +78 |
| `css-ui` | 1,077 | 89.6% | 91.0% | +1.4 | +577 |
| `css-masking` | 222 | 53.7% | 60.7% | +7.0 | +261 |
| `css-text` | 1,983 | 72.3% | 73.1% | +0.8 | +406 |
| `CSS2` | 6,322 | 93.1% | 93.3% | +0.2 | +13 |
| `filter-effects` | 397 | 51.9% | 55.1% | +3.1 | +91 |
| `css-conditional` | 468 | 50.4% | 52.7% | +2.3 | +27 |
| `css-multicol` | 31 | 53.5% | 82.6% | +29.0 | +50 |

Notes on the numbers:

- Both builds ran the same tests, including the CSS directories that Armature added to `include.ini` and upstream
  Servo skips. Tests that set a preference only Armature has (Observable, popovers, `ProcessingInstruction`
  attributes, ARIA reflection) error on the upstream build, whose engine lacks those features anyway.
- The Armature run matched the expectations recorded in `tests/wpt/meta` for all but about 250 tests, so the scores
  should be close to what Servo's CI would measure.
- Areas that went down: a dozen CSS reftests and subtests involving `zoom`, subgrid line names and `skew()` broke when
  the Stylo preferences were enabled. The other small drops in the table (WebGL, Upgrade Insecure Requests, Focus)
  come from tests that time out or fail intermittently when the whole suite runs, and score the same for both builds
  when run alone.

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
- `AbortController.abort()` now rethrows an exception from an abort algorithm, and `Document.contentType` returns only
  the essence.
- `./mach fmt` and the crown GC-rooting lint pass, and the tested directories have no unexpected results other than a
  few tests that also fail or are flaky on the unmodified tree.

## What has not been done

- One Observable subtest still fails: `when()` in a detached document, because ordinary event listeners still run in
  detached documents.
- `xml-stylesheet` processing instructions do not yet react when inserting or removing the document element moves
  them into or out of the prolog, and style sheets still decode with UTF-8 as the fallback instead of the document's
  encoding (one `css-syntax/charset` subtest).
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
  `resize` has no resize handles, and `Element.animate()` is missing, so many CSS animation subtests still fail.
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
