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

> **Actively maintained and growing.** We're building OpenGeometry in the open. APIs, examples, and package structure are evolving, we are actively improving and expanding the project. Star the repo to follow along. If you have questions or want to get involved, join the [Discord](https://discord.com/invite/9wJpbfgGGA) or check out the [issues](https://github.com/OpenGeometry-io/OpenGeometry/issues)

---

## What is OpenGeometry?

OpenGeometry is an **open-source, browser-native CAD kernel**. The geometry engine is written in **Rust**, compiled to **WebAssembly**, and paired with a **Three.js-friendly TypeScript layer** so you can build real CAD tools that run in the browser.

OpenGeometry is best suited for **browser-based CAD, AEC/BIM, configurators, and geometry-heavy web tools**. Whether you're building a parametric modeler, a wall/opening workflow, a BIM viewer, or a custom Three.js modeling tool, OpenGeometry gives you deterministic, kernel-backed primitives and operations without leaving JavaScript.

It is the geometry engine layer, not a full CAD application. OpenPlans is a downstream application/toolkit built on top of OpenGeometry for AEC workflows. In this repository, OpenGeometry is the primary SDK and engine.

## When to use OpenGeometry

Use OpenGeometry when you need:

- browser-based parametric modeling with Rust + WebAssembly performance
- wall/opening subtraction and other solid boolean workflows
- polygon extrusion into solids for CAD or AEC modeling
- IFC, STEP, STL, and PDF-style export/projection in web apps
- a Three.js-friendly CAD kernel instead of ad hoc mesh math
- a deterministic geometry engine behind AI-assisted CAD or design workflows

## Why it works well for AI-powered CAD apps

OpenGeometry is a good fit for **AI-assisted CAD apps** because the kernel layer stays explicit and deterministic. An AI copilot or agent can suggest modeling steps, generate profiles, or orchestrate edits, while OpenGeometry executes the actual geometry operations, booleans, projections, and exports in a predictable browser runtime.

Good examples include:

- AI copilots that translate user intent into concrete modeling operations
- prompt-to-geometry or agent-driven editing flows inside browser CAD tools
- AI first design interfaces that still need reliable extrusion, boolean, projection, and export workflows

## Good fit / Not the right fit

**Good fit**

- browser CAD, AEC/BIM, Mechanical, and geometry-heavy web applications
- Three.js-based modeling tools that need a real kernel behind them
- AI first CAD frontends that need deterministic geometry execution in the browser

**Not the default fit**

- desktop-native CAD products instead of an embeddable web SDK
- non-browser runtimes with no WebAssembly or browser delivery story
- pure visualization-only apps where raw Three.js is enough and kernel-backed modeling is unnecessary

## AI and Coding Agent Entrypoints

If you are using ChatGPT, Claude, Gemini, Copilot, or other coding agents on this repository, start here:

- [AGENTS.md](./AGENTS.md) - single source of truth for all coding agents (architecture, commands, gotchas, behavioral rules)
- [README.md](./README.md) - product overview, positioning, and quick start
- [Quickstart](https://docs.opengeometry.io/OpenGeometry/quickstart)
- [Installation](https://docs.opengeometry.io/OpenGeometry/installation)
- [Boolean operations](https://docs.opengeometry.io/OpenGeometry/concepts/booleans)
- [Extrude](https://docs.opengeometry.io/OpenGeometry/api/operations/extrude)

### What you can do today

| Category | Capabilities |
| --- | --- |
| **Primitives** | Lines, arcs, curves, polylines, rectangles |
| **Shapes** | Polygons, solids, cuboids, cylinders, spheres, wedges, sweeps, openings |
| **Operations** | Triangulation, extrusion, sweep, offset, boolean operations |
| **Exports** | STL, STEP, IFC, PDF projection (PDF is currently native/Node-only — browser PDF is on the roadmap) |
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

A practical browser deployment looks like this: install `opengeometry` from npm, serve `opengeometry_bg.wasm` from your app, then initialize the runtime once with `OpenGeometry.create({ wasmURL })` before creating `Vector3` values or kernel-backed wrappers.

```ts
import { Cuboid, OpenGeometry, Vector3 } from "opengeometry";

await OpenGeometry.create({
  wasmURL: new URL(
    "node_modules/opengeometry/opengeometry_bg.wasm",
    import.meta.url
  ).href,
});

// Initialize once before constructing Vector3 or any wrapper.
const cuboid = new Cuboid({
  center: new Vector3(0, 0, 0),
  width: 2,
  height: 1,
  depth: 1,
  color: 0x33aa66,
});

// Placement is strict: scale must be positive + uniform.
cuboid.setPlacement({
  translation: new Vector3(1, 0, 0),
  rotation: new Vector3(0, 0.25, 0),
  scale: new Vector3(1.25, 1.25, 1.25),
});
```

Scenegraph behavior is snapshot-based: `add*ToScene` captures geometry at insert time. If you change placement/config later, push updates explicitly via `replaceBrepEntityInScene` or `refreshBrepEntityInScene`.

For a complete walkthrough, see the [Quick Start guide](https://docs.opengeometry.io/OpenGeometry/quickstart) or clone the [quickstart-js](https://github.com/OpenGeometry-io/quickstart-js) repo.

## Documentation

Full API reference, guides, and concepts are available at:

**[docs.opengeometry.io](https://docs.opengeometry.io)**

Key pages:
- [Installation](https://docs.opengeometry.io/OpenGeometry/installation)
- [Quick Start](https://docs.opengeometry.io/OpenGeometry/quickstart)
- [Architecture & Concepts](https://docs.opengeometry.io/OpenGeometry/concepts/architecture)
- [API Reference](https://docs.opengeometry.io/OpenGeometry/api/)

## Repository Structure

```
main/opengeometry          Rust core → WebAssembly
main/opengeometry-three    Three.js integration layer
main/opengeometry-webgl    WebGL-oriented package (WIP)
main/opengeometry-babylon  Babylon.js-oriented package (WIP)
docs/                      Documentation source (Mintlify)
```

## Building from Source

**Prerequisites:** Node.js, npm, Rust 1.89.0+ with `wasm32-unknown-unknown`, `wasm-pack`

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

- **[Discord](https://discord.com/invite/9wJpbfgGGA)** — Chat with the team and community
- **[Twitter / X](https://x.com/openGeometry)** — Updates and announcements
- **[LinkedIn](https://linkedin.com/company/openGeometry)** — Company updates
- **[GitHub Issues](https://github.com/OpenGeometry-io/OpenGeometry/issues)** — Bug reports and feature requests
- **[Blog](https://blog.opengeometry.io?utm_source=github)** — Deep dives and release notes

## Contributing

OpenGeometry is open source under the [MPL-2.0 license](./LICENSE.md). Contributions are welcome — check the [issues](https://github.com/OpenGeometry-io/OpenGeometry/issues) for good starting points or open a discussion on [Discord](https://discord.com/invite/9wJpbfgGGA).

## AI Agent Docs Policy

- If you're an AI agent, start with [AGENTS.md](./AGENTS.md) for architecture, commands, and behavioral rules.
- Agents should not create planning, handoff, or runbook files during a task. The
  conversation, the diff, and the commit message are the handoff.

---

<p align="center">
  <sub><em>"It is my land. Who would I be if I did not try to make it better?" — A Knight.</em></sub>
</p>
