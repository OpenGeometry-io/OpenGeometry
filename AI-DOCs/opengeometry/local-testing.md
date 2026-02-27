# OpenGeometry Local Testing Guide

This file documents how to locally validate the camera projection + scenegraph pipeline implemented in this branch.

## 1) Run from the Rust crate root

```bash
cd main/opengeometry
```

## 2) Build and test

```bash
cargo fmt --check
cargo check --examples
cargo test -q
```

Expected: build succeeds and unit tests pass.

## 3) Generate projection PDFs

Create an output folder:

```bash
mkdir -p ./out
```

### Perspective camera projection

```bash
cargo run --example pdf_camera_projection -- ./out/pdf_camera_projection.pdf
```

### Orthographic HLR on/off comparison

```bash
cargo run --example pdf_camera_projection_views -- ./out/pdf_camera_projection_views
```

Expected output files:
- `./out/pdf_camera_projection_views_hlr_on.pdf`
- `./out/pdf_camera_projection_views_hlr_off.pdf`

### All supported primitives

```bash
cargo run --example pdf_primitives_all -- ./out/pdf_primitives
```

Expected output files:
- `./out/pdf_primitives_line.pdf`
- `./out/pdf_primitives_polyline.pdf`
- `./out/pdf_primitives_arc.pdf`
- `./out/pdf_primitives_rectangle.pdf`
- `./out/pdf_primitives_polygon.pdf`
- `./out/pdf_primitives_cuboid.pdf`
- `./out/pdf_primitives_cylinder.pdf`

### Scenegraph projection (shared projectTo2DCamera + PDF path)

```bash
cargo run --example scenegraph_projection -- ./out/scenegraph_projection.pdf
```

Expected output file:
- `./out/scenegraph_projection.pdf`

### Inspect projectTo2DCamera JSON payloads

```bash
cargo run --example scenegraph_projection_dump_json -- ./out/projection_dump
```

Expected output files:
- `./out/projection_dump_scene2d.json` (raw `Scene2D` shape from `projectTo2DCamera`)
- `./out/projection_dump_lines2d.json` (normalized `Scene2DLines` payload from `projectTo2DLines`)

Optional pretty inspection with `jq`:

```bash
jq . ./out/projection_dump_scene2d.json
jq . ./out/projection_dump_lines2d.json
```

## 4) Verify files were created

```bash
ls -1 ./out/*.pdf
```

## 5) Frontend / WASM note

- `projectTo2DCamera` is designed for frontend usage (returns serialized 2D scene data).
- `projectToPDF` is native-only in this crate build (`not(target_arch = "wasm32")`).
- In browser builds, use `projectTo2DCamera` and render lines in Three.js.
