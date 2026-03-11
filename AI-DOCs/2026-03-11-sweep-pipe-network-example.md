# Sweep Pipe Network Example Handoff

## What changed
- Added a new operation example page: `main/opengeometry-three/examples-vite/operations/sweep-pipe-network.html`.
- The example builds a building-like routed pipe network using `Sweep` with explicit path + profile.
- Added 4 pipe types (Supply Air, Return Air, Chilled Water, Drain), each with color, dimensions, elevation, cap settings, and multiple routes.
- Added cuboid-based walls and a control toggle for outlines.
- Added on-screen legend that maps colors to pipe types and profile sizes.
- Added FPS monitor via Three.js `Stats` module.
- Added console config dump (`console.groupCollapsed`) for the full network setup.
- Registered the example in `main/opengeometry-three/examples-vite/index.html` and updated operations count.

## Why it changed
- To provide a closer real-world routing sample that combines sweep path/profile with visualization controls and diagnostics.
- To support performance visibility (FPS) and quick configuration inspection in the browser console.

## How to test locally
1. Ensure wasm bindings are built for `main/opengeometry/pkg` (not available in this environment run).
2. Run examples dev server:
   - `npm --prefix main/opengeometry-three run dev-example-three -- --host 0.0.0.0 --port 4173`
3. Open:
   - `http://localhost:4173/operations/sweep-pipe-network.html`
4. Validate:
   - Toggle outlines in the control panel.
   - Confirm 4 pipe legend entries and color correspondence.
   - Confirm FPS panel appears at top-left.
   - Confirm network config prints in browser console.

## Backward compatibility
- Existing examples remain untouched.
- Only added one new example page and one index card entry.

## Known caveats / follow-ups
- This environment did not contain generated wasm assets under `main/opengeometry/pkg`, so vite build/dev fails import resolution here.
- After generating wasm bindings, the example should run as expected with the existing architecture.
