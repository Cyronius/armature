# Vendored resvg

`third_party/resvg/` is a copy of the `resvg` 0.48.1 crate from crates.io, used to rasterize SVG images. The copy
retains its Apache-2.0 and MIT license files. The root `Cargo.toml` patches the crate to this local copy.

## Armature changes

### AR-001: Sample axis-aligned pattern tiles without filtering

- Files: `src/path.rs`
- Why: Bicubic sampling of a repeated pattern tile blends colors from the opposite edge of the tile into its
  edges. When the pattern maps to the canvas without rotation or skew, sample the tile without filtering.
