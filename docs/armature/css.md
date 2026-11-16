# Armature: CSS

Detail behind the CSS row of the [README](../../README.md) conformance table: what Armature changed and what is still missing. Other areas are listed in [the index](README.md).

## What has been done

- **Vendored Stylo**: Stylo, Servo's CSS engine, is vendored in `third_party/stylo/` as an unmodified import plus a
  log of every Armature change (`third_party/stylo.md`), so changes can be ported to and from upstream Stylo.
  `./mach test-tidy` checks that the log matches the actual diff.
- **CSS features behind Stylo preferences**: Servo now exposes 65 of Stylo's own `layout.css.*` preferences (52 are on
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
- **Balanced flex lines**: Experimental `flex-wrap: balance` minimizes squared unused main-axis space while preserving
  the normal wrapped line count or the `flex-line-count` minimum. Intrinsic sizing accounts for balanced lines and
  flex items measure against the cross-axis space available to each line. The focused `css-flexbox` run gains 41.0
  passing test-equivalents, with 35 new reftest passes and no status regressions.
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
- **CSS counters and counter styles**: Layout resolves `counter-reset`, `counter-increment`, `counter-set`, implicit
  `list-item` increments and generated counter content in tree order before parallel box construction. It renders
  common built-in styles, `symbols()` and named `@counter-style` systems, including additive systems, named fallbacks,
  marker prefixes and suffixes, negative, range, and pad descriptors, styles scoped to shadow roots, and declarations
  from `::part` rules. Negative `calc()` additive weights compute as zero. Predefined Greek,
  kana, CJK, Armenian, Georgian, and Hebrew representations are supported, along with the required limited-range
  Japanese, Korean, and Chinese longhand forms. The 866-test counter, list and generated-content rerun gained 195.6
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
- **Orthogonal intrinsic contributions**: when a flow block crosses its parent's axes, its intrinsic width comes
  from its block content size after resolving its orthogonal inline size. Twelve shrink-to-fit subtests, nested
  orthogonal flows, and dynamic percentage padding now pass; the comparable full run gains 16.5 passing
  test-equivalents in writing modes.
- **CSS grid lanes**: Experimental WPT runs enable `display: grid-lanes`. Layout uses grid track sizing in the lane
  axis, then places each in-flow item after the previous item in its shortest eligible lane. Both row and column
  lanes, gaps, spans, explicit lane placement, and natural item sizing in the stacking axis work. Direction values
  select the lane axis, reverse track preference, and reverse fill. The 1,005-test
  grid-lanes suite rose from 133.93 to 380.17 passing test-equivalents across these changes; the
  surrounding grid and display suites had no status changes outside grid lanes. Trailing single-lane items in
  auto-sized column lanes now honor end, center, and stretch alignment. The latest focused run reached 412.54
  passing test-equivalents. Subgrid, intrinsic auto-repeat, dense packing, and other older grid-lanes properties
  still need work.
- **Vertical CSS Grid layout**: Grid tracks and item positions follow the container's inline and block axes in
  vertical writing modes. Item dimensions, margins, padding, borders, and self alignment map back to physical
  sides. The full CSS run gains 79.0 passing test-equivalents, while one vertical Grid Lanes alignment reftest still
  needs an intrinsic-size correction.
- **Vertical inline baselines and default block margins**: replaced inline boxes without a content baseline use
  the center of their margin box in vertical text. Default paragraph and similar block margins follow the block
  axis. The focused writing-modes run gains 67 passing reftests, primarily vertical-lr absolute-position cases.
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
- **Grid self alignment**: `self-start` and `self-end` align grid items to the physical sides of their own writing
  mode, including vertical and right-to-left items. The CSS Grid alignment directory gains 1.74 passing
  test-equivalents across five tests in a 454-test rerun, with no regressions against the earlier CSS-only run.
- **Direct-line `line-clamp`**: layout suppresses lines after the requested count, sizes the block to the retained
  lines and appends a default ellipsis when it fits after simple text. Stylo now parses `-webkit-line-clamp` in Servo
  so it participates correctly in the cascade (AS-524). A 934-test CSS Overflow run gains 22.4 passing
  test-equivalents, including 21 reftests. Clamped inline blocks now export the last visible line's baseline, making
  one more reftest pass in a 324-test `line-clamp` rerun. `line-clamp: auto` now derives the number of visible direct
  lines from the used block-size limit, passing 15 more reftests in a focused rerun. Clamp counting now ignores phantom
  lines, and direct text lines can show custom block ellipsis strings or an ellipsis after a line break. Nine more
  reftests pass in the next focused rerun. Full direct-text lines now displace words at soft wrap opportunities and
  trim removable whitespace before inserting the ellipsis. When wrapping is disabled, the ellipsis can replace the
  entire line. Six more reftests pass in the next focused rerun. A leading out-of-flow block stays in the direct
  inline formatting context, so four more abspos and fixed-position reftests pass without changing the other
  320 line-clamp results. A numbered clamp with `auto` also stops at the block-size limit when that comes first,
  passing one more reftest in a 324-test focused rerun.
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
- **CSS Gap Decorations**: `column-rule-*` accept value lists with `repeat()`, and the `row-rule-*`, `rule-*`,
  `rule-break`, `rule-visibility-items`, `rule-inset` and `rule-overlap` properties and shorthands are parsed and
  interpolated. Rules are painted in the gaps of grid, flex and Grid Lanes containers, with breaks around spanning
  items, visibility, insets at caps and junctions, and paint order. Grid Lanes placement honours `flow-tolerance`.
