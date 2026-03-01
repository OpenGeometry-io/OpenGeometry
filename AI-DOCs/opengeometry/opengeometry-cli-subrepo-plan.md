# OpenGeometry CLI subrepo plan (`opengeometry/cli`)

## Goal
Create a TypeScript-based CLI subrepo that provides command-line and interactive workflows for OpenGeometry kernel operations.

## Scope and principles
- Add a new subrepo/package dedicated to CLI functionality.
- Reuse existing kernel wasm APIs from `main/opengeometry/pkg`.
- Keep initial release additive and non-breaking for current packages.
- Provide both:
  - Scriptable commands for CI/automation.
  - Interactive guided commands for local users.

## Proposed repository layout
Add a new package at:
- `main/opengeometry-cli/`

Suggested structure:
- `main/opengeometry-cli/package.json`
- `main/opengeometry-cli/tsconfig.json`
- `main/opengeometry-cli/src/bin/opengeometry.ts`
- `main/opengeometry-cli/src/cli/root-command.ts`
- `main/opengeometry-cli/src/commands/`
- `main/opengeometry-cli/src/interactive/`
- `main/opengeometry-cli/src/kernel/kernel-runtime.ts`
- `main/opengeometry-cli/src/state/session-store.ts`
- `main/opengeometry-cli/src/io/`
- `main/opengeometry-cli/test/`

NPM identity:
- Package name: `@opengeometry/cli`
- Binary name: `opengeometry`

## Runtime integration with kernel
### Current kernel facts
- Kernel wasm bindings are generated in `main/opengeometry/pkg`.
- Current generation target is `wasm-pack build --target web`.

### CLI runtime strategy
Use a dedicated Node-friendly runtime wrapper in CLI:
- Import `main/opengeometry/pkg/opengeometry.js`.
- Initialize wasm explicitly in Node by loading `opengeometry_bg.wasm` bytes and passing bytes/module to `initSync` (avoid relying on `fetch(file://...)` behavior).

Optional hardening (recommended for medium term):
- Add a Node-target build artifact for kernel, e.g. `wasm-pack build --target nodejs --out-dir pkg-node`.
- Let CLI consume `pkg-node` by default when available.

## CLI command design (v1)
### Non-interactive commands
- `opengeometry scene create <name>`
- `opengeometry scene list`
- `opengeometry scene use <sceneId>`
- `opengeometry scene show [sceneId]`
- `opengeometry scene remove <sceneId>`

- `opengeometry add line --start x,y,z --end x,y,z [--id ...]`
- `opengeometry add polyline --points ...`
- `opengeometry add arc --center ... --radius ... --start ... --end ... --segments ...`
- `opengeometry add rectangle --center ... --width ... --height ...`
- `opengeometry add polygon --points ...`
- `opengeometry add cuboid --center ... --width ... --height ... --depth ...`
- `opengeometry add cylinder --center ... --radius ... --height ... --angle ... --segments ...`
- `opengeometry add wedge --center ... --width ... --height ... --depth ...`

- `opengeometry project 2d --scene <id> [--camera-json path] [--hlr-json path] [--pretty]`
- `opengeometry project lines --scene <id> [--camera-json path] [--hlr-json path] [--pretty]`
- `opengeometry export pdf --scene <id> --out <file.pdf> [--camera-json path] [--hlr-json path]`

- `opengeometry state export --out scene.json`
- `opengeometry state import --from scene.json`

### Interactive commands
- `opengeometry interactive`
  - Launches a guided terminal app for:
    - creating/selecting scene
    - adding primitives via prompts
    - projecting/exporting
    - viewing scene summaries

- `opengeometry repl`
  - Lightweight command shell (`og>`) for repeated operations without restarting process.

## Session and persistence model
Use a local working state file (default):
- `.opengeometry/session.json`

State stores:
- current scene id
- scene summaries
- optional command history
- last used camera/HLR presets

Safety defaults:
- Explicit `--force` for overwrite operations.
- Atomic writes to prevent corrupt session state.

## Implementation phases
### Phase 0: scaffolding
- Create `main/opengeometry-cli` package.
- Add TS build (`tsup` or `tsc` + `esbuild`) and test harness (Vitest).
- Add root scripts:
  - `build-cli`
  - `test-cli`
  - `lint-cli`

### Phase 1: kernel adapter and core commands
- Implement kernel loader and lifecycle management.
- Implement scene commands and primitive `add` commands.
- Implement `project 2d` + `project lines`.

### Phase 2: interactive UX
- Add `interactive` guided flow.
- Add `repl` mode with command completion and help.
- Add basic progress/status rendering.

### Phase 3: export and robustness
- Add `export pdf` flow.
- Add state import/export.
- Improve errors with actionable remediation text.

### Phase 4: packaging and CI
- Package binary via npm (`bin` field).
- Add CI checks for CLI package.
- Add smoke tests against sample kernel flows.

## Command framework and deps
Recommended minimal stack:
- Command parser: `commander`
- Interactive prompts: `@inquirer/prompts`
- Validation: `zod`
- Styling/logging: `chalk` + `ora`
- Tests: `vitest`

Reasoning:
- Small dependency surface, good TypeScript support, easy scriptable and interactive flow.

## Validation gates
For CLI package work:
- `npm run lint-cli`
- `npm run build-cli`
- `npm run test-cli`

For kernel compatibility during CLI work:
- `npm run build-core`
- Sample end-to-end smoke:
  - create scene
  - add primitive
  - project to 2d
  - export pdf (non-wasm target only)

## Risks and mitigations
- Wasm initialization differences in Node environments:
  - Mitigation: explicit loader using bytes/module input; optional `pkg-node` target.
- Drift between CLI argument model and kernel API payload shape:
  - Mitigation: centralized schema layer (`zod`) and adapter tests.
- Interactive UX becoming hard to test:
  - Mitigation: isolate prompt layer from command handlers and test handlers directly.

## Backward compatibility
- Additive only: existing packages remain unchanged.
- New scripts must not break existing `build`, `build-core`, `build-three` workflows.

## Definition of done for v1
- New package `@opengeometry/cli` builds and runs locally.
- Non-interactive commands for scenes, primitives, projection work end-to-end.
- Interactive flow can create a scene and add at least one primitive.
- Basic automated tests pass.
- README section and usage examples added.

## Suggested first implementation slice
Start with a thin vertical slice:
1. `scene create`
2. `add line`
3. `project 2d --pretty`

This validates kernel loading, command parsing, state handling, and output formatting before expanding command surface.
