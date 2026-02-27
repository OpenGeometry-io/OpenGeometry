### Creation of Elements and Primitives

#### Cyclinder
There are two ways to create a Cyclinder
1. Create a Circle Primitive, then create Circle Poly Face and Then Extrude The Polygon by given height
2. Create a Cylinder Primitive and provide height and radius

### Camera Projection PDF Examples

Run from `main/opengeometry`:

```bash
cargo run --example pdf_camera_projection
```

Perspective example with custom camera input:

```bash
cargo run --example pdf_camera_projection -- ./camera_custom.pdf 5.0 3.0 6.0 0.0 0.0 0.0 0.0 1.0 0.0 0.1
```

Orthographic HLR comparison (writes `_hlr_on.pdf` and `_hlr_off.pdf`):

```bash
cargo run --example pdf_camera_projection_views
```

Custom output prefix for orthographic comparison:

```bash
cargo run --example pdf_camera_projection_views -- ./views_compare
```

Generate PDFs for all currently exported primitives (`line`, `polyline`, `arc`, `rectangle`, `polygon`, `cuboid`, `cylinder`):

```bash
cargo run --example pdf_primitives_all
```

Custom output prefix for per-primitive PDFs:

```bash
cargo run --example pdf_primitives_all -- ./all_primitives
```

Scenegraph-based projection flow (single source for `projectTo2DCamera` and PDF):

```bash
cargo run --example scenegraph_projection
```

Custom output file for scenegraph projection:

```bash
cargo run --example scenegraph_projection -- ./scenegraph_projection_custom.pdf
```

Inspect serialized `projectTo2DCamera` data (`Scene2D`) and normalized line list (`Scene2DLines`):

```bash
cargo run --example scenegraph_projection_dump_json -- ./projection_dump
```
