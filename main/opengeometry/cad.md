### Creation of Elements and Primitives

#### Cyclinder
There are two ways to create a Cyclinder
1. Create a Circle Primitive, then create Circle Poly Face and Then Extrude The Polygon by given height
2. Create a Cylinder Primitive and provide height and radius

### Kernel Validation

Run from `main/opengeometry`:

```bash
cargo fmt --check
cargo check --all-targets
cargo test
```

Targeted export and projection checks:

```bash
cargo test test_scene_projection_from_edge_entity
cargo test test_scene_projection_lines_json_payload
cargo test test_scene_stl_export_binary_payload
cargo test test_scene_step_export_text_payload
cargo test test_scene_ifc_export_text_payload
```

The crate no longer ships standalone Rust examples, scripts, or the native sandbox. Release validation now happens through compiled library tests and downstream consumers that call the public APIs directly.
