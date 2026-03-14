<p align="center">
  <a href="https://opengeometry.io">
    <img src="https://img.shields.io/badge/OpenGeometry-4460FF?style=for-the-badge&logo=data:image/svg+xml;base64,&logoColor=white" alt="OpenGeometry" />
  </a>
</p>

<h1 align="center">OpenGeometry</h1>

<p align="center">
  <strong>CAD kernel for the web — built with Rust, WebAssembly, and Three.js.</strong>
</p>

<p align="center">
  <a href="https://www.npmjs.com/package/opengeometry"><img src="https://img.shields.io/npm/v/opengeometry?style=flat-square&color=4460FF&label=npm" alt="npm version" /></a>
  <a href="https://github.com/OpenGeometry-io/OpenGeometry/blob/main/LICENSE.md"><img src="https://img.shields.io/github/license/opengeometry-io/opengeometry?style=flat-square" alt="License" /></a>
  <a href="https://discord.com/invite/cZY2Vm6E"><img src="https://img.shields.io/badge/Discord-Join%20us-5865F2?style=flat-square&logo=discord&logoColor=white" alt="Discord" /></a>
  <a href="https://x.com/openGeometry"><img src="https://img.shields.io/badge/Twitter-Follow-1DA1F2?style=flat-square&logo=x&logoColor=white" alt="Twitter" /></a>
  <a href="https://linkedin.com/company/openGeometry"><img src="https://img.shields.io/badge/LinkedIn-Connect-0A66C2?style=flat-square&logo=linkedin&logoColor=white" alt="LinkedIn" /></a>
</p>

<p align="center">
  <a href="https://opengeometry.io">Website</a> · <a href="https://docs.opengeometry.io">Documentation</a> · <a href="https://demos.opengeometry.io">Live Demos</a> · <a href="https://opengeometry.io/blog">Blog</a> · <a href="https://www.npmjs.com/package/opengeometry">npm</a>
</p>

---

> **Actively under development.** We're building OpenGeometry in the open. APIs, examples, and package structure are evolving — breaking changes are still possible. Star the repo to follow along.

---

## What is OpenGeometry?

OpenGeometry is an **open-source CAD kernel** purpose-built for the browser. The geometry engine is written in **Rust**, compiled to **WebAssembly**, and ships with a **Three.js integration layer** — so you can build real CAD tools that run entirely on the web.

Whether you're building a parametric modeler, a BIM viewer, a 3D configurator, or any geometry-heavy web app, OpenGeometry gives you the primitives and operations you need without leaving JavaScript.

### What you can do today

| Category | Capabilities |
| --- | --- |
| **Primitives** | Lines, arcs, curves, polylines, rectangles |
| **Shapes** | Polygons, cylinders, cuboids, spheres, wedges |
| **Operations** | Triangulation, extrusion, sweep, offset, boolean operations |
| **Exports** | STL, STEP, IFC, PDF projection |
| **Integration** | Three.js scene management, WebAssembly-powered performance |

## Demos

See OpenGeometry in action — interactive, browser-based demos showcasing the kernel's capabilities:

**[demos.opengeometry.io](https://demos.opengeometry.io)**

Demos include primitives rendering, shape generation, sweep operations, boolean operations, file exports, and more. All running client-side via WebAssembly.

## Quick Start

Install from npm:

```bash
npm install opengeometry
```

```javascript
import { OpenGeometry } from "opengeometry";

const og = new OpenGeometry();
await og.init();

// Create a rectangle and extrude it into a 3D shape
const rect = og.rectangle({ width: 2, height: 1 });
const extruded = og.extrude(rect, { depth: 0.5 });
```

For a complete walkthrough, see the [Quick Start guide](https://docs.opengeometry.io/quickstart) or clone the [quickstart-js](https://github.com/OpenGeometry-io/quickstart-js) repo.

## Documentation

Full API reference, guides, and concepts are available at:

**[docs.opengeometry.io](https://docs.opengeometry.io)**

Key pages:
- [Installation](https://docs.opengeometry.io/installation)
- [Quick Start](https://docs.opengeometry.io/quickstart)
- [Architecture & Concepts](https://docs.opengeometry.io/concepts/architecture)
- [API Reference](https://docs.opengeometry.io/api)

## Repository Structure

```
main/opengeometry          Rust core → WebAssembly
main/opengeometry-three    Three.js integration layer
main/opengeometry-webgl    WebGL-oriented package (WIP)
main/opengeometry-babylon  Babylon.js-oriented package (WIP)
docs/                      Documentation source (Mintlify)
```

## Building from Source

**Prerequisites:** Node.js, npm, Rust toolchain, `wasm-pack`

```bash
# Install dependencies
npm install

# Build Rust core → WebAssembly
npm run build-core

# Build everything (core + Three.js + WASM copy)
npm run build

# Run the Three.js example app locally
npm --prefix main/opengeometry-three run dev-example-three

# Run tests
npm test
```

## Who is this for?

- Teams building **browser-based CAD/BIM/geometry tools**
- Developers evaluating **WebAssembly-powered 3D** for the web
- Contributors interested in the **Rust → WASM geometry pipeline**
- Anyone exploring **open-source CAD kernel internals**

If you just want a quick look, start with the [hosted demos](https://demos.opengeometry.io) or the [quickstart repo](https://github.com/OpenGeometry-io/quickstart-js).

## Community

We'd love to have you involved — whether you're using OpenGeometry, building on it, or just curious.

- **[Discord](https://discord.com/invite/cZY2Vm6E)** — Chat with the team and community
- **[Twitter / X](https://x.com/openGeometry)** — Updates and announcements
- **[LinkedIn](https://linkedin.com/company/openGeometry)** — Company updates
- **[GitHub Issues](https://github.com/OpenGeometry-io/OpenGeometry/issues)** — Bug reports and feature requests
- **[Blog](https://opengeometry.io/blog)** — Deep dives and release notes

## Contributing

OpenGeometry is open source under the [MPL-2.0 license](./LICENSE.md). Contributions are welcome — check the [issues](https://github.com/OpenGeometry-io/OpenGeometry/issues) for good starting points or open a discussion on [Discord](https://discord.com/invite/cZY2Vm6E).

## AI Agent Docs Policy

- Repository-level AI agent instructions are in [AGENTS.md](./AGENTS.md).
- All AI-generated documentation must live under [`AI-DOCs/`](./AI-DOCs/).
- AI-generated docs should not be added under app/code folders unless explicitly requested.

---

<p align="center">
  <sub><em>"It is my land. Who would I be if I did not try to make it better?" — A Knight.</em></sub>
</p>
