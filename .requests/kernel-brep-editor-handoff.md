# Kernel Handoff: Requirements For A Proper B-Rep Editor

## Purpose

This document defines the kernel-side capabilities needed to build a real B-Rep editor in `@opengeometry/editor-controls`.

The current package can render and inspect B-Reps, but its editing path is still wrapper-first:

- It reads geometry through `getBrepSerialized()`
- It mutates host objects through `setConfig(...)` / `setTransformation(...)`
- It rebuilds the B-Rep after each wrapper patch

That is enough for parametric editing of primitives, but it is not enough for true face/edge/vertex editing.

The next version of the editor should be kernel-first for 3D editing.

## Current Constraint

Today the editor binds to wrapper objects and only knows how to apply option patches.

Relevant code in this package:

- `src/adapters/default-adapters.ts`
- `src/core/editor.ts`
- `examples-vite/sandbox.ts`

Why this blocks a real B-Rep editor:

- Arbitrary face push/pull is a topology operation, not a config patch.
- After a topology edit, the result may no longer be representable as the same primitive wrapper.
- Curved faces need semantic or kernel-native handling, not ad hoc UI shortcuts.
- Continued interaction needs stable face/edge/vertex identity across rebuilds.

Example:

- A cuboid face push/pull can be faked as width/height/depth plus center adjustment.
- A cylinder top face push/pull can map to height.
- A cylinder side face push/pull can map to radius only in the uniform analytic case.
- A partial side-face edit or local face extrusion no longer remains a `Cylinder` primitive.

This is why the editor cannot become universal by adding more wrapper-specific patches.

## Kernel Information Already Suggesting The Right Direction

The kernel architecture notes already describe `Brep` as canonical state and mention `extrude_brep_face`:

- `../OpenGeometry-Kernel/knowledge/opengeometry-architecture.md`

That is the right direction. The editor needs this kind of capability exposed cleanly in the JS/TS surface.

## What The Editor Needs From Kernel

### 1. A kernel-native editable solid/entity surface

The editor needs a public JS/TS object that represents an editable B-Rep entity directly, not only primitive wrappers like `Cuboid`, `Cylinder`, `Wedge`, etc.

Minimum capabilities:

- return serialized or structured B-Rep
- return current placement / transformation
- apply topology edits
- apply placement edits
- regenerate renderable geometry after edits
- return stable topology ids or a remap after edits

This can be either:

- a new generic `EditableBrep` / `BrepEntity` wrapper, or
- the existing wrappers plus a shared low-level kernel edit API they all expose

The important part is that the editor must not be forced to express every edit as `setConfig(...)`.

### 2. Stable topology identifiers

The editor needs stable ids for:

- shells
- faces
- loops
- edges
- vertices

Requirements:

- ids must exist in the public B-Rep payload
- ids must survive no-topology-change edits
- when topology does change, the kernel must return a remap or change report

Needed result shape:

```ts
type TopologyId = string | number;

interface TopologyRemap {
  faces?: Record<TopologyId, TopologyId | null>;
  edges?: Record<TopologyId, TopologyId | null>;
  vertices?: Record<TopologyId, TopologyId | null>;
}
```

Without this, the editor cannot keep a selected face or edge alive across edit steps.

### 3. Public B-Rep inspection helpers

The editor needs public access to topology and geometric properties without reverse engineering them from triangle meshes.

Minimum helpers:

- face centroid
- face normal at representative point
- face surface type
- edge endpoints
- edge curve type
- vertex position
- adjacency:
  - face -> loops
  - loop -> halfedges / edges / vertices
  - edge -> incident faces
  - vertex -> connected edges / faces

Suggested surface:

```ts
interface BrepInspectionApi {
  getFaceInfo(entity: EditableBrepEntity, faceId: TopologyId): FaceInfo;
  getEdgeInfo(entity: EditableBrepEntity, edgeId: TopologyId): EdgeInfo;
  getVertexInfo(entity: EditableBrepEntity, vertexId: TopologyId): VertexInfo;
}
```

### 4. Kernel edit operations for 3D topology editing

The editor needs actual kernel operations, not only primitive parameter mutation.

Minimum phase-1 operations:

- push/pull face along local face normal
- offset or translate face
- move vertex
- move edge

At minimum, the face operation must work for planar and analytic faces and must return a valid edited solid.

Suggested surface:

```ts
interface BrepEditApi {
  pushPullFace(entity: EditableBrepEntity, faceId: TopologyId, distance: number, options?: FaceEditOptions): BrepEditResult;
  moveFace(entity: EditableBrepEntity, faceId: TopologyId, translation: Vec3, options?: FaceEditOptions): BrepEditResult;
  moveEdge(entity: EditableBrepEntity, edgeId: TopologyId, translation: Vec3, options?: EdgeEditOptions): BrepEditResult;
  moveVertex(entity: EditableBrepEntity, vertexId: TopologyId, translation: Vec3, options?: VertexEditOptions): BrepEditResult;
}
```

If the kernel already has an internal `extrude_brep_face` path, expose that through a stable JS/TS API.

### 5. Result payloads that the editor can consume incrementally

Every edit result should return:

- updated B-Rep
- updated placement
- updated renderable geometry if available
- topology remap
- validity / healing report
- whether the result is still representable as the original primitive type

Suggested shape:

```ts
interface BrepEditResult {
  entity: EditableBrepEntity;
  brepSerialized: string;
  geometrySerialized?: string;
  outlineGeometrySerialized?: string;
  topologyRemap?: TopologyRemap;
  validity: {
    ok: boolean;
    healed?: boolean;
    warnings?: string[];
    errors?: string[];
  };
  representation: {
    kind: "primitive" | "generic_brep";
    primitiveType?: "cuboid" | "cylinder" | "sphere" | "wedge" | "opening" | "sweep";
  };
}
```

