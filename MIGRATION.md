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

## Unreleased

### What changed

- `OG_PRIMITIVE_SPHERE` makes a solid sphere: `{ radius }`, centred on the `plane` origin.
- `OG_PRIMITIVE_CONE` makes a solid cone or frustum standing on the `plane`, its axis along the normal: `{ radiusBottom, radiusTop, height }`. One radius may be `0` for a pointed cone; radii within four times the geometric tolerance of each other make a cylinder.
- `OG_PRIMITIVE_TORUS` makes a solid ring torus lying on the `plane`, its axis along the normal: `{ majorRadius, minorRadius }`, with the minor radius below the major one.
- `OG_PRIMITIVE_PARAMS_SPHERE`, `OG_PRIMITIVE_PARAMS_CONE` and `OG_PRIMITIVE_PARAMS_TORUS` type their parameters. The entry exports the types `SphereParams`, `ConeParams` and `TorusParams`. A size that is not finite or not above four times the geometric tolerance, two zero cone radii, or a minor radius not below the major one by more than four times the geometric tolerance throws `InvalidParameter`.
- An `operate` error says which handler family refused the bodies and which handlers were tried.
- `OG_OPERATION_REVOLVE` turns a closed `Profile` wire a full turn into a solid: `{ profile, axis? }`. `axis` is `{ point: [u, v], direction: [du, dv] }` in the profile's own `[u, v]` and defaults to `{ point: [0, 0], direction: [0, 1] }`, the profile plane's v axis. `OG_OPERATION_PARAMS_REVOLVE` types the parameters. Each profile edge off the axis becomes one plane, cylinder, cone, sphere or ring torus face; an edge on the axis makes no face. A profile that crosses the axis, touches it at a single point, or meets it along two separate stretches, an arc whose circle meets the axis, and a zero direction throw `InvalidParameter`.
- `OG_OPERATION_SHELL` hollows a solid in place: `solid.operate(OG_OPERATION_SHELL, { thickness })`. It takes a cuboid, cylinder, sphere, ring torus, cone, frustum, planar profile extrusion, or extrusion of a line-and-arc profile with or without holes, and leaves one closed void `thickness` inside every face. The inner walls of a line-and-arc extrusion are its exact inset: lines parallel to the sides, arcs on the same centres, and arcs of radius `thickness` at inward corners. The report's `face_mappings` lists, for each face of the body before the call, its outer face and every inner face offset from any part of it; `handlers` is `[]`. A thickness that is not finite or not above four times the geometric tolerance throws `InvalidParameter`. A cavity that would vanish, and a profile inset that is empty, splits into separate regions or narrows to within four times the geometric tolerance, throw `UnresolvedIntersection`. A shape shared by other instances throws `SharedShape` unless `instances: 'all'` is given, a solid of another kind throws `CoverageGap`, and a solid with approximate geometry throws `UnsupportedGeometry`.
- Not built: a Sheet is not thickened into a solid. `Sheet` has no `operate`, and the shell of an open sheet in the kernel graph throws `CoverageGap` with `details.families` equal to `['restricted analytic shell', 'open sheet']`.
- `body.classifyPoint([x, y, z])` is new. It says whether a point in the body's own coordinates is `'Inside'`, `'Outside'`, on the `'Boundary'` or `'Unknown'`. A Wire or a Sheet throws `InvalidOperand`.

### Before and after: sphere, cone and torus

3.0 had no sphere, cone or torus primitive. Now:

```ts
import {
  OpenGeometry, Solid, OG_PRIMITIVE_CONE, OG_PRIMITIVE_PARAMS_CONE, OG_PRIMITIVE_PARAMS_SPHERE,
  OG_PRIMITIVE_PARAMS_TORUS, OG_PRIMITIVE_SPHERE, OG_PRIMITIVE_TORUS,
} from 'opengeometry';

await OpenGeometry.create({ wasmURL: '/opengeometry_bg.wasm' }, { workerURL: '/tessellation-worker.js' });

const knob = new Solid(OG_PRIMITIVE_SPHERE, OG_PRIMITIVE_PARAMS_SPHERE({ radius: 0.03 }));
const flare = OG_PRIMITIVE_PARAMS_CONE({ radiusBottom: 0.25, radiusTop: 0.15, height: 0.7 });
const pedestal = new Solid(OG_PRIMITIVE_CONE, flare);
const ring = new Solid(OG_PRIMITIVE_TORUS, OG_PRIMITIVE_PARAMS_TORUS({ majorRadius: 0.2, minorRadius: 0.02 }));
console.log(knob.ogId, pedestal.ogId, ring.ogId);
```

### Before and after

3.0 had no point classification. Now:

```ts
import { OpenGeometry, Solid, OG_PRIMITIVE_CYLINDER, OG_PRIMITIVE_PARAMS_CYLINDER } from 'opengeometry';
import type { PointClassification } from 'opengeometry';

await OpenGeometry.create({ wasmURL: '/opengeometry_bg.wasm' }, { workerURL: '/tessellation-worker.js' });

const leg = new Solid(OG_PRIMITIVE_CYLINDER, OG_PRIMITIVE_PARAMS_CYLINDER({ radius: 0.1, height: 0.7 }));
const side: PointClassification = leg.classifyPoint([0, 0.35, 0]);
console.log(side);
```

### Before and after: revolve a profile

3.0 had no revolve.
A `Profile` on the default plane maps `[u, v]` to `(u, 0, -v)`: it lies flat on the ground, and the default axis, its v axis, runs along -z, so the solid lies on its side.
With `plane: { normal: [0, 0, 1], xDirection: [1, 0, 0] }` the profile maps `[u, v]` to `(u, v, 0)`, and a bowl drawn with u as the radius and v as the height stands upright about the y axis:

