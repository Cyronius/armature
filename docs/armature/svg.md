# Armature: SVG

Detail behind the SVG row of the [README](../../README.md) conformance table: what Armature changed and what is still missing. Other areas are listed in [the index](README.md).

## What has been done

- **SVG image sizing**: SVG images get their natural width, height and ratio from the outermost `svg` element as SVG 2
  specifies, and `img.naturalWidth`/`naturalHeight` follow the default sizing rules. `naturalWidth-naturalHeight-width-height`
  went from 182 failing subtests to 0, and about 340 more pass in the image, background, sizing and SVG tests.
- **SVG fill-box transform origins**: Inline SVG rasterization resolves percentage `transform-origin` values against
  the source element's fill box when `transform-box: fill-box` is selected. All 46
  `svg-origin-relative-length` reftests and one `transform-box` reftest now pass.
- **SVG documents and SVG scripts**: navigating to an `image/svg+xml` resource creates an SVG document (it used to
  be shown as an image inside an HTML page, with no DOM or scripts), and SVG `<script>` elements run, sharing the HTML
  script processing model (`SVGScriptElement`, `document.currentScript`). Of the 256 testharness tests written as
  `.svg` files, which all errored, 249 now complete, with about 1,300 passing subtests. SVG scripts inside inline
  `<svg>` in HTML documents run too. `svg` elements fire `load` once parsed (`<svg onload>`), and SVG `<image>`
  elements fetch their image and fire `load` or `error`.
- **SVG video posters**: `<video poster>` keeps an SVG image as a vector until layout knows its `object-fit` size,
  then rasterizes it at that size. All 30 previously failing SVG poster object-fit reftests and two related
  `object-position` reftests now pass, while the focused PNG poster regressions remain passing.
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

## What has not been done

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
