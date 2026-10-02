# Migration guide

This guide tells you how to move code from one OpenGeometry version to the next.
The newest version comes first.
It is written for people and for coding assistants alike: each section says what changed, shows the same code before and after, and lists what to replace.

## How to use this guide

1. Find the section for the version you are leaving.
2. Go through its "Replace this with that" table and change your code row by row.
3. Read its "Different behaviour" list: it says what now works differently, even where the names look the same.
4. For a name the table does not list, look in "Partly replaced". Anything listed under "Removed" has no replacement: take that code out, or stay on the older version.
5. Finish with its "Check your result" steps.

## 2.0 to 2.5

### What changed

- Two classes make every body: `Wire` for wires and `Solid` for solids. You pass a kind constant, its parameters and options, for example `new Solid(OG_PRIMITIVE_CUBOID, { width, height, depth })`.
- Points and vectors are plain arrays, `[x, y, z]`. There is no `Vector3`.
- Booleans change the body you call them on and return nothing. In 2.0 they returned a new body.
- Bodies move with `transform`. There is no `setPlacement`.
- `OpenGeometry.create` takes a second argument with the URL of the display worker, `tessellation-worker.js`, which your site must serve as well as `opengeometry_bg.wasm`.
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
The 2.5 block builds its own scene, which is why it adds a light.
Bodies are Three.js objects in both versions, so `scene.add(body)` does not change.

### Replace this with that

`[cx, cy, cz]` are the three numbers of the 2.0 `center`, and `[tx, ty, tz]` are those of `translation`.
`[rx, ry, rz]` are the three angles of the 2.0 `rotation`, in radians.
In the 2.5 column, a `?` after a parameter name marks it as optional, and rows that end with "(!)" also behave differently, so read "Different behaviour" before you change them.

