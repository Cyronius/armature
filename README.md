# Armature: Servo, accelerated with AI

Armature is an unofficial fork of [Servo](https://github.com/servo/servo) that uses AI to accelerate development of
the engine. It is built with an AI coding assistant (Claude Code), and progress is measured by
[web-platform-tests](https://web-platform-tests.org/) conformance. It is **not** affiliated with or endorsed by the
Servo project.

Servo's [AI usage policy](https://book.servo.org/policy/ai-usage.html) does not accept AI-generated contributions,
so this work is not intended for, and has not been sent to, upstream Servo.

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
- `AbortController.abort()` now rethrows an exception from an abort algorithm, and `Document.contentType` returns only
  the essence.
- `./mach fmt` and the crown GC-rooting lint pass, and the tested `dom`, `html/dom`, `mimesniff`, `fetch` and
  `FileAPI` directories have no unexpected results other than a few tests that also fail on the unmodified tree.

## What has not been done

- One Observable subtest still fails: `when()` in a detached document, because ordinary event listeners still run in
  detached documents.
- 26 `ProcessingInstruction` subtests need the HTML parser to produce processing instructions (html5ever) or
  xml5ever's serializer to escape `>` in attribute values.
- About 1,200 `mimesniff` Blob/File subtests expect `new Blob([], {type})` to parse and re-serialise the type, which
  Servo does not do; about 190 more need header values with control characters, which Servo's header storage rejects.
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
