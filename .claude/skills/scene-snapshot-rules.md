---
name: scene-snapshot-rules
description: Use when working with `OGSceneManager` or the new `OGEntityRegistry`, when implementing or calling projection (`projectTo2DCamera`, `projectTo2DLines`, `projectCurrentToViews`), when implementing or calling export (`exportSceneToStl`, `exportSceneToStep`, `exportSceneToIfc`, native PDF), or when a user reports "my scene shows old geometry after I edited the shape." Also fires on questions about the lifecycle of BRep snapshots inside the scene.
---

# Scene & Snapshot Rules

`OGSceneManager` (and its successor `OGEntityRegistry`) is the bridge between live
wrapper objects in the TS layer and batch operations in the Rust kernel — projection,
export, multi-entity rendering. The scene is **a registry of serialized BRep snapshots**,
not a live view of the wrapper objects.

This is the most common source of "why does my export show the old geometry?" bugs.

## Mental model

```
TS wrapper object (Cuboid, Polygon, ...)         OGSceneManager (Rust, WASM)
   ├── live BRep, mutable                            ├── snapshot of BRep #1 — frozen at insertion
   ├── live placement                                ├── snapshot of BRep #2 — frozen at insertion
   └── re-emits geometry on setConfig()              └── ...
```

`addBrepEntityToScene(sceneId, entityId, kind, brepJson)` copies the BRep JSON into the
scene. After that call, the wrapper object and the scene are independent. Editing the
wrapper does **not** update the scene.

## Push updates explicitly

If you change a wrapper after inserting it and want the scene to reflect it, you must
push the new snapshot:

```ts
// Pattern (verify exact method names with `git grep` — APIs are mid-rename)
const updatedJson = toBrepSerialized(wall);            // see helper below
manager.replaceBrepEntityInScene(sceneId, "wall-1", "Cuboid", updatedJson);
//   or
manager.refreshBrepEntityInScene(sceneId, "wall-1");   // pull from a registered source
```

### Serialization helper

Use the BRep accessor precedence chain:

```ts
function toBrepSerialized(source: unknown): string {
  const get = (k: string) =>
    source && typeof (source as any)[k] === "function" ? (source as any)[k]() : null;

  const value = get("getBrepSerialized") ?? get("getBrepData") ?? get("getBrep") ?? source;
  return typeof value === "string" ? value : JSON.stringify(value);
}
```

Note: `Polygon.getBrepData()` already returns serialized JSON — passing it through
`JSON.stringify` again would double-encode. The helper above handles this because the
`typeof value === "string"` branch short-circuits.

## Projection

`projectTo2DCamera(...)` and `projectTo2DLines(...)` (and the in-flight
`projectCurrentToViews(...)` for batched multi-view) operate on the snapshots stored in
the scene. They do **not** read from wrapper objects.

If the projection result looks wrong:
1. Confirm the snapshot is current (replace/refresh, then re-project).
2. Confirm the camera parameters JSON is what you expect.
3. Confirm `HlrOptions` matches the desired silhouette/hidden-line behavior.

## Edge classification (in-flight)

The technical-drawing PDF flow adds `EdgeClass` (VisibleOutline / VisibleCrease / Hidden /
SectionCut) to projected segments via `ClassifiedSegment`. If you are implementing a new
projection path, prefer emitting `ClassifiedSegment` rather than unclassified `Segment2D`,
so downstream PDF/SVG/DXF emitters can apply ISO 128 line weights.

Spec: `knowledge/technical-drawing-pdf-export.md`.

## Export

Export functions (`exportSceneToStl`, `exportSceneToStep`, `exportSceneToIfc`) iterate the
snapshots. Same rule: snapshots, not wrappers.

Native PDF (`pdf.rs`) is gated behind `cfg(not(target_arch = "wasm32"))`. It uses the
`printpdf` crate, which does not compile to WASM. Browser PDF is **not** supported in
this kernel — see the spec for the planned downstream package.

## When to use a single direct BRep helper instead

For one-off operations on a single shape (`exportBrepToStl`, `exportBrepToStep`, etc.),
skip `OGSceneManager` and call the direct helper. The scene manager is only worth its
overhead when you have multiple entities and want batched projection or multi-entity
export.

## OGSceneManager vs OGEntityRegistry

Both currently live in `main/opengeometry/src/scenegraph.rs`. The new `OGEntityRegistry`
is the API direction for the technical-drawing work; `OGSceneManager` remains the stable
public name for the moment. Verify exact symbols with `git grep` before recommending one
over the other in new code.

## Verification

When changing scene/projection/export code:
1. Run `cargo test` — unit tests cover snapshot round-tripping.
2. Manually run the projection example:
   ```bash
   cargo run --manifest-path main/opengeometry/Cargo.toml \
     --example scenegraph_projection -- ./out/scenegraph_projection.pdf
   ```
3. Or dump JSON for inspection:
   ```bash
   cargo run --manifest-path main/opengeometry/Cargo.toml \
     --example scenegraph_projection_dump_json -- ./out/projection_dump
   jq . ./out/projection_dump_scene2d.json
   ```
