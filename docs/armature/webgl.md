# Armature: WebGL

Detail behind the WebGL row of the [README](../../README.md) conformance table: what Armature changed and what is still missing. Other areas are listed in [the index](README.md).

## What has been done

- **WebGL2 texture sources and dEQP coverage**: WebGL2 image and subimage uploads now accept canvas, image, SVG,
  video and `ImageData` sources in 2D and 3D. Typed-array 3D subimages and pixel unpack buffer 2D subimages are
  supported. Twelve WebGL2 texture source directories and eight dEQP groups (456 tests) are enabled, bringing the
  full WebGL conformance score to 82.2%.
- **WebGL canvas pixels and texture uploads**: WebGL canvas snapshots now put the top row first for `toDataURL`,
  canvas consumers and texture uploads. `OffscreenCanvas` is accepted as a `TexImageSource`. Seventeen WebGL canvas
  conformance tests that previously failed or ended in error now complete successfully, including 200 subtests
  previously expected to fail.

## What has not been done

- WebGL2 texture overload coverage remains incomplete (`tex-unpack-params.html` still errors), and 368 dEQP files
  remain disabled.
