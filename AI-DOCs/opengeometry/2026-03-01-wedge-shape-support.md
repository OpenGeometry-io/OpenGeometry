# Wedge shape support in kernel and three integration

## What changed
- Added a new kernel primitive `OGWedge` with configurable `center`, `width`, `height`, and `depth`.
- Registered wedge in the kernel exports and scene manager APIs (`addWedgeToScene` and `addWedgeToCurrentScene`).
- Extended the PDF primitives example to generate a dedicated wedge projection PDF.
- Added a new `Wedge` shape wrapper in `opengeometry-three` and exported it in the shapes index.
- Added a browser example page at `main/opengeometry-three/examples/wedge.html` to visualize wedge geometry in Three.js.

## Why it changed
The kernel already supports common primitive and solid shapes (e.g., cuboid and cylinder). Wedge support was added to align with this existing shape model and make wedge available both in kernel usage and the three.js integration package.

## How to test locally
1. Kernel checks:
   - `cd main/opengeometry && cargo fmt -- --check`
   - `cd main/opengeometry && cargo check`
   - `cd main/opengeometry && cargo test`
2. Build wasm bindings used by three package:
   - `npm run build-core`
3. Three package type/build checks:
   - `npm run build-three`
4. Kernel example output:
   - `cd main/opengeometry && cargo run --example pdf_primitives_all -- wedge_demo`
   - Confirm `wedge_demo_wedge.pdf` is generated.
5. Browser example:
   - `python3 -m http.server 8080`
   - Open `http://localhost:8080/main/opengeometry-three/examples/wedge.html`
4. Example output:
   - `cd main/opengeometry && cargo run --example pdf_primitives_all -- wedge_demo`
   - Confirm `wedge_demo_wedge.pdf` is generated.

## Backward-compatibility notes
- No existing primitive behavior or API signatures were modified.
- Changes are additive only: a new primitive and new scene manager methods.

## Known caveats and follow-ups
- `opengeometry-three` consumes wasm symbols from `main/opengeometry/pkg/opengeometry`; ensure `npm run build-core` is run before consuming wedge from JS/TS.
- If package-level docs enumerate available shapes, add wedge there in a follow-up.
