# Developer Documentation

Contributor-facing notes. For agent guidance see [AGENTS.md](./AGENTS.md). For end-user
docs see [README.md](./README.md) and [docs.opengeometry.io](https://docs.opengeometry.io).

## Prerequisites

- Node.js (CI uses 18; any LTS is fine locally)
- Rust toolchain 1.89.0 or newer with `wasm32-unknown-unknown`
- `wasm-pack` — `brew install wasm-pack` on macOS, or `cargo install wasm-pack`

## Project layout

- `main/opengeometry/` — Rust core compiled to WebAssembly
- `main/opengeometry-three/` — Three.js wrapper (the published TypeScript SDK)
- `main/opengeometry-{webgl,babylon,ios,export-io,export-schema}/` — empty scaffolds
- `dist/` — generated NPM bundle (do not edit)

## Local build

```bash
npm install
npm run build              # Full pipeline: Rust → WASM → TS bundle → dist/
npm test                   # Cargo unit + integration tests (no TypeScript tests yet)
```

`npm run build` runs `build-core` (wasm-pack + cargo release), `build-three` (Rollup),
and `prepare-dist` in order. Running them out of order produces stale `pkg/` and bundle
mismatches — see `.claude/skills/wasm-build-flow.md` if you hit that.

## Running the example app

```bash
npm --prefix main/opengeometry-three run dev-example-three      # Vite dev server
npm --prefix main/opengeometry-three run build-example-three    # Static build
npm --prefix main/opengeometry-three run preview-example-three  # Preview the build
```

The example catalog lives at `main/opengeometry-three/examples-vite/`. Use it (rather
than copying the build into a sibling repo) for local validation.

## Release process

1. Bump `version` in both `package.json` and `main/opengeometry/Cargo.toml` so they
   match.
2. Run the full build + test locally:
   ```bash
   npm run build
   npm test
   ```
3. Push to `main`. The GitHub Action at `.github/workflows/release.yml` detects the
   version bump, rebuilds, runs tests (now required to pass), and publishes to NPM if
   the version is not already on the registry.
4. The action also creates a GitHub release tagged `v<version>`.

If the publish step fails for an environmental reason but the version was already
bumped, re-running the workflow will re-attempt publish (the action checks NPM and only
publishes if the version is missing).
