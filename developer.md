# Developer Documentation

Contributor-facing notes. For agent guidance see [AGENTS.md](./AGENTS.md). For end-user
docs see [README.md](./README.md) and [docs.opengeometry.io](https://docs.opengeometry.io).

## Prerequisites

- Node.js 26 (what CI uses)
- Rust toolchain 1.88.0
- `wasm-pack` — `brew install wasm-pack` on macOS, or `cargo install wasm-pack`

## Project layout

- `main/opengeometry/` — Rust core compiled to WebAssembly
- `main/opengeometry-three/` — Three.js wrapper (the published TypeScript SDK)
- `main/dist/` — generated NPM bundle (do not edit)

## Local build

```bash
npm --prefix main ci
npm run build              # Full pipeline: Rust → WASM → TS bundle → main/dist/
npm test                   # Cargo unit + integration tests
```

`npm run build` runs `build-core` (wasm-pack only), then `rollup -c`, then
`node scripts/build/prepare-dist.mjs` (copy WASM and package metadata) in order, all
inside `main/`. There is no clean step. Running the stages out of order produces stale
`pkg/` and bundle mismatches.

## Running the example app

```bash
npm --prefix main run dev-example      # Vite dev server
npm --prefix main run build-example    # Static build
```

The example catalog lives at `main/opengeometry-three/examples-vite/`. Use it (rather
than copying the build into a sibling repo) for local validation.

## Release process

1. Bump `version` in `main/package.json`, `main/package-lock.json`,
   `main/opengeometry/Cargo.toml`, `main/opengeometry/Cargo.lock` and
   `main/opengeometry/test-support/Cargo.lock` so they match (CI fetches with `--locked`).
2. Run the full build + test locally:
   ```bash
   npm run build
   npm test
   ```
3. Merge to `main`, then start the GitHub Action at `.github/workflows/release.yml` by
   hand on `main`; a push does not publish. It rebuilds, runs `npm run check` (required
   to pass), and publishes to NPM if the version is not already on the registry.
4. The action also creates a GitHub release tagged `v<version>`.

If the publish step fails for an environmental reason but the version was already
bumped, re-running the workflow will re-attempt publish (the action checks NPM and only
publishes if the version is missing).