```ts
import {
  OpenGeometry, Solid, Wire, OG_OPERATION_PARAMS_REVOLVE, OG_OPERATION_REVOLVE, OG_PRIMITIVE_PARAMS_PROFILE,
  OG_PRIMITIVE_PROFILE,
} from 'opengeometry';

await OpenGeometry.create({ wasmURL: '/opengeometry_bg.wasm' }, { workerURL: '/tessellation-worker.js' });

const pan = OG_PRIMITIVE_PARAMS_PROFILE({
  start: [0, 0],
  segments: [
    { to: [0.12, 0] },
    { to: [0.19, 0.3] },
    { to: [0.19, 0.34], arc: { center: [0.19, 0.32], counterClockwise: true } },
    { to: [0.16, 0.34] },
    { to: [0, 0.18], arc: { center: [0, 0.34], counterClockwise: false } },
    { to: [0, 0] },
  ],
  closed: true,
});
const profile = new Wire(OG_PRIMITIVE_PROFILE, pan, { plane: { normal: [0, 0, 1], xDirection: [1, 0, 0] } });
const bowl = new Solid(OG_OPERATION_REVOLVE, OG_OPERATION_PARAMS_REVOLVE({ profile }));
console.log(bowl.ogId);
```

A partial turn is not built: there is no sweep angle, and every revolve is a full turn.
A profile with holes is not built: `OG_OPERATION_REVOLVE` takes no `holes`.

### Different behaviour

- When a tool of `operate` fails, `details` now has `attempted` beside `handlers`, `toolIndex` and `ogIds`: every handler tried, in order.
- When that failure is `CoverageGap`, `details` also has `families`. It names the specialised gap the kernel met, such as `['cylinder', 'oblique cuboid intersection']`; for two planar bodies, that of the last box handler tried. With no specialised gap, it is the error's own pair. The `message` is the same as in 3.0, which for most gaps is the generic pair, and the `code` is still `CoverageGap`.
- `OGError.details` is typed as `ErrorDetails`, which the package exports with `OperateErrorDetails`. A `details` value that is not an object becomes `{}`.
- Point classification counts each solid of a body on its own, outer shell and cavities. A body is Inside if any solid holds the point. Before, the faces of all solids were counted together, so a point inside one of two touching solids could read Outside, and a Boolean that tested such a point could go wrong.
- A full circle or ellipse, and the round direction of a cylinder, cone, sphere or torus, now has at least 12 segments, rounded up to an even number, at every display bucket. Before, a cylinder of radius 0.011 showed 5 sides in plan view, and an odd count made a lying cylinder look different from its mirror image. An arc has at least `ceil(12 * sweep / 2π)` segments, so a sphere has at least 6 rows from pole to pole. Curved bodies have more triangles. Above this minimum the count still changes with the display bucket; to keep one count at every zoom, set `appearance.deflection`.
- `operate` now builds a union, intersection or subtraction of two bodies that are prisms along one shared cylinder axis, however the pair is placed: every flat face is square to the axis or parallel to it, every curved face is a cylinder around a parallel axis, and every side face is a plain strip with no notch or hole. This covers an arc-edged extrusion minus a blind inset or pocket, a cuboid minus a blind arc-edged pocket, and a cuboid with an upright cylinder that crosses a side face. Before, these threw `CoverageGap`, or `UnresolvedIntersection` when the pair was turned off the world axes. The report's `handlers` is `['prismatic_curved_boolean']`, and a gap that reaches the generic handlers has `'prismatic_curved_boolean'` last in `details.attempted`.
- An arc of one such body that touches a side or an arc of the other, within four times the geometric tolerance, throws `UnresolvedIntersection`. A touch at the end of the other body's own arc on the same circle is not refused, so a bore of the same radius at a D-shape's round end builds. A face of one body square to the axis that lies closer than four times the geometric tolerance to such a face of the other, without being level with it, also throws `UnresolvedIntersection`.
- Not built: a cylinder tilted against every axis of a cuboid still throws `CoverageGap`, with `details.families` equal to `['cylinder', 'oblique cuboid intersection']`.

### Check your result

1. A cuboid minus an upright cylinder that crosses one side face is one solid, and its report's `handlers` is `['prismatic_curved_boolean']`. Tilt the cylinder by 10 degrees and the same call throws `CoverageGap` with `details.families` equal to `['cylinder', 'oblique cuboid intersection']`.
2. `classifyPoint` on a cylinder of radius 0.1 and height 0.7 gives `'Inside'` at `[0, 0.35, 0]` and `'Outside'` at `[1, 0.35, 0]`, before and after the body is moved.

## 2.5 to 3.0

2.5 was not published. From 2.0, follow 2.0 to 2.5, then 2.5 to 3.0.

### What changed

