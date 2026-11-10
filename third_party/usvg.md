# Vendored usvg

`third_party/usvg/` is a copy of the `usvg` 0.48.1 crate from crates.io, used by `resvg` to parse SVG images. The copy retains its Apache-2.0 and MIT license files. The root `Cargo.toml` patches the crate to this local copy.

## Armature changes

### AU-001: Calibrate SVG stroke dashes with path length

- Files: `src/parser/svgtree/parse.rs`, `src/parser/svgtree/mod.rs`, `src/parser/converter.rs`, `src/parser/text.rs`
- Why: Resolve CSS `path-length` alongside the SVG `pathLength` attribute and scale stroke dashes, dash offsets, and text path start offsets by geometric path length. A declared zero length produces a solid stroke, moves nonzero absolute text offsets outside the path, and places percentage text offsets at its start.

### AU-002: Apply the CSS d property to path geometry

- Files: `Cargo.toml`, `src/parser/svgtree/mod.rs`, `src/parser/svgtree/parse.rs`, `src/parser/shapes.rs`
- Why: Let valid CSS `d: path(...)` and `d: none` declarations override path attributes when parsed SVG documents are rasterized. Invalid CSS path declarations leave the XML attribute in effect.

### AU-003: Resolve nested SVG images against their image viewport

- Files: `src/parser/mod.rs`, `src/parser/image.rs`, `src/parser/converter.rs`
- Why: A nested SVG without absolute root dimensions inherits the embedding `<image>` viewport. Reparse it with that viewport so percentage geometry and further nested images keep their proper scale.

### AU-004: Stop recursive pattern paint conversion

- Files: `src/parser/converter.rs`, `src/parser/paint_server.rs`
- Why: A pattern child can paint itself with a pattern that inherits that child through `href`. Track paint servers while converting them so a cyclic reference uses its fallback paint instead of overflowing the parser stack.

### AU-005: Use the image-rendering viewport for embedded SVG images

- Files: `src/parser/image.rs`
- Why: Fit an embedded SVG using its intrinsic size, then parse viewBox-based images with non-scaling strokes in the resulting image viewport. This keeps their stroke at the specified width when the image is stretched, including non-uniform scaling.

### AU-006: Render solid non-scaling strokes as strokes in viewport coordinates

- Files: `src/parser/converter.rs`
- Why: Outlining a stroke and filling the outline changes edge antialiasing. Transform solid-painted path geometry into the outer viewport and let the rasterizer stroke it there, then cancel the ancestor transform so the requested width is unchanged.

### AU-007: Resolve viewport units in embedded SVG images

- Files: `src/parser/mod.rs`, `src/parser/svgtree/parse.rs`
- Why: The SVG length parser does not recognize `vw` and `vh`. Resolve these units in nested SVG length attributes and inline styles against the embedding image viewport before parsing the image. Accept SVG 2 geometry properties from CSS declarations so inline `width` and `height` styles can size shapes.

### AU-008: Resolve calc geometry in the SVG viewport

- Files: `src/parser/units.rs`, `src/parser/converter.rs`, `src/parser/shapes.rs`
- Why: SVG 2 geometry properties accept CSS `calc()` expressions in presentation attributes and CSS declarations. Resolve their lengths and percentages in the current SVG viewport, including when an embedding viewport changes, and apply the negative-radius rules after evaluating the expression.

### AU-009: Resolve viewport units in standalone SVG geometry

- Files: `src/parser/converter.rs`, `src/parser/units.rs`
- Why: CSS viewport units on SVG geometry refer to the initial SVG viewport. Keep that viewport while converting nested SVG content and resolve `vw`, `vh`, `vmin` and `vmax` in lengths and `calc()` expressions.

### AU-010: Accept lowercase currentColor in SVG paints

- Files: `src/parser/style.rs`
- Why: CSS paint keywords are case-insensitive. Canonicalize `currentcolor` in fill and stroke values, including a fallback after a paint-server URL, while preserving case-sensitive fragment IDs.

### AU-011: Resolve paint-server transforms from CSS and templates

- Files: `src/parser/paint_server.rs`, `src/parser/svgtree/parse.rs`
- Why: Gradient and pattern transforms inherit through `href` when their own declaration is absent, empty, or invalid. CSS `transform` declarations with supported SVG transform syntax set their presentation attributes, and singular transforms make the paint server unavailable so a paint fallback can be used.

### AU-012: Parse CSS pixel translations on paint servers

- Files: `src/parser/svgtree/parse.rs`
- Why: CSS `translate()`, `translateX()` and `translateY()` accept pixel lengths. Convert those values to SVG user units before applying a gradient or pattern transform, while preserving the existing SVG transform syntax.

### AU-013: Decode percent-encoded SVG fragment references

- Files: `src/parser/svgtree/mod.rs`, `src/parser/svgtree/parse.rs`, `src/parser/style.rs`, `src/parser/filter.rs`
- Why: URL fragment references in paint, filters, clipping, masking and `href` resolve decoded element IDs. Keep exact-ID lookup separate from URL lookup, preserve malformed percent triplets, and detect pattern cycles through encoded references.