| 2.0 | 2.5 |
| --- | --- |
| `await OpenGeometry.create({ wasmURL })` | `await OpenGeometry.create({ wasmURL }, { workerURL })`, and serve `tessellation-worker.js` too |
| `new Vector3(x, y, z)` | `[x, y, z]` |
| `ogid` option and property | `ogId` option and property |
| `new Cuboid({ center, width, height, depth })` | `new Solid(OG_PRIMITIVE_CUBOID, { width, height, depth }, { plane: { origin: [cx, cy - height / 2, cz] } })`; the 2.0 constructor also took `translation`, `rotation` and `scale`, which you give as in the `setPlacement` rows, translation first (!) |
| `new Cylinder({ center, radius, height })` | `new Solid(OG_PRIMITIVE_CYLINDER, { radius, height }, { plane: { origin: [cx, cy - height / 2, cz] } })` (!) |
| `new AnalyticSolid({ kind: 'cylinder', radius, height, frame })` | `new Solid(OG_PRIMITIVE_CYLINDER, { radius, height }, { plane: { origin: frame.origin, normal: frame.z, xDirection: frame.x } })` |
| `new AnalyticSolid({ kind: 'cuboid', width, depth, height, frame })` | `new Solid(OG_PRIMITIVE_CUBOID, { width, height, depth }, { plane: { origin, normal: frame.z, xDirection: frame.x } })`, with `origin` the 2.0 `frame.origin` moved by `width / 2` along `frame.x` and by `depth / 2` along `frame.y`, because 2.0 put the frame origin at a corner of the base |
| `new Opening({ ... }).subtractFrom(wall)` | make the opening a cuboid `Solid`, then `wall.operate(OG_OPERATION_SUBTRACT, { tools: [opening] })`; do not add the opening solid to the scene (a 2.0 `Opening` was invisible; a 2.5 solid you add is drawn) (!) |
| `new Rectangle({ center, width, breadth })` | `new Wire(OG_PRIMITIVE_RECTANGLE, { width, breadth }, { plane: { origin: [cx, cy, cz] } })`; the 2.0 constructor also took `translation`, `rotation` and `scale`, which you give as in the `setPlacement` rows, translation first |
| `new Line({ start, end })`, `new Polyline({ points })` | `new Wire(OG_PRIMITIVE_POLYLINE, { points, closed? })`; a line is a polyline with two points; the 2.0 constructors also took `translation`, `rotation` and `scale`, which you give as in the `setPlacement` rows, translation first |
| `new Arc({ center, radius })` as a full circle | `new Wire(OG_PRIMITIVE_CIRCLE, { radius }, { plane: { origin: [cx, cy, cz] } })` |
| `new Polygon({ vertices, holes })` as a profile | closed polyline wires, used as `profile` and `holes` |
| `polygon.extrude(h)`, `Solid.extrude(...)`, `extrudeBrepFace(...)` | `new Solid(OG_OPERATION_EXTRUDE, { profile, holes?, distance: h })` |
| `new Sweep({ path, profile })` | `new Solid(OG_OPERATION_SWEEP, { profile, path })`, both wires: the path an open polyline of the 2.0 path points, the profile placed at the start of the path as "Sweeps: where the profile goes" below describes (!) |
| `setPlacement({ translation })` | `transform(OG_TRANSFORM_TRANSLATE, { offset })`, with `offset` the new `translation` minus the old one; or `transform(OG_TRANSFORM_PLACE, { origin })` with `origin` `[cx + tx, cy + ty - height / 2, cz + tz]` for a cuboid or opening, or `[cx + tx, cy + ty, cz + tz]` for a rectangle (!) |
| `setPlacement({ rotation })` | `transform(OG_TRANSFORM_ROTATE, { axis, degrees, pivot })`, one call per angle that is not zero, in this order: `[0, 0, 1]` by `rz`, then `[0, 1, 0]` by `ry`, then `[1, 0, 0]` by `rx`. Use `degrees = radians * 180 / Math.PI` and the same `pivot` each time (see Placement). This is for a body not turned yet: to change a turn, first `PLACE` it at the `origin` the translation row gives for a cuboid, opening or rectangle, or at `[tx, ty, tz]` for a polyline wire made without a `plane`, a solid extruded from such a wire, or a sweep along such a path (this resets turn and scale), then turn and scale it again. (!) |
| `setPlacement({ scale })` | `transform(OG_TRANSFORM_SCALE, { factor, pivot })`, where `factor` is the new `s` divided by the current one (`s` itself on a body not scaled yet), for a 2.0 scale `(s, s, s)`. 2.5 has no uneven scale (!) |
| `getPlacement()` gives `{ translation, rotation, scale }` | `getPlacement()` gives `{ origin, xDirection, normal, scale }` (!) |
| `setConfig({ width })`, `cuboid.width = 3` | `rebuild(OG_PRIMITIVE_CUBOID, { width, height, depth })`, with every parameter given (!) |
| `color` option, `body.color = c`, `body.outline = true`, `deflection` option, `setConfig({ deflection })` | `appearance: { color, outline, deflection }` in the options, or `body.setAppearance({ color, outline, deflection })` (!) |
| `booleanUnion(a, b)`, `a.union(b)` | `a.operate(OG_OPERATION_UNION, { tools: [b] })` (!) |
| `booleanSubtraction(a, b)`, `a.subtract([b, c])`, `executeBooleanSubtractionMany(a, [b, c])` | `a.operate(OG_OPERATION_SUBTRACT, { tools: [b, c] })` (!) |
| `booleanIntersection(a, b)`, `a.intersection(b)` | `a.operate(OG_OPERATION_INTERSECT, { tools: [b] })` (!) |
| `result.report` | `solid.getReport()?.report`, on the body you called `operate` on |
| `solid.exportStep()`, `solid.exportStep('metre')` | `await OpenGeometry.exportStep({ nodes: [solid], unit: 'metre', upAxis: 'Y' })`; for `exportStep('millimetre')` pass `unit: 'millimetre'` (!) |
| `exportBrepToStep(brepJson, configJson)` | `await OpenGeometry.exportStep({ nodes: [solid], unit: 'metre', upAxis: 'Y' })`, which keeps the numbers 2.0 wrote with its default config; the result has `report`, an object, where 2.0 had `reportJson` (!) |
| `getBrepSerialized()` | `getBrep()`, which returns the parsed object, so drop the `JSON.parse`. It describes the shape in the body's own coordinates, without its placement, which includes the `plane` it was made with. It has the fields of the 2.0 `AnalyticSolid` JSON; the older layout that `Rectangle.getBrep()` and `Sweep.getBrep()` returned is gone |
| `getModelBounds()` | `getBounds()` (!) |
| `dispose()` | `dispose()`; `OpenGeometry.reset()` frees every body at once (!) |
| catch `AnalyticGeometryError` or `WorldGraphError`, read `code` and `detail` | catch `OGError`, read `code`, `call` and `details` (!) |

