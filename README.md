<p align="center">
  <a href="https://opengeometry.io?utm_source=github">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/OpenGeometry-io/OpenGeometry/main/docs/logo/logo-dark.png" />
      <img src="./docs/logo/logo-light.png" alt="OpenGeometry" width="420" />
    </picture>
  </a>
</p>

<h1 align="center">OpenGeometry</h1>

<p align="center">
  <strong>Open-source CAD kernel for the web: exact B-rep solids, booleans, extrude, sweep and revolve, and STEP export, in Rust and WebAssembly with a TypeScript SDK for Three.js. For product configurators, mechanical parts, buildings, robotics and AI agents that model.</strong>
</p>

<p align="center">
  <a href="https://www.npmjs.com/package/opengeometry"><img src="https://img.shields.io/npm/v/opengeometry?style=flat-square&color=4460FF&label=npm" alt="npm version" /></a>
  <a href="https://github.com/OpenGeometry-io/OpenGeometry/blob/main/LICENSE.md"><img src="https://img.shields.io/github/license/opengeometry-io/opengeometry?style=flat-square" alt="License" /></a>
  <a href="https://discord.gg/9wJpbfgGGA"><img src="https://img.shields.io/badge/Discord-Join%20us-5865F2?style=flat-square&logo=discord&logoColor=white" alt="Discord" /></a>
  <a href="https://x.com/openGeometry"><img src="https://img.shields.io/badge/Twitter-Follow-1DA1F2?style=flat-square&logo=x&logoColor=white" alt="Twitter" /></a>
  <a href="https://linkedin.com/company/openGeometry"><img src="https://img.shields.io/badge/LinkedIn-Connect-0A66C2?style=flat-square&logo=linkedin&logoColor=white" alt="LinkedIn" /></a>
</p>

<p align="center">
  <a href="https://opengeometry.io?utm_source=github">Website</a> · <a href="https://docs.opengeometry.io?utm_source=github">Documentation</a> · <a href="https://demos.opengeometry.io?utm_source=github">Live Demos</a> · <a href="https://blog.opengeometry.io?utm_source=github">Blog</a> · <a href="https://www.npmjs.com/package/opengeometry">npm</a>
</p>

---

> **Actively maintained and growing.** We're building OpenGeometry in the open. APIs, examples, and package structure are evolving, and we are actively improving and expanding the project. Star the repo to follow along. If you have questions or want to get involved, join the [Discord](https://discord.com/invite/9wJpbfgGGA) or check out the [issues](https://github.com/OpenGeometry-io/OpenGeometry/issues).

