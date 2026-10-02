# Migration guide

This guide tells you how to move code from one OpenGeometry version to the next.
The newest version comes first.
It is written for people and for coding assistants alike: each section says what changed, shows the same code before and after, and lists what to replace.

## How to use this guide

1. Find the section for the version you are leaving.
2. Go through its "Replace this with that" table and change your code row by row.
3. Read its "Different behaviour" list: it says what now works differently, even where the names look the same.
4. Anything listed under "Removed" has no replacement. Take that code out, or stay on the older version.

## 2.0 to 2.5

### What changed

- Two classes make every body: `Wire` for wires and `Solid` for solids. You pass a kind constant, its parameters and options, for example `new Solid(OG_PRIMITIVE_CUBOID, { width, height, depth })`.
- Points and vectors are plain arrays, `[x, y, z]`. There is no `Vector3`.
- Booleans change the body you call them on and return nothing. In 2.0 they returned a new body.
- Bodies move with `transform`. There is no `setPlacement`.
- `OpenGeometry.create` takes a second argument with the URL of the display worker, `tessellation-worker.js`, which your site serves next to `opengeometry_bg.wasm`.
- STEP export is `OpenGeometry.exportStep`. It returns a Promise.
- Every failure throws one error class, `OGError`.

### Before and after

2.0:

```js
import { OpenGeometry, Cuboid, Vector3 } from 'opengeometry';
await OpenGeometry.create({ wasmURL: '/opengeometry_bg.wasm' });
const cuboid = new Cuboid({ center: new Vector3(0, 0.8, 0), width: 1.5, height: 1.6, depth: 1.2, color: 0x10b981 });
cuboid.outline = true;
scene.add(cuboid);
```

2.5:

```ts
import * as THREE from 'three';
import { OpenGeometry, Solid, OG_PRIMITIVE_CUBOID } from 'opengeometry';

await OpenGeometry.create({ wasmURL: '/opengeometry_bg.wasm' }, { workerURL: '/tessellation-worker.js' });

const cuboid = new Solid(
  OG_PRIMITIVE_CUBOID,
  { width: 1.5, height: 1.6, depth: 1.2 },
  { appearance: { color: 0x10b981, outline: true } },
);

const scene = new THREE.Scene();
scene.add(new THREE.HemisphereLight(0xffffff, 0x444444, 2), cuboid);
```

Both make the same box: 1.5 m wide, 1.6 m high and 1.2 m deep, standing on the ground at the origin.
Bodies are Three.js objects in both versions, so `scene.add(body)` does not change.

### Replace this with that

| 2.0 | 2.5 |
| --- | --- |
| `await OpenGeometry.create({ wasmURL })` | `await OpenGeometry.create({ wasmURL }, { workerURL })`, and serve `tessellation-worker.js` too |
| `new Vector3(x, y, z)` | `[x, y, z]` |
| `ogid` option | `ogId` option |
| `new Cuboid({ center, width, height, depth })` | `new Solid(OG_PRIMITIVE_CUBOID, { width, height, depth }, { plane: { origin: [cx, cy - height / 2, cz] } })` |
| `new Cylinder({ center, radius, height })` | `new Solid(OG_PRIMITIVE_CYLINDER, { radius, height }, { plane: { origin: [cx, cy - height / 2, cz] } })` |
| `new Opening({ ... }).subtractFrom(wall)` | make the opening a cuboid `Solid`, then `wall.operate(OG_OPERATION_SUBTRACT, { tools: [opening] })` |
| `new Rectangle({ center, width, breadth })` | `new Wire(OG_PRIMITIVE_RECTANGLE, { width, breadth }, { plane: { origin: center } })` |
| `new Line({ start, end })`, `new Polyline({ points })` | `new Wire(OG_PRIMITIVE_POLYLINE, { points, closed? })`; a line is a polyline with two points |
| `new Arc({ center, radius })` as a full circle | `new Wire(OG_PRIMITIVE_CIRCLE, { radius }, { plane: { origin: center } })` |
| `new Polygon({ vertices, holes })` as a profile | closed polyline wires, used as `profile` and `holes` |
| `polygon.extrude(h)`, `Solid.extrude(...)`, `extrudeBrepFace(...)` | `new Solid(OG_OPERATION_EXTRUDE, { profile, holes?, distance: h })` |
| `new Sweep({ path, profile })` | `new Solid(OG_OPERATION_SWEEP, { profile, path })`, where both are wires |
| `setPlacement({ translation })` | `transform(OG_TRANSFORM_PLACE, { origin })` to set the position, or `transform(OG_TRANSFORM_TRANSLATE, { offset })` to move by an amount |
| `setPlacement({ rotation })` | `transform(OG_TRANSFORM_ROTATE, { axis, degrees, pivot? })` |
| `setPlacement({ scale })` | `transform(OG_TRANSFORM_SCALE, { factor, pivot? })` |
| `getPlacement()` gives `{ translation, rotation, scale }` | `getPlacement()` gives `{ origin, xDirection, normal, scale }` |
| `setConfig({ width })`, `cuboid.width = 3` | `rebuild(OG_PRIMITIVE_CUBOID, { width, height, depth })`, with every parameter given |
| `color` option, `body.color = c`, `body.outline = true` | `appearance: { color, outline }` in the options, or `body.setAppearance({ color, outline })` |
| `booleanUnion(a, b)`, `a.union(b)` | `a.operate(OG_OPERATION_UNION, { tools: [b] })` |
| `booleanSubtraction(a, b)`, `a.subtract([b, c])`, `executeBooleanSubtractionMany(a, [b, c])` | `a.operate(OG_OPERATION_SUBTRACT, { tools: [b, c] })` |
| `booleanIntersection(a, b)`, `a.intersection(b)` | `a.operate(OG_OPERATION_INTERSECT, { tools: [b] })` |
| `result.report` | `solid.getReport()` |
| `solid.exportStep('metre')`, `exportBrepToStep(...)` | `await OpenGeometry.exportStep({ nodes: [solid], unit: 'metre', upAxis: 'Y' })` |
| `getBrepSerialized()` | `getBrep()` |
| `getModelBounds()` | `getBounds()` |
| `dispose()` | `dispose()`; `OpenGeometry.reset()` frees every body at once |
| catch `AnalyticGeometryError`, read `code` and `detail` | catch `OGError`, read `code`, `call` and `details` |