### Different behaviour

- **Where a cuboid or cylinder sits.** In 2.0 it was centred on `center`. In 2.5 it stands on its plane origin, which is the middle of its base. That is why the table lowers the origin by `height / 2`.
- **Booleans change the target.** In 2.0 a boolean returned a new body and left the target alone. In 2.5 `operate` changes the target and returns nothing. In both versions the tools stay as they were.
- **No result object.** Because `operate` returns nothing, `const result = a.union(b)` has no 2.5 form: use `a` afterwards, and call `a.duplicate()` first if your code still needs `a` as it was.
- **Placement.** `setPlacement` set absolute values and kept the ones you left out. In 2.5, `TRANSLATE`, `ROTATE` and `SCALE` apply on top of the current placement (`SCALE` multiplies), and `PLACE` sets the origin and resets the turn and scale you leave out. 2.0 turned and scaled a `Cuboid` or `Opening` about the point `translation`, a `Rectangle` about `center + translation`, and a `Line`, `Polyline`, `Polygon` or `Sweep` about the middle of the box around its points moved by `translation`. 2.5 turns and scales a body about its own origin unless you pass `pivot`: the middle of the base for a cuboid or cylinder, the `plane` origin for a rectangle or circle, `[0, 0, 0]` for a polyline made from points. To place a cuboid where a 2.0 one sat, lower its origin by half its height.
- **`getPlacement`.** 2.5's `origin` is the body's origin (the middle of the base for a cuboid), not 2.0's `translation`.
- **Rebuild keeps the base.** A 2.0 `Cuboid` kept its centre when `setConfig` changed its height; `rebuild` keeps the base where it is, so the centre moves. `setConfig({ center })` becomes `TRANSLATE` by the change in `center`, or `PLACE`.
- **Sweeps: where the profile goes.** 2.0 moved the profile to the path's first point P, centred it there and turned it square to the first direction D. 2.5 sweeps the profile where it is, and throws when the profile's plane misses P or is not square to D. So build the profile there: a rectangle or circle wire with `{ plane: { origin: P, normal: D } }`, or, when the 2.0 profile points all had the same y, a closed polyline of those points moved so the average of their distinct points is `[0, 0, 0]`, given the same `plane`; if the points run clockwise seen from above, as a 2.0 `Rectangle`'s did, write each `[x, 0, z]` as `[x, 0, -z]`, or the profile comes out mirrored. A profile in the right plane but off-centre is swept beside the path. A closed path has no 2.5 form.
- **Sweeps: how the profile is turned.** `xDirection` sets how the profile is turned about the path: when D points along x you must give it, and for a rectangle `[0, 1, 0]` matches 2.0. In every other case a profile that is not a circle can come out turned about the path: compare with your 2.0 result and set `xDirection` to match.
- **Do not write `.position`, `.rotation` or `.scale`.** 2.0 placed some shapes that way. In 2.5 such a change is undone, with a `PlacementOverwritten` warning.
- **`Solid` is a different class.** 2.0 had `new Solid({ brep, color })`. 2.5 has `new Solid(kind, params, options)`.
- **No swapping objects.** A body redraws itself after `transform`, `rebuild` or `operate`. There is no `setConfig` and no new object to put into the scene. Geometry is made when the scene is rendered, not when the body is created.
- **STEP units and axes.** `solid.exportStep()` wrote metres. `exportBrepToStep` and `WorldGraph.exportStep` wrote your numbers unchanged under a millimetre header by default. 2.5 reads model numbers as metres: `unit: 'metre'` writes them unchanged, `unit: 'millimetre'` multiplies them by 1000, and the default is millimetres with Z up. 2.0 kept Y up, so pass `upAxis: 'Y'`.
- **Bounds.** `getModelBounds()` gave the body's own bounds as a `THREE.Box3`. `getBounds()` gives bounds in world space as `[minX, minY, minZ, maxX, maxY, maxZ]`, or `null`.
- **`dispose()`** also removes the body from its parent.
- **Colours** are numbers only, such as `0x10b981`. 2.0 `body.color` returned a `THREE.Color`.
- **Errors.** There is one class, `OGError`. Codes are now PascalCase, such as `InvalidParameter`, and `detail` is now `details`.
- **Extruded solids work in booleans.** 2.0 refused them.

### Partly replaced

