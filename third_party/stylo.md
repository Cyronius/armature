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

### AS-501: Carry the referrer of an external style sheet's fetches in its URL data
- Files: `style/stylesheets/mod.rs`, `style/url/servo.rs`
- Why: Resources that an external style sheet refers to (images, fonts and imported style sheets) must be fetched
  with the style sheet's URL as the referrer and with the referrer policy of the style sheet's response, but Servo's
  `UrlExtraData` held only the base URL, and `url()` values lost track of the style sheet they came from. Servo's
  `UrlExtraData` is now a struct with the base URL and an optional `StylesheetReferrer` (URL and referrer policy
  token), and `CssUrl` and `ComputedUrl::Valid` keep that referrer, so layout and the font loader can use it. A
  function named `UrlExtraData` keeps the old tuple constructor working, with no referrer. Affects the image, font
  and child style sheet tests in `referrer-policy/css-integration/`.
- Upstream: Could go upstream (Gecko keeps the same information in its `URLExtraData`'s `ReferrerInfo`), but Servo's
  AI policy currently prevents sending it.

### AS-502: Zoom the line width keywords
- Files: `style/values/specified/border.rs`
- Why: `thin`, `medium` and `thick` compute to 1px, 3px and 5px, absolute lengths that, like any other, are
  multiplied by the element's effective zoom. They were not, so with `zoom: 10` a `thin` border was drawn 1px wide
  and `getComputedStyle()`, which divides by the zoom, returned `0.1px`. Affects
  `css/css-viewport/zoom/computed-{border,outline,column-rule}-width-keywords.html`.
- Upstream: Could go upstream as is, but Servo's AI policy currently prevents sending it.

### AS-503: List the properties that a Servo preference enables in content in css-properties.json
- Files: `style/properties/build.py`, `style/properties/properties.mako.rs`
- Why: `css-properties.json`, from which Servo generates the attributes of `CSSStyleDeclaration`, listed only the
  properties with `enabled_in = "content"`. But a property that is enabled only in chrome and has a `servo_pref`
  (only `zoom` today) is enabled in content when that preference is set, so it could be used in style sheets but
  `"zoom" in getComputedStyle(element)` was false and `element.style.zoom` did nothing. The
  `css_properties_accessors!` macro, which Servo implements those attributes with, now lists the same properties.
  Affects `css/css-viewport/zoom/parsing/zoom-computed.html` and the other tests that use `style.zoom`.
- Upstream: Could go upstream as is, but Servo's AI policy currently prevents sending it.

### AS-504: Treat more property types that contain lengths as zoom dependent
- Files: `style/properties/data.py`
- Why: When an element's effective zoom differs from its parent's, an explicitly inherited (`inherit`) value of a
  zoom-dependent property is recomputed so that it stays the same length; other properties copy the parent's
  zoomed value, which then shows up divided by the new zoom. Only types in a fixed list counted as zoom dependent,
  and the check failed for types written with a module path (`length::NonNegativeLengthPercentageOrNormal`). The
  check now ignores the module path and lists the other types that contain lengths (border radii, `flex-basis`,
  grid tracks, background and mask sizes and positions, images, basic shapes, `translate`, `transform-origin`,
  `perspective` and others). Affects `css/css-viewport/zoom/explicit-inherit/all-length-properties.html`.
- Upstream: Could go upstream as is, but Servo's AI policy currently prevents sending it.

### AS-505: Zoom the initial values of the line widths and of inherited zoom-dependent properties
- Files: `style/properties/cascade.rs`, `style/properties/helpers.mako.rs`
- Why: Initial values come from the default style, unzoomed. That is right for zero lengths, but `medium`, the
  initial line width of the border, outline and column rule widths, is 3px, which an element with `zoom: 10` should
  draw 30px wide. The cascade now gives those six longhands their initial value again, computed with the zoom,
  when they are not set or are set to `initial` or `unset` on an element whose effective zoom is not 1. Inherited
  zoom-dependent properties set to `initial` (`font-size`, `stroke-width`) likewise compute their initial specified
  value with the zoom. Affects `css/css-viewport/zoom/computed-initial.html`, `border-width.html` and the
  `*-width-keywords.html` reftests there.
- Upstream: Could go upstream as is, but Servo's AI policy currently prevents sending it.

### AS-506: Apply animations created by script through the Web Animations API
- Files: `style/servo/animation.rs`, `style/matching.rs`
- Why: Servo's animation sets held only CSS animations and transitions, so an animation created with
  `Element.animate()` or `new Animation()` could not change the style of its target. Each element's
  `ElementAnimationSet` now also holds `ScriptAnimation`s: the keyframes of the effect and the iteration progress
  that script computes from the animation's timing. When the element is restyled, style resolves the keyframe
  values against the element's style without animations, and when the animation rule is built it computes the
  effect value as the Web Animations specification describes: neutral keyframes at offsets 0 and 1, interval
  endpoints, the keyframe's easing function and its composite operation (`replace`, `add` or `accumulate`), on top
  of CSS animations and earlier script animations. Elements with script animations do not share styles. Affects
  the "Web Animations" and "Compositing Web Animations" subtests of the `*-interpolation.html` and
  `*-composition.html` tests throughout `css/`.
- Upstream: Could go upstream, but Servo's AI policy currently prevents sending it.

### AS-507: Let script own the timing of CSS animations and transitions
- Files: `style/servo/animation.rs`
- Why: CSS animations and transitions had their own timing in style, so script could not expose them as
  `CSSAnimation` and `CSSTransition` objects that `getAnimations()` returns and that can be paused, seeked or
  reversed. Each CSS animation and transition now has an identifier, and once script has created its object,
  script computes its timing with the Web Animations model and gives style the iteration progress, which style
  samples instead of using its own clock. CSS animations sample their keyframes at that progress with each
  keyframe's timing function applied to the interval that starts at it; transitions interpolate between their
  start and end values at the eased progress. Style keeps generating, updating and canceling them from the
  computed values: it reports the timing properties and `animation-play-state` of each animation, keeps the
  identity and timing state of an animation when its properties change, keeps updating the remaining animations
  after an existing one is updated (it used to stop at the first), sorts animations by their position in
  `animation-name`, generates animations with a zero duration, uses the progress from script for reversed
  transitions, and exposes the keyframes of an animation with their computed values for
  `KeyframeEffect.getKeyframes()`. A completed transition keeps its end value until it is removed. Running
  transitions of a property that a script animation sets keep running (this changes AS-506, which canceled
  them). Affects most of `css/css-animations` and `css/css-transitions` and the animated reftests that wait for
  `document.getAnimations()`.
- Upstream: Could go upstream, but Servo's AI policy currently prevents sending it.

### AS-508: Build the SVG values of pointer-events for Servo
- Files: `style/values/specified/ui.rs`
- Why: `pointer-events` only had `auto` and `none` outside Gecko, so the SVG values (`visiblePainted`,
  `visibleFill`, `visibleStroke`, `visible`, `painted`, `fill`, `stroke` and `all`) were invalid. Servo now hit
  tests the shapes inside `svg` elements, which needs them. Servo's layout only compares the value with `none`,
  so HTML boxes treat the new values as `auto`, as other engines do. Affects
  `svg/interact/scripted/pointer-events-fill-stroke-hittest.html`, `svg/interact/parsing` and the
  `pointer-events` presentation attribute.
- Upstream: Could go upstream, but Servo's AI policy currently prevents sending it.

### AS-509: Resolve display in script animation effects
- Files: `style/servo/animation.rs`
- Why: SVG SMIL `<set>` can switch `display` at an eventbase. Script animations need to resolve the
  discrete display value instead of dropping its declaration, while CSS animation keyframes keep
  their existing display handling. Affects `svg/animations/slider-switch.html`.
- Upstream: Could go upstream as part of script animation support.

### AS-510: Add the CSS path-length property
- Files: `style/properties/longhands.toml`, `style/values/generics/svg.rs`, `style/values/specified/svg.rs`, `style/values/computed/svg.rs`
- Why: SVG 2 maps `pathLength` to an animatable CSS `path-length` property. Parse `none` or a
  non-negative length, compute and serialize it, and interpolate lengths. Affects
  `svg/path/parsing/path-length-*.tentative.svg`, `svg/path/animations/path-length-interpolation.tentative.html`,
  and `svg/animations/animate-pathLength.tentative.html`.
- Upstream: Could go upstream, but Servo's AI policy currently prevents sending it.

### AS-511: Parse the SVG pointer-events bounding-box keyword
- Files: `style/values/specified/ui.rs`
- Why: SVG 2 adds `bounding-box` to `pointer-events`, allowing a graphics or container element to receive events
  anywhere in its geometric bounding box, independent of its fill, stroke and visibility. Servo's SVG hit testing
  needs the computed value to distinguish this region from `auto`.
- Upstream: Could go upstream, but Servo's AI policy currently prevents sending it.

### AS-512: Resolve normal line-height for Servo font-relative units
- Files: `style/device/servo.rs`
- Why: Servo returned zero for the `lh` and `rlh` units when `line-height` was `normal`. Ask the
  font metrics provider for the first available font's line height, with a fallback when no font
  is available. This also supplies a nonzero basis for SVG lengths using those units.
- Upstream: Could go upstream, but Servo's AI policy currently prevents sending it.

### AS-513: Preserve SVG user-unit numbers in painting properties
- Files: `style/values/specified/svg.rs`
- Why: Keep unitless numbers, including number-only `calc()` expressions, distinct from CSS lengths
  while serializing `stroke-width`, `stroke-dashoffset`, and `stroke-dasharray`. Convert them to
  user-unit lengths at computed-value time. Affects `svg/painting/parsing/stroke-*-valid.svg`.
- Upstream: Could go upstream, but Servo's AI policy currently prevents sending it.

### AS-514: Expose numeric type exponents to script
- Files: `style/typed_om/numeric_type.rs`
- Why: CSS Typed OM math values compose numeric types in Stylo, and script needs each base type's
  exponent to return the corresponding `CSSNumericType` dictionary from `type()`.
- Upstream: Could go upstream, but Servo's AI policy currently prevents sending it.

### AS-515: Resolve script animations on pseudo-elements
- Files: `style/matching.rs`, `style/style_resolver.rs`
- Why: Resolve Web Animations keyframes against `::before` and `::after` styles, then apply their
  declarations through the existing pseudo-element animation sets. Preserve a pseudo-element's
  style while script animates it even if its `content` does not generate a box. This lets a
  `KeyframeEffect` animate a pseudo-element and follow changes to its target or pseudo selector.
- Upstream: Could go upstream, but Servo's AI policy currently prevents sending it.

### AS-516: Accumulate keyframe values across iterations
- Files: `style/servo/animation.rs`
- Why: A script animation can request `iterationComposite: accumulate`. Compose the final
  property keyframe value onto each sampled interval endpoint for every preceding iteration,
  using the property's accumulation procedure. Unsupported properties retain their normal
  per-iteration value. Servo supplies the current iteration count to this sampler.
- Upstream: Could go upstream as part of script animation support.

### AS-517: Serialize flow alignment aliases in computed styles
- Files: `style/values/specified/align.rs`, `style/values/computed/align.rs`
- Why: CSS Box Alignment now accepts `flow-start` and `flow-end` as aliases of `flex-start` and
  `flex-end`. Preserve the specified spelling in declarations, canonicalize computed styles to
  the flow spelling, and keep their layout behavior equivalent. This also fixes discrete Web
  Animations comparisons for the alignment properties.
- Upstream: Could go upstream, but Servo's AI policy currently prevents sending it.

### AS-518: Clamp opacity after animation composition
- Files: `style/values/specified/mod.rs`, `style/properties/helpers/animated_properties.mako.rs`
- Why: Retain out-of-range opacity values while resolving keyframes so interpolation and
  composition use the authored values. Clamp the final sampled opacity when applying the effect
  stack to computed style. Ordinary computed opacity remains clamped during the cascade.
- Upstream: Could go upstream, but Servo's AI policy currently prevents sending it.

### AS-519: Clamp SVG opacity after animation composition
- Files: `style/properties/helpers/animated_properties.mako.rs`, `style/rule_tree/mod.rs`,
  `style/servo/animation.rs`, `style/values/specified/mod.rs`
- Why: SVG opacity properties share opacity's keyframe value type and also retain out-of-range
  values until composition. Mark only keyframe resolution to defer clamping; final animation
  declarations cascade at the same origin and must clamp. Directly applied animated styles
  also clamp their final numeric values, preserving context-opacity keywords.
- Upstream: Could go upstream, but Servo's AI policy currently prevents sending it.

### AS-520: Preserve parsing mode through CSS variable substitution
- Files: `style/properties/mod.rs`
- Why: Declarations with `var()` are reparsed after substitution. Retain the declaration's parsing
  mode so SVG geometry attributes in quirks mode can resolve unitless custom property values as
  user units, while standards mode continues to reject those values.
- Upstream: Could go upstream, but Servo's AI policy currently prevents sending it.

### AS-521: Accept CSS and SVG image rendering keywords in Servo
- Files: `style/values/specified/image.rs`
- Why: Build `smooth`, `high-quality`, `optimizequality` and `optimizespeed` for Servo. CSS Images
  requires the first two and requires accepting the deprecated SVG keywords. This fixes parsing,
  computed style and inheritance tests in CSS Images and SVG painting.
- Upstream: Could go upstream, but Servo's AI policy currently prevents sending it.
