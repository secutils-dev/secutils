# 3MF fixtures

The four Consortium fixtures come from [3MFConsortium/3mf-samples](https://github.com/3MFConsortium/3mf-samples).
Their BSD-2-Clause license is in `LICENSE`.

| Local file | Upstream path | Coverage |
| --- | --- | --- |
| `box.3mf` | `examples/core/box.3mf` | Uncolored mesh |
| `multiple-cylinders.3mf` | `examples/core/multiple_cylinders.3mf` | Components, build transforms, repeated instances |
| `vertex-colors.3mf` | `examples/material/pyramid_vertexcolor.3mf` | Per-vertex color-group gradients |
| `face-colors.3mf` | `examples/material/rhombicuboctahedron_color.3mf` | Triangle color assignments |
| `prusaslicer.3mf` | `prusa3d/PrusaSlicer:src/slic3r-shared/test/data/fdm_roundtrip1.3mf` | PrusaSlicer 2.9.6 project, geometry and vendor metadata |

The PrusaSlicer fixture uses AGPL-3.0, the same license as this repository. The full text is in the root `LICENSE`.
All files were retrieved on 2026-10-18.
The importer tests generate small packages for Core base materials, object defaults, nested transforms, units, and invalid inputs.

`zip64-colors.3mf` is an original two-triangle fixture with standard blue/red color-group properties.
Its ZIP64 directory and per-entry extra fields reproduce the packaging used by the supplied `one_3mf.3mf` file.
The original user file is not stored in this repository.
The fixture uses this repository's license.
To regenerate it, run `node src/generate-zip64-fixture.mjs` from `dev/tools/js/three-mf/`.
