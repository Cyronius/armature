# Armature: how the conformance numbers were measured

Notes on the table in the [README](../../README.md).

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
- The WebNN row was measured on 15 November 2026 by running only `webnn/` (598 tests, counting the `?cpu`, `?gpu`
  and `?npu` variants separately). Upstream Servo has no WebNN and does not run these tests, so it has no upstream
  score, and the row is not included in the Total.
