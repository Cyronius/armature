# Armature: native SVG design

Status: step 0 is complete (25 November 2026). Step 1 paints basic SVG behind
`layout_servo_svg_enabled`. Full reftest parity remains open.

- **Step 0** ports #46688. With `--pref=layout_servo_svg_enabled`, layout builds a `servo_svg` tree for each outer
  `<svg>` (only `<rect>` so far), caches subtrees by DOM version and computed style, and emits `Fragment::Svg`,
  which paints nothing. SVG attribute changes bump the DOM version only while the pref is on. Two Armature-specific
  changes: upstream's UA rule `svg > * { display: inline }` is dropped, because Armature's UA sheets never hide SVG
  children, and text-overflow treats the new fragment as an atomic inline like an image. With the pref off,
  `svg/shapes`, `svg/painting` and `svg/types` give the same results as before; with it on, `svg/struct` is added and
  the 391 tests run without a crash.

- **Step 1** builds basic shapes and paths against the laid-out viewport. The renderer supports
  solid fill and stroke, 2D transforms, nested viewports, `viewBox` and `preserveAspectRatio`,
  visibility, display and group opacity. A worker rasterizes each outer SVG with tiny-skia. Display
  list construction waits for the raster, reuses images by tree identity and device scale, and
  releases images once the next display list no longer references them. The preference is opt-in.

  On Windows, 13 focused shape, solid paint, transform, nested viewport and conditional-processing
  reftests passed with the preference on and off. In the initial 173-reftest comparison across
  `svg/shapes`, `svg/painting` and `svg/struct`, the native path passed 101 and the default path
  passed 122. The losses included markers, masks, non-scaling strokes and `<use>`.

- **Shape coverage** reached 39 of 39 tests with the preference on and off, including all 29
  reftests and all 38 assertions in the 10 scripted tests. Native stroke dashes now resolve in
  SVG user units and calibrate their intervals and offset against the computed `pathLength`.
  Circle and ellipse dashes start at the SVG-defined rightmost point.

This note plans the replacement of Armature's inline-SVG pipeline with SVG that Servo lays out and paints itself.
It is based on reading how Armature works today, upstream Servo's work in progress, Gecko and Ladybird. The
[SVG notes](svg.md) record what the current pipeline achieves (88.3% of the SVG tests on 25 November 2026, against
88.6% for Firefox).

## Why

Conformance is not the problem; the architecture is. Today an outer `<svg>` in HTML is a replaced box whose content
is an image:

1. Layout finds the `<svg>` during box construction and queues it.
2. After the reflow, script deep-clones the subtree, writes in a handful of resolved values, serializes it to XML and
   caches it as a base64 `data:image/svg+xml` URL (`SVGSVGElement::serialize_and_cache_subtree`).
3. The next reflow loads that URL through the image cache like any image: it is fetched, parsed by usvg and drawn by
   resvg on a thread pool at the box size times the device pixel ratio.
4. The bitmap becomes a WebRender image.

The consequences:

- **Everything is redone on any change.** One attribute change, or one frame of a SMIL animation, repeats the clone,
  serialization, fetch, parse and full raster. First paint takes about four layout passes.
- **Most of the cascade never reaches the renderer.** Only `fill`, `stroke`, `stroke-width`, `vector-effect` and
  `transform` are copied from computed style. Document CSS for `opacity`, dashes, fonts, `clip-path`, `filter`,
  `mask`, `visibility` and the rest is ignored unless it is written inline or in a `<style>` inside the `<svg>`.
- **Some changes are never shown.** Stylesheet changes, `:hover` and CSS animations or transitions on SVG elements
  do not invalidate the raster.
- **Geometry exists twice.** Script recomputes geometry, text, hit testing and transforms in about 5,600 lines
  (`svggeometry.rs`, `svgtext.rs` and friends). Script measures text without shaping while resvg shapes it, and
  strokes, markers and clipping are implemented separately on each side, so the two can disagree.
- **Bitmaps, not vectors.** Transforms and pinch zoom scale the bitmap, which blurs, and rasters are clamped to 5,000
  px a side.
- **`<foreignObject>` does not render**, because layout builds no boxes inside the replaced `<svg>` and usvg drops
  the element.

## What the references do