- `OG_PRIMITIVE_PROFILE` makes a wire of line and arc segments: `{ start, segments, closed }`, with points as `[u, v]` in the body's `plane`. A segment is `{ to }` for a line, or `{ to, arc: { center, counterClockwise } }` for an arc.
- `OG_PRIMITIVE_ARC` makes a partial arc wire: `{ radius, startAngle, sweepAngle }`, angles in radians, centred on the `plane` origin.
- `OG_OPERATION_EXTRUDE` takes a closed Profile as the profile or as a hole, so the 2.0 arc-edged extrusion is an extrude again.
- `OG_PRIMITIVE_CYLINDER_SECTOR` makes a solid cylinder sector: `{ radius, height, startAngle, sweepAngle }`, angles in radians from the `plane` x direction. A positive angle turns counter-clockwise seen from the side the `plane` normal points to; on the default ground plane that is seen from above, from +X towards −Z. The height runs along the normal.
- `OG_PRIMITIVE_POLYHEDRON` makes a planar solid from `{ points, faces }`: `points` are `[x, y, z]` in the body's own space, Y up, in metres, and each face is a ring of point indices, counter-clockwise seen from outside. It replaces 2.0's `planarPolyhedron` analytic solid, which took `vertices`.
- `OG_PRIMITIVE_PARAMS_PROFILE`, `OG_PRIMITIVE_PARAMS_ARC`, `OG_PRIMITIVE_PARAMS_CYLINDER_SECTOR` and `OG_PRIMITIVE_PARAMS_POLYHEDRON` type their parameters. The entry exports the types `ProfileParams`, `ProfileSegment`, `ProfileArc`, `ArcParams`, `CylinderSectorParams` and `PolyhedronParams`.
- `body.getWorldBrep()` reads a body's BRep placed in the world. It makes no body and changes nothing.
- `body.faceNormalAt(faceId, point)` returns the unit normal of a face at a point, in the body's own coordinates, with the face's orientation applied.
- `OG_OPERATION_SWEEP` takes `caps`. It is `true` when left out, so existing sweeps are unchanged. `caps: false` leaves both ends open.
- `Sheet` is a new body class beside `Wire` and `Solid`, with the body type `'Sheet'`: faces that enclose no volume. `new Sheet(OG_OPERATION_SWEEP, { profile, path, caps: false })` makes an open-ended sweep. A Sheet shows its surface, from the front only, and its outline.
- `OG_OPERATION_SWEEP` takes a path with arcs: an `OG_PRIMITIVE_ARC` wire, or an open `OG_PRIMITIVE_PROFILE` with arc segments. Each arc sweeps into an exact bend. A circle profile gives torus faces; a polygon profile gives planes, cylinders and cones. A 2.0 elbow swept along many short straight segments becomes one arc.
- `OpenGeometry.exportIfcBody(body)` returns a solid's IFC exchange body: the same version 2 object as 2.0.18's `prepareIfcExchange()`, in world metres with Z up, read-only. The entry exports its types `AnalyticExchangeBodyV2` and `IfcPreparedValue`.
- `OG_OPERATION_SWEEP` takes a closed path: a closed `OG_PRIMITIVE_POLYLINE`, a closed `OG_PRIMITIVE_PROFILE` or an `OG_PRIMITIVE_CIRCLE`. The sweep closes into a ring-shaped `Solid` with no end faces; its last joint is mitred like the others, and a full circle path sweeps as two bends.
- The 2D region booleans are back as `OpenGeometry.booleanRegions2D(a, b, operation)` and `OpenGeometry.booleanCurvedRegions2D(a, b, operation)`, with `operation` one of `'union'`, `'intersection'` or `'subtraction'`. They take and return the 2.0 shapes, now exported as types: `OffsetRegion`, an `outer` ring and `holes` of `{ x, y, z }` points all at one height, outer rings clockwise in the XZ plane and holes counter-clockwise; and `CurvedRegion2D`, rings of `CurvedRegionEdge`, `{ kind: 'line', from, to }` or `{ kind: 'arc', center, radius, start_angle, sweep_angle }` with angles in radians. Results come back at the input height, wound the same way, each planar ring starting at its smallest x. The tolerance is fixed at `OG_REGION_TOLERANCE`, 1e-6 m, as in 2.0.
- An arched opening is one cutter: extrude a closed `OG_PRIMITIVE_PROFILE` (jambs, sill and an arch) across a curved or straight wall and subtract it with `wall.operate(OG_OPERATION_SUBTRACT, { tools: [arch] })`. 2.0 needed a box and a half-cylinder for it, and a curved wall then failed.
- An arched door from the floor is one cutter too: when its sill is at or below the wall's bottom face, the subtraction cuts a notch out of the wall instead of a hole. A cutter through both the floor and the top of the wall still throws `CoverageGap`.
- The 2D offsets are back as statics. `OpenGeometry.offsetRingVariable(ring, distances, holes?)` insets a closed ring by one distance per edge, each hole grown by its own distances. `OpenGeometry.offsetPolylineRegions(path, width, closed, miterLimit?)` strokes a path, with mitres up to `miterLimit` (default 4) and bevels past it. Both take 2.0's flat `[x, y, z, ...]` lists or a `Float64Array` and return `CurvedRegion2D[]`, exact line and arc edges in XZ. `OpenGeometry.offsetPolyline(path, distance, options?)` replaces 2.0's `polyline.getOffset(distance, acuteThresholdDegrees, bevel)`. It returns `OffsetPath2D`, `{ edges, bevelled, closed }`, and its options are `{ acuteThresholdDegrees?, bevel?, closed? }`, with defaults 35 and `true`. When `closed` is left out, a path whose ends meet is closed. The entry exports the types `OffsetHole`, `OffsetPath2D` and `OffsetPolylineOptions`.

### Before and after

2.0:

```js
import { AnalyticSolid, Arc, Vector3 } from 'opengeometry';
const sector = new AnalyticSolid({
  kind: 'arcEdgedExtrusion',
  outer: [
    { kind: 'line', from: [1, 0], to: [2, 0] },
    { kind: 'arc', center: [0, 0], radius: 2, startAngle: 0, sweepAngle: Math.PI / 2 },
    { kind: 'line', from: [0, 2], to: [0, 1] },
    { kind: 'arc', center: [0, 0], radius: 1, startAngle: Math.PI / 2, sweepAngle: -Math.PI / 2 },
  ],
  height: 3,
});
const arc = new Arc({ center: new Vector3(0, 0, 0), radius: 2, startAngle: 0, sweepAngle: Math.PI / 2 });
const wedge = new AnalyticSolid({ kind: 'cylinderSector', radius: 2, height: 3, startAngle: 0, sweepAngle: Math.PI / 2 });
const roof = new AnalyticSolid({
  kind: 'planarPolyhedron',
  vertices: [[0, 0, 0], [4, 0, 0], [4, 0, 2], [0, 0, 2], [1, 1, 1], [3, 1, 1]],
  faces: [[0, 1, 2, 3], [1, 0, 4, 5], [2, 1, 5], [3, 2, 5, 4], [0, 3, 4]],
});
```

3.0:

```ts
import {
  OpenGeometry,
  Solid,
  Wire,
  OG_OPERATION_EXTRUDE,
  OG_PRIMITIVE_ARC,
  OG_PRIMITIVE_CYLINDER_SECTOR,
  OG_PRIMITIVE_PARAMS_ARC,
  OG_PRIMITIVE_PARAMS_CYLINDER_SECTOR,
  OG_PRIMITIVE_PARAMS_POLYHEDRON,
  OG_PRIMITIVE_POLYHEDRON,
  OG_PRIMITIVE_PARAMS_PROFILE,
  OG_PRIMITIVE_PROFILE,
} from 'opengeometry';

await OpenGeometry.create({ wasmURL: '/opengeometry_bg.wasm' }, { workerURL: '/tessellation-worker.js' });

const profile = new Wire(OG_PRIMITIVE_PROFILE, OG_PRIMITIVE_PARAMS_PROFILE({
  start: [1, 0],
  segments: [
    { to: [2, 0] },
    { to: [0, 2], arc: { center: [0, 0], counterClockwise: true } },
    { to: [0, 1] },
    { to: [1, 0], arc: { center: [0, 0], counterClockwise: false } },
  ],
  closed: true,
}));
const sector = new Solid(OG_OPERATION_EXTRUDE, { profile, distance: 3 });

const arc = new Wire(OG_PRIMITIVE_ARC, OG_PRIMITIVE_PARAMS_ARC({ radius: 2, startAngle: 0, sweepAngle: Math.PI / 2 }));

const wedge = new Solid(
  OG_PRIMITIVE_CYLINDER_SECTOR,
  OG_PRIMITIVE_PARAMS_CYLINDER_SECTOR({ radius: 2, height: 3, startAngle: 0, sweepAngle: Math.PI / 2 }),
);

const roof = new Solid(OG_PRIMITIVE_POLYHEDRON, OG_PRIMITIVE_PARAMS_POLYHEDRON({
  points: [[0, 0, 0], [4, 0, 0], [4, 0, 2], [0, 0, 2], [1, 1, 1], [3, 1, 1]],
  faces: [[0, 1, 2, 3], [1, 0, 4, 5], [2, 1, 5], [3, 2, 5, 4], [0, 3, 4]],
}));
```

Both make the same quarter ring sector, 3 m high, the same quarter arc, the same quarter cylinder and the same hip roof.

2.0:

```js
import { AnalyticSolid, Vector3 } from 'opengeometry';
const world = AnalyticSolid.fromBrep(solid.getWorldBrepSerialized());
const normal = solid.normalAtFace(faceId, new Vector3(0, 0, 0));
```

3.0:

```ts
import * as THREE from 'three';
import { OpenGeometry, Solid, OG_PRIMITIVE_CUBOID, OG_TRANSFORM_ROTATE } from 'opengeometry';

await OpenGeometry.create({ wasmURL: '/opengeometry_bg.wasm' }, { workerURL: '/tessellation-worker.js' });

const solid = new Solid(OG_PRIMITIVE_CUBOID, { width: 2, height: 2, depth: 2 });
solid.transform(OG_TRANSFORM_ROTATE, { axis: [0, 0, 1], degrees: 30 });
OpenGeometry.flush();

const world = solid.getWorldBrep();
const bottom = solid.getBrep().topology.faces.find((face) => face.key === 'bottom');
if (bottom) {
  const local = solid.faceNormalAt(bottom.id, [0, 0, 0]);
  const worldNormal = new THREE.Vector3(...local).transformDirection(solid.matrixWorld);
  console.log(world.topology.vertices.length, local, worldNormal.toArray());
}
```

Both read the box's world vertices without making a body. The 3.0 normal is `[0, -1, 0]` in the box's own coordinates; `worldNormal` is that normal turned by the box's 30 degree rotation, about `[0.5, -0.866, 0]`. `OpenGeometry.flush()` brings `matrixWorld` up to date with the transform.

2.0:

```js
import { Sweep, Vector3 } from 'opengeometry';
const duct = new Sweep({
  path: [new Vector3(0, 0, 0), new Vector3(0, 3, 0), new Vector3(3, 3, 0)],
  profile: [new Vector3(-0.35, 0, -0.2), new Vector3(0.35, 0, -0.2), new Vector3(0.35, 0, 0.2), new Vector3(-0.35, 0, 0.2)],
  color: 0x2a9d8f,
  capStart: false,
  capEnd: false,
});
```

3.0:

```ts
import {
  OpenGeometry,
  Sheet,
  Wire,
  OG_OPERATION_PARAMS_SWEEP,
  OG_OPERATION_SWEEP,
  OG_PRIMITIVE_POLYLINE,
  OG_PRIMITIVE_RECTANGLE,
} from 'opengeometry';

await OpenGeometry.create({ wasmURL: '/opengeometry_bg.wasm' }, { workerURL: '/tessellation-worker.js' });

const profile = new Wire(OG_PRIMITIVE_RECTANGLE, { width: 0.7, breadth: 0.4 });
const path = new Wire(OG_PRIMITIVE_POLYLINE, { points: [[0, 0, 0], [0, 3, 0], [3, 3, 0]] });
const duct = new Sheet(OG_OPERATION_SWEEP, OG_OPERATION_PARAMS_SWEEP({ profile, path, caps: false }));
console.log(duct.bodyType, duct.getBrep().topology.faces.length);
```

Both make the same open duct, 0.7 m by 0.4 m, along an L-shaped path: eight side faces and no end faces. The 3.0 line prints `Sheet 8`.

2.0:

```js
import { Sweep, Vector3 } from 'opengeometry';
const steps = [0, 1, 2, 3, 4, 5, 6, 7, 8].map((step) => (step * Math.PI) / 16);
const elbow = new Sweep({
  path: steps.map((angle) => new Vector3(0.6 * Math.cos(angle), 0, -0.6 * Math.sin(angle))),
  profile: [new Vector3(0.4, -0.125, 0), new Vector3(0.8, -0.125, 0), new Vector3(0.8, 0.125, 0), new Vector3(0.4, 0.125, 0)],
});
```

3.0:

```ts
import {
  OpenGeometry,
  Solid,
  Wire,
  OG_OPERATION_SWEEP,
  OG_PRIMITIVE_ARC,
  OG_PRIMITIVE_CIRCLE,
  OG_PRIMITIVE_PARAMS_ARC,
  OG_PRIMITIVE_RECTANGLE,
} from 'opengeometry';

await OpenGeometry.create({ wasmURL: '/opengeometry_bg.wasm' }, { workerURL: '/tessellation-worker.js' });

const ductPath = new Wire(OG_PRIMITIVE_ARC, OG_PRIMITIVE_PARAMS_ARC({ radius: 0.6, startAngle: 0, sweepAngle: Math.PI / 2 }));
const ductProfile = new Wire(
  OG_PRIMITIVE_RECTANGLE,
  { width: 0.4, breadth: 0.25 },
  { plane: { origin: [0.6, 0, 0], normal: [0, 0, -1], xDirection: [1, 0, 0] } },
);
const duct = new Solid(OG_OPERATION_SWEEP, { profile: ductProfile, path: ductPath });

const pipePath = new Wire(OG_PRIMITIVE_ARC, OG_PRIMITIVE_PARAMS_ARC({ radius: 0.3, startAngle: 0, sweepAngle: Math.PI / 2 }));
const pipeProfile = new Wire(
  OG_PRIMITIVE_CIRCLE,
  { radius: 0.1 },
  { plane: { origin: [0.3, 0, 0], normal: [0, 0, -1], xDirection: [1, 0, 0] } },
);
const pipe = new Solid(OG_OPERATION_SWEEP, { profile: pipeProfile, path: pipePath });

const { report } = await OpenGeometry.exportStep({ nodes: [duct, pipe] });
console.log(duct.getBrep().topology.faces.length, pipe.getBrep().topology.faces.length, report.pcurvelessEdges);
```

Both follow a quarter turn at a bend radius of 0.6 m, one and a half times the 0.4 m duct width. 2.0 can only follow the arc with straight pieces, here eight. 3.0 makes one exact bend with two cylindrical walls and prints `6 4 0`; the pipe elbow has two torus faces.

2.0:

```js
const body = solid.prepareIfcExchange();
```

3.0:

```ts
import { OpenGeometry, Solid, OG_PRIMITIVE_CUBOID, OG_TRANSFORM_TRANSLATE, type AnalyticExchangeBodyV2 } from 'opengeometry';

await OpenGeometry.create({ wasmURL: '/opengeometry_bg.wasm' }, { workerURL: '/tessellation-worker.js' });

const solid = new Solid(OG_PRIMITIVE_CUBOID, { width: 2, height: 3, depth: 0.2 });
solid.transform(OG_TRANSFORM_TRANSLATE, { offset: [4, 0, 1] });
const body: AnalyticExchangeBodyV2 = OpenGeometry.exportIfcBody(solid);
console.log(body.representation_type, body.up_axis, body.solids.length, body.faces.length);
```

Both give the solid's exchange body in world metres with Z up. The 3.0 line prints `AdvancedBrep Z 1 6`.

2.0:

```js
const left = booleanRegions2D([square(0)], [square(0.5)], 'subtraction');
```

3.0:

```ts
import { OpenGeometry, OG_REGION_TOLERANCE, type OffsetRegion } from 'opengeometry';

await OpenGeometry.create({ wasmURL: '/opengeometry_bg.wasm' }, { workerURL: '/tessellation-worker.js' });

const square = (x: number): OffsetRegion => ({
  outer: [{ x, y: 0, z: 0 }, { x, y: 0, z: 1 }, { x: x + 1, y: 0, z: 1 }, { x: x + 1, y: 0, z: 0 }],
  holes: [],
});
const left: OffsetRegion[] = OpenGeometry.booleanRegions2D([square(0)], [square(0.5)], 'subtraction');
console.log(left.length, left[0]?.outer.length, OG_REGION_TOLERANCE);
```

