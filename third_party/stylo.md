# Vendored Stylo

`third_party/stylo/` is a copy of [Stylo](https://github.com/servo/stylo), the CSS engine that Servo shares with
Firefox. Armature builds against this copy instead of a Git dependency, so that it can enable and fix CSS features
that the upstream copy builds only for Gecko. This file records where the copy came from and every change Armature
makes to it, so that upstream changes can be brought in, and our changes carried forward, without guesswork.

## Upstream

| | |
|---|---|
| Upstream repository | https://github.com/servo/stylo (itself synced from mozilla-central's `servo/` directory) |
| Imported revision | `cd230184a620c3e6c0f25faa07838fddcefec32d` ("Enable `background-clip: text` behind a pref (#501)", 2026-10-29) |
| Import commit | The Armature commit whose message has the trailer `Stylo-Import: cd230184a620c3e6c0f25faa07838fddcefec32d` |

The import commit contains the upstream tree unmodified, so `git diff <import commit> -- third_party/stylo` always
shows exactly what Armature has changed.

## How Servo uses the copy

- The root `Cargo.toml` points the eight Stylo crates (`selectors`, `servo_arc`, `stylo`, `stylo_atoms`, `stylo_dom`,
  `stylo_malloc_size_of`, `stylo_static_prefs`, `stylo_traits`) at `third_party/stylo/<crate>` with path dependencies.
- `third_party/stylo` is in the workspace's `exclude` list. Stylo keeps its own `Cargo.toml` workspace manifest, which
  its crates inherit from, so the copy builds exactly as it does upstream and its own files need no changes.
- `Cargo.lock` still pins every external dependency of Stylo; the import only removed the Git `source` lines.
- Tidy, taplo and ruff already ignore `third_party/`, and `./mach fmt` formats only workspace members, so neither
  reformats the copy.

## Rules for changing the copy

1. **One change, one entry.** Every change to `third_party/stylo/` belongs to an entry in [Changes](#changes) below,
   with an ID of the form `AS-NNN` (Armature Stylo). Add the entry in the same commit as the change.
2. **Keep Stylo commits separate.** A commit that touches `third_party/stylo/` may also touch this file, and nothing
   else. Make Servo-side changes (prefs, layout, tests, expectations) in separate commits. This keeps every Stylo
   change exportable as a plain patch against upstream Stylo.
3. **Name the change in the commit.** Start the subject with `stylo:` and add a `Stylo-Change: AS-NNN` trailer.
4. **List every file.** Each entry's *Files* line lists every file it changes, as a path relative to
   `third_party/stylo/`. If a later change touches a file again, add the file to that change's entry too.
5. **Mark the code.** Put a `// Armature (AS-NNN): ...` comment (or `# Armature (AS-NNN): ...` in Python, TOML and
   Mako) at each changed spot, so the change is easy to find when resolving merge conflicts.
6. **Prefer small, additive diffs.** Change upstream lines only where needed, and prefer switching a property from
   `engine = "gecko"` to both engines (or behind a `servo_pref`) over restructuring code, so upstream merges stay easy.

`./mach test-tidy` enforces rules 1 to 4: it lists the files that differ from the import commit and checks that each
is named in an entry, that every entry's files really differ, and that every commit since the import that touches the
copy has a valid `Stylo-Change` trailer and touches nothing outside `third_party/stylo/` and this file.

## Bringing in a newer upstream Stylo

1. Export Armature's changes against the current import:
   `git diff <import commit> HEAD -- third_party/stylo > /tmp/armature-stylo.diff`
2. Replace the copy with the new upstream revision, unmodified:
   `rm -r third_party/stylo && mkdir third_party/stylo && git -C <stylo clone> archive <new rev> | tar -x -C third_party/stylo`
3. Update the *Imported revision* row above and commit with the trailers `Stylo-Upstream: https://github.com/servo/stylo`
   and `Stylo-Import: <new rev>`. That commit becomes the new baseline.
4. Re-apply the changes from the repository root: `git apply --3way /tmp/armature-stylo.diff` (or entry by entry,
   using the commits found with `git log --grep "Stylo-Change: AS-NNN"`). Drop entries that upstream has made
   unnecessary, and say so under the entry. Commit the re-applied changes with their `Stylo-Change` trailers.
5. Build, run crown and `./mach test-tidy`, and rerun the CSS test directories.

## Sending a change upstream

Because each Stylo commit touches only the copy (and this file), it can be exported as a patch that applies to a
checkout of `servo/stylo`:

```sh
git format-patch --relative=third_party/stylo <import commit>..HEAD -- third_party/stylo
```

Note that Servo's AI policy also covers the `servo/stylo` repository.

## Changes

Each entry has this form (the check reads the `### AS-NNN` headings and the `Files` lines):

```markdown
### AS-001: Short summary
- Files: `style/properties/longhands.toml`, `style/values/specified/box.rs`
- Why: What the change enables or fixes, and which WPT tests it affects.
- Upstream: Whether the change could go upstream, or why it is Armature-only.
```

### AS-101: Enable the scroll snap properties for Servo
- Files: `style/properties/longhands.toml`, `style/properties/shorthands.toml`, `stylo_static_prefs/preferences.toml`
- Why: Builds `scroll-snap-type`, `scroll-snap-align`, `scroll-snap-stop`, the `scroll-margin-*` and
  `scroll-padding-*` longhands and the `scroll-margin`, `scroll-margin-block`, `scroll-margin-inline`,
  `scroll-padding`, `scroll-padding-block` and `scroll-padding-inline` shorthands for Servo, behind the new
  `layout.css.scroll-snap.enabled` preference. Parsing, computed values, serialization and interpolation work as in
  Gecko; Servo does not snap yet. Affects css/css-scroll-snap/parsing, css/cssom-view/scrollIntoView-scroll*.
- Upstream: Could go upstream as is (the value types already build for Servo).

### AS-102: Enable `scrollbar-gutter` for Servo
- Files: `style/properties/longhands.toml`, `stylo_static_prefs/preferences.toml`
- Why: Builds `scrollbar-gutter` for Servo behind the new `layout.css.scrollbar-gutter.enabled` preference, for
  css/css-overflow/parsing/scrollbar-gutter-*.html and css/css-overflow/inheritance.html. Servo's scrollbars take no
  space, so the gutter is always zero wide, which is what the property means for overlay scrollbars.
- Upstream: Could go upstream as is.

### AS-103: Enable `overflow-anchor` for Servo
- Files: `style/properties/longhands.toml`, `stylo_static_prefs/preferences.toml`
- Why: Builds `overflow-anchor` for Servo behind the new `layout.css.scroll-anchoring.enabled` preference (the name
  Gecko uses). Only parsing and computed values; Servo does no scroll anchoring.
- Upstream: Could go upstream as is.

### AS-104: Enable the scroll-driven animation timeline properties for Servo
- Files: `style/properties/longhands.toml`, `style/properties/shorthands.toml`, `style/properties/shorthands.rs`
- Why: Builds `scroll-timeline-name`, `scroll-timeline-axis`, `view-timeline-name`, `view-timeline-axis`,
  `view-timeline-inset`, `timeline-scope` and the `scroll-timeline`, `view-timeline` and `animation-range`
  shorthands for Servo, behind the existing `layout.css.scroll-driven-animations.enabled` preference that already
  gates `animation-timeline` and `animation-range-start`/`-end` there. The three shorthand modules in `shorthands.rs`
  lose their `#[cfg(feature = "gecko")]`. Affects css/css-animations/parsing/animation-range-shorthand*.html and
  scroll-animations/css/*-parsing.html, *-computed.html and *-shorthand.html. Servo does not run scroll-driven
  animations yet.

### AS-201: Build the SVG painting properties for Servo
- Files: `style/properties/longhands.toml`, `style/properties/shorthands.toml`, `style/properties/shorthands.rs`, `stylo_static_prefs/preferences.toml`
- Why: Builds `clip-rule`, `marker-start`, `marker-mid`, `marker-end`, the `marker` shorthand, `paint-order`,
  `text-anchor`, `color-interpolation`, `color-interpolation-filters`, `shape-rendering`, `flood-color`,
  `flood-opacity`, `lighting-color`, `stop-color`, `stop-opacity` and `vector-effect` for Servo behind the new
  preference `layout.css.svg-properties.enabled`, so that they parse, compute, serialize, inherit and interpolate, and
  so that Servo can map their SVG presentation attributes. Affects the parsing, computed and inheritance tests in
  `svg/painting`, `svg/pservers`, `svg/text`, `svg/coordinate-systems`, `svg/styling`, `css/css-masking` and
  `css/filter-effects`.
- Upstream: Could go upstream as is; the properties need no Gecko-only code.

### AS-202: Build accent-color for Servo
- Files: `style/properties/longhands.toml`, `stylo_static_prefs/preferences.toml`
- Why: Builds `accent-color` for Servo behind the new preference `layout.css.accent-color.enabled`, for the
  `css/css-ui` parsing, computed and animation tests. Servo's form controls do not use it yet.
- Upstream: Could go upstream as is.

### AS-203: Build print-color-adjust for Servo
- Files: `style/properties/longhands.toml`, `stylo_static_prefs/preferences.toml`
- Why: Builds `print-color-adjust` (and its `color-adjust` alias) for Servo behind the new preference
  `layout.css.print-color-adjust.enabled`, for the `css/css-color-adjust` parsing and inheritance tests. Servo does
  not print, so the value has no effect.
- Upstream: Could go upstream as is.

### AS-204: Build image-orientation for Servo
- Files: `style/properties/longhands.toml`, `stylo_static_prefs/preferences.toml`
- Why: Builds `image-orientation` for Servo behind the new preference `layout.css.image-orientation.enabled`, for the
  `css/css-images/image-orientation` parsing and computed-style tests. Servo always applies the EXIF orientation when
  it decodes an image, so `image-orientation: none` is parsed but not yet honoured.
- Upstream: Could go upstream as is.

### AS-401: Enable the CSS Shapes properties for Servo behind a preference
- Files: `style/properties/longhands.toml`, `stylo_static_prefs/preferences.toml`, `style/values/specified/basic_shape.rs`
- Why: Builds `shape-outside`, `shape-margin` and `shape-image-threshold` for Servo behind the new preference
  `layout.css.shapes.enabled`, so they parse, compute, serialize and interpolate. Servo's float layout does not wrap
  content around the shapes yet and ignores the values. Servo does not parse images in `shape-outside`, because it
  can neither fetch them (with CORS, as the spec requires) nor use them; otherwise
  `css/fetching/fetch-resources.https.sub.html` waits for a request that never comes and times out. Affects
  `css/css-shapes/parsing`, `css/css-shapes/animation` and `css/css-shapes/inheritance.html`.
- Upstream: Could go upstream as is, but Servo's AI policy currently prevents sending it.

### AS-402: Enable the motion path properties and the offset shorthand for Servo
- Files: `style/properties/longhands.toml`, `style/properties/shorthands.toml`, `style/properties/shorthands.rs`
- Why: Builds `offset-position`, `offset-distance`, `offset-rotate`, `offset-anchor` and the `offset` shorthand for
  Servo behind `layout.css.motion-path.enabled`, the preference that already gates `offset-path`, and builds the
  shorthand's parser and serializer for Servo. Servo's layout does not apply motion paths yet. Affects
  `css/motion/parsing`, `css/motion/animation` and `css/motion/inheritance.html`.
- Upstream: Could go upstream as is, but Servo's AI policy currently prevents sending it.

### AS-403: Enable transform-box for Servo behind a preference
- Files: `style/properties/longhands.toml`, `stylo_static_prefs/preferences.toml`, `style/values/specified/transform.rs`
- Why: Builds `transform-box` for Servo behind the new preference `layout.css.transform-box.enabled`, and lets Servo
  parse all five keywords, since Servo's layout now uses the content box or the border box as the reference box.
  Affects `css/css-transforms/parsing/transform-box-*.html`, `css/css-transforms/transform-box/cssbox-*.html` and
  `css/css-transforms/inheritance.html`.
- Upstream: Could go upstream together with Servo's layout support, but Servo's AI policy currently prevents sending
  it.

### AS-404: Enable text-decoration-skip-ink for Servo behind a preference
- Files: `style/properties/longhands.toml`, `stylo_static_prefs/preferences.toml`
- Why: Builds `text-decoration-skip-ink` for Servo behind the new preference
  `layout.css.text-decoration-skip-ink.enabled`. Servo's painting ignores the value. Affects
  `css/css-text-decor/parsing/text-decoration-skip-ink-*.html`, `css/css-text-decor/text-decoration-skip-ink.html`
  and `css/css-text-decor/inheritance.html`.
- Upstream: Could go upstream as is, but Servo's AI policy currently prevents sending it.

### AS-405: Enable hyphenate-limit-chars for Servo behind the hyphens preference
- Files: `style/properties/longhands.toml`
- Why: Builds `hyphenate-limit-chars` for Servo behind `layout.css.hyphens.enabled`, which already gates `hyphens`
  and `hyphenate-character`. Servo's layout ignores the value. Affects
  `css/css-text/parsing/hyphenate-limit-chars-*.html`.
- Upstream: Could go upstream as is, but Servo's AI policy currently prevents sending it.

### AS-406: Enable math-shift for Servo behind the math properties preference
- Files: `style/properties/longhands.toml`
- Why: Builds `math-shift` for Servo behind `layout.css.math-properties.enabled`, which already gates `math-depth`
  and `math-style`. Servo's layout ignores the value. Affects
  `mathml/relations/css-styling/parsing/math-shift-*.html`.
- Upstream: Could go upstream as is, but Servo's AI policy currently prevents sending it.

### AS-301: Build contain-intrinsic-size for Servo behind a pref
- Files: `style/properties/longhands.toml`, `style/properties/shorthands.toml`, `stylo_static_prefs/preferences.toml`
- Why: Enables `contain-intrinsic-width`, `-height`, `-block-size`, `-inline-size` and the `contain-intrinsic-size`
  shorthand for Servo behind the new `layout.css.contain-intrinsic-size.enabled` preference, for parsing, computed
  values and interpolation (css/css-sizing/contain-intrinsic-size/parsing and animation). Servo's layout does not
  implement size containment, so the properties have no layout effect yet.
- Upstream: Could go upstream as a Servo preference.

### AS-302: Build content-visibility for Servo behind a pref
- Files: `style/properties/longhands.toml`, `style/properties/helpers/animated_properties.mako.rs`, `stylo_static_prefs/preferences.toml`
- Why: Enables `content-visibility` for Servo behind the new `layout.css.content-visibility.enabled` preference, with
  its discrete animation behaviour (css/css-contain/content-visibility/parsing and content-visibility-interpolation).
  Parsing only: Servo's layout does not skip the contents, and the Gecko-only style adjustments for containment are
  left as they are.
- Upstream: Could go upstream as a Servo preference.

### AS-303: Build the break properties for Servo behind a pref
- Files: `style/properties/longhands.toml`, `style/properties/shorthands.toml`, `stylo_static_prefs/preferences.toml`
- Why: Enables `break-before`, `break-after`, `break-inside` and the legacy `page-break-before`, `-after` and
  `-inside` shorthands for Servo behind the new `layout.css.break-properties.enabled` preference
  (css/css-break/parsing). Servo does not fragment, so they have no layout effect.
- Upstream: Could go upstream as a Servo preference.

### AS-304: Build column-rule and column-fill for Servo behind the columns pref
- Files: `style/properties/longhands.toml`, `style/properties/shorthands.toml`, `style/properties/shorthands.rs`
- Why: Enables `column-rule-color`, `column-rule-style`, `column-rule-width`, `column-fill` and the `column-rule`
  shorthand for Servo behind the existing `layout.css.columns.enabled` preference, like the other multicol
  properties (css/css-multicol/parsing, css/css-gaps/parsing). Servo does not paint column rules yet.
- Upstream: Could go upstream as a Servo preference.

### AS-305: Build counter-set for Servo behind the counters pref
- Files: `style/properties/longhands.toml`
- Why: Enables `counter-set` for Servo behind the existing `layout.css.counters.enabled` preference, next to
  `counter-reset` and `counter-increment` (css/css-lists/parsing/counter-set-*, and the `li value` presentational
  hint in html/rendering/non-replaced-elements/lists).
- Upstream: Could go upstream as a Servo preference.

### AS-306: Build margin-trim for Servo behind a pref
- Files: `style/properties/longhands.toml`, `stylo_static_prefs/preferences.toml`
- Why: Enables `margin-trim` for Servo behind `layout.css.margin-trim.enabled`, the name Gecko also uses
  (css/css-box/parsing/margin-trim*). Parsing only; Servo's layout does not trim margins.
- Upstream: Could go upstream as a Servo preference.

### AS-307: Build resize for Servo behind a pref
- Files: `style/properties/longhands.toml`, `stylo_static_prefs/preferences.toml`
- Why: Enables `resize` for Servo behind the new `layout.css.resize.enabled` preference
  (css/css-ui/parsing/resize-*). Parsing only; Servo has no resize handles.
- Upstream: Could go upstream as a Servo preference.

### AS-308: Build field-sizing for Servo behind a pref
- Files: `style/properties/longhands.toml`, `stylo_static_prefs/preferences.toml`
- Why: Enables `field-sizing` for Servo behind `layout.css.field-sizing.enabled`, the name Gecko also uses
  (css/css-ui/parsing/field-sizing-*). Parsing only; Servo's form controls keep their fixed size.
- Upstream: Could go upstream as a Servo preference.