### 6. Clear behavior for primitive preservation vs promotion

The editor needs a deterministic answer to this question:

When a topology edit no longer matches a primitive wrapper, what happens?

Required behavior:

- if the edited result is still exactly representable as the same primitive, keep that primitive representation
- otherwise promote to a generic B-Rep entity

Examples:

- cuboid top-face push/pull: can stay `Cuboid`
- cylinder top-face push/pull: can stay `Cylinder`
- cylinder side-face uniform radial edit: can stay `Cylinder`
- local face edit that breaks primitive invariants: must become generic B-Rep

This promotion behavior is necessary for a universal editor.

### 7. Placement separated from local B-Rep

The editor needs a clear separation between:

- local shape B-Rep
- world placement / transformation

This is especially important for:

- rotated editing
- local face normals vs world face normals
- stable topological ids
- future instancing / scenegraph support

The editor does not need `OGSceneManager` projection, but it does need reliable local/world transforms.

### 8. Face/edge/vertex geometry suitable for picking and overlays

The editor can project B-Rep itself, but for 3D picking and gizmos it needs kernel data that maps visible geometry back to topology ids.

Useful deliverables:

- triangulated face buffers with face ids preserved per triangle
- edge render buffers with edge ids preserved per segment
- vertex positions with ids

Suggested surface:

```ts
interface TopologyRenderData {
  faces: Array<{
    faceId: TopologyId;
    positions: Float32Array;
    indices: Uint32Array | Uint16Array;
  }>;
  edges: Array<{
    edgeId: TopologyId;
    positions: Float32Array;
  }>;
  vertices: Array<{
    vertexId: TopologyId;
    position: Vec3;
  }>;
}
```

This avoids editor-side guessing about which rendered triangle belongs to which face.

### 9. Validity and healing feedback

The editor needs to know if an edit:

- succeeded
- was auto-healed
- produced warnings
- failed because the requested edit is impossible

This must be part of the public result contract, not hidden in logs.

### 10. Undo-friendly deterministic operations

Kernel edit operations must be deterministic and serializable enough for editor undo/redo.

The editor can manage history, but it needs operations whose outputs are:

- repeatable
- stable enough to reapply
- free of hidden mutable global state

## Minimum Public API Request

If the kernel team wants the smallest useful surface, this is the minimum request:

```ts
interface EditableBrepEntity {
  getId(): string;
  getBrepSerialized(): string;
  getPlacement(): ObjectTransformation;
  setPlacement(transform: ObjectTransformation): void;
  getTopologyRenderData(): TopologyRenderData;
  getFaceInfo(faceId: TopologyId): FaceInfo;
  getEdgeInfo(edgeId: TopologyId): EdgeInfo;
  getVertexInfo(vertexId: TopologyId): VertexInfo;
  pushPullFace(faceId: TopologyId, distance: number, options?: FaceEditOptions): BrepEditResult;
  moveEdge(edgeId: TopologyId, delta: Vec3, options?: EdgeEditOptions): BrepEditResult;
  moveVertex(vertexId: TopologyId, delta: Vec3, options?: VertexEditOptions): BrepEditResult;
}
```

## What The Editor Does Not Need From Kernel

The editor package does not need these kernel-side deliverables for this phase:

- 2D scene projection from `OGSceneManager`
- editor UI widgets
- SVG rendering
- camera controls
- browser event handling

The editor will handle:

- orthographic SVG projection
- 2D snap logic
- 2D/3D selection state
- handles and gizmos
- history stack
- multi-view coordination

## Acceptance Criteria For The Kernel Work

The kernel-side delivery is sufficient when all of the following are possible through the public JS/TS API:

1. Select a cuboid face by stable `faceId`, push/pull it, and continue editing the same face across drag steps.
2. Select a cylinder top face and push/pull it as height change.
3. Select a cylinder side face and perform the kernel-defined valid edit:
   either preserve `Cylinder` if analytic constraints still hold, or promote to generic B-Rep.
4. Select a face on a non-primitive edited solid and continue push/pull without falling back to primitive-only logic.
5. Select an edge by stable `edgeId` and move it.
6. Select a vertex by stable `vertexId` and move it.
7. Receive a topology remap or stable ids after every operation.
8. Receive explicit validity/healing feedback after every operation.

## Suggested Implementation Order

### Phase 1

- expose stable topology ids in public B-Rep payload
- expose `pushPullFace(...)`
- expose face triangulation / edge / vertex id mapping
- expose validity report
- support cuboid and cylinder through the same public edit contract

### Phase 2

- expose `moveEdge(...)`
- expose `moveVertex(...)`
- add topology remap details
- add primitive preservation vs promotion reporting

### Phase 3

- extend the same contract to wedge, opening, sweep, boolean results, and generic edited solids

## Direct Instruction For The Kernel Agent

Build the public JS/TS kernel surface required for a kernel-first B-Rep editor.

Do not optimize for wrapper-only editing.

The target is not "make cuboid and cylinder demos easier." The target is:

- a stable editable B-Rep entity
- stable topology ids
- public face/edge/vertex inspection
- public face/edge/vertex edit operations
- deterministic edit results with remap and validity reporting

If an internal kernel capability already exists, expose it cleanly instead of adding more wrapper-specific shortcuts.

## What The Editor Agent Will Do After Kernel Delivery

Once this kernel surface exists and is republished through npm, the editor package will be rebuilt around:

- B-Rep-first 3D selection
- face/edge/vertex gizmos bound to topology ids
- true push/pull
- generic solid editing after primitive promotion
- consistent multi-view SVG + 3D editing from the same kernel entity