Both cut one unit square by another moved half a metre along x. The 3.0 line prints `1 4 0.000001`.

2.0:

```js
const inset = offsetRingVariable(lot, [2, 2, 2, 2, 2, 2]);
const points = inset[0].outer.map((point) => [point.x, point.z]);
const offset = polyline.getOffset(1, 35, true);
```

3.0:

```ts
import { OpenGeometry, type CurvedRegion2D, type OffsetPath2D } from 'opengeometry';

await OpenGeometry.create({ wasmURL: '/opengeometry_bg.wasm' }, { workerURL: '/tessellation-worker.js' });

const flat = (points: [number, number][]): number[] => points.flatMap(([x, z]) => [x, 0, z]);
const lot = flat([[0, 0], [10, 0], [10, 5], [5, 5], [5, 10], [0, 10]]);
const inset: CurvedRegion2D[] = OpenGeometry.offsetRingVariable(lot, [2, 2, 2, 2, 2, 2]);
const corner = inset[0]?.outer.find((edge) => edge.kind === 'arc');
const duct: CurvedRegion2D[] = OpenGeometry.offsetPolylineRegions(flat([[0, 0], [4, 0], [4, 3]]), 0.3, false);
const side: OffsetPath2D = OpenGeometry.offsetPolyline(flat([[0, 0], [5, 0]]), 1);
console.log(inset.length, corner?.kind === 'arc' ? corner.radius : 0, duct.length, side.edges.length);
```

Both inset an L-shaped lot by 2 m. 2.0 draws the inside corner as short segments outside the circle; the 3.0 line prints `1 2 1 1`: one region whose inside corner is one arc of radius 2, one duct region and one offset edge.

### Replace this with that

| 2.0 | 3.0 |
| --- | --- |
| `new AnalyticSolid({ kind: 'arcEdgedExtrusion', outer, holes, height, frame })` | one `new Wire(OG_PRIMITIVE_PROFILE, { start, segments, closed: true }, { plane })` per loop, then `new Solid(OG_OPERATION_EXTRUDE, { profile, holes, distance: height })` |
| `{ kind: 'line', from, to }` | `{ to }`; the first `from` becomes `start` |
| `{ kind: 'arc', center, radius, startAngle, sweepAngle }` | `{ to: its end point, arc: { center, counterClockwise: sweepAngle > 0 } }` |
| `frame` `{ origin, x, y, z }` | `plane` `{ origin, normal: z, xDirection: x }` |
| `new AnalyticSolid({ kind: 'cylinderSector', radius, height, startAngle, sweepAngle, frame })` | `new Solid(OG_PRIMITIVE_CYLINDER_SECTOR, { radius, height, startAngle, sweepAngle }, { plane })` |
| `new AnalyticSolid({ kind: 'planarPolyhedron', vertices, faces })` | `new Solid(OG_PRIMITIVE_POLYHEDRON, { points: vertices, faces })`; a 2.0 `frame` becomes `plane`, which places the body instead of mapping each vertex, so check the result |
| `new Arc({ center, radius, startAngle, sweepAngle })` | `new Wire(OG_PRIMITIVE_ARC, { radius, startAngle, sweepAngle }, { plane: { origin: [center.x, center.y, center.z] } })` |
| `AnalyticSolid.fromBrep(solid.getWorldBrepSerialized())`, to read a world copy | `body.getWorldBrep()`, which returns the BRep object and makes no body; for a copy you will change, `body.duplicate()` |
| `solid.normalAtFace(faceId, new Vector3(x, y, z))` | `body.faceNormalAt(faceId, [x, y, z])`, which returns `[x, y, z]` |
| `new Sweep({ path, profile, capStart: false, capEnd: false })` | `new Sheet(OG_OPERATION_SWEEP, { profile, path, caps: false })`, with the profile and path as wires (see the 2.0 to 2.5 sweep rows) |
| `new Sweep({ path, profile, capStart: true, capEnd: true })` | `new Solid(OG_OPERATION_SWEEP, { profile, path })` |
| `new Sweep({ path: points along an arc, profile })` | `new Solid(OG_OPERATION_SWEEP, { profile, path })` with `path` an `OG_PRIMITIVE_ARC` wire, or an open `OG_PRIMITIVE_PROFILE` whose arc segments meet its lines tangentially |
| `solid.prepareIfcExchange()` | `OpenGeometry.exportIfcBody(solid)` |
| `import type { AnalyticExchangeBodyV2, IfcPreparedValue } from 'opengeometry'` | unchanged |
| `booleanRegions2D(a, b, operation)` | `OpenGeometry.booleanRegions2D(a, b, operation)` |
| `booleanCurvedRegions2D(a, b, operation)` | `OpenGeometry.booleanCurvedRegions2D(a, b, operation)` |
| `import type { OffsetRegion, CurvedRegion2D, CurvedRegionEdge } from 'opengeometry'` | unchanged |
| `offsetRingVariable(ring, distances, holes)` | `OpenGeometry.offsetRingVariable(ring, distances, holes)`; read the result as line and arc edges, not `{ x, y, z }` points |
| `offsetPolylineRegions(centreline, width, closed, miterLimit)` | `OpenGeometry.offsetPolylineRegions(path, width, closed, miterLimit)` |
| `polyline.getOffset(distance, acuteThresholdDegrees, bevel)` | `OpenGeometry.offsetPolyline(path, distance, { acuteThresholdDegrees, bevel })`; `points` become the ends of `edges`, `beveledVertexIndices` becomes `bevelled` and `isClosed` becomes `closed` |
| `booleanRegions2D(inset, zones, operation)` on an inset | `OpenGeometry.booleanCurvedRegions2D(inset, zones, operation)`, with the zones as `CurvedRegion2D` |
| `offsetPolylineGroupRegions(polylines)` | stroke each polyline with `OpenGeometry.offsetPolylineRegions` and unite them with `OpenGeometry.booleanCurvedRegions2D(a, b, 'union')` |
| `line.getOffset(distance, acuteThresholdDegrees, bevel)` | `OpenGeometry.offsetPolyline([start.x, start.y, start.z, end.x, end.y, end.z], distance, { acuteThresholdDegrees, bevel })` |
| `rectangle.getOffset(distance, acuteThresholdDegrees, bevel)` | `OpenGeometry.offsetPolyline(corners, distance, { acuteThresholdDegrees, bevel, closed: true })`, with `corners` the four corners in the order 2.0 drew them, from `(-width / 2, -breadth / 2)` round to `(-width / 2, breadth / 2)` about the centre |
| `import type { OffsetPolyline } from 'opengeometry'` | gone with `offsetPolylineGroupRegions`; a polyline's `centreline`, `width` and `closed` are the arguments of `OpenGeometry.offsetPolylineRegions` |
| `import type { OffsetRegionPoint } from 'opengeometry'` | gone; the offset calls return `CurvedRegion2D` edges, and an `OffsetRegion` point is `{ x, y, z }` |
| `import type { OffsetHole } from 'opengeometry'` | unchanged |

