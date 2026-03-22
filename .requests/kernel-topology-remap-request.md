# Kernel Request: Non-Identity Topology Remap Contract For B-Rep Editor Continuity

## Context

`@opengeometry/editor-controls` is now using kernel-native B-Rep editing (`OGEditableBrepEntity`) for face/edge/vertex editing.

Current gap:

- `BrepEditResult.topology_remap` is currently identity-only via `identity_topology_remap(...)`.
- Reference: `main/opengeometry/src/editor/mod.rs` around `serialize_edit_result` (`topology_remap: Some(identity_topology_remap(&self.local_brep))`).

This is sufficient for topology-preserving edits, but not for topology-changing edits.

## Why This Blocks A Proper Editor

When topology changes (split/merge/delete/create), identity remap breaks editor continuity:

- Active face/edge/vertex selection cannot be tracked reliably across drag frames.
- Push/pull and edge/vertex drag can jump, deselect, or bind to stale IDs.
- Undo/redo selection restoration becomes inconsistent.
- Overlay handles and hit targets cannot rebind deterministically.

## Request (Required API Behavior)

For each edit operation (`pushPullFace`, `moveFace`, `moveEdge`, `moveVertex`), return a real remap that represents what changed.

### Required fields in `BrepEditResult`

Keep existing fields. Add these semantics (or equivalent shape):

1. `topology_changed: boolean`
2. `topology_remap` with per-domain mapping for `faces`, `edges`, `vertices`
3. Non-1:1 mapping support:
- unchanged: old -> [same]
- split: old -> [new1, new2, ...]
- deleted: old -> []
- merge: multiple old -> [sameNew]
4. `primary_id` per old id for editor continuity when mapping is 1:N.
5. Deterministic/stable IDs for untouched topology.

If you prefer a new field name (for backward compatibility), add `topology_remap_v2` and keep old `topology_remap` for now.

## Suggested JSON Shape

```json
{
  "topology_changed": true,
  "topology_remap_v2": {
    "faces": [
      { "old_id": 12, "new_ids": [31, 32], "primary_id": 31, "status": "split" },
      { "old_id": 9, "new_ids": [], "primary_id": null, "status": "deleted" },
      { "old_id": 7, "new_ids": [7], "primary_id": 7, "status": "unchanged" }
    ],
    "edges": [],
    "vertices": []
  }
}
```

Status enum can be:

- `unchanged`
- `split`
- `merged`
- `deleted`
- `created` (optional if represented via created lists)

## Minimal Acceptable First Version

If full generic remap is not ready immediately, minimum needed to unblock editor:

1. Accurate remap for the edited domain:
- face ops: face remap must be correct
- edge ops: edge remap must be correct
- vertex ops: vertex remap must be correct
2. `topology_changed` flag
3. `primary_id` (or equivalent) for edited element continuity

Then expand to full face+edge+vertex remap in follow-up.

## Determinism Rules

1. Same input state + same operation -> same output IDs/remap.
2. IDs for untouched entities must remain stable.
3. Remap must always reference IDs in the returned post-edit B-Rep.

## Operation-Level Expectations

1. `pushPullFace(faceId, ...)`
- If face survives: map old face to surviving face (`primary_id`).
- If face splits: map to all new faces with a stable `primary_id`.
- If deleted/consumed: map to empty and mark deleted.

2. `moveEdge(edgeId, ...)`
- Return edge remap at minimum; include affected faces/vertices when changed.

3. `moveVertex(vertexId, ...)`
- Return vertex remap at minimum; include affected edges/faces when changed.

4. `moveFace(faceId, ...)`
- Same expectations as push/pull for face continuity + collateral topology.

## Acceptance Criteria For Kernel Delivery

1. After any topology-changing edit, editor can keep one deterministic active target using remap `primary_id`.
2. Editor can rebuild overlays by remapping all tracked selected IDs (face/edge/vertex).
3. Undo/redo rebinds selected topology IDs without stale references.
4. Cylinder/cuboid primitive-preserving paths still return stable identity mapping when topology does not change.
5. Non-primitive edited solids return correct remap after subsequent edits.

## Test Cases Requested In Kernel

1. Face split case:
- one face old id maps to 2+ new face ids; `primary_id` present.
2. Deletion/consumption case:
- old id maps to empty.
3. Merge case:
- multiple old ids map to one new id.
4. Topology-preserving case:
- identity mapping for unaffected IDs.
5. Determinism:
- repeated same operation produces identical remap and IDs.

## Compatibility Notes

- Keep existing `BrepEditResult` payload shape stable.
- Introduce new remap field additively if needed.
- Avoid breaking current `opengeometry-three` parsing; additive changes only.

## Priority

High. This is the remaining kernel contract gap preventing production-grade B-Rep editing continuity in the editor package.
