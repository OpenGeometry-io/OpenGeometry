![OpenGeometry Version](https://img.shields.io/github/package-json/v/opengeometry-io/opengeometry?style=for-the-badge&color=4460FF)

## Under Active Development

**OpenGeometry is still under development.**
Expect APIs, examples, and package structure to evolve as the kernel matures. Breaking changes are still possible.

# OpenGeometry

CAD kernel for the web, built with Rust, WebAssembly, and Three.js.

OpenGeometry is focused on making geometry and modeling operations available in browser-based tools. The repository contains the Rust core, the generated WebAssembly package, and the Three.js integration layer used by the examples and downstream apps.

Documentation is available at [OpenGeometry Documentation](https://docs.opengeometry.io).

#### What OpenGeometry currently covers

- Triangulation
- Shapes such as rectangles, circles, and polygons
- Extrusion and sweep-style geometry workflows
- Offset operations
- Boolean operations for selected workflows, with ongoing refinement
- Web-first integration through WebAssembly and Three.js

#### Quick links

- Basic example: [Quick Start](https://github.com/OpenGeometry-io/quickstart-js)
- Additional source examples: [OpenGeometry Examples](https://github.com/OpenGeometry-io/OpenGeometry-examples)
- Live demo catalog: [Kernel Examples](https://demos.opengeometry.io/src/kernel/index.html)

#### Repository structure

- `main/opengeometry` - Rust core compiled to WebAssembly
- `main/opengeometry-three` - Three.js wrapper and example app
- `main/opengeometry-webgl` - WebGL-oriented package work
- `main/opengeometry-babylon` - Babylon.js-oriented package work
- `docs` - Product and API documentation source

#### Getting started locally

Prerequisites:

- Node.js and npm
- Rust toolchain
- `wasm-pack`

Install dependencies:

```bash
npm install
```

Build the Rust core and WebAssembly package:

```bash
npm run build-core
```

Build the distributable packages:

```bash
npm run build
```

Run the Three.js example app locally:

```bash
npm --prefix main/opengeometry-three run dev-example-three
```

Run tests:

```bash
npm test
```

#### Who this repo is for

This repository is most useful if you want to:

- build browser-based geometry tooling
- evaluate the Three.js integration
- contribute to the Rust/WebAssembly geometry pipeline
- explore the kernel internals

If you only want to try the library quickly, start with the quickstart repo or the hosted demos first.

#### Project status

OpenGeometry is usable for experimentation, demos, and early integrations, but it should still be treated as an evolving project rather than a fully stabilized public platform. Some areas are production-oriented, while others are still being hardened, documented, and simplified for broader open-source adoption.

## AI Agent Docs Policy

- Repository-level AI agent instructions are in [AGENTS.md](./AGENTS.md).
- All AI-generated documentation must live under [`AI-DOCs/`](./AI-DOCs/).
- AI-generated docs should not be added under app/code folders unless explicitly requested.

----

> It is my land. Who would I be if I did not try to make it better? - A Knight.
