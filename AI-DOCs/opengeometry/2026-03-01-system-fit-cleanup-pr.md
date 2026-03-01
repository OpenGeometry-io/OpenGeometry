# 2026-03-01 System Fit Cleanup PR

## What Changed
- Removed unused and/or empty source files from Rust and TypeScript packages.
- Removed legacy commented-out Rust primitive source (`cylinderOld.rs`).
- Removed empty legacy ESLint config file (`.eslintrc.json`) in favor of active flat config (`eslint.config.js`).
- Fixed Babylon package naming mismatch:
  - `main/opengeometry-babylon/package.json` now uses `@opengeometry/kernel-babylon`.

Deleted files:
- `.eslintrc.json`
- `main/opengeometry-three/src/markup/baseMarker.ts`
- `main/opengeometry-three/src/shapes/base-shape.ts`
- `main/opengeometry-three/src/snapper/angle-snapper.ts`
- `main/opengeometry-three/src/snapper/edge-snapper.ts`
- `main/opengeometry-three/src/snapper/index.ts`
- `main/opengeometry-three/src/snapper/vertex-snapper.ts`
- `main/opengeometry-three/src/utils/event.ts`
- `main/opengeometry-three/src/utils/store.ts`
- `main/opengeometry/src/primitives/cylinderOld.rs`
- `main/opengeometry/src/primitives/point.rs`
- `main/opengeometry/src/utility/random-uuid.rs`

## Why It Changed
- These files were unreferenced placeholders or legacy code and introduced maintenance noise.
- `event.ts` also caused TypeScript lint errors in current CI/local lint execution.
- Babylon package naming was inconsistent with its package purpose.

## How To Test Locally
From repository root:

```bash
npm run lint:check
cargo check --manifest-path main/opengeometry/Cargo.toml
cargo test --manifest-path main/opengeometry/Cargo.toml
```

Expected result:
- `lint:check` passes with warnings only (no errors).
- Rust check/tests pass, with existing pre-existing compiler warnings unchanged.

## Backward-Compatibility Notes
- No public runtime API was changed.
- Removed files were not imported by active code paths.
- Babylon package name fix aligns metadata and should reduce packaging confusion.

## Known Caveats / Follow-ups
- Rust warning debt remains and should be addressed in a separate warning-reduction PR.
- One existing lint warning remains in `main/opengeometry-three/src/shapes/polygon.ts` (`console.log`).
