# 2026-03-04 Boolean Operations Implementation

## What changed

- Updated the kernel boolean module to support **BRep-like outlines** for boolean outputs rather than raw triangulation edges.
- `OGBoolean` remains stateful (`last_result`) and now computes outlines by edge-adjacency analysis:
  - build undirected edge buckets from result polygons
  - suppress edges shared by coplanar faces (triangle-split seams)
  - apply a feature-angle filter so smooth tessellation edges are hidden
  - keep boundary edges and sharp-feature edges
- `OGBoolean::get_outline_geometry_serialized` now returns this BRep-like feature outline buffer.
- Added kernel test `coplanar_shared_triangle_edge_is_removed_from_outline` to verify internal triangulation diagonals are not emitted.
- Updated Three.js boolean binding API:
  - Introduced `BooleanOperationKind` and `parseBooleanOperation` for deterministic operation selection.
  - `BooleanShape` uses kernel outline output as before, but operation parsing now avoids ambiguous string casting in examples.
- Updated boolean example (`operations-boolean.ts`):
  - added **Show Outline** toggle
  - kept left/right shape selectors (Cuboid, Sphere, Cylinder, Wedge)
  - operation dropdown now resolves via `parseBooleanOperation` for consistent behavior

## Why it changed

Follow-up requirements requested:

1. BRep-like outlines instead of triangulated outlines
2. consistency in operation selection behavior
3. an explicit outline toggle in the example

These are now addressed directly in kernel + wrapper + example layers.

## Robustness strategy

Boolean CSG core still uses robust controls:

- epsilon-based plane classification (`front`/`back`/`coplanar`/`spanning`)
- snap-grid normalization for deterministic splits
- post-op weld by epsilon snapping

Outline extraction now adds topology-aware edge filtering to remove coplanar split seams and smooth-surface tessellation artifacts.

## How to test locally

1. Kernel checks:

```bash
cargo fmt --manifest-path main/opengeometry/Cargo.toml
cargo test --manifest-path main/opengeometry/Cargo.toml
```

2. TS lint:

```bash
npm run lint:check
```

3. Build wasm + three package:

```bash
npm run build-core
npm run build-three
```

4. Example run:

```bash
npm --prefix main/opengeometry-three run dev-example-three
```

Then open:

- `main/opengeometry-three/examples-vite/operations/boolean.html`

## Backward compatibility

- Existing non-boolean wrappers remain unchanged.
- Boolean API stays additive; operation constants are still `BooleanOperation.Union|Intersection|Difference`.
- New parser helper improves call-site safety without breaking existing valid operation strings.

## Known caveats and follow-ups

- In this environment, wasm package artifacts (`main/opengeometry/pkg/opengeometry`) are unavailable, so full TS build/example rendering remains blocked.
- Existing repo-level lint errors outside this change still fail `npm run lint:check`.
