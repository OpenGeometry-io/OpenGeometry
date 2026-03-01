# OpenGeometry CLI bootstrap handoff (2026-03-01)

## What changed
- Added a new CLI subrepo scaffold at `main/opengeometry-cli`.
- Added root scripts:
  - `npm run lint-cli`
  - `npm run build-cli`
  - `npm run test-cli`
- Implemented a first vertical CLI slice:
  - `opengeometry scene create <name>`
  - `opengeometry scene list`
  - `opengeometry scene use <sceneId>`
  - `opengeometry scene show [sceneId]`
  - `opengeometry add line --start x,y,z --end x,y,z [--id ...] [--scene ...]`
  - `opengeometry project 2d [--scene ...] [--camera-json path] [--hlr-json path] [--pretty]`
- Added a Node WASM kernel loader for `main/opengeometry/pkg/opengeometry_bg.wasm` using explicit `initSync({ module })`.
- Added local session persistence with atomic writes at `.opengeometry/session.json`.
- Added tests for option parsing and session store behavior.
- Ignored local CLI session state in `.gitignore` (`.opengeometry/`).

## Why it changed
- Establishes a working CLI foundation while validating:
  - command routing
  - kernel initialization in Node
  - state persistence between CLI invocations
  - end-to-end projection output

## How to test locally
1. `npm run lint-cli`
2. `npm run build-cli`
3. `npm run test-cli`
4. Manual smoke flow:
   - `node main/opengeometry-cli/dist/bin/opengeometry.js scene create demo`
   - `node main/opengeometry-cli/dist/bin/opengeometry.js add line --start 0,0,0 --end 1,0,0`
   - `node main/opengeometry-cli/dist/bin/opengeometry.js project 2d --pretty`

## Backward-compatibility notes
- Additive changes only.
- Existing core/three/webgl/babylon package code paths are untouched.
- New CLI scripts are opt-in and do not alter existing `build`, `build-core`, or `build-three` behavior.

## Known caveats and follow-ups
- Current session model supports only line entities.
- CLI scene IDs are managed by the CLI store (not kernel-generated scene IDs).
- Interactive mode/repl is scaffolded but not implemented yet.
- Type checking currently uses local Node type shims because `@types/node` is not present in this repo.
- Next phase should add additional primitives, `project lines`, `state import/export`, and interactive flows.
