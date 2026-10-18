# 3MF browser importer

This bundle imports 3MF packages in a dedicated worker. The tool loads the bundle on first import or `su-mesh` use.

Dependencies are pinned: `fflate` 0.8.3 (MIT), `saxes` 6.0.0 (ISC), and build-only `esbuild` 0.25.12 (MIT).
The bundle includes their license notices. It needs no CDN or network access.

## Compatibility decision

The prototype uses published 3MF Consortium samples and a PrusaSlicer 2.9.6 project.
The samples cover uncolored geometry, face colors, vertex gradients, and transformed component instances.
The browser suite checks actual colored pixels and offline exports.

Three.js `ThreeMFLoader` was evaluated from its source (MIT).
It depends on Three.js, fflate, geometry utilities, and browser DOM APIs.
It has no dedicated worker API and does not enforce the required archive or geometry limits.
Its vertex-color conversion precedes interpolation. Materials 1.2.1 recommends barycentric interpolation in sRGB.
This importer retains sRGB values and uses the existing WebGL renderer instead.

References:

- [Core 1.4.0](https://github.com/3MFConsortium/spec_core/blob/master/3MF%20Core%20Specification.md)
- [Materials 1.2.1](https://github.com/3MFConsortium/spec_materials/blob/master/3MF%20Materials%20Extension.md)
- [Three.js loader](https://github.com/mrdoob/three.js/blob/dev/examples/jsm/loaders/3MFLoader.js)
- [Fixture provenance](../../../../e2e/tools/fixtures/3mf/README.md)

## Supported subset

The importer resolves the package StartPart relationship and parses XML by namespace URI.
It supports Core meshes, build items, nested components, repeated instances, transforms, and all six Core units.
It supports Core base-material display colors and Materials color groups, including object defaults and triangle overrides.
Both ZIP and ZIP64 packages are supported within the same size limits.
Color-group vertices interpolate in sRGB. Base materials use one color per triangle.
Standalone surface colors are opaque, including eight-digit colors.

Optional textures, advanced materials, and vendor paint data use the default blue material.
Required unknown extensions fail. Required Materials extensions with unsupported resource features also fail.
Production-extension cross-part components, beam lattices, slices, and external relationships are unsupported.
PrusaSlicer paint and filament metadata do not define portable Core display colors.
The PrusaSlicer fixture imports its geometry with the default material.

Limits: 128 MiB archive and expanded content, 4,096 entries, 3,000,000 input vertices, and 1,000,000 rendered triangles.
Component traversal has depth and instance limits. Invalid indices, missing resources, cycles, DTDs, and non-finite coordinates fail.
ZIP extraction uses bounded input chunks. Cancellation terminates the worker.
Worker processing stops after 30 seconds.

## Persisted mesh

`su-mesh` version 1 is the only persisted import representation. The original ZIP is discarded.
The fence contains JSON with these fields:

| Field | Meaning |
| --- | --- |
| `version` | `1` |
| `positions` | Base64 little-endian Float32 XYZ, three separate vertices per triangle |
| `colors` | Base64 little-endian Float32 sRGB, one RGB triplet per vertex |
| `count` | Triangle count |
| `min`, `max` | Bounds in viewer coordinates, recomputed on source load |
| `unit` | Original Core unit |
| `coordinateUnit` | `millimeter` |
| `instances` | Object ID, name, component path, triangle range, and original local-to-parent transform chain |
| `warnings` | Import appearance limitations |

Positions include component and build transforms, unit scaling, and Z-up to Y-up conversion `(x, z, -y)`.
Triangle IDs are their zero-based offsets in the position buffer. Separate vertices preserve color boundaries.
Import processing uses typed vertex/index buffers and transfers the expanded position/color buffers to the browser.
The renderer normalizes a copy for display. It does not change persisted geometry.

Source and Copy source expose the fence body. Markdown downloads retain the complete fence.
HTML exports embed mesh data, the renderer, and a static color image for print.
They do not embed the importer. PDF uses that static image.
The fence is specific to Secutils.dev and is not GitHub-compatible.
Optional `view={...}` metadata on the opening `su-mesh` fence stores the viewer settings independently of the geometry JSON.
The same metadata works with `stl` fences. The full schema is in [`md.skill.md`](../../md.skill.md#saved-model-views).

## Local performance check

Chromium on macOS, 2026-10-18, at 1280×800:

| Input | Import to canvas | Rotate button | HTML preview | Viewer DOM nodes |
| --- | --- | --- | --- | --- |
| Generated colored 3MF, 140,000 distributed triangles | 1.43 s | 104 ms | 1.66 s | 55 |
| Local Eiffel Tower STL, approximately 140,000 triangles | 574 ms | 60 ms | Not measured | 55 |

The 3MF import allowed 111 timer callbacks at a 10 ms interval.
The observed main-thread long tasks were 97 ms and 157 ms, during final document/render integration.
Archive extraction, XML parsing, transforms, and color expansion ran in the worker.
These measurements are local observations, not cross-device budgets.
