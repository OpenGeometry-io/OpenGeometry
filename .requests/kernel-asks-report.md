# Kernel Ask Report for Production B-Rep Editor

Date: 2026-03-21  
Kernel audited: `../OpenGeometry-Kernel/main/opengeometry` and `../OpenGeometry-Kernel/main/opengeometry-three`

---

## 1) Recheck Summary

The kernel has a solid editable B-Rep baseline and currently passes editor tests, including:

- face push/pull
- face/edge/vertex move
- topology render data
- edit result payload control flags

Confirmed by local test run:

- `cargo test editor::tests -- --nocapture`  
- Result: `7 passed, 0 failed`

---

## 2) What Is Already Good Enough

## 2.1 Editable operations exist and are wired to JS/TS wrappers

- Rust exports:
  - `pushPullFace`, `moveFace`, `moveEdge`, `moveVertex`
  - [mod.rs](/Users/vishwajeetmane/Work/OpenGeometry/OpenGeometry-Kernel/main/opengeometry/src/editor/mod.rs#L202)
- TS wrapper mirrors these methods:
  - [editor.ts](/Users/vishwajeetmane/Work/OpenGeometry/OpenGeometry-Kernel/main/opengeometry-three/src/operations/editor.ts#L198)

## 2.2 Polygon can enter editable B-Rep pipeline

- `Polygon` exposes B-Rep payload:
  - `getBrepData()`
  - [polygon.ts](/Users/vishwajeetmane/Work/OpenGeometry/OpenGeometry-Kernel/main/opengeometry-three/src/shapes/polygon.ts#L469)
- `createEditableBrepEntity()` accepts `getBrepData/getBrepSerialized`:
  - [editor.ts](/Users/vishwajeetmane/Work/OpenGeometry/OpenGeometry-Kernel/main/opengeometry-three/src/operations/editor.ts#L251)

## 2.3 Topology remap payload exists

- `topology_changed` and `topology_remap` are returned in edit results:
  - [mod.rs](/Users/vishwajeetmane/Work/OpenGeometry/OpenGeometry-Kernel/main/opengeometry/src/editor/mod.rs#L359)

---

## 3) Production Gaps and Kernel Asks

## P0 (Blockers for robust CAD-grade behavior)

### P0.1 Face extrude operation (true topology-creating push/pull)

Current `push_pull_face_internal` moves existing face vertices along normal, then recomputes normals:

- [edits.rs](/Users/vishwajeetmane/Work/OpenGeometry/OpenGeometry-Kernel/main/opengeometry/src/editor/edits.rs#L27)

This is not sufficient for planar polygon "push/pull to solid" workflows expected in CAD.

Ask:

- Add `extrudeFace(face_id, distance, options)` on editable entity.
- It must create proper side faces + cap/bottom topology where applicable.
- Return standard `BrepEditResult` with topology changes.

Acceptance:

- extruding a single-face polygon creates a watertight prism-like shell (unless explicitly open-surface mode).
- face/edge/vertex IDs remap correctly after extrusion.

### P0.2 Real semantic topology remap from actual edits

Current remap builder used in edit result is domain default mapping:

- [remap.rs](/Users/vishwajeetmane/Work/OpenGeometry/OpenGeometry-Kernel/main/opengeometry/src/editor/remap.rs#L106)

This mostly maps `old_id -> same_id if still present`, or deleted.  
`split/merged` support is currently synthetic via helper tests, not emitted by real topology-changing ops.

Ask:

- Replace default ID-overlap remap with edit-aware remap generation for each topology-changing operation.
- Ensure `status` reflects real behavior (`split`, `merged`, `deleted`, `created`, `unchanged`).
- Ensure `primary_id` is deterministic and useful for selection continuity.

Acceptance:

- topology-changing ops produce non-identity remap entries in real edit tests.
- remap is deterministic across repeated identical operation sequences.

### P0.3 Created-ID reporting contract

`TopologyRemapStatus` includes `Created`, but current remap structure is `old_id -> new_ids`; it has no first-class entries for purely new IDs.

Ask:

- Add explicit created-feature reporting, either:
  - a `created_ids` section per domain, or
  - a remap schema that can encode created entries without `old_id`.
- Keep backward compatibility or version payload (`topology_remap_v2`) if needed.

Acceptance:

- editor can select newly created face/edge/vertex from result payload without heuristic scan.

## P1 (Strongly recommended for v1 UX quality)

### P1.1 Topology-edit operations for 2D/polygon workflows

Needed for direct polygon/ polyline-like editing ergonomics:

- `insertVertexOnEdge(edge_id, t_or_position, options)`
- `removeVertex(vertex_id, options)` with validity guards
- optional: `splitEdge(edge_id, t, options)`

Acceptance:

- polygon point insertion/removal works through kernel ops.
- remap and validity are returned consistently.

### P1.2 Constraint-aware edit operations

Current move ops are free-vector transforms and can unintentionally break intended constraints (coplanar editing, axis constraints).

Ask:

- Add operation variants/flags:
  - plane constrained
  - axis constrained
  - preserve coplanarity for selected contexts

Acceptance:

- same operation with constraints yields deterministic constrained geometry without UI-side post-fix hacks.

### P1.3 Capability discovery API

Editor needs to know what controls to show/hide per entity/feature.

Ask:

- Add `getEditCapabilities()` at entity and/or feature level:
  - e.g. `canPushPullFace`, `canExtrudeFace`, `canMoveEdge`, `canInsertVertex`, etc.

Acceptance:

- UI does not expose invalid operations and avoids trial-and-error calls.

## P2 (Scale/performance and robustness)

### P2.1 Delta payload support

Current result can include full serialized geometry; useful but heavy under repeated drags.

Ask:

- Optional changed-domain delta payloads (`changed_faces`, `changed_edges`, etc.) to reduce bandwidth/work.

### P2.2 Better validity diagnostics taxonomy

Current validity returns warnings/errors strings. Helpful, but categorization would improve UX.

Ask:

- Add machine-readable codes/severity per diagnostic entry.

---

## 4) Explicit Notes About Polygon Editing

- Polygon edge/vertex dragging is feasible today via editable B-Rep `moveEdge/moveVertex` after creating editable entity from polygon B-Rep.
- Polygon "push/pull" today is geometric face translation, not solid extrusion.
- For AutoCAD/Blender-like expected push/pull on polygon faces, `extrudeFace` is the key missing operation.

---

## 5) Minimal Kernel API Ask Set (If You Want Strict MVP)

If we only request the minimum to unblock production editor quality:

1. `extrudeFace(...)`
2. true semantic remap for topology-changing ops
3. created-ID reporting in edit result
4. `insertVertexOnEdge(...)` and `removeVertex(...)`

Everything else can follow in later iterations.
