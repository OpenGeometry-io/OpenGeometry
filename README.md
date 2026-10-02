<p align="center">
  <a href="https://opengeometry.io?utm_source=github">
    <img src="https://raw.githubusercontent.com/OpenGeometry-io/.github/main/profile/opengeometryTextLogo.png" alt="OpenGeometry" />
  </a>
</p>

<h1 align="center">OpenGeometry</h1>

<p align="center">
  <strong>Browser-native CAD kernel for Web Apps and AI CAD built with Rust, WebAssembly, and Three.js.</strong>
</p>

<p align="center">
  <a href="https://www.npmjs.com/package/opengeometry"><img src="https://img.shields.io/npm/v/opengeometry?style=flat-square&color=4460FF&label=npm" alt="npm version" /></a>
  <a href="https://github.com/OpenGeometry-io/OpenGeometry/blob/main/LICENSE.md"><img src="https://img.shields.io/github/license/opengeometry-io/opengeometry?style=flat-square" alt="License" /></a>
  <a href="https://discord.gg/9wJpbfgGGA"><img src="https://img.shields.io/badge/Discord-Join%20us-5865F2?style=flat-square&logo=discord&logoColor=white" alt="Discord" /></a>
  <a href="https://x.com/openGeometry"><img src="https://img.shields.io/badge/Twitter-Follow-1DA1F2?style=flat-square&logo=x&logoColor=white" alt="Twitter" /></a>
  <a href="https://linkedin.com/company/openGeometry"><img src="https://img.shields.io/badge/LinkedIn-Connect-0A66C2?style=flat-square&logo=linkedin&logoColor=white" alt="LinkedIn" /></a>
</p>

<p align="center">
  <a href="https://opengeometry.io?utm_source=github">Website</a> · <a href="https://docs.opengeometry.io/OpenGeometry?utm_source=github">Documentation</a> · <a href="https://demos.opengeometry.io?utm_source=github">Live Demos</a> · <a href="https://blog.opengeometry.io?utm_source=github">Blog</a> · <a href="https://www.npmjs.com/package/opengeometry">npm</a>
</p>

---

> **Actively maintained and growing.** We're building OpenGeometry in the open. APIs, examples, and package structure are evolving, and we are actively improving and expanding the project. Star the repo to follow along. If you have questions or want to get involved, join the [Discord](https://discord.com/invite/9wJpbfgGGA) or check out the [issues](https://github.com/OpenGeometry-io/OpenGeometry/issues).

> **Version note.** This README describes OpenGeometry 2.5. Check which line you have installed with `npm ls opengeometry`: the 2.0 line has a different API. The hosted documentation still describes 2.0.

---

## What is OpenGeometry?

OpenGeometry is an **open-source CAD kernel for the browser**. It models **B-rep solids**, runs **boolean operations** (union, subtract, intersect), builds solids by **extrude and sweep**, and writes **STEP** files. The kernel is written in **Rust** and compiled to **WebAssembly**, and a **TypeScript SDK for Three.js** puts every body straight into your scene.

OpenGeometry is best suited for **browser-based CAD, configurators, and geometry-heavy web tools**. Whether you're building a parametric modeler, a solid modeling workflow, a geometry viewer, or a custom Three.js modeling tool, OpenGeometry gives you deterministic, kernel-backed primitives and operations without leaving JavaScript.

It is the geometry engine layer, not a full CAD application. OpenPlans is a downstream application/toolkit built on top of OpenGeometry for AEC workflows. In this repository, OpenGeometry is the primary SDK and engine.

## Features

| Area | What 2.5 has |
| --- | --- |
| **Primitives** | Wires: rectangle, circle, polyline (open or closed). Solids: cuboid, cylinder. Each can be placed on any plane. |
| **Operations** | Extrude a closed wire into a solid, with holes. Sweep a profile along a path. Union, subtract and intersect, with one to a hundred tool solids in one call. |
| **Parametric edits** | `rebuild` gives an existing body new parameters in place. |
| **Assemblies** | Assemblies with parent and child placement; translate, rotate, scale and place; independent copies and shared instances. |
| **History** | Marks with rollback and release; transactions, with a dry-run option. |
| **Display for Three.js** | Every body is a `THREE.Group`. Level of detail that follows the camera, tessellation in a web worker or inline, picking down to face and edge, outlines, colour and opacity. |
| **Queries** | Bounds, placements, the B-rep of a body, and the report of its last boolean. |
| **Export** | STEP, in millimetres or metres, with Y or Z up, together with an export report. |