### Different behaviour

- Some curved subtractions that threw `UnresolvedIntersection` now give a body, for example a round or arched opening through a curved wall; others now throw `CoverageGap` or a STEP export error instead.
- A hole that encloses the whole profile throws `OGError` `InvalidParameter`. It used to fail as `InvalidGeometry`.
- A circular hole away from the profile centre now extrudes. 2.0 refused some of these.
- A Profile refuses repeated points, arcs whose end is not on their circle, self-crossing segments and a closed profile that does not end at its start, with `InvalidParameter`. An Arc refuses a zero sweep and a full turn; use `OG_PRIMITIVE_CIRCLE` for a full circle.
- `faceNormalAt` takes and returns the body's own coordinates, those of `getBrep()`. 2.0 code that read the normal from a world copy got a world normal; now turn the result into the world yourself, for example with `transformDirection(body.matrixWorld)` after `OpenGeometry.flush()`. Do not pass a point read from `getWorldBrep()`.
- `getWorldBrep()` places the BRep by the kernel's world placement, not by a Three.js parent. Its `id` and `revision` equal those of `getBrep()`; the 2.0 copy had the revision plus one. A scaled body's tolerances and accuracy are scaled too. An unmoved body returns exactly the `getBrep()` result.
- `getWorldBrep()` throws `LimitExceeded` for a body placed so far from the origin that its coordinates lose precision, as 2.0 did; `getBrep()` still works.
- `faceNormalAt` throws `InvalidParameter` for a face id that is not an unsigned integer or not a face of the body, and for a point that is not three finite numbers. A point on a cylinder axis, a cone apex or a sphere centre throws `InvalidGeometry`. A point off the face is projected onto it, as in 2.0.
- A cylinder sector refuses a zero, non-finite or full-turn sweep with `InvalidParameter`; 2.0 turned a full turn into a cylinder. Use `OG_PRIMITIVE_CYLINDER` for that.
- `caps` opens both ends or neither. A 2.0 sweep with one cap, such as `capStart: true, capEnd: false`, has no form here.
- `new Solid(OG_OPERATION_SWEEP, { ..., caps: false })` throws `BodyTypeMismatch` and makes no body; use `Sheet`. `new Sheet` with a primitive or a capped sweep throws `BodyTypeMismatch` too.
- A Sheet cannot be exported: `OpenGeometry.exportStep` with a Sheet in `nodes` rejects with `InvalidOperand`, and a Sheet inside an exported assembly is skipped and listed in the report.
- `rebuild` keeps the body type: a Sheet rebuilt with `caps` true, or a Solid rebuilt with `caps: false`, throws `BodyTypeMismatch`.
- Sheets get their own default ids, `sheet-1`, `sheet-2` and so on, and show their outline by default.
- A polyhedron refuses, with `InvalidParameter`, non-finite or coincident points (closer than four times the geometric tolerance), points no face uses, bad or repeated indices in a ring, and a face whose points are off its plane by more than the geometric tolerance. It refuses, with `InvalidTopology`, an edge not used by exactly two faces in opposite directions (an open shell or a T-junction), a vertex where the faces do not form one fan, faces that form more than one shell, and inward winding. 2.0 accepted inward winding, unused points and coincident points. A shell that encloses no volume, such as a flat one that folds over itself, is also refused with `InvalidTopology`.
- A polyhedron does not check whether faces cross each other, or whether a ring touches itself at a vertex; build such shapes from separate solids.
- `exportIfcBody` places the body by the kernel's world placement, not by a Three.js parent. Its `revision` is the shape's revision; the 2.0 body of a moved solid had the revision plus one. The entities, faces and numbers are the same as 2.0.18 gave for the same placed body.
- `exportIfcBody` throws `InvalidOperand` for a Sheet, a Wire or an assembly, and `Disposed` for a disposed body. It throws `LimitExceeded`, as 2.0.18 did, for more than about 100,000 entities or for fitted curves beyond the exchange error budget, and `UnsupportedGeometry` for a circle or ellipse edge longer than one turn.
- 2.0's `solid.exportIfc()`, which also wrote the IFC file text, has no form here. Write the file from the exchange body's `entities`.
- A planar Boolean whose 2D outline cannot close, or that leaves a hole outside every region, throws `OGError` `InvalidTopology`, as 2.0.18 does. 2.0.17 and earlier returned a body.
- An arc in a sweep path must meet its neighbours tangentially, within 1e-9 radians. A corner at an arc throws `InvalidParameter`; a corner between two lines is still mitred.
- Each bend radius must exceed the profile's reach from the path start, the largest distance from the start to the profile, by four geometric tolerances. A tighter bend throws `SweepSelfIntersection`.
- An arc above half a turn sweeps as equal bends of at most half a turn each, and each bend has its own `seg-` face keys.
- A closed path must bring the profile back to where it started. A path that does not lie in one plane can turn the profile on the way round; it throws `InvalidParameter`.
- A closed path has no ends, so `caps: false` with a closed path throws `InvalidParameter`.
- The overlap rule is unchanged: two path segments that are not neighbours must stay more than twice the reach apart. A tight line, bend, line path can still throw `SweepSelfIntersection` although the bend keeps its two lines apart.
- A Boolean of solids with circular-arc sides (arc extrusions or cylinders) that leaves a hole outside every region throws `OGError` `InvalidTopology`. 2.0 dropped the hole and returned a body.
- The 2D region booleans throw an `OGError` instead of a string; `String(error)` still contains the 2.0 text. In both, an unknown operation or an operand that is not a list throws `InvalidParameter`. In `booleanRegions2D`, bad input, such as a ring with fewer than three points, points at two heights, wrong winding or an unknown field on a region or a point, throws `InvalidParameter`; 2.0 dropped unknown fields. In `booleanCurvedRegions2D`, an unknown field throws `InvalidParameter`, and input that fails its checks, such as wrong winding, a ring that does not close or an invalid arc, throws `UnresolvedIntersection`. A result with an open boundary or a hole outside every region throws `InvalidTopology`, also from `booleanCurvedRegions2D`, which in 2.0 dropped such a hole.
- A 2D region boolean with more than 100,000 points or edges across both operands throws `LimitExceeded`. 2.0 had no limit.
- Offset corners are exact. An inset's inside corner, and each corner of a grown hole, is one arc of the setback round the corner. 2.0 drew those corners as segments outside the circle, up to 0.86 % deeper than the setback.
- The offset region calls return `CurvedRegion2D` edges in XZ. 2.0 returned `OffsetRegion` points that kept the input `y`; the `y` of the input is now ignored.
- `offsetRingVariable` throws `InvalidTopology` for an outer ring that crosses itself, such as a bowtie; 2.0 returned an empty list. It throws `InvalidTopology` for a degenerate hole, and `InvalidParameter` for a distance count that does not match the edges or a negative or non-finite distance; 2.0 threw a string for these. A collapsed inset is still an empty list.
- `offsetPolylineRegions` throws `InvalidParameter` for a width that is not finite; 2.0 returned an empty list. A width of 4e-6 m or less, or a path of one point, still gives an empty list, and a miter limit that is not finite and positive still means 4.
- Some strokes now have their true area where 2.0 was wrong. A closed loop wider than its sides is solid: a 1 m square loop at width 1.2 covers 4.84 m², where 2.0 left a 0.2 m hole (4.8 m²). A U whose legs are shorter than its width covers 33 m² for a 10 m by 1 m U at width 2; 2.0 returned a ring that crosses itself (42 m²). An open path that crosses itself covers each place once, with a hole where it encloses space; 2.0 dropped or added pieces, and the mirrored path gave a different answer.
- The offset calls throw `InvalidParameter` before the kernel for input of the wrong shape: a ring, path or distance list that is not a list of numbers, holes that are not a list of `{ ring, distances }`, an unknown hole or option field, a width, miter limit, distance or threshold that is not a number, or a `closed` or `bevel` that is not `true` or `false`. More than 100,000 points throws `LimitExceeded`; 2.0 had no limit.

