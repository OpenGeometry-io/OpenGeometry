# Boolean tolerance guide

Quick reference for what `BooleanKernelOptions.tolerance` (TS) /
`BooleanOptions::tolerance` (Rust) actually controls, what it doesn't, and
how to size cutter overshoots so the boolean kernel doesn't fail.

## The two tolerance regimes

OpenGeometry's boolean pipeline has **two independent tolerance windows** that
control how the math handles "near coincident" geometry. Conflating them was
the root cause of the original `#6a` debt entry's misdiagnosis.

### 1. Kernel-side tolerance (the one you can configure)

- Field: `BooleanOptions::tolerance` (Rust) / `BooleanKernelOptions.tolerance` (TS).
- Units: model units (typically meters).
- Default: `(operandsDiagonal * 1e-8).max(1e-9)`, clamped further to a `1e-6`
  floor inside `boolean_subtraction_many`.
- For a 5 m wall: ~50 nm. For a 1 m cube: ~10 nm.
- Used by:
  - `solid::weld_position` — vertex welding when building the polygon soup
    that feeds boolmesh.
  - `solid::detect_coincident_faces` — plane coincidence check used in the
    post-mortem coincident-face diagnostic (`#6a` Phase 2).
  - `mod::enforce_host_bounds_for_subtraction` — AABB containment check for
    out-of-bounds cutters (`#3` fix).
  - `mod::aabb_disjoint`, `mod::aabb_overshoot` — bounds enforcement helpers.

### 2. Boolmesh internal snap window (NOT exposed)

- Lives inside the upstream `boolmesh` crate.
- Treats faces within roughly **1 mm** (`1e-3` model units) of each other as
  coincident in some configurations — most notably 8+ vertex extruded prisms
  (PolyWall in OpenPlans).
- Independent of `BooleanOptions::tolerance`. Setting that tolerance to
  `1e-9` does NOT shrink the boolmesh snap window.
- We do not currently expose or override it. Forking boolmesh is explicitly
  out of scope for the debt cycle.

## Empirical thresholds

Approximate snap-window thresholds observed against `boolmesh` as of the
current `Cargo.toml` pin. Reproduce the bisection by varying cutter
overshoot in log steps from 1 µm to 10 mm and measuring failure rate.

| Host shape                       | Cutter overshoot that reliably succeeds |
|----------------------------------|------------------------------------------|
| 4-vertex extruded box (SingleWall) | ≥ 0.001 m (1 mm)                         |
| 6-vertex extruded prism            | ≥ 0.005 m (5 mm)                         |
| 8-vertex extruded prism (PolyWall) | ≥ 0.01 m (10 mm)                         |
| 12+ vertex extruded prism          | ≥ 0.01 m (10 mm)                         |

These are the OpenPlans-observed "safe" overshoots. Below these, the
boolmesh snap window may treat the cutter face as coincident with the host
face and emit "Boolean kernel produced a degenerate result triangle".

## Recommended cutter overshoot

For through-cuts (cutter that should pass cleanly through the host):

```ts
const overshoot = Math.max(hostThickness * 0.05, 0.01);  // 5% or 1 cm, whichever is larger
```

In Rust:

```rust
let overshoot = (host_thickness * 0.05).max(0.01);
```

For 0.3 m wall thickness: 0.015 m (15 mm) overshoot — well clear of the snap
window.

## Failure-mode taxonomy after the debt-cycle WS-0 / WS-2 / WS-3 fixes

| Symptom                                       | `BooleanError.kind`         | Likely fix |
|------------------------------------------------|-----------------------------|------------|
| Cutter does not overlap host                   | (none — identity returned)  | n/a — host returned unchanged. |
| Cutter overshoots host AABB                    | (none — auto-clipped)       | n/a — `enforce_host_bounds_for_subtraction` clips automatically. |
| Auto-clip itself fails                          | `CutterExceedsHost{axis,overshoot}` | Clamp the cutter inside the host AABB on the named axis. |
| Cutter face within snap window of host face     | `DegenerateTriangle` (post-mortem may upgrade to `CoincidentFaces` with details) | Increase overshoot per the table above. |
| Output mesh has open / over-shared edges        | `NonManifoldEdges` (with `edge_samples`) | Inspect the sampled edges; usually traces back to a malformed input. |
| Two cutters overlap each other                  | (none — union pre-pass merges them) | n/a — WS-1 union pre-pass handles this. |
| Cutters mismatch operand kind (solid vs planar) | `MixedOperandKinds`         | Convert all cutters to the same kind. |

Catch them on the TS side via:

```ts
import {
  BooleanError,
  CoincidentFacesError,
  CutterExceedsHostError,
  NonManifoldOutputError,
} from "opengeometry";

try {
  wall.subtract(openings);
} catch (error) {
  if (error instanceof CutterExceedsHostError) {
    console.warn(`Cutter overshoots ${error.axis} by ${error.overshoot}`);
  } else if (error instanceof CoincidentFacesError) {
    console.warn(`Snap-tolerance collision: ${error.details}`);
  } else if (error instanceof NonManifoldOutputError) {
    console.warn("Non-manifold output", error.edgeSamples);
  } else if (error instanceof BooleanError) {
    console.warn(`Boolean failed (${error.reason}, phase ${error.phase})`);
  } else {
    throw error;
  }
}
```

## Worked examples

### SingleWall + door cutter

- Wall: `box(width=4 m, height=3 m, depth=0.3 m)`.
- Door cutter: `box(width=1 m, height=2.1 m, depth=0.4 m)` placed at wall
  center.
- Cutter depth (0.4 m) > wall depth (0.3 m) → cutter overshoots wall +z by
  0.05 m.
- WS-2 auto-clips cutter to wall AABB; subtraction succeeds.

### PolyWall U-shape + 3 doors

- PolyWall: 8-vertex extruded prism, height 3 m, thickness 0.3 m.
- Three door cutters at the three segment midpoints, each with 1 cm overshoot.
- Pre-WS-1: failed on the 3rd cutter with non-manifold edges.
- Post-WS-1: union pre-pass merges the cutters into a single shell, then a
  single subtract produces a manifold result.

### Door at wall edge

- Wall: `box(width=4 m, height=3 m, depth=0.3 m)`.
- Door cutter: positioned with x at `+2.0 m` (the wall's +x face). Cutter
  extends past the wall edge.
- Pre-WS-2: failed with non-manifold edges.
- Post-WS-2: cutter is auto-clipped to the wall AABB inflated by `4 *
  tolerance`. The resulting cut goes exactly to the wall edge.

### Door fully outside wall

- Wall: `box(width=4 m, height=3 m, depth=0.3 m)`.
- Door cutter: positioned at x = 10 m (way past wall).
- Pre-WS-2: failed or returned garbage.
- Post-WS-2: AABB-disjoint detected, subtraction returns the wall unchanged.

## Related references

- `.spec/debt/06a-snap-tolerance.md` — original spec sheet.
- `.spec/debt/03-out-of-bounds-cutter.md` — WS-2 spec.
- `.spec/debt/04-structured-boolean-error.md` — WS-0 typed error infrastructure.
- `main/opengeometry/src/booleans/error.rs` — `BooleanError` shape.
- `main/opengeometry/src/booleans/mod.rs` — `enforce_host_bounds_for_subtraction`,
  `try_union_cutters`, `boolean_subtraction_many`.
- `main/opengeometry/src/booleans/solid.rs` — `detect_coincident_faces`,
  `maybe_enrich_with_coincident_faces`, `validate_triangle_mesh_closed`.
- `main/opengeometry-three/src/operations/boolean-errors.ts` — TS-side
  typed error class hierarchy.