Not in 2.5, though 2.0 had them: IFC and STL export; projection; offset; loft; triangulation as a public call; arcs, curves, spheres and wedges; and the 2.0 classes such as `Cuboid`, `Polygon`, `Opening` and `Vector3`.

## Installation

```bash
npm install opengeometry three
```

- `three` is a peer dependency: versions `>=0.168.0 <0.185.0` are supported.
- The package is ES modules only, and its TypeScript types are included.
- The package ships `opengeometry_bg.wasm` (the kernel) and `tessellation-worker.js` (the display worker), and your page must be able to fetch both. With no arguments, `OpenGeometry.create()` looks for them next to the package's own `index.js`. If your bundler moves or rewrites `index.js` without the two files, that lookup fails, so the dependable way is the one the quick start uses: copy both files from `node_modules/opengeometry/` into the folder your site serves at its root (in Vite, the `public/` folder by default) and pass their URLs to `create`.

## Quick start

```ts
import * as THREE from 'three';
import {
  OpenGeometry, Solid, Wire,
  OG_OPERATION_EXTRUDE, OG_OPERATION_SUBTRACT, OG_PRIMITIVE_CUBOID, OG_PRIMITIVE_RECTANGLE, OG_TRANSFORM_TRANSLATE,
} from 'opengeometry';

await OpenGeometry.create({ wasmURL: '/opengeometry_bg.wasm' }, { workerURL: '/tessellation-worker.js' });

const profile = new Wire(OG_PRIMITIVE_RECTANGLE, { width: 6, breadth: 0.2 });
const wall = new Solid(OG_OPERATION_EXTRUDE, { profile, distance: 3 });
const door = new Solid(OG_PRIMITIVE_CUBOID, { width: 0.9, height: 2.1, depth: 0.4 });
door.transform(OG_TRANSFORM_TRANSLATE, { offset: [2, 0, 0] });
wall.operate(OG_OPERATION_SUBTRACT, { tools: [door] });

const scene = new THREE.Scene();
scene.add(new THREE.HemisphereLight(0xffffff, 0x444444, 2), wall);
const camera = new THREE.PerspectiveCamera(50, innerWidth / innerHeight, 0.1, 100);
camera.position.set(6, 5, 8);
camera.lookAt(0, 1.5, 0);
const renderer = new THREE.WebGLRenderer({ antialias: true });
renderer.setSize(innerWidth, innerHeight);
document.body.append(renderer.domElement);
renderer.setAnimationLoop(() => renderer.render(scene, camera));

const { text, report } = await OpenGeometry.exportStep({ nodes: [wall], unit: 'millimetre', upAxis: 'Z' });
```

This loads the kernel, makes a 6 m by 0.2 m rectangle wire on the ground plane, extrudes it 3 m up into a wall, and cuts a door opening through the wall with a cuboid placed at x = 2. The wall is a `THREE.Group`, so it goes into the scene like any other object and is drawn on every frame. The last line writes the wall as a STEP file: `text` is the file and `report` counts what was written.

