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
The same test type-checks the `ts` blocks of `MIGRATION.md` and fails on a fence it does
not read there; the quick-start check covers the README only. The build ships the README,
with its relative links made absolute, and `LICENSE.md` in `main/dist/`. `MIGRATION.md`
is not shipped: the package README links to it on GitHub's `main` branch.

`.github/workflows/verify.yml` runs on every push and pull request that changes `main/**`,
`README.md`, `MIGRATION.md`, `LICENSE.md` or the workflow file, on Ubuntu and macOS. It
runs `npm run check`, then the two browser runs (`node scripts/check/verify.mjs --full
--only browser:three-168` and `--only browser:three-184`), then a fixed two-second guard
on one kernel boolean (`cargo run --release --example budgets` in `main/opengeometry/`),
which compares with no recorded result. It uploads the check logs and, from Ubuntu, the
STEP exports. It can also be started by hand.

Three checks compare with numbers recorded on one machine, so they run on a developer's
machine and not on CI (where `CI` is `true`). Nothing is recorded on CI.

- The kernel golden test compares the hash of every scene with the golden file of its
  target; the one recorded file is
  `main/opengeometry/tests/fixtures/golden/aarch64-apple-darwin.json`. On CI, and on a
  target with no golden file, it still builds every scene and compares no hash. Its other
  checks apply there too, except that on CI a boolean matrix row that differs from its
  stored result is not a failure. `TARGETS.md` in that folder says how to record.
- The timing budgets compare the performance medians and the storey timings with the
  block of `timings` in `main/scripts/bench/performance-baseline.json` that is named after
  the machine, such as `darwin-arm64`. On CI the timings are measured and printed, not
  checked, and a machine with no block is not checked either. The size budgets in the
  same file are checked everywhere. To record a machine, copy into its block the
  `timings` that `npm run test:performance` prints, and the `renderMs`,
  `transformFlushMs` and `stepMs` of the "Storey performance baseline:" line of the
  browser run as `storeyFirstRenderMs`, `storeyTransformFlushMs` and `storeyStepMs`, and
  set `measuredOn` to the date. A block needs the three storey keys.
- The kernel tests of the `cases` and `step_oracle` test binaries that compare a body
  built by the kernel or returned by a boolean, a tessellation, a STEP text or a STEP
  report digit for digit with one stored under `main/opengeometry/tests/fixtures/cases/`
  make that comparison off CI only. On CI they still run every case and keep their other
  checks, such as validity, volumes, handler lists and stored errors. A stored body that
  is only read and written back is still compared digit for digit everywhere, and
  `main/opengeometry/tests/wasm_core.rs` compares, on every machine, the stored
  tessellations within four ULP and the stored metre STEP texts entity for entity with
  their numbers within four ULP.

The kernel tests also read `main/opengeometry/tests/fixtures/cases/`, which holds test
inputs and stored expected results. The BRep bodies are both: each is the stored result
of a builder or of one of five booleans, and an input of the STEP and round-trip tests;
the twelve builder bodies are also inputs of the tessellation tests. The boolean and
batch matrix rows hold their inputs with their stored
result and handlers, and each validity body has its stored verdict. The other stored
results are tessellations, STEP texts, reports and errors, and handler lists. They were
first recorded from the 2.0 kernel by a tool that no longer exists, so no command
regenerates them. A change that moves one edits the stored file by hand in the same
commit and says why. The golden file is the only fixture with a record command.

## Release process

1. Bump `version` in `main/package.json`, `main/package-lock.json`,
   `main/opengeometry/Cargo.toml`, `main/opengeometry/Cargo.lock` and
   `main/opengeometry/test-support/Cargo.lock` so they match (CI fetches with `--locked`).
2. If the release changes how existing code must be written, check that `MIGRATION.md`
   has its entry (its "Adding an entry" section asks for it in the change that broke the
   code) and rename an "Unreleased" section after the two versions, such as "2.5 to 2.6".
3. Run both gates with `CI` unset on a machine that has a golden file for its target and
   a `timings` block, where the golden comparison, the comparisons with stored results and
   the timing budgets run:
   ```bash
   npm run check
   npm run check:full
   ```
4. Merge to the `main` branch, then start the GitHub Action at
   `.github/workflows/release.yml` by hand on the `main` branch; a push does not publish.
   It rebuilds, runs `npm run check` (required to pass), and publishes `main/dist/` to npm
   if the version is not already on the registry.
5. The action also creates a GitHub release tagged `v<version>`.

If the publish step fails for an environmental reason but the version was already
bumped, re-running the workflow will re-attempt publish (the action checks npm and only
publishes if the version is missing).