### Check your result

1. `profile.getBrep()` has one edge per segment, and the extruded solid has one cylindrical face per arc.
2. The `text` of `await OpenGeometry.exportStep({ nodes: [sector] })` has one `CYLINDRICAL_SURFACE` per arc.
3. For an unmoved body, `getWorldBrep()` equals `getBrep()`; for a moved one its vertices are the `getBrep()` vertices moved by `matrixWorld`.
4. Each `faceNormalAt` result has length 1 and points out of the body in its own coordinates.
5. An open-ended sweep's `getBrep()` has no faces whose `key` starts with `cap-`, and `solids` is empty.
6. A duct elbow along one arc has six faces, two of them `Cylinder`; a pipe elbow has two `Torus` faces. Its STEP report has `pcurvelessEdges` 0.
7. `OpenGeometry.exportIfcBody(solid)` has `schema_version` 2, `length_unit` `'metre'`, `up_axis` `'Z'` and `coordinate_space` `'world'`, and a solid with a cavity has `representation_type` `'CSG'`.
8. A sweep along a closed path has no faces whose `key` starts with `cap-`, and its `bodyType` is `'Solid'`.
9. Each ring that `OpenGeometry.booleanRegions2D` returns has its points at the input height, a negative XZ area for an outer ring and a positive one for a hole.
10. Each arc that `OpenGeometry.offsetRingVariable` returns has a radius equal to one of the distances and its centre on a corner of the ring or of a hole.

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

1. Open `node_modules/opengeometry/package.json`. Its `version` starts with `3.`.
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