The two URLs assume both files were copied to the site root, as described under Installation. The block uses top-level `await`, so it must run as a module. If your build rejects top-level `await`, raise its target (the examples set Vite's `build.target` to `'esnext'`) or move the code after the imports into an `async` function and call it.

Runnable examples live in [`main/opengeometry-three/examples-vite/`](./main/opengeometry-three/examples-vite/): a showcase, a sweep, booleans, an opening, a cuboid, a cylinder and STEP export. From a clone, run `npm --prefix main ci`, `npm run build` and then `npm run dev-example`.

## Demos

See OpenGeometry in action — interactive, browser-based demos showcasing the kernel's capabilities:

**[demos.opengeometry.io](https://demos.opengeometry.io?utm_source=github)**

Demos include primitives rendering, shape generation, sweep operations, boolean operations, file exports, and more. All running client-side via WebAssembly.

## When to use OpenGeometry

Use OpenGeometry when you need:

- browser-based parametric modeling with Rust + WebAssembly performance
- cutout subtraction and other solid boolean workflows
- profile extrusion into solids for CAD or AEC modeling
- STEP export in web apps
- a Three.js-friendly CAD kernel instead of ad hoc mesh math
- a deterministic geometry engine behind AI-assisted CAD or design workflows

### Why it works well for AI-powered CAD apps

OpenGeometry is a good fit for **AI-assisted CAD apps** because the kernel layer stays explicit and deterministic. An AI assistant or agent can suggest modeling steps, generate profiles, or orchestrate edits, while OpenGeometry executes the actual geometry operations, booleans, and exports in a predictable browser runtime.

Good examples include:

- AI assistants that translate user intent into concrete modeling operations
- prompt-to-geometry or agent-driven editing flows inside browser CAD tools
- AI-first design interfaces that still need reliable extrusion, boolean, and export workflows

## Good fit / Not the right fit

**Good fit**

- browser CAD, mechanical design, and geometry-heavy web applications
- Three.js-based modeling tools that need a real kernel behind them
- AI-first CAD frontends that need deterministic geometry execution in the browser

**Not the default fit**

- desktop-native CAD products instead of an embeddable web SDK
- non-browser runtimes with no WebAssembly or browser delivery story
- pure visualization-only apps where raw Three.js is enough and kernel-backed modeling is unnecessary

## Who is this for?

- Teams building **browser-based CAD and geometry tools**
- Developers evaluating **WebAssembly-powered 3D** for the web
- Contributors interested in the **Rust → WASM geometry pipeline**
- Anyone exploring **open-source CAD kernel internals**

If you just want a quick look, start with the [hosted demos](https://demos.opengeometry.io?utm_source=github) or the [examples](./main/opengeometry-three/examples-vite/).

## Using OpenGeometry with AI coding assistants

This section is a compact reference for writing OpenGeometry 2.5 code, by hand or with a coding assistant.

**Package.** `opengeometry` on npm. ES modules only, types included, peer dependency `three` `>=0.168.0 <0.185.0`.

**Which line is installed.** The 2.5 entry exports `OpenGeometry.create`, `Solid` and `Wire`. Code that uses `Cuboid`, `Vector3` or `setPlacement` is written for 2.0 and does not work with 2.5. The `version` in `node_modules/opengeometry/package.json` settles it.

**The whole public surface.** The entry exports `OpenGeometry`, `Solid`, `Wire`, `SystemAssembly`, `OGMark`, `OGError` and the `OG_*` constants. Every point and vector is `[x, y, z]`. Model units are metres, and Y is up.

| Export | Call and parameters |
| --- | --- |
| `OpenGeometry.create` | `await OpenGeometry.create(input?, options?)`. `input`: `{ wasmURL?, wasmModule? }`. `options`: `{ workerURL?, tessellation?: 'worker' \| 'inline' }`. |
| `Wire` | `new Wire(kind, params, options?)`. `OG_PRIMITIVE_RECTANGLE` `{ width, breadth }`, `OG_PRIMITIVE_CIRCLE` `{ radius }`, `OG_PRIMITIVE_POLYLINE` `{ points, closed? }`. |
| `Solid` | `new Solid(kind, params, options?)`. `OG_PRIMITIVE_CUBOID` `{ width, height, depth }`, `OG_PRIMITIVE_CYLINDER` `{ radius, height }`, `OG_OPERATION_EXTRUDE` `{ profile, holes?, distance }`, `OG_OPERATION_SWEEP` `{ profile, path }`; `profile`, `holes` and `path` are wires. |
| Body options | `{ ogId?, parent?, appearance?, plane? }`. `plane` is `{ origin?, normal?, xDirection? }` and applies to primitives only; by default it is the ground plane through the origin, normal `[0, 1, 0]`, x direction `[1, 0, 0]`. |
| Booleans | `solid.operate(kind, { tools, instances? })` with `OG_OPERATION_SUBTRACT`, `OG_OPERATION_UNION` or `OG_OPERATION_INTERSECT`; `tools` is one to a hundred solids. |
| Transforms | `node.transform(kind, params)` on a body or an assembly: `OG_TRANSFORM_TRANSLATE` `{ offset }`, `OG_TRANSFORM_ROTATE` `{ axis, degrees, pivot? }`, `OG_TRANSFORM_SCALE` `{ factor, pivot? }`, `OG_TRANSFORM_PLACE` `{ origin?, xDirection?, normal?, scale? }`. `getPlacement()` and `getWorldPlacement()` return `{ origin, xDirection, normal, scale }`. |
| Edits | `body.rebuild(kind, params, { instances? })` with the kinds and parameters above; a wire stays a wire and a solid a solid. |
| Copies | `body.duplicate({ ogId?, parent? })` makes an independent copy; `body.instance({ ogId?, parent? })` shares the shape. `getInstanceCount()`, `makeUnique()`. |
| `SystemAssembly` | `new SystemAssembly({ ogId?, parent? })`. `addChild(children, { keepWorld? })`, `removeChild(child, { keepWorld? })`, `getChildren()`, `getParent()`, `getBounds()`, `transform`, `dispose()`. Bodies have the same tree methods. |
| Queries | `getBounds()` (`[minX, minY, minZ, maxX, maxY, maxZ]`, or `null`), `getBrep()`, `solid.getReport()` (the last boolean's report, or `null`). |
| Appearance | `appearance` in the options, or `body.setAppearance({ color?, opacity?, outline?, pickOutline?, deflection? })`. |
| Display | `OpenGeometry.settled({ deflection? })` resolves to `{ failed }`; `OpenGeometry.flush({ geometry?: 'sync' })`; `OpenGeometry.setDisplayDeflection(world?)`; `OpenGeometry.setCameraMotion(moving)`. |
| Picking | `OpenGeometry.resolveHit(intersection)` takes a Three.js raycast hit and returns `{ ogId, shapeRevision, faceId?, edgeId? }` or `undefined`. |
| `OGMark` | `const mark = OpenGeometry.mark()`, then `mark.rollback()` and `mark.release()`. `OpenGeometry.markStats()` returns `{ liveMarks, retainedRevisions }`. `OpenGeometry.transaction(fn, { dryRun? })` runs a synchronous `fn` and rolls back if it throws. |
| Events | `OpenGeometry.on(event, handler)` with `'geometry'`, `'warning'`, `'error'` or `'fatal'`; it returns a function that unsubscribes. |
| STEP | `await OpenGeometry.exportStep({ nodes, unit?, upAxis?, name?, timestamp? })` returns `{ text, report }`. `nodes`: bodies, assemblies or `ogId` strings. `unit`: `'millimetre'` (default) or `'metre'`; `upAxis`: `'Z'` (default) or `'Y'`. |
| `OGError` | Failures throw an `OGError` with `code` (for example `InvalidParameter`, `SharedShape`, `Disposed`), `call`, `message` and `details`. |
| Reset | `OpenGeometry.reset()` frees every body and leaves an empty document. |
| Typed parameters | `OG_PRIMITIVE_PARAMS_<KIND>(params)`, `OG_OPERATION_PARAMS_EXTRUDE(params)` and `OG_OPERATION_PARAMS_SWEEP(params)` return their argument unchanged; they only give the object a type. |

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
- STEP export defaults to millimetres and Z up. A wire cannot be exported on its own: the selection must contain a solid.
- Call `dispose()` on bodies and assemblies you no longer need.
- Catch `OGError` and branch on `code`. After a `'fatal'` event the runtime refuses every call until `OpenGeometry.reset()`.

**What does not exist.** IFC, STL and PDF export, projection, offset, loft, public triangulation, arcs, curves, spheres, wedges, and the 2.0 classes. Do not call them.

**Where the truth is.** `index.d.ts` in the installed package lists every export and its types. The examples in [`main/opengeometry-three/examples-vite/`](./main/opengeometry-three/examples-vite/) are working 2.5 code, except that they import the built package by path (`../../../dist/index.js`) where an app imports from `'opengeometry'`.

## Documentation

The hosted documentation at **[docs.opengeometry.io](https://docs.opengeometry.io?utm_source=github)** still describes the 2.0 API. For 2.5, use this README, the types in the package and the examples. To move code written for 2.0 to 2.5, follow [MIGRATION.md](./MIGRATION.md).

## Repository structure

| Path | What it holds |
| --- | --- |
| [`main/opengeometry/`](./main/opengeometry/) | The Rust kernel, built to WebAssembly |
| [`main/opengeometry-three/`](./main/opengeometry-three/) | The Three.js SDK, published as `opengeometry` |
| [`main/scripts/`](./main/scripts/) | Build, check, benchmark and snapshot scripts |
| [`docs/`](./docs/) | The 2.0 documentation source, not yet rewritten for 2.5 |

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
