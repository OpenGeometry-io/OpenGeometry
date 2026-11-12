# Changelog

All notable changes to the `opengeometry` package are in this file. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the package follows
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [3.0.0] - 2026-11-10

3.0 is a new API. Code written for 2.0 does not run without changes. Version 2.5 was not
published. Follow [MIGRATION.md](./MIGRATION.md): it gives every change with code before and after.

### Changed

- **Breaking:** bodies are made with three classes and a kind constant: `new Wire(kind, params)`,
  `new Solid(kind, params)` and `new Sheet(OG_OPERATION_SWEEP, params)`. The 2.0 classes
  (`Cuboid`, `Cylinder`, `Polygon`, `Opening`, `Sweep`, `AnalyticSolid` and the others) are gone.
  See "Replace this with that" in MIGRATION.md.
- **Breaking:** points and vectors are `[x, y, z]` arrays. `Vector3` is gone.
- **Breaking:** bodies move with `body.transform(kind, params)` and `OG_TRANSFORM_*`. Setting
  `position`, `rotation` or `scale` is reverted with a `PlacementOverwritten` warning.
- **Breaking:** start the kernel with `await OpenGeometry.create(input, options)` before the first
  body. `workerURL` goes in the second argument.
- **Breaking:** the 2D region and offset functions are statics of `OpenGeometry`, such as
  `OpenGeometry.booleanRegions2D` and `OpenGeometry.offsetRingVariable`. The offset calls return
  `CurvedRegion2D` line and arc edges, not `{ x, y, z }` points.
- **Breaking:** `solid.prepareIfcExchange()` is `OpenGeometry.exportIfcBody(solid)`.
- **Breaking:** failures throw `OGError` with a `code`, a `call`, a `message` and `details`.
- **Breaking:** `classifyPoint` takes a point in the body's own coordinates and returns
  `'Inside'`, `'Outside'`, `'Boundary'` or `'Unknown'`.
- **Breaking:** the shell hollows a solid in place:
  `solid.operate(OG_OPERATION_SHELL, { thickness })`. To keep the first solid, shell
  `solid.duplicate()`.
- **Breaking:** a body is displayed when it is drawn or walked by `updateMatrixWorld`. Its new
  mesh is installed after the render that draws it, so the next frame shows it.
- **Breaking:** a body's `surface` and `outline` meshes have `matrixAutoUpdate` set to `false`.
  Code that writes their `position`, `rotation`, `quaternion` or `scale` must call
  `updateMatrix()` on that mesh.
- The outline leaves out smooth edges, and planar faces average their normals across them, so a
  polygon that approximates a curve shades as a curve. The smooth angle is 20° by default; set
  `appearance.smoothAngle` to `0` for the 2.0 look.

### Added

- Solid primitives: cuboid, cylinder, cylinder sector, sphere, cone or frustum, ring torus and
  planar polyhedron. Wire primitives: rectangle, circle, polyline, line-and-arc profile and arc.
  Each goes on any plane through the `plane` option.
- Extrude with holes, from any closed wire, including a line-and-arc profile.
- Sweep along lines and arcs, with one exact bend per arc, into a capped `Solid`, an open-ended
  `Sheet` (`caps: false`), or a closed ring.
- Revolve a closed line-and-arc profile a full turn about an axis in its plane.
- Union, subtract and intersect with one to a hundred tool solids in one `operate` call, and a
  report of the result with `solid.getReport()`.
- `body.rebuild(kind, params)` gives a body new parameters in place. `body.mirror(plane)` rebuilds
  it as its reflection.
- `SystemAssembly` with parent and child placement; `duplicate()` for an independent copy and
  `instance()` for a shared shape.
- Marks with `rollback` and `release`, and `OpenGeometry.transaction(fn, { dryRun })`.
- Queries: `getBounds`, `getBrep`, `getWorldBrep`, `faceNormalAt`, `classifyPoint`, `measure`
  (volume and area within a stated error bound) and `validity`.
- Display for Three.js: every body is a `THREE.Group`, with level of detail that follows the
  camera, tessellation in a web worker or inline, picking to a face or an edge with
  `OpenGeometry.resolveHit`, `OpenGeometry.settled` and `OpenGeometry.flush`.
- Drawing through your own layers: `OpenGeometry.setHostDrawn`, `OpenGeometry.markDisplayed` and
  `OpenGeometry.hostGroup`. `OpenGeometry.releaseIdleGeometry()` frees cached display meshes.
- `OpenGeometry.exportStep` writes STEP in millimetres or metres with Y or Z up, with an export
  report.
- `OpenGeometry.insetCurvedRegion2D` and `OpenGeometry.offsetCurvedRingVariable` inset a
  line-and-arc region exactly, with one distance, or one distance per edge.
- Solid primitives `OG_PRIMITIVE_SLOT_PRISM`, `OG_PRIMITIVE_ROUNDED_BOX` and
  `OG_PRIMITIVE_PIPE_SEGMENT`, with their `OG_PRIMITIVE_PARAMS_*` helpers and the
  `SlotPrismParams`, `RoundedBoxParams` and `PipeSegmentParams` types.
- `OG_PRIMITIVE_POLYSOLID`: a solid wall of constant width and height along a path of straight
  legs and tangent arcs, with `justify` `'centre'`, `'left'` or `'right'`, mitred corners and an
  open or closed path. A joint where an arc meets a segment at an angle throws `InvalidParameter`
  that names the joint. `OG_PRIMITIVE_PARAMS_POLYSOLID` and the type `PolysolidParams` give it a
  type.
- Wire primitives `OG_PRIMITIVE_REGULAR_POLYGON`, `OG_PRIMITIVE_SLOT` and
  `OG_PRIMITIVE_ROUNDED_RECTANGLE`, with their `OG_PRIMITIVE_PARAMS_*` helpers and the
  `RegularPolygonParams`, `SlotParams` and `RoundedRectangleParams` types.

### Removed

- **Breaking:** IFC file text, STL and PDF export, projection, loft, triangulation as a public
  call, and curves other than lines, circles, arcs and polylines, such as the elliptical arc.
  They have no replacement in 3.0. 3.0 has no spline, no ellipse and no elliptical arc. The
  wedge and the 2.0 `Curve` class are gone too. Build a wedge as a five-face
  `OG_PRIMITIVE_POLYHEDRON`, and a 2.0 `Curve` as an `OG_PRIMITIVE_POLYLINE` wire through its
  points.

### Fixed

- A circular hole away from the profile centre extrudes. 2.0 refused some of these.
- Offset corners are exact arcs. 2.0 drew an inside corner as short segments outside the circle.
- A point inside one of two touching solids of a body is `Inside`. 2.0 could read it as outside.
