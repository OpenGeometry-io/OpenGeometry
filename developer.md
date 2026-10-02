# Developer Documentation

Contributor-facing notes. For end-user docs see [README.md](./README.md);
[docs.opengeometry.io](https://docs.opengeometry.io) still describes the 2.0 API.

## Prerequisites

- Node.js 26 (what CI uses)
- Rust toolchain 1.88.0
- `wasm-pack` (CI uses 0.13.1) — `brew install wasm-pack` on macOS, or
  `cargo install wasm-pack`

## Project layout

- `main/opengeometry/` — Rust core compiled to WebAssembly
- `main/opengeometry-three/` — Three.js wrapper (the published TypeScript SDK)
- `main/dist/` — generated npm bundle (do not edit)

## Local build

```bash
npm --prefix main ci
npm run build              # Full pipeline: Rust → WASM → TS bundle → main/dist/
npm test                   # Cargo unit + integration tests
```

`npm run build` runs `build-core` (wasm-pack only), then `rollup -c`, then
`node scripts/build/prepare-dist.mjs` (copy WASM, package metadata, the README and the
licence) in order, all inside `main/`. There is no clean step. Running the stages out of
order produces stale `pkg/` and bundle mismatches.

## Running the example app

```bash
npm --prefix main run dev-example      # Vite dev server
npm --prefix main run build-example    # Static build
```

Run `npm run build` first, because the examples import `main/dist/index.js` by path and
are served with `main/dist/` as their public folder.

The example catalog lives at `main/opengeometry-three/examples-vite/`. Use it (rather
than copying the build into a sibling repo) for local validation.

## Verification and CI

Both check commands work from the repository root and from the `main/` folder, and
write their logs to `main/.check/`.

- `npm run check` runs the 18-step gate: formatting, clippy and tests of the kernel
  and `test-support` crates, the WebAssembly tests, the build, the worker-bundle check,
  lint, type checks, source rules, import cycles, duplicates, the example build, and the
  binding, Node and performance tests.
- `npm run check:full` runs the same 18 steps plus the Playwright browser tests on
  three 0.168 and 0.184 (20 steps). It needs Playwright's Chromium:
  `npx playwright install chromium` in `main/` (on Linux with `--with-deps`, as CI does).

The root `README.md` is part of the gate. In the Node tests,
`main/scripts/build/readme-code.test.mjs` type-checks every `ts` block of the README
against `main/dist/`, fails on a `ts`, `typescript` or `tsx` block it does not read, and
checks that `main/opengeometry-three/tests/browser/pages/quick-start.ts` holds the README's
`OpenGeometry.create(` line. The browser suite runs that page, which is a hand copy of the
README's quick start, so a change to the quick start changes `pages/quick-start.ts` too.
The same test checks the `ts` blocks of `MIGRATION.md` the same way. The build ships the
README, with its relative links made absolute, and `LICENSE.md` in `main/dist/`.

`.github/workflows/verify.yml` runs on every push and pull request that changes `main/**`,
`README.md`, `MIGRATION.md`, `LICENSE.md` or the workflow file, on Ubuntu and macOS. It
runs `npm run check`, then the two browser runs (`node scripts/check/verify.mjs --full
--only browser:three-168` and `--only browser:three-184`), then the release-mode kernel
time budgets (`cargo run --release --example budgets` in `main/opengeometry/`). It uploads
the check logs and, from Ubuntu, the STEP exports.

The same workflow can be started by hand. Its one required input, `record`, chooses
what to record on Ubuntu and macOS:

- `goldens` records the kernel golden file and a kernel snapshot, and uploads them as
  the `goldens-<os>` artifact.
- `baselines` measures the performance medians and the storey baseline, and uploads
  them as the `baselines-<os>` artifact.

A record run commits nothing. A person downloads the artifacts, reviews them and
commits them: the golden file goes to `main/opengeometry/tests/fixtures/golden/`, and the
timings are copied by hand into `main/scripts/bench/performance-baseline.json`, from
`baselines-ubuntu-latest` into `timings.linux-x64-ci` and from `baselines-macos-latest`
into `timings.darwin-arm64-ci`. Copy the `timings` of `performance-medians.json`, and the
`renderMs`, `transformFlushMs` and `stepMs` of the storey line in `storey-baseline.txt` as
`storeyFirstRenderMs`, `storeyTransformFlushMs` and `storeyStepMs`. Every timings block
needs the three storey keys, so copy all of them in one commit.

## Release process

1. Bump `version` in `main/package.json`, `main/package-lock.json`,
   `main/opengeometry/Cargo.toml`, `main/opengeometry/Cargo.lock` and
   `main/opengeometry/test-support/Cargo.lock` so they match (CI fetches with `--locked`).
2. For the first 2.5 release, remove the version note near the top of `README.md`. The
   build copies the README into the package, so the published page would carry it.
3. If the release changes how existing code must be written, add an entry to
   `MIGRATION.md`, as its "Adding an entry" section describes.
4. Run both gates locally:
   ```bash
   npm run check
   npm run check:full
   ```
5. Merge to the `main` branch, then start the GitHub Action at
   `.github/workflows/release.yml` by hand on the `main` branch; a push does not publish.
   It rebuilds, runs `npm run check` (required to pass), and publishes `main/dist/` to npm
   if the version is not already on the registry.
6. The action also creates a GitHub release tagged `v<version>`.

If the publish step fails for an environmental reason but the version was already
bumped, re-running the workflow will re-attempt publish (the action checks npm and only
publishes if the version is missing).

A release run cannot pass yet. `npm run check` fails on the Ubuntu runner until the
Linux golden (`main/opengeometry/tests/fixtures/golden/x86-64-unknown-linux-gnu.json`,
now `{}`) and `timings.linux-x64-ci` are recorded, reviewed and committed as described
in [Verification and CI](#verification-and-ci). The macOS job of `verify.yml` also needs
`timings.darwin-arm64-ci`.
