# STL Export Handoff (Binary STL + Three.js Validation)

## What Changed

- Added binary STL export support in the Rust core:
  - `main/opengeometry/src/export/stl.rs`
  - best-effort and strict policies
  - single-BREP and multi-BREP export APIs
  - native file output helpers (`cfg(not(target_arch = "wasm32"))`)
- Wired STL export through `scenegraph`:
  - `exportBrepToStl(...)`
  - `exportSceneToStl(...)`
  - `exportCurrentSceneToStl(...)`
  - wasm return payload includes bytes + report metadata (`OGStlExportResult`)
- Hardened incoming serialized BREP handling:
  - `addBrepEntityToScene(...)` now validates topology before storing.
- Fixed stale half-edge usage in native sandbox:
  - `main/opengeometry/sandbox-native/src/main.rs` now resolves endpoints via `get_edge_endpoints(...)`.
- Added Three.js validation examples:
  - Legacy page: `main/opengeometry-three/examples/stl-export.html`
  - Vite page: `main/opengeometry-three/examples-vite/operations/stl-export.html`
  - Vite script: `main/opengeometry-three/examples-vite/src/pages/operations-stl-export.ts`
  - Vite specs index updated to include STL export card.
  - Vite STL page now includes a shape dropdown (`Cuboid`, `Cylinder`, `Sphere`, `Wedge`) with per-shape parameter groups and shape-specific STL filenames (`opengeometry-<shape>.stl`).
  - Vite STL controls were consolidated into a single panel to prevent control container overlap.
- Exposed STL-related wasm types from package entry:
  - `main/opengeometry-three/index.ts` now re-exports `OGSceneManager` and `OGStlExportResult`.

## API Summary

- Rust core:
  - `export_brep_to_stl_bytes(&Brep, &StlExportConfig) -> Result<(Vec<u8>, StlExportReport), StlExportError>`
  - `export_breps_to_stl_bytes(...) -> Result<(Vec<u8>, StlExportReport), StlExportError>`
  - `export_brep_to_stl_file(...)` and `export_breps_to_stl_file(...)` for native file output
- Wasm scenegraph:
  - `OGSceneManager.exportBrepToStl(brep_serialized, config_json?)`
  - `OGSceneManager.exportSceneToStl(scene_id, config_json?)`
  - `OGSceneManager.exportCurrentSceneToStl(config_json?)`
  - return type: `OGStlExportResult` with:
    - `bytes: Uint8Array`
    - `reportJson: string`

## Config Defaults

- Binary STL only.
- Default policy is best-effort.
- Units are preserved as-is (`scale: 1.0` by default).
- Topology validation is enabled by default (`validate_topology: true`).

## How To Test Locally

### Rust quality gates

```bash
cargo fmt --manifest-path main/opengeometry/Cargo.toml
cargo check --manifest-path main/opengeometry/Cargo.toml
cargo test --manifest-path main/opengeometry/Cargo.toml
cargo test --examples --manifest-path main/opengeometry/Cargo.toml
cargo check --target wasm32-unknown-unknown --manifest-path main/opengeometry/Cargo.toml
```

### Native sandbox check

```bash
cargo check --offline --manifest-path main/opengeometry/sandbox-native/Cargo.toml
```

Note: online `cargo check` for `sandbox-native` may fail in restricted environments that cannot reach `index.crates.io`.

### Regenerate wasm package and build examples

```bash
npm run build-core
npm --prefix main/opengeometry-three run build-example-three
```

### Run examples

- Legacy page (static host):
  - `main/opengeometry-three/examples/stl-export.html`
- Vite built page:
  - `main/opengeometry-three/examples-dist/operations/stl-export.html`
  - Use the Shape dropdown and export button to verify shape-specific downloads (`opengeometry-cuboid.stl`, `opengeometry-cylinder.stl`, etc.).

## STL Validation Notes

- Binary STL writer uses `stl_io::write_stl` (spec-compliant binary structure).
- Unit tests cover:
  - expected binary size (`84 + 50 * triangle_count`)
  - triangle count consistency with STL header
  - custom header injection
  - best-effort skip behavior and strict failure behavior

## Backward Compatibility

- Existing primitive generation, projection, and PDF APIs remain intact.
- STL functionality is additive.
- One behavior hardening change:
  - serialized BREPs added through scene manager are now topology-validated before insertion.

## Known Caveats / Follow-ups

- Existing non-critical warnings remain:
  - `operations/windingsort.rs` (`ccw_and_flag` naming)
  - `geometry/triangle.rs` (`crso` unused variable)
- `sandbox-native` still has pre-existing `unused_must_use` warnings in some call sites (not introduced by STL work).