A `?` marks an optional parameter.

### Different behaviour

- **Where a cuboid or cylinder sits.** In 2.0 it was centred on `center`. In 2.5 it stands on its plane origin, which is the middle of its base. That is why the table lowers the origin by `height / 2`.
- **Booleans change the target.** In 2.0 a boolean returned a new body and left the target alone. In 2.5 `operate` changes the target and returns nothing. In both versions the tools stay as they were.
- **Placement.** `setPlacement` set absolute values and kept the ones you left out. In 2.5, `TRANSLATE`, `ROTATE` and `SCALE` add to the current placement. `PLACE` sets the placement and resets everything you leave out. Rotation was three Euler angles in radians; it is now one axis and an angle in degrees.
- **Do not write `.position`, `.rotation` or `.scale`.** 2.0 placed some shapes that way. In 2.5 such a change is undone, with a `PlacementOverwritten` warning.
- **`Solid` is a different class.** 2.0 had `new Solid({ brep, color })`. 2.5 has `new Solid(kind, params, options)`.
- **No swapping objects.** A body redraws itself after `transform`, `rebuild` or `operate`. There is no `setConfig` and no new object to put into the scene. Geometry is made when the scene is rendered, not when the body is created.
- **STEP defaults.** 2.0 exported in metres and kept the model's axes. 2.5 defaults to millimetres and Z up, so pass `unit: 'metre', upAxis: 'Y'` to keep the 2.0 units and axes.
- **Bounds.** `getModelBounds()` gave the body's own bounds as a `THREE.Box3`. `getBounds()` gives bounds in world space as `[minX, minY, minZ, maxX, maxY, maxZ]`, or `null`.
- **`dispose()`** also removes the body from its parent.
- **Colours** are numbers only, such as `0x10b981`.
- **Errors.** There is one class, `OGError`. Codes are now PascalCase, such as `InvalidParameter`, and `detail` is now `details`.
- **Extruded solids work in booleans.** 2.0 refused them.

### Removed in 2.5

These have no replacement:

- STL, IFC and PDF export.
- Projection and 2D views, offset, loft, and public triangulation (`tessellate_brep`, `tessellateFacetedBrep`).
- `Sphere`, `Wedge`, `Curve`, `EllipticalArc`, `AnalyticCurve`, arcs that are not full circles, and cylinder sectors.
- `AnalyticSolid` kinds other than cuboid and cylinder, `AnalyticSolid.fromBrep`, and `new Solid({ brep })`.
- `WorldGraph` with `clash`, `clearance`, `distance`, `projectLines`, `projectViews` and `bind`, and the older scene manager (`add*ToScene`, `replaceBrepEntityInScene`, `refreshBrepEntityInScene`). To export several bodies to STEP, pass them all to `OpenGeometry.exportStep({ nodes })`.
- The pattern helpers: `AnalyticPattern`, `linearPattern`, `rectangularPattern`, `circularPattern`.
- `shell`, `classifyPoint` and `normalAtFace`.
- The freeform geometry and editor classes, such as `FreeformGeometry` and `FreeformEditor`.
- `SpotLabel`.
- The 2D region booleans (`booleanRegions2D`, `booleanCurvedRegions2D`), `analyzeProfile`, `validateAnalyticBrep` and `WorkPlane`.
- Sweep caps (`capStart`, `capEnd`).
- `OpenGeometry.version`, `enableDebug` and the `debugMeshes` option.
- The `create*Example` builders.

### Check your result

1. Open `node_modules/opengeometry/package.json`. Its `version` should start with `2.5`.
2. Run your type check, for example `npx tsc --noEmit`. Imports that no longer exist, such as `Cuboid` or `Vector3`, fail here.
3. Run your app. Check that bodies sit where they did before, cuboids and cylinders above all, and that STEP files have the units and up axis you expect.

## Adding an entry

When a change to OpenGeometry breaks code written for the previous version, add a section to this guide in the same change.

- Put the new section at the top, just under "How to use this guide", and name it after the two versions, such as "2.5 to 2.6".
- Say what changed, show the same code before and after, give the replace table, and list what was removed. Add "Different behaviour" when something now works differently.
- Code for the current version goes in a code block labelled `ts` that starts at the left margin and is a complete module, imports included. The checks type-check every such block against the built package.
- Code for an older version goes in a code block labelled `js`, which is not checked.
- When a later change breaks a `ts` block in an older section, change that block's label to `js`.

Copy this outline:

```md
## <old> to <new>

### What changed

- One short line per change.

### Before and after

<old>: a code block labelled js with the old code.

<new>: a code block labelled ts with the same code for the new version.

### Replace this with that

| <old> | <new> |
| --- | --- |
| old call | new call |

### Different behaviour

- What now works differently.

### Removed in <new>

- What has no replacement.

### Check your result

1. How to tell the change worked.
```