> **Version note.** This README describes OpenGeometry 3.0.0. Check which line you have installed with `npm ls opengeometry`: the 2.0 line has a different API. The documentation at [docs.opengeometry.io](https://docs.opengeometry.io?utm_source=github) describes 3.0.

---

## What is OpenGeometry?

OpenGeometry is an **open-source CAD kernel for the web**. It models **B-rep solids** with exact planes, cylinders, spheres, cones and tori, runs **boolean operations** (union, subtract, intersect), builds solids by **extrude, sweep and revolve**, hollows them with **shell**, and writes **STEP** files that other CAD and CAM tools open. The kernel is written in **Rust** and compiled to **WebAssembly**. A **TypeScript SDK for Three.js** puts every body straight into your scene, and the same calls run in **Node.js** with no browser.

It is the geometry engine, not a CAD application: you build the product, OpenGeometry does the solid modelling. The same calls give the same shape every time, so an app, a script or an AI agent can drive it and check the result with exact queries: volume, area, bounds, validity, and whether a point is inside.

## Use cases

| Field | What you build | What OpenGeometry gives you |
| --- | --- | --- |
| **Product configurators and e-commerce** | A shopper picks sizes and options and sees the exact part, then orders it | Parametric primitives that `rebuild` in place, booleans for cut-outs, STEP for the factory |
| **Mechanical parts and enclosures** | Plates, brackets, housings, flanges, pipe parts | Rounded boxes, slots, cylinders, pipe segments, through-holes in one call, shell for walls, STEP in millimetres |
| **Manufacturing and fabrication** | Web tools that turn an order into a production file | Exact B-rep, valid and closed solids, STEP export with an export report |
| **Architecture, construction and interiors** | Walls, openings, slabs, furniture, MEP parts | Polysolid walls along a path, extrude with holes, sweeps along arcs, the IFC exchange body for an IFC writer |
| **Robotics and automation** | Fixtures, grippers, end-of-arm tools, work-cell layouts | Assemblies with placements, point-inside tests, measured volume, picking down to face and edge |
| **AI agents and prompt-to-CAD** | An agent that turns a request into a part, then checks it | Plain JSON parameters, deterministic results, `validity()` and `measure()` to verify, typed errors with a `code` |
| **Education and geometry tools** | Teaching solids, booleans and B-rep in a browser tab | Open source, runs client-side, the B-rep of every body is readable |
| **Server-side generation** | A Node.js service that makes STEP files from parameters | The same API in Node, with inline tessellation and no browser |

Floorkit, a toolkit for building design, is one application built on OpenGeometry.

## Features

| Area | What 3.0 has |
| --- | --- |
| **Primitives** | Wires: rectangle, circle, polyline (open or closed), line-and-arc profile, arc, regular polygon, slot, rounded rectangle. Solids: cuboid, cylinder, cylinder sector, sphere, cone or frustum, ring torus, planar polyhedron, slot prism, rounded box, pipe segment, polysolid wall along a path of straight legs and tangent arcs. Each can be placed on any plane. |
| **Operations** | Extrude a closed wire into a solid, with holes. Sweep a profile along an open or closed path of lines and arcs, each arc an exact bend, capped into a solid, open-ended into a sheet, or closed into a ring. Revolve a closed line-and-arc profile a full turn about an axis in its plane. Shell a solid into a hollow one with walls of one thickness. Union, subtract and intersect, with one to a hundred tool solids in one call. |
| **Parametric edits** | `rebuild` gives an existing body new parameters in place. `mirror` rebuilds a body as its reflection across a plane. |
| **Assemblies** | Assemblies with parent and child placement; translate, rotate, scale and place; independent copies and shared instances. |
| **History** | Marks with rollback and release; transactions, with a dry-run option. |
| **Display for Three.js** | Every body is a `THREE.Group`. Level of detail that follows the camera, tessellation in a web worker or inline, picking down to face and edge, outlines, colour and opacity. |
| **Queries** | Bounds, placements, the B-rep of a body, that B-rep placed in the world, the normal of a face at a point, whether a point is inside, volume and area within a stated error bound, whether a body is valid and closed, and the report of its last boolean. |
| **Export** | STEP, in millimetres or metres, with Y or Z up, together with an export report. The IFC exchange body of a solid, in world metres with Z up, for an IFC writer. |

Not in 3.0, though 2.0 had them: IFC file text and STL export; projection; loft; triangulation as a public call; curves other than lines, circles, arcs and polylines, such as the elliptical arc; and the 2.0 classes such as `Cuboid`, `Polygon`, `Opening` and `Vector3`. 3.0 has no spline, no ellipse and no elliptical arc. Build a wedge as a five-face `OG_PRIMITIVE_POLYHEDRON`, and a 2.0 `Curve` as an `OG_PRIMITIVE_POLYLINE` wire through its points.

## Installation

```bash
npm install opengeometry three
```

- `three` is a peer dependency: versions `>=0.168.0 <0.185.0` are supported.
- The package is ES modules only, and its TypeScript types are included. They use the types of `three`, which `three` does not ship: in a TypeScript project, also run `npm install --save-dev @types/three`.
- The package ships `opengeometry_bg.wasm` (the kernel) and `tessellation-worker.js` (the display worker), and your page must be able to fetch both. With no arguments, `OpenGeometry.create()` looks for them next to the package's own `index.js`. If your bundler moves or rewrites `index.js` without the two files, that lookup fails, so the dependable way is the one the quick start uses: copy both files from `node_modules/opengeometry/` into the folder your site serves at its root (in Vite, the `public/` folder by default) and pass their URLs to `create`.

## Quick start

```ts
import * as THREE from 'three';
import {
  OpenGeometry, Solid,
  OG_OPERATION_SUBTRACT, OG_PRIMITIVE_CYLINDER, OG_PRIMITIVE_ROUNDED_BOX, OG_TRANSFORM_TRANSLATE,
} from 'opengeometry';

await OpenGeometry.create({ wasmURL: '/opengeometry_bg.wasm' }, { workerURL: '/tessellation-worker.js' });

const plate = new Solid(OG_PRIMITIVE_ROUNDED_BOX, { width: 0.12, height: 0.01, depth: 0.08, radius: 0.01 });
const holes = [[-0.045, -0.025], [0.045, -0.025], [-0.045, 0.025], [0.045, 0.025]].map(([x, z]) => {
  const hole = new Solid(OG_PRIMITIVE_CYLINDER, { radius: 0.004, height: 0.02 });
  hole.transform(OG_TRANSFORM_TRANSLATE, { offset: [x, -0.005, z] });
  return hole;
});
plate.operate(OG_OPERATION_SUBTRACT, { tools: holes });

const scene = new THREE.Scene();
scene.add(new THREE.HemisphereLight(0xffffff, 0x444444, 2), plate);
const camera = new THREE.PerspectiveCamera(50, innerWidth / innerHeight, 0.001, 10);
camera.position.set(0.12, 0.1, 0.14);
camera.lookAt(0, 0, 0);
const renderer = new THREE.WebGLRenderer({ antialias: true });
renderer.setSize(innerWidth, innerHeight);
document.body.append(renderer.domElement);
renderer.setAnimationLoop(() => renderer.render(scene, camera));

const { text, report } = await OpenGeometry.exportStep({ nodes: [plate], unit: 'millimetre', upAxis: 'Z' });
```

This loads the kernel and makes a 120 mm by 80 mm mounting plate, 10 mm thick, with rounded corners (model units are metres). It drills four 8 mm bolt holes through the plate with one boolean call: the four cylinders are the tools. The plate is a `THREE.Group`, so it goes into the scene like any other object and is drawn on every frame. The last line writes the plate as a STEP file in millimetres, ready for a CAD or CAM tool: `text` is the file and `report` counts what was written.

The two URLs assume both files were copied to the site root, as described under Installation. The block uses top-level `await`, so it must run as a module. If your build rejects top-level `await`, raise its target (the examples set Vite's `build.target` to `'esnext'`) or move the code after the imports into an `async` function and call it.

Runnable examples live in [`main/opengeometry-three/examples-vite/`](./main/opengeometry-three/examples-vite/). Shapes: cuboid, cylinder, slot prism, rounded box, pipe segment, polysolid wall, the regular polygon, slot and rounded rectangle wires, and a rounded section. Operations: booleans, an opening, a sweep, a sweep along an arc path, a curved profile united with a straight one, and STEP export. From a clone, run `npm --prefix main ci`, `npm run build` and then `npm run dev-example`.

## Demos

Interactive demos run the kernel in your browser, with nothing to install: **[demos.opengeometry.io](https://demos.opengeometry.io?utm_source=github)**.

## When not to use it

- You need a desktop CAD application. OpenGeometry is a library you embed.
- You only show meshes you already have. Plain Three.js is enough.
- You need splines, ellipses, lofts, fillets or STL export. 3.0 does not have them.

## For AI agents and coding assistants

OpenGeometry is built to be driven by code, including code an agent writes. Every call takes plain parameters, gives the same shape for the same input, and can be checked with `validity()`, `measure()`, `getBounds()` and `classifyPoint()`.

- **[llms.txt](https://docs.opengeometry.io/llms.txt)**: an index of every documentation page, in Markdown, for a model's context.
- **[Use a coding agent](https://docs.opengeometry.io/opengeometry/agents)**: the 3.0 rules that agents get wrong most often, and the sources to trust.
- **`index.d.ts`** in the installed package: every export and its types.
- The reference below: the whole public surface on one page.

### API reference on one page

This is a compact reference for writing OpenGeometry 3.0 code, by hand or with a coding assistant.

**Package.** `opengeometry` on npm. ES modules only, types included, peer dependency `three` `>=0.168.0 <0.185.0`.

**Which line is installed.** The 3.0 entry exports `OpenGeometry.create`, `Solid` and `Wire`. Code that uses `Cuboid`, `Vector3` or `setPlacement` is written for 2.0 and does not work with 3.0. The `version` in `node_modules/opengeometry/package.json` settles it.

**The whole public surface.** The entry exports `OpenGeometry`, `Solid`, `Sheet`, `Wire`, `SystemAssembly`, `OGMark`, `OGError` and the `OG_*` constants. Every point and vector is `[x, y, z]`. Model units are metres, and Y is up.

| Export | Call and parameters |
| --- | --- |
| `OpenGeometry.create` | `await OpenGeometry.create(input?, options?)`. `input`: `{ wasmURL?, wasmModule? }`. `options`: `{ workerURL?, tessellation?: 'worker' \| 'inline' }`. |
| `Wire` | `new Wire(kind, params, options?)`. `OG_PRIMITIVE_RECTANGLE` `{ width, breadth }`, `OG_PRIMITIVE_CIRCLE` `{ radius }`, `OG_PRIMITIVE_POLYLINE` `{ points, closed? }`, `OG_PRIMITIVE_REGULAR_POLYGON` `{ sides, radius }` (3 to 1024 corners on a circle about the `plane` origin, the first on the x direction), `OG_PRIMITIVE_SLOT` `{ length, width }` (the outline of `OG_PRIMITIVE_SLOT_PRISM`), `OG_PRIMITIVE_ROUNDED_RECTANGLE` `{ width, breadth, radius }` (a rectangle with rounded corners; `radius` below half the width and half the breadth). |
| Profile wire | `new Wire(OG_PRIMITIVE_PROFILE, { start, segments, closed }, options?)`. Points are `[u, v]` in `plane`; a segment is `{ to }` for a line or `{ to, arc: { center, counterClockwise } }` for an arc. A closed profile is an extrude profile or hole. |
| Arc wire | `new Wire(OG_PRIMITIVE_ARC, { radius, startAngle, sweepAngle }, options?)`. Angles in radians from the `plane` x direction, centred on the `plane` origin; the sweep is nonzero and less than one turn. |
| `Solid` | `new Solid(kind, params, options?)`. `OG_PRIMITIVE_CUBOID` `{ width, height, depth }`, `OG_PRIMITIVE_CYLINDER` `{ radius, height }`, `OG_PRIMITIVE_CYLINDER_SECTOR` `{ radius, height, startAngle, sweepAngle }` (radians from the `plane` x direction, counter-clockwise seen from the side the normal points to; sweep nonzero and less than one turn), `OG_PRIMITIVE_POLYHEDRON` `{ points, faces }` (planar faces as rings of point indices, counter-clockwise seen from outside), `OG_PRIMITIVE_SPHERE` `{ radius }` (centred on the `plane` origin), `OG_PRIMITIVE_CONE` `{ radiusBottom, radiusTop, height }` (standing on the `plane`; one radius may be zero, equal radii make a cylinder), `OG_PRIMITIVE_TORUS` `{ majorRadius, minorRadius }` (lying on the `plane`; minor below major), `OG_OPERATION_EXTRUDE` `{ profile, holes?, distance }`, `OG_OPERATION_SWEEP` `{ profile, path, caps? }`, `OG_OPERATION_REVOLVE` `{ profile, axis? }` (a full turn about `axis` `{ point: [u, v], direction: [du, dv] }` in the profile's `[u, v]`, by default its v axis), `OG_PRIMITIVE_SLOT_PRISM` `{ length, width, height }` (a stadium standing on the `plane`, centred on its origin; `length` between the arc centres along the x direction), `OG_PRIMITIVE_ROUNDED_BOX` `{ width, height, depth, radius }` (a cuboid with its four upright edges rounded; `radius` below half the width and half the depth), `OG_PRIMITIVE_PIPE_SEGMENT` `{ outerRadius, innerRadius, height, startAngle, sweepAngle }` (a ring sector placed and angled like the cylinder sector), `OG_PRIMITIVE_POLYSOLID` `{ path, width, height, justify? }` (a wall of constant width along `path`, a `{ start, segments, closed }` of straight legs and tangent arcs in the `plane`'s `[u, v]`, standing on the `plane` and rising along its normal; `justify` is `'centre'` (default), `'left'` or `'right'` of the path direction); `profile`, `holes` and `path` are wires. |
| `Sheet` | `new Sheet(OG_OPERATION_SWEEP, { profile, path, caps: false })`: an open-ended sweep, faces that enclose no volume. It shows its surface, from the front only, and its outline; `OpenGeometry.exportStep` refuses it. |
| Body options | `{ ogId?, parent?, appearance?, plane? }`. `plane` is `{ origin?, normal?, xDirection? }` and applies to primitives only; by default it is the ground plane through the origin, normal `[0, 1, 0]`, x direction `[1, 0, 0]`. |
| Booleans | `solid.operate(kind, { tools, instances? })` with `OG_OPERATION_SUBTRACT`, `OG_OPERATION_UNION` or `OG_OPERATION_INTERSECT`; `tools` is one to a hundred solids. |
| Shell | `solid.operate(OG_OPERATION_SHELL, { thickness, instances? })` hollows the solid in place and leaves walls `thickness` thick inside every face. |
| Transforms | `node.transform(kind, params)` on a body or an assembly: `OG_TRANSFORM_TRANSLATE` `{ offset }`, `OG_TRANSFORM_ROTATE` `{ axis, degrees, pivot? }`, `OG_TRANSFORM_SCALE` `{ factor, pivot? }`, `OG_TRANSFORM_PLACE` `{ origin?, xDirection?, normal?, scale? }`. `getPlacement()` and `getWorldPlacement()` return `{ origin, xDirection, normal, scale }`. |
| Edits | `body.rebuild(kind, params, { instances? })` with the kinds and parameters above; a wire stays a wire and a solid a solid. |
| Mirror | `body.mirror({ origin, normal }, { instances? })` rebuilds the shape as its reflection across the plane through `origin` with normal `normal`, both in the body's own coordinates (type `MirrorPlane`). The placement does not change. A placement cannot hold a reflection: a negative scale throws `InvalidTransform`. |
| Copies | `body.duplicate({ ogId?, parent? })` makes an independent copy; `body.instance({ ogId?, parent? })` shares the shape. `getInstanceCount()`, `makeUnique()`. |
| `SystemAssembly` | `new SystemAssembly({ ogId?, parent? })`. `addChild(children, { keepWorld? })`, `removeChild(child, { keepWorld? })`, `getChildren()`, `getParent()`, `getBounds()`, `transform`, `dispose()`. Bodies have the same tree methods. |
| Queries | `getBounds()` (`[minX, minY, minZ, maxX, maxY, maxZ]`, or `null`), `getBrep()`, `getWorldBrep()` (the BRep placed in the world), `faceNormalAt(faceId, [x, y, z])` (the face's unit normal in the body's own coordinates), `classifyPoint([x, y, z])` (`'Inside'`, `'Outside'`, `'Boundary'` or `'Unknown'`, in the body's own coordinates), `measure({ deflection? })` (`{ volume, area, errorBound, areaErrorBound, deflection }`; the volume is within `errorBound` of the exact one, the area within `areaErrorBound`), `validity()` (`{ valid, closed, bodyType, error? }`), `solid.getReport()` (the last boolean's report, or `null`). |
| Appearance | `appearance` in the options, or `body.setAppearance({ color?, opacity?, outline?, pickOutline?, deflection?, smoothAngle? })`. `smoothAngle` is in degrees, 20 by default: edges whose faces fold by less are left out of the outline and shaded smooth; `0` keeps every edge. |
| Display | `OpenGeometry.settled({ deflection? })` resolves to `{ failed }`; `OpenGeometry.flush({ geometry?: 'sync', bodies? })` (with `bodies`, meshes and places only those bodies now; hidden bodies, bodies with no shape and bodies in limbo (disposed while a mark is live) are skipped; a fully disposed body or one from a reset runtime throws `Disposed` before anything is meshed; `bodies` without `geometry: 'sync'`, a non-array `bodies` or an item that is not a Wire, Sheet or Solid throws `InvalidParameter`); `OpenGeometry.setDisplayDeflection(world?)`; `OpenGeometry.setCameraMotion(moving)`; `OpenGeometry.observeCamera(renderer, camera)` observes the first camera seen per renderer frame (`renderer.info.render.frame`) for level of detail; a `render()` nested inside an `onBeforeRender` (Reflector style) starts a new frame, and an `ArrayCamera` is observed through its first sub-camera (a host whose drawables are not kernel bodies, such as instanced or batched layers, calls it from their `onBeforeRender`); `OpenGeometry.releaseIdleGeometry()` disposes the display geometry cached for bodies not showing it and returns `{ records, bytes }`; `OpenGeometry.markDisplayed(bodies)` counts those bodies as displayed for the current render pass (a host that draws bodies through its own instanced or batched layers calls it from that layer's `onBeforeRender`; it works for a body hidden by layer, skips a body with `visible = false`, and validates `bodies` as `flush({ bodies })` does); `OpenGeometry.setHostDrawn(bodies, hostDrawn)` hands the drawing of those bodies to the host (`true`) or gives it back (`false`): while host-drawn, a body's `visible` is `false`, so three.js skips it and everything under it, and the kernel keeps the body's own visibility apart, so `markDisplayed`, a walk, `flush({ geometry: 'sync', bodies })`, level of detail and record swaps treat it as shown, and a dispose, a rollback or a reset changes that visibility instead of `visible`; giving it back sets `visible` to that visibility (a host calls it when a body joins or leaves its instanced or batched layers; do not set `visible` on a host-drawn body; it validates `bodies` as `flush({ bodies })` does, except that giving bodies back ignores a disposed or finalized body, and a `hostDrawn` that is not a boolean throws `InvalidParameter`); `OpenGeometry.hostGroup(bodies)` returns a `HostGroup` that stamps all its bodies at once: `group.drawn()` counts every member as displayed for the current render pass exactly as `markDisplayed` does, at a cost that does not grow with the number of members (a host calls it from each instanced or batched layer's `onBeforeRender` in place of `markDisplayed`), `group.set(bodies)` replaces the members and `group.dispose()` ends the group, after which `drawn()` does nothing and its members no longer count through it; a body may be in several groups, a hidden or host-drawn member keeps its own visibility and gets its geometry when it is shown again through `setHostDrawn` or a rollback, a member that is not host-drawn and has `visible` set to `true` later gets it when it is drawn or walked, a member disposed later leaves its groups, a group that is dropped without `dispose()` is ended when it is garbage collected, `hostGroup` and `set` validate `bodies` once as `markDisplayed` does, and `set` on a disposed group or one from a reset runtime throws `Disposed`. A body is displayed when it is drawn (its own `onBeforeRender` or `markDisplayed`) or walked by `updateMatrixWorld`; a draw installs new display geometry after the render, a walk at once. Each body dispatches `geometrychanged` after it installs new display geometry and `placementchanged` when a flush moves it (`BodyEventMap`); their listeners run synchronously, after the whole flush is applied or at the end of the render pass, and a listener that throws is reported through `OpenGeometry.on('error', …)` with the thrown value while the flush and the pass complete. |
| Picking | `OpenGeometry.resolveHit(intersection)` takes a Three.js raycast hit and returns `{ ogId, shapeRevision, faceId?, edgeId? }` or `undefined`. |
| `OGMark` | `const mark = OpenGeometry.mark()`, then `mark.rollback()` and `mark.release()`. `OpenGeometry.markStats()` returns `{ liveMarks, retainedRevisions }`. `OpenGeometry.transaction(fn, { dryRun? })` runs a synchronous `fn` and rolls back if it throws. |
| Events | `OpenGeometry.on(event, handler)` with `'geometry'`, `'warning'`, `'error'` or `'fatal'`; it returns a function that unsubscribes. |
| STEP | `await OpenGeometry.exportStep({ nodes, unit?, upAxis?, name?, timestamp? })` returns `{ text, report }`. `nodes`: bodies, assemblies or `ogId` strings. `unit`: `'millimetre'` (default) or `'metre'`; `upAxis`: `'Z'` (default) or `'Y'`. |
| IFC body | `OpenGeometry.exportIfcBody(solid)` returns the IFC exchange body (type `AnalyticExchangeBodyV2`): `schema_version` 2, world metres, Z up, `representation_type` `'AdvancedBrep'` or `'CSG'` for a solid with a cavity, `entities`, `solids` and `faces`. It refuses a Sheet, a Wire or an assembly. |
| 2D regions | `OpenGeometry.booleanRegions2D(a, b, operation)` and `OpenGeometry.booleanCurvedRegions2D(a, b, operation)` with `'union'`, `'intersection'` or `'subtraction'`. A region is `{ outer, holes }`: an outer ring and a list of hole rings, each ring of `{ x, y, z }` points at one height (type `OffsetRegion`) or of line and arc edges (type `CurvedRegion2D`). Outer rings run clockwise in XZ and holes counter-clockwise. A plain ring is not a region and throws `InvalidParameter`; the tolerance is `OG_REGION_TOLERANCE`. |
| 2D offsets | `OpenGeometry.offsetRingVariable(ring, distances, holes?)` insets a closed ring by one distance per edge, each hole `{ ring, distances }` grown by its own; `OpenGeometry.offsetCurvedRingVariable(region, distances, holeDistances?)` insets one `CurvedRegion2D` of lines and arcs by one distance (>= 0) per edge, one list per hole, each arc kept on its centre at its own distance and refused when that distance passes the centre; `OpenGeometry.offsetPolylineRegions(path, width, closed, miterLimit?)` strokes a path. Both return `CurvedRegion2D[]` with exact arcs. `OpenGeometry.insetCurvedRegion2D(region, distance)` insets one `CurvedRegion2D` by a distance above zero, holes grown by the same, and returns `CurvedRegion2D[]` of exact lines and arcs: each arc keeps its centre and a reflex corner rounds to an arc of the distance; an inset past the half width is an empty list. `OpenGeometry.offsetPolyline(path, distance, { acuteThresholdDegrees?, bevel?, closed? })` offsets a path to one side and returns `{ edges, bevelled, closed }` (type `OffsetPath2D`). Rings and paths are flat `[x, y, z, ...]` lists; `y` is ignored. |
| `OGError` | Failures throw an `OGError` with `code` (for example `InvalidParameter`, `SharedShape`, `Disposed`), `call`, `message` and `details`. |
| Reset | `OpenGeometry.reset()` frees every body and leaves an empty document. |
| Typed parameters | `OG_PRIMITIVE_PARAMS_<KIND>(params)`, `OG_OPERATION_PARAMS_EXTRUDE(params)`, `OG_OPERATION_PARAMS_SWEEP(params)` and `OG_OPERATION_PARAMS_REVOLVE(params)` return their argument unchanged; they only give the object a type. The entry exports the parameter types too, such as `ProfileParams` and `PolysolidParams`. |

**Rules that are easy to get wrong.**

- Await `OpenGeometry.create` once before creating any body.
- `workerURL` belongs to the second argument of `create`, not the first.
- Node has no `Worker` and cannot fetch a `file:` URL. Pass the compiled module instead; tessellation then runs inline:

  ```js
  import { readFileSync } from 'node:fs';
  import { OpenGeometry } from 'opengeometry';

  const wasm = readFileSync(new URL(import.meta.resolve('opengeometry/opengeometry_bg.wasm')));
  await OpenGeometry.create({ wasmModule: new WebAssembly.Module(wasm) });
  ```

- `operate` changes the target's shape once. The tools stay separate, live solids: moving or disposing a tool later does not change the target, and a tool is drawn only if you add it to the scene.
- `rebuild` replaces the shape, so an earlier `operate` result is gone; call `operate` again after it.
- A shape shared by instances cannot be edited one node at a time: pass `{ instances: 'all' }` or call `makeUnique()` first, or the call throws `SharedShape`.
- Move bodies with `transform`. Setting `body.position`, `rotation` or `scale` is reverted with a `PlacementOverwritten` warning.
- Add bodies to the Three.js scene yourself; a `SystemAssembly` is not a Three.js object.
- Surfaces use `MeshStandardMaterial`, so the scene needs a light.
- Rendering is what asks for geometry. Render in a loop, or render once, `await OpenGeometry.settled()`, and render again. `settled` returns at once if nothing has been rendered.
- In Node nothing renders, so nothing asks for geometry: add the body to a `THREE.Scene`, call `scene.updateMatrixWorld()`, then call `OpenGeometry.flush({ geometry: 'sync' })` or `await OpenGeometry.settled()`; the mesh is then in `body.surface.geometry`. After an edit, `OpenGeometry.flush({ geometry: 'sync', bodies: [body] })` meshes only that body and places its mesh, so the next frame shows it with no extra scene walk.
- STEP export defaults to millimetres and Z up. A wire cannot be exported on its own: the selection must contain a solid.
- To check that a body is a valid solid, call `body.validity()`: a valid closed solid gives `{ valid: true, closed: true, bodyType: 'Solid' }`.
- Call `dispose()` on bodies and assemblies you no longer need.
- Catch `OGError` and branch on `code`. After a `'fatal'` event the runtime refuses every call until `OpenGeometry.reset()`.

**What does not exist.** IFC file text, STL and PDF export, projection, loft, public triangulation, curves other than lines, circles, arcs and polylines, such as the elliptical arc, and the 2.0 classes. Do not call them. 3.0 has no spline, no ellipse and no elliptical arc. Build a wedge as a five-face `OG_PRIMITIVE_POLYHEDRON`, and a 2.0 `Curve` as an `OG_PRIMITIVE_POLYLINE` wire through its points.

**Where the truth is.** `index.d.ts` in the installed package lists every export and its types. The examples in [`main/opengeometry-three/examples-vite/`](./main/opengeometry-three/examples-vite/) are working 3.0 code, except that they import the built package by path (`../../../dist/index.js`) where an app imports from `'opengeometry'`.

## Documentation

The documentation at **[docs.opengeometry.io](https://docs.opengeometry.io?utm_source=github)** describes 3.0: a quickstart, concepts, guides and the API reference. What changed in each release is in [CHANGELOG.md](./CHANGELOG.md). To move code written for 2.0 to 3.0, follow [MIGRATION.md](./MIGRATION.md).

## Repository structure

| Path | What it holds |
| --- | --- |
| [`main/opengeometry/`](./main/opengeometry/) | The Rust kernel, built to WebAssembly |
| [`main/opengeometry-three/`](./main/opengeometry-three/) | The Three.js SDK, published as `opengeometry` |
| [`main/scripts/`](./main/scripts/) | Build, check, benchmark and snapshot scripts |
| [`docs/`](./docs/) | The source of the documentation site |

## Building from source

**Prerequisites:** Node.js 26 and npm, Rust 1.88.0 with the `wasm32-unknown-unknown` target (pinned in `main/rust-toolchain.toml`), and `wasm-pack`.

```bash
npm --prefix main ci
npm run build
npm run dev-example
npm test
npm run check
npm run check:full
```

- `npm --prefix main ci` installs the dependencies.
- `npm run build` builds the kernel to WebAssembly and the SDK, and writes the package to `main/dist/`.
- `npm run dev-example` serves the example pages on a local Vite server; build first.
- `npm test` runs the kernel's Rust tests.
- `npm run check` runs the gate: formatting, lints, type checks, source rules, the build, the Rust, WebAssembly and Node tests, and the performance budgets.
- `npm run check:full` runs the same gate plus the browser tests on three 0.168 and 0.184. The browser tests need Playwright's Chromium, installed with `npx playwright install chromium` in `main/` (on Linux, add `--with-deps` to install the system libraries, as CI does).

More in [developer.md](./developer.md).

## Built with OpenGeometry

If your project uses OpenGeometry, please add the badge to your README and link back to us. It helps other people find the project.

[![Built with OpenGeometry](https://img.shields.io/badge/built%20with-OpenGeometry-4460FF?style=flat-square)](https://opengeometry.io?utm_source=built-with-badge)

```md
[![Built with OpenGeometry](https://img.shields.io/badge/built%20with-OpenGeometry-4460FF?style=flat-square)](https://opengeometry.io?utm_source=built-with-badge)
```

Then tell us about it on [Discord](https://discord.com/invite/9wJpbfgGGA): we like to see what you build.

## Community

We'd love to have you involved — whether you're using OpenGeometry, building on it, or just curious.

- **[Discord](https://discord.com/invite/9wJpbfgGGA)** — Chat with the team and community
- **[Twitter / X](https://x.com/openGeometry)** — Updates and announcements
- **[LinkedIn](https://linkedin.com/company/openGeometry)** — Company updates
- **[GitHub Issues](https://github.com/OpenGeometry-io/OpenGeometry/issues)** — Bug reports and feature requests
- **[Blog](https://blog.opengeometry.io?utm_source=github)** — Deep dives and release notes

## Contributing

Contributions are welcome — check the [issues](https://github.com/OpenGeometry-io/OpenGeometry/issues) for good starting points or open a discussion on [Discord](https://discord.com/invite/9wJpbfgGGA). [developer.md](./developer.md) covers building, checks and releases.

## License

OpenGeometry is released under the Mozilla Public License 2.0 (MPL-2.0). See [LICENSE.md](./LICENSE.md).

---

<p align="center">
  <sub><em>"It is my land. Who would I be if I did not try to make it better?" — A Knight.</em></sub>
</p>
