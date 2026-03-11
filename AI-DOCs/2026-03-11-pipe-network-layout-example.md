# Pipe Network Layout Example Handoff

## What changed

- Added a new operations example: `operations/pipe-network-layout.html`.
- Example builds a multi-branch pipe layout using sweep path + profile primitives (4 pipe types).
- Added an on-page legend describing each pipe type and profile dimensions.
- Added scene walls built with `Cuboid` to mimic room boundaries.
- Added UI toggles for outlines, fat outlines, outline width, wall visibility, and FPS stats visibility.
- Added Three.js FPS monitoring using `stats.module.js`.
- Added config logging in the console on each state update, including pipe metadata.
- Registered the new example in `examples-vite/index.html` and updated the operations item count.

## Why it changed

To provide a richer, architecture-consistent example similar to HVAC / MEP layouts:
- sweep + profile usage,
- multiple pipe classes with visual legend,
- wall context geometry,
- quick outline controls,
- and runtime perf visibility.

## How to test locally

1. Ensure the OpenGeometry wasm package is available at `main/opengeometry/pkg/opengeometry_bg.wasm`.
2. Start examples dev server:
   - `npm --prefix main/opengeometry-three run dev-example-three`
3. Open:
   - `http://localhost:5173/operations/pipe-network-layout.html`
4. Validate:
   - outlines toggle applies to walls + sweeps,
   - 4 pipe types render with distinct colors,
   - legend entries match rendered pipe colors/profiles,
   - console prints `[OpenGeometry] Pipe network config ...`,
   - FPS panel appears and can be toggled.

## Backward-compatibility notes

- No existing APIs changed.
- Existing examples were not behaviorally modified.
- Only example catalog index was updated to add one new operation card.

## Known caveats and follow-ups

- Example build depends on generated wasm artifacts in `main/opengeometry/pkg/`; without them, Vite build fails on wasm URL resolution.
- If desired later, extract shared example helpers to reduce repeated boilerplate across example HTML pages.
