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