| | Upstream Servo (in progress) | Gecko | Ladybird |
|---|---|---|---|
| Outer `<svg>` | Replaced box | Replaced-like frame | Replaced box with children |
| SVG content | `servo_svg::SvgTree` built by layout from computed style, cached per subtree | One frame per element; "reflow" computes bounds only | One box per element in an SVG formatting context |
| Painting | Not decided (`render_svg_tree` only logs) | Display items per element; mostly recorded into WebRender blob images and rasterized by Skia; native WebRender only for rects, text, images, simple clips, many filters | Skia display list; paths in user space; transforms as spatial nodes |
| Text | Not started | Normal inline layout, then glyphs repositioned (`SVGTextFrame`, ~5,700 lines) | HTML shaping pipeline; glyph outlines painted as paths |
| `getBBox`, CTM, text APIs | Not started | Read frames after a layout flush | Read committed layout after forcing layout |
| Path APIs (`getTotalLength`, `isPointInFill`) | — | Built from content and computed style; no frame needed | Built with the same shape builder layout uses |
| `<use>` | — | UA shadow tree clone | UA shadow tree clone |
| SVG as image | — | Separate document, same pipeline | Separate document, same pipeline |

Upstream's pieces so far:

- the `servo-svg` data model (`components/svg`), merged on 2 October and already in Armature;
- pull request [#46688](https://github.com/servo/servo/pull/46688), still open. It builds an `SvgTree` in
  `components/layout/svg/`, caches subtrees keyed by DOM version and computed-style identity, adds
  `Fragment::Svg`, and sits behind the pref `layout_servo_svg_enabled`. It only handles `<rect>`.

Upstream tried to keep resvg by feeding it a programmatic `usvg::Tree`, but resvg's maintainers refused to make the
tree writable ([linebender/resvg#1137](https://github.com/linebender/resvg/pull/1137)).

## Decisions

1. **Build on upstream's seams.**
   - Keep the outer `<svg>` a replaced box.
   - Build the SVG content in layout (`components/layout/svg/`), using the `servo-svg` crate as the tree type.
   - Port #46688's builder, subtree cache and `Fragment::Svg`, with credit, and use its pref name.
   - Extend the model where it is thin, for example `TextSpan` is empty and `NodeStyle` is unfilled.
   - When upstream lands more, we merge rather than maintain a third design.
2. **The SVG tree is layout's box tree for SVG.** Each `SvgNode` records:
   - its `OpaqueNode`;
   - its computed geometry: the path in user space (`kurbo::BezPath`), object and stroke bounding boxes;
   - its local transform, and any nested viewport transform.

   Nodes are `Arc`-shared so an unchanged subtree is reused, as in #46688 and in Ladybird's committed fragments.
3. **One shape builder for layout and script.**
   - A `geometry` module in `servo-svg` builds a shape's path from computed values (x, y, r, `d`, and so on, which
     are Stylo properties).
   - Layout uses it. Script's `SVGGeometryElement` methods (`getTotalLength`, `getPointAtLength`, `isPointInFill`,
     `isPointInStroke`) use it too, so they keep working on elements without boxes, as in Gecko and Ladybird.
   - `getBBox`, `getCTM`, `getScreenCTM`, the text-content methods and hit testing become layout queries that read
     the committed tree.
   - The duplicated script code is deleted once the queries match.
4. **Computed style is the only input.**
   - Geometry and paint properties come from Stylo; nothing is re-parsed from attributes in layout except what is
     not a property (`viewBox`, `preserveAspectRatio`, `points`, text `x`/`y` lists, `href`).
   - CSS animations, transitions, `:hover` and SMIL all arrive as style changes. SMIL already feeds Stylo through
     script animations (AS-506).
   - SVG paint-property changes need restyle damage that reaches the `<svg>`. jdm suggested on #46688 that this
     belongs in Stylo's `servo_restyle_damage`, not in a layout workaround.
5. **Painting: record, then rasterize on the CPU.**
   - WebRender cannot fill or stroke arbitrary paths. Gecko also rasterizes most SVG on the CPU.
   - The tree is painted into a backend-neutral list of SVG drawing commands, rasterized with tiny-skia: fills,
     strokes, paint servers, clips, masks and filter graphs.
   - tiny-skia is what resvg uses today, so its stroking, gradients and patterns are known to pass the current tests.
     The canvas backend vello_cpu is an option later; we found it blends incorrectly under clips.
   - First stage: one raster per outer `<svg>`, keyed by tree identity and scale, rendered off the script thread as
     it is now.
   - Later stage: a WebRender blob-image handler, as in Gecko. It gives scale-aware, tiled, asynchronous rasters with
     no blur on zoom, and only dirty tiles are re-rasterized.
   - Per-element `transform`, `opacity` and `filter` become WebRender reference frames, stacking contexts and filter
     ops where that is possible, so animating them does not re-rasterize.
6. **Text uses Servo's fonts.**
   - Shape with `components/fonts` (HarfBuzz) using the element's font, including web fonts.
   - Position glyphs by the SVG rules: x, y, dx, dy, rotate, text-anchor, textLength, textPath.
   - Paint glyph outlines as paths, as Ladybird does.
   - Commit per-character cells for the text DOM APIs.
   - This removes the per-parse fontdb copies and the unshaped script measurement.
7. **References are tracked, not copied.**
   - Gradients, patterns, clip paths, masks, markers, filters and `<use>` targets are resolved by id when the tree is
     built.
   - Each outer `<svg>` records the ids it depends on, extending today's `cached_external_reference_ids`. A change to
     a referenced element dirties its dependents, like Gecko's rendering observers.
   - Ladybird's approach of building resource boxes under every referencing element is avoided.
8. **`<use>` becomes a UA shadow tree clone** (Gecko, Ladybird), so the clone gets ordinary nodes and styles.
9. **`<foreignObject>` is an independent formatting context** laid out by normal layout inside the SVG fragment, and
   always a stacking context.
10. **SVG as an image stays on resvg at first.** `<img src=x.svg>`, CSS backgrounds and posters keep the current
    image-cache path until inline SVG is done. They then move to a separate document using the same pipeline.

## Plan

Each step lands behind `layout_servo_svg_enabled` (off by default), keeps the old path working, and is measured with
the pref on and off.

| Step | Contents | Exit check |
|---|---|---|
| 0 | Port #46688: builder, subtree cache, `Fragment::Svg`, pref, restyle damage reaching `<svg>` | Builds; no change with pref off |
| 1 | Basic shapes and paths, solid fill and stroke, `transform`, `viewBox`/`preserveAspectRatio`, nested `<svg>`, `opacity`, `visibility`, `display`; tiny-skia raster per `<svg>` from the tree | Shape and painting reftests in `svg/` match the old path |
| 2 | Paint servers (gradients, patterns), stroke details (dashes, joins, `paint-order`, `vector-effect`, `pathLength`), markers, `clip-path`, `mask` | `svg/painting`, `svg/pservers`, `css/css-masking` SVG tests match |
| 3 | Text through Servo fonts, with per-character cells | `svg/text` matches or improves |
| 4 | Layout queries for `getBBox`, CTMs, text methods and hit testing; the shared shape builder for path methods; delete the duplicated script geometry | `svg/types`, `svg/interact`, `svg/geometry` scripted tests match |
| 5 | `<use>` shadow trees, `<foreignObject>`, filters (mapped to WebRender filter ops where possible, otherwise CPU) | `svg/struct`, `svg/embedded`, filter tests match |
| 6 | Performance: blob-image rasters, per-element layers for transform, opacity and filter, reference tracking | Animated-SVG frame time measured before and after |
| 7 | Turn the pref on by default, remove the serialization path, then move SVG-as-image onto the pipeline | Full WPT run shows no SVG regression |

The bar for switching the default is no net loss on the 1,422 tests under `svg/` and on the SVG-related CSS tests,
plus no crash. Armature's usvg patches AU-001 to AU-015 show where those tests depend on details of the current
renderer.

## Risks

- **Matching the current score is a lot of work.** Much of the 88.3% comes from resvg plus 15 Armature patches to
  usvg. The plan keeps both paths so progress can be measured test by test.
- **Layout runs on the script thread.** Rasterization must stay off it: the thread pool first, then blob images.
- **Upstream will change underneath us.** The `servo-svg` model is young, and #46688's module names are still under
  review. We copy its seams (crate split, builder in layout, cache keys, fragment type, pref) and keep our extensions
  separate where we can.
- **Text is the largest single piece.** Gecko's `SVGTextFrame` is about 5,700 lines. It is scheduled after shapes and
  paint servers so the pipeline is proven first.

## Licences and credit

- Code ported from upstream Servo (#46688) is MPL 2.0, like Armature.
- Gecko is MPL 2.0.
- Ladybird is BSD 2-Clause: ported code keeps its notice (see [LICENSE_LADYBIRD](../../LICENSE_LADYBIRD)).

Comments name the source of ported logic, as `AGENTS.md` requires.
