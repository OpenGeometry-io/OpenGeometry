---
name: brep-invariants
description: Use when editing files under `main/opengeometry/src/brep/`, when modifying primitives that build BRep (`primitives/`, `operations/extrude.rs`, `operations/sweep.rs`, `booleans/`), or when applying transforms to existing BRep (`spatial/placement.rs`). Fires on questions about halfedge twin pairing, loop chains, winding order, or "do I need to update X when I update Y in the BRep?".
---

# B-Rep Invariants

B-Rep (Boundary Representation) is the canonical geometry representation in this kernel.
Every primitive, operation, and boolean produces or modifies a `Brep` struct. The
triangulated mesh shown in Three.js is *derived* from BRep; BRep is not derived from the
mesh.

If you violate the invariants below, downstream code (projection, boolean, export, scene
serialization) will produce silent garbage or panic in unrelated places.

## Topology

```
Brep
├── vertices: Vec<Vertex>          (3D positions, unique IDs)
├── edges:    Vec<Edge>            (one per pair of vertices, owns 2 halfedges)
├── faces:    Vec<Face>            (one outer Loop + 0..N inner Loops, normal, winding)
├── wires:    Vec<Wire>            (open polylines not bounded by faces)
├── shells:   Vec<Shell>           (face collections — solid or open)
└── id

HalfEdge (lives inside Edge)
├── start vertex
├── twin    → opposite-direction halfedge (same Edge)
├── next    → next halfedge in the Loop (cyclic)
└── face    → the Face whose Loop contains this halfedge
```

## Invariants you must preserve

1. **Twin pairing is total.** Every HalfEdge has exactly one twin, and `twin(twin(h)) == h`.
   When you add an Edge, you add two HalfEdges; never one.
2. **Loop chains are cyclic.** Following `next` around a Loop returns to the starting
   HalfEdge. No dangling `next` pointers, no premature termination.
3. **Outer loops are CCW; inner (hole) loops are CW.** Winding determines which side is
   "inside." Use `operations::windingsort` helpers — don't eyeball it.
4. **Face normal matches loop winding.** If you flip a loop's winding, recompute the
   normal. Mismatched normal/winding breaks projection (silhouette extraction) and STL
   export (wrong-facing triangles).
5. **A Vertex referenced by an Edge must exist in `Brep::vertices`.** Same for HalfEdge →
   Edge, Face → Loop, Loop → HalfEdge. The `BrepBuilder` enforces this; manual
   construction must not bypass the builder.
6. **Shells reference Faces; they do not own them.** Mutating a Face changes the geometry
   for every Shell that includes it.

## Placement transforms — the open question

`Placement3D` (`spatial/placement.rs`) currently applies translation + rotation + uniform
scale by **updating vertex positions** in the BRep and the geometry buffer.

This is sufficient when:
- Topology is unchanged (same vertices, same edges, same faces)
- The transform is rigid or uniform-scale (preserves co-planarity, preserves loop winding)

It may be insufficient when:
- The transform is non-uniform scale (could break face planarity if vertices shift
  off-plane — though this kernel currently rejects non-uniform scale at the API level)
- Edge classifications have been pre-cached (e.g., `EdgeClass` from technical-drawing
  projection) — those caches do not auto-invalidate
- Halfedge `start` / `next` chains are recomputed elsewhere from positions rather than
  topology — they must not be

**Action when adding a new transform code path:** check whether the transform changes
topology. If it does (e.g., a future "mirror with face-flip" operation), update
halfedges, loops, normals, and any cached projections in addition to vertex positions.
If it does not, vertex updates are sufficient. Document which case applies in a
comment at the call site.

## When you add a new primitive

Use `BrepBuilder` (in `brep/builder.rs`). The builder validates twin pairing and loop
closure on `build()`. Bypassing it produces "valid until you try to use it" BReps.

Pattern:
1. Add vertices.
2. Add edges referencing vertex IDs (creates twin halfedges).
3. Build loops by chaining halfedges; mark outer vs. inner.
4. Build faces from loops; compute normal from outer loop winding.
5. (If solid) build shells from faces.
6. Call `build()` — it returns `Result<Brep, BrepError>`. Don't `unwrap()` in
   wasm-exposed paths.

## Tests to run after touching BRep

```bash
cargo test --manifest-path main/opengeometry/Cargo.toml -q
# Watch specifically for:
#   rectangle_generates_face_loop_without_duplicate_halfedges
#   sphere_geometry_and_outline_are_non_empty
#   sphere_segment_inputs_are_clamped
```

If you added a new primitive, add a smoke test to `tests/primitives_smoke.rs` that
asserts:
- BRep is non-empty (`vertices.len() > 0`, `faces.len() > 0` for solid primitives)
- The geometry buffer triangulates without panic
- Outline extraction (if applicable) returns at least one segment

## Booleans

Boolean operations go through the `boolmesh` crate via `booleans/`. The output is a
freshly-built BRep, not a mutation of an input. Do not assume vertex/edge IDs persist
across a boolean.