- `WorldGraph`: nodes and hierarchy become `SystemAssembly`, `addChild` or the `parent` option; `getWorldBounds(id)` becomes `body.getBounds()`, six numbers instead of a `THREE.Box3`; `removeNode(id)` becomes `parent.removeChild(body)`, which keeps the body (pass `{ keepWorld: true }` to leave it where it is), or `body.dispose()`, which frees it and everything under it; a body with no parent has only `dispose()`; `exportStep` becomes `OpenGeometry.exportStep({ nodes })` (nodes may be bodies or assemblies). Its `clash`, `clearance`, `distance`, `projectLines`, `projectViews` and `bind` are gone.
- The pattern helpers (`AnalyticPattern`, `linearPattern`, `rectangularPattern`, `circularPattern`): make copies with `solid.instance()` and move each with `transform`; check the positions, since 2.0 drew a pattern around its own origin. `rebuild` and `operate` on an instanced body need `{ instances: 'all' }` or `makeUnique()` first.
- `AnalyticSolid` kinds: `linearExtrusion` becomes `OG_OPERATION_EXTRUDE` with closed polyline wires for the outline and each hole, each 2.0 point `[u, v]` written `[u, 0, -v]` when the 2.0 `frame` was the default (with another `frame`, also give each wire `plane: { origin: frame.origin, normal: frame.z, xDirection: frame.x }` and check which way it extrudes); `revolvedRectangle` (a tube) becomes `OG_OPERATION_EXTRUDE` with a circle wire as `profile` and a smaller circle wire with the same centre as the one hole.

### Removed in 2.5

These are gone:

- STL and IFC export. PDF export was never in the browser build.
- Projection and 2D views, offset, loft, and public triangulation (`tessellate_brep`, `tessellateFacetedBrep`).
- `Sphere`, `Wedge`, `Curve`, `EllipticalArc`, `AnalyticCurve`, arcs that are not full circles, and cylinder sectors.
- `AnalyticSolid` kinds other than cuboid, cylinder, `linearExtrusion` and `revolvedRectangle`, `AnalyticSolid.fromBrep`, and `new Solid({ brep })`.
- `WorldGraph`'s queries and projection: `clash`, `clearance`, `distance`, `projectLines`, `projectViews` and `bind`.
- The older scene manager (`add*ToScene`, `replaceBrepEntityInScene`, `refreshBrepEntityInScene`), already removed in 2.0.16.
- `shell`, `classifyPoint` and `normalAtFace`.
- The freeform geometry and editor classes, such as `FreeformGeometry` and `FreeformEditor`.
- `SpotLabel`.
- The 2D region booleans (`booleanRegions2D`, `booleanCurvedRegions2D`), `analyzeProfile`, `validateAnalyticBrep` and `WorkPlane`.
- Sweep caps (`capStart`, `capEnd`).
- `OpenGeometry.version`, `enableDebug` and the `debugMeshes` option.
- The `create*Example` builders.

A 2.0 export this guide does not name has no direct 2.5 form; the full 2.5 surface is `index.d.ts` in the package.

### Check your result

1. Open `node_modules/opengeometry/package.json`. Its `version` should start with `2.5`.
2. Run your type check, for example `npx tsc --noEmit`. Imports that no longer exist, such as `Cuboid` or `Vector3`, fail here.
3. Run your app. Check that bodies sit and face the way they did before, cuboids, cylinders and sweeps above all, and that STEP files have the units and up axis you expect.

## Adding an entry

When a change to OpenGeometry breaks code written for the previous version, add a section to this guide in the same change.

- Put the new section at the top, just under "How to use this guide", and name it after the two versions, such as "2.5 to 2.6". If the next version number is not decided when you make the change, name the section "Unreleased" and rename it at release.
- Say what changed, show the same code before and after, give the replace table, list what was removed, and say how to check the result. Add "Different behaviour" when something now works differently, and "Partly replaced" when a name has only a partial new form.
- Code for the current version goes in a code block labelled `ts` that starts at the left margin, ends with a bare closing fence at the left margin, and is a complete module, imports included. The checks type-check every such block against the built package. The checks fail on a `ts`, `typescript` or `tsx` block written any other way, and when the guide has no `ts` block at all.
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

### Partly replaced

- A name with only a partial new form, and what to use instead.

### Removed in <new>

- What has no replacement.

### Check your result

1. How to tell the change worked.
```
