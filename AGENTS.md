# AGENTS.md

This file provides guidance to coding agents working with code in this repository.

## What this repository is

Armature is an unofficial fork of [Servo](https://github.com/servo/servo) developed with AI coding agents. Progress is
measured by web-platform-tests (WPT) conformance against upstream Servo at the fork point (`90c3f623175`); the
README's Conformance, "What has been done" and "What has not been done" sections record that and should be updated
when a batch of work lands. Servo's AI policy forbids AI-generated contributions, so nothing here is sent upstream.
Remotes: `origin` is github.com/Cyronius/armature, `upstream` is servo/servo. Push only when the user approves.

## Commands

Everything goes through `./mach` (Python, run via `uv`).

- Build: `./mach build --release`. `./mach check` / `cargo check` don't work (webrender's build script panics), so
  compile-check with a release build.
- GC-rooting lint: `./mach build --release --use-crown`. Use a separate `CARGO_TARGET_DIR` so it doesn't invalidate
  `target/release`.
- Format and lint: `./mach fmt`, `./mach test-tidy`.
- Run a page: `./target/release/servoshell --headless -x file:///path/to/page.html` (print results with
  `console.log`).
- WPT:
  - One test or directory: `./mach test-wpt --release css/css-grid/foo.html` (paths are relative to
    `tests/wpt/tests`; `_mozilla/...` selects `tests/wpt/mozilla/tests`).
  - Record results: `./mach test-wpt --release --log-raw out.log [paths]`, then `./mach update-wpt out.log`.
    `--update-expectations` can't be combined with `--log-raw`.
  - After adding or changing tests: `./mach update-manifest`.
  - Run one WPT job at a time, and don't rebuild `target/release` while a run is using it.
- Unit tests: `./mach test-unit` needs cargo-nextest. Without it use e.g. `cargo test --release -p servo-net --test main`.

## WPT expectations

- `tests/wpt/include.ini` decides which WPT directories run. Armature enables more than upstream does.
- Passing explicit paths to `test-wpt` ignores `include.ini`. Before `update-wpt`, delete expectation files created
  for tests that `include.ini` excludes.
- Expected results live in `.ini` files under `tests/wpt/meta/` (mirroring `tests/wpt/tests/`) and
  `tests/wpt/mozilla/meta/`. Only failures are recorded.
- Before recording, rerun suspicious results and check them against an upstream build. Many reftest and font
  failures are flaky or specific to one machine.
- Stage `tests/wpt/meta/<subdir>` rather than all of `tests/wpt/meta`, so that a locally dirty `MANIFEST.json` isn't
  committed. `tests/wpt/mozilla/meta/MANIFEST.json` *is* committed when mozilla tests change.
- Exposing a new interface makes `tests/wpt/mozilla/tests/mozilla/interfaces.https.html` fail until the interface is
  added to its list.
- WPT runs pass `--enable-experimental-web-platform-features`, which turns on the prefs in `EXPERIMENTAL_PREFS`
  (`ports/servoshell/prefs.rs`).

## Architecture (big picture)

- `ports/servoshell`: the browser shell (winit/egui, headless mode).
- `components/servo`: the embedding API.
- `components/constellation`: manages pipelines (one per document) and event loops.
- `components/script`: the DOM and the JS bindings, running SpiderMonkey via `mozjs`.
  - DOM objects live in `components/script/dom/<area>/`.
  - WebIDL files are in `components/script_bindings/webidls/`.
  - Codegen (`components/script_bindings/codegen/`) generates a `FooMethods` trait from each IDL file, which the Rust
    `#[dom_struct]` implements.
  - New elements must be added to `components/script/dom/element/create.rs`, and `VirtualMethods` dispatch is in
    `dom/node/virtualmethods.rs`.
- `components/layout`: builds the box and fragment trees, and answers script's layout queries
  (`layout/query.rs`, for example `getComputedStyle`, offset* and hit testing).
- `components/paint`: WebRender rendering.
- `components/shared/*`: the message and type crates that the other crates share.
- Style comes from Stylo (CSS parsing, cascade, animation values), vendored in `third_party/stylo`.
  - Stylo prefs are forwarded from `components/config/prefs.rs`.
  - Many properties are built only for Gecko upstream. Enabling them for Servo is a Stylo change.
- Inline SVG is not laid out by Servo:
  - The `<svg>` subtree is serialized and rasterized with resvg as a single replaced box.
  - Elements inside `<svg>` compute `display: none` and may have no style data.
  - SVG DOM geometry (`getBBox`, `getCTM`, hit testing inside the svg) is therefore computed in script from
    attributes, in `dom/svg/svggeometry.rs` and `svgtext.rs`.
- Web Animations timing is owned by script: `dom/animations/` and `document/animation_manager.rs`. Script feeds
  animation values to Stylo, which applies them for both script and CSS animations.

## Code conventions that the build enforces

- **Bindings.conf `'cx'` lists:** methods listed there receive `&mut JSContext`. Element interfaces get it on
  attribute setters implicitly; listing those `Set*` methods trips a codegen assertion.
- **Generated names:** a getter for a nullable attribute is generated as `GetFoo`. A `[Throws]` getter returns
  `Fallible<...>`.
- **Crown (rooting lint):**
  - A type that holds a `Dom<T>` (for example an enum) needs `#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]`.
  - Don't bind unrooted must-root values to locals. Root them (`as_rooted()`, collect into `DomRoot`s), or build
    them inline in the call that stores them.
- **Tidy:**
  - Each `.webidl` file needs a spec link with an allowed prefix (see `WEBIDL_STANDARDS` in `python/tidy/tidy.py`).
    Use `w3c.github.io/...` URLs for FXTF and SVG specs, not drafts.fxtf.org or svgwg.org.
  - New `Error::*` uses need a message.
- **Vendored copies:** `third_party/stylo` (AS-NNN), `html5ever` (AH), `xml5ever` (AX) and `markup5ever` (AM). Each
  has a change log, `third_party/<name>.md`, whose rules tidy checks:
  - A commit touching a copy touches only that copy and its `.md` file.
  - The subject starts with `stylo:` (or the copy's name) and the commit has a `Stylo-Change: AS-NNN`-style trailer.
  - The `.md` entry lists every changed file.
  - Each changed spot carries a `// Armature (AS-NNN): ...` comment.
  - Servo-side follow-ups go in separate commits, and every commit must build. When a Stylo API changes, land a Servo
    preparation commit first.
- **Commit subjects:** use `component: Imperative summary` (`script:`, `layout:`, `tests:`, `stylo:`, `README:`). Put
  expectation updates in their own `tests:` commit.