- **Shapes and masks**: boxes with non-round `corner-shape` values paint their background, border and unblurred box
  shadows from a rasterized contour, and `clip-path` geometry boxes follow the shaped corners. `clip-path: polygon()`
  and the top image layer of `mask-image` clip through image masks on the element's stacking context.
- **Text**: control characters render as U+FFFD; lines that break after a soft hyphen show the `hyphenate-character`;
  `text-overflow` elides overflowing horizontal left-to-right lines of clipping containers with an ellipsis or string;
  `text-decoration-inset` shortens decorations at the decorating box's edges.
- **Other**: `:focus-within` matches; SVG images paint their root element's background; resvg (vendored in
  `third_party/resvg`) samples axis-aligned pattern tiles without filtering; spread box shadows adjust their corner
  radii; `aspect-ratio` transfers a definite block size to an auto inline size; absolutely positioned items in
  vertical grids are placed in their grid areas.

## What has not been done

- CSS counter support still lacks some predefined and custom numbering systems, counter-style descriptors, and
  `CSSCounterStyleRule` mutation. Four reversed-list counter scope reftests still fail.
- `margin-trim` still needs flex and grid item trimming and some block fragmentation cases; its WPT directory remains
  excluded from the default CSS run.
- Vertical in-flow and orthogonal box placement remain incomplete. Some nested shrink-to-fit cases, border, padding,
  dynamic resize, and grid percentage sizing still fail with the new sizing path. In
  `css-writing-modes/relpos-inline-overflowing-block-vrl.html`, the corrected absolute box now exposes the existing
  misplaced inline box, causing a reproducible reftest failure. Vertical coordinate conversion also remains
  provisional when the containing block's block size is indefinite; several containment and vertical text reftests
  still fail. Two text-combine reftests regress with the logical default block margins; vertical-rl block placement
  still needs a coordinated fix for fragment positions and the root box offset.
- `line-clamp: auto` now applies block-size limits to direct lines, but `line-clamp` still needs ellipsis placement
  when the last line has nested inline boxes, floats, or emergency wrap opportunities, counting lines inside
  descendant blocks, hiding out-of-flow descendants whose containing block follows the clamp point, vertical and
  right-to-left text, and the legacy `-webkit-box` display behavior.
- Container queries still have failures involving hidden ancestors, unsupported query forms, and other dynamic
  cases. Six previously passing subtests in `query-evaluation.html` and `display-none.html` now fail, and three
  existing timeout cases remain.
- Implicit `@scope` roots are not passed from stylesheet owners to layout. Scopes without a prelude can therefore
  match outside the intended root or fail to style that root; three previously passing subtests now fail.
- Not enabled from Stylo: anchor positioning, `@starting-style`, `@page` margin rules, custom media,
  `background-clip: text`/`border-area`, `random()`, tree-counting functions and a few more, because
  Servo's layout or CSSOM would panic on them; `size`, `page-orientation` and `page`, which need `@page` support.
  `resize` has no resize handles, and `subgrid` is parsed but not laid out.
- One `-webkit-box` balance reftest still fails. Two radial-gradient masking reftests also fail after their
  absolutely positioned aspect-ratio boxes gain height. Their WPT expectations record those failures.
- Web Animations: script animations on pseudo-elements beyond `::before` and `::after` are not yet applied; CSS
  functional pseudo-elements such as `::part()` and `::highlight()` are not yet supported; and
  changing `iterationComposite` on a CSS animation does not yet affect its style-generated keyframes.
  Script keyframe offsets do not yet accept percentage or named timeline-range strings from Web Animations 2.
  `commitStyles()` of an animation below others
  briefly recomputes the style of the target without them, which can start transitions. A `CSSAnimation` per repeated name in `animation-name` is missing
  (Stylo matches animations by name), `getKeyframes()` does not keep keyframes that share an offset but not an easing
  apart and shows additive keyframe values already added, and a finished `CSSTransition` cannot be replayed once
  style has dropped it. Container query units in keyframes are not supported, as in style sheets.
- Gap decorations are not painted in multi-column containers or across fragmentation, and subgrid gaps are not
  decorated. `text-overflow` does not handle scroll containers, right-to-left or vertical lines, or anonymous block
  wrappers. `mask-image` uses only the top image layer as a single tile and treats gradients as opaque, so
  `mask-repeat`, `mask-composite` and inline-box masks are not supported. `corner-shape` does not yet clip overflow or
  outlines. `::first-line`, ruby (which the `text-emphasis` references need), `text-spacing-trim`, intrinsic
  `auto-fill` tracks and Grid Lanes subgrid are not implemented.
