# Armature: Servo, accelerated with AI

Armature is an unofficial fork of [Servo](https://github.com/servo/servo) that uses AI to accelerate development of
the engine. It is built with an AI coding assistant (Claude Code), and progress is measured by
[web-platform-tests](https://web-platform-tests.org/) conformance. It is **not** affiliated with or endorsed by the
Servo project.

Servo's [AI usage policy](https://book.servo.org/policy/ai-usage.html) does not accept AI-generated contributions,
so this work is not intended for, and has not been sent to, upstream Servo.

## Conformance

Except for CSS, the table below was measured on 7 November 2026 using the 61,548 web-platform-tests selected then.
Two builds ran on the same machine: upstream Servo at the commit Armature forked from
(`90c3f623175`, 30 October 2026) and Armature at `568f69bdeac`. As on [wpt.fyi](https://wpt.fyi/), each test counts
once, with partial credit for the share of its subtests that pass; the last column counts individual subtests. The
Total row reports 63,770 more passing subtests than upstream Servo (2,088,378 against 2,024,608) and predates the
WebGL rerun. The CSS row records Armature's 15 November 2026 result on the current test selection; its upstream and
subtest figures are carried forward until the next full-suite rerun. On 22 November 2026 the 10,601 tests under
`html/` scored 67.9% before the batch of canvas, `dir`, body margin, lazy loading and render-blocking, preload scanner, lazy loading, form
control, pseudo-class, history, template and editing-attribute work described in [docs/armature/html.md](docs/armature/html.md) and 77.3% after it; the HTML row has not been remeasured against
upstream Servo yet.

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

## Added features

Capabilities Armature has that upstream Servo does not, beyond fixes that raise conformance.

### Experimental web chrome

Run `./target/release/servoshell --web-chrome` after a release build to open the HTML desktop shell.
A fresh desktop session and new tabs open the bundled Armature start page.
With a persistent profile, launching without a URL reopens the previous windows, tabs, and selected tab in each
window. Back and Forward reload saved URLs on demand (up to 50 entries in each direction per tab). Session
restore saves HTTP(S) addresses, blank pages, and new tabs. It does not save form contents or resubmit POST bodies.
Passing a URL opens that page directly. Previous windows can then be opened from the Armature menu.
Ctrl/Cmd+L selects the address, Enter opens the URL or searches, and Escape cancels the edit and returns to the page.
The address toolbar includes a bookmark star and shows Downloads after the first transfer. The Armature @ menu
opens History, Bookmarks, Downloads, Settings, find-in-page, the tab switcher, and tab recovery. Settings can place tabs on top or on the left. F11 toggles
browser fullscreen. The Downloads panel accepts public HTTP(S) file URLs and tracks pages saved as HTML. The site settings button inside the address field shows
the current web origin and URL scheme.

Bundled chrome scripts can call `window.armature.tabs()` to get tab IDs, then use `queryDOM`, `mutateDOM`, and
`evaluateScript` on a target with a tab ID and `frame: "top"`. A query returns a document ID that edits and script
evaluations must include. Navigation to a new document invalidates the previous ID while that document is inactive.
WebDriver and remote DevTools are unavailable in this mode.

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
