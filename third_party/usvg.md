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
