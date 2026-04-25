# AGENTS.md

Single source of truth for AI coding agents (Claude, Codex, Copilot, etc.) working in
this repository. `CLAUDE.md` and `.github/copilot-instructions.md` are thin redirects to
this file.

If a subdirectory has stricter instructions, follow both; if they conflict, use the more
restrictive rule.

---

## 1. Project Identity

**OpenGeometry** is a browser-native CAD kernel: geometry logic written in **Rust**,
compiled to **WebAssembly**, wrapped in **TypeScript** for **Three.js** integration.
Published to NPM as `opengeometry`.

Use cases: browser CAD, AEC/BIM, configurators, geometry-heavy web tools, AI-assisted
CAD frontends that need deterministic geometry execution.

**Not the right fit for:** desktop-native CAD, non-browser runtimes without WebAssembly,
visualization-only apps where raw Three.js suffices.

OpenPlans is a *downstream* application/toolkit built on OpenGeometry. Do not position
OpenPlans as the SDK itself.

---

## 2. Architecture

```
App Code (Browser/Node)
    │
opengeometry-three  ← TypeScript wrapper (main/opengeometry-three/)
    │                 Shapes:     Polygon, Cuboid, Cylinder, Sphere, Wedge, Sweep, Opening, Solid
    │                 Primitives: Line, Arc, Curve, Polyline, Rectangle
    │                 Editor:     parametric/freeform editing helpers
    │                 Markup:     SpotLabel
    │
wasm-bindgen glue  ← Generated (main/opengeometry/pkg/, never edited directly)
    │
Rust Kernel        ← main/opengeometry/src/
    ├── brep/         B-Rep topology: Vertex, Edge, HalfEdge, Face, Loop, Wire, Shell, Brep, BrepBuilder
    ├── primitives/   OGPolygon, OGCuboid, OGCylinder, OGSphere, OGWedge, OGSweep, etc.
    ├── operations/   Triangulation (Earcut), extrude, offset, sweep, winding
    ├── geometry/     GeometryBuffer (vertex/normal/index serialization), Triangle
    ├── spatial/      Placement3D (translation, rotation, scale)
    ├── scenegraph.rs OGSceneManager + OGEntityRegistry: multi-entity orchestration, BRep snapshots
    ├── export/       projection (HLR, EdgeClass), pdf (native-only), step, stl, ifc, part21
    ├── booleans/     Union / Intersection / Subtraction (via boolmesh crate)
    ├── editor/       Parametric and freeform editing
    ├── freeform/     OGFreeformGeometry — non-parametric editing wrapper
    └── utility/      bgeometry helpers
```

### Runtime flow

1. App calls `await OpenGeometry.create({ wasmURL })` — **required** before any `Vector3`
   or shape construction.
2. App constructs primitives/shapes; each `set_config(...)` call regenerates the internal
   B-Rep.
3. App retrieves Three.js geometry via `get_geometry_serialized()` → `BufferGeometry`.
4. Scene assembly + projection + export goes through `OGSceneManager` (or the new
   `OGEntityRegistry`).

### Build pipeline

- `wasm-pack build --target web` compiles Rust → WASM + JS glue → `main/opengeometry/pkg/`
- `cargo build --release` produces native binaries (used for PDF export CLI examples)
- Rollup bundles `main/opengeometry-three/index.ts` → `dist/index.js` (ESM, Three.js as
  external peer dep)
- `scripts/prepare-dist.mjs` copies WASM, rewrites import paths, builds `dist/package.json`

---

## 3. Repository Structure

```
main/opengeometry/            Rust core → WebAssembly (Cargo crate)
main/opengeometry-three/      Three.js wrapper (the published TS SDK)
main/opengeometry-{webgl,babylon,ios,export-io,export-schema}/
                              Scaffolds — empty placeholders, see each dir's README
docs/                         Mintlify documentation source (user-facing)
knowledge/                    Stable architecture / domain notes (long-lived)
dist/                         Generated NPM bundle — never edit by hand
scripts/                      Build orchestration (prepare-dist.mjs)
.github/                      CI workflows + Copilot redirect
.claude/                      Claude Code skills + settings (hooks)
```

**No `Cargo` workspace.** Each Rust package has its own `Cargo.toml`. NPM root manages
the published `opengeometry` package; `main/opengeometry-three/` has its own
`package.json` for the example app.

---

## 4. Commands

All from repo root unless noted.

### Build

```bash
npm run build           # Full pipeline: build-core → build-three → prepare-dist
npm run build-core      # Rust → WASM (wasm-pack) + native release build
npm run build-three     # Rollup bundle → dist/index.js
npm run prepare-dist    # Copy WASM, rewrite imports, write dist/package.json
```

**Build order matters.** `build-three` depends on the generated `pkg/` from `build-core`.
`npm run build` does both in order.

### Test

```bash
npm test                                                     # Cargo unit + integration tests
cargo test --manifest-path main/opengeometry/Cargo.toml      # Same, direct
cargo test -q --manifest-path main/opengeometry/Cargo.toml   # Quiet
cargo test --manifest-path main/opengeometry/Cargo.toml \
           rectangle_generates_face_loop                     # Single test by name
```

### Lint / format

```bash
npm run lint:check                                            # ESLint check (TypeScript)
npm run lint                                                  # ESLint with --fix
cargo fmt --check --manifest-path main/opengeometry/Cargo.toml
cargo fmt --manifest-path main/opengeometry/Cargo.toml        # Apply
cargo check --manifest-path main/opengeometry/Cargo.toml      # Type/borrow check
```

### Examples (Three.js)

```bash
npm --prefix main/opengeometry-three run dev-example-three      # Vite dev server
npm --prefix main/opengeometry-three run build-example-three    # Static build
npm --prefix main/opengeometry-three run preview-example-three  # Preview build
```

Examples live at `main/opengeometry-three/examples-vite/`. Each is a standalone HTML
page importing the local SDK build.

### CLI examples (native, PDF/projection)

```bash
# Run from main/opengeometry/
cargo run --example pdf_camera_projection -- ./out/pdf_camera_projection.pdf
cargo run --example pdf_camera_projection_views -- ./out/pdf_camera_projection_views
cargo run --example pdf_primitives_all -- ./out/pdf_primitives
cargo run --example scenegraph_projection -- ./out/scenegraph_projection.pdf
cargo run --example scenegraph_projection_dump_json -- ./out/projection_dump
# Inspect: jq . ./out/projection_dump_scene2d.json
```

---

## 5. Testing & Validation

### What `npm test` covers

`npm test` runs **only** `cargo test`. Specifically:
- ~107 Rust unit tests (`#[cfg(test)]` blocks across modules)
- 4 integration tests in `main/opengeometry/tests/primitives_smoke.rs`
- 0 doc tests
- 0 cargo `--examples` targets matched (a no-op currently — the warning is benign)

### What `npm test` does NOT cover

**There are no TypeScript unit tests in this repository.** No Vitest, Jest, or Mocha
suite. Lint is the only TypeScript-side gate.

**Do not claim TypeScript code is tested after running `npm test`.** It isn't.
Validation of the TS wrapper layer is currently:
1. `npm run lint:check`
2. Manual inspection via the example pages (`dev-example-three`)
3. `npm run build` succeeds (catches type errors via `@rollup/plugin-typescript`)

If you change `main/opengeometry-three/src/`, run all three. If you ship behavior changes
without an example exercising them, say so explicitly in the handoff.

### Mandatory pre-PR checklist

Before declaring a task done, run the gates that match the touched area:

| Touched | Gates |
|---|---|
| `main/opengeometry/src/**` (Rust) | `cargo fmt --check`, `cargo check`, `cargo test`, `npm run build` |
| `main/opengeometry-three/src/**` (TS) | `npm run lint:check`, `npm run build`, manual example check |
| Both | All of the above |
| Docs / instruction files only | `npm run build` (sanity), no other gates required |
| `Cargo.toml` / `package.json` deps | Full `npm run build` + `npm test` |

If any gate cannot run, report exactly: which command, why it could not run, and the
residual risk.

### CI

`.github/workflows/release.yml` runs on push/PR to `main`. It checks for version bumps,
builds, tests, and publishes to NPM. The `npm test` step **is now required to pass** for
publish (no `continue-on-error`). Don't bump the version unless you've run the full
suite locally.

---

## 6. Behavioral Guidelines

### Generic principles

- **Think before coding.** State assumptions before writing. When a request is ambiguous,
  surface multiple interpretations and ask, rather than silently choosing one.
- **Simplicity first.** Minimum code that solves the problem. No speculative abstractions,
  unrequested features, or just-in-case error handling.
- **Surgical changes.** Touch only what the task requires. Match the existing style.
  Remove only code your change made obsolete — do not clean up pre-existing dead code
  unless explicitly asked.
- **Goal-driven execution.** Define verifiable success criteria up front. Use a brief
  multi-step plan with checkpoints.

### Codebase-specific rules

These exist because of past incidents or non-obvious constraints. They override anything
in the generic section if there's tension.

- **`Vector3` is wasm-backed.** Constructing it before `OpenGeometry.create({ wasmURL })`
  resolves throws at runtime. Never recommend or write code that does this.
- **Do not edit `main/opengeometry/pkg/`.** It is generated by `wasm-pack`. Edit Rust
  source and rebuild.
- **Do not bump `earcutr` past `=0.3.0`.** The pin (with `=`) is intentional —
  triangulation behavior is sensitive to algorithmic changes. If you need a newer
  version, raise the question explicitly with the user before changing.
- **`printpdf` is native-only.** The PDF export path in `main/opengeometry/src/export/pdf.rs`
  is gated with `cfg(not(target_arch = "wasm32"))`. **PDF export does not work in the
  browser.** Browser PDF requires a downstream package (planned via `pdf-lib`) — see the
  in-flight spec in `knowledge/technical-drawing-pdf-export.md`.
- **No feature flags in the Rust crate.** All functionality compiles unconditionally.
  Don't add `cfg(feature = "...")` gates without explicit approval — bundle-size
  optimization is a separate, deliberate effort.
- **Scene snapshots are not live.** `OGSceneManager.addBrepEntityToScene(...)` captures
  the BRep at insertion time. Later changes to wrapper objects do not auto-propagate;
  callers must explicitly call `replaceBrepEntityInScene` or `refreshBrepEntityInScene`.
  Document this in any new scene-related API.
- **One canonical local `brep` per primitive; `Placement3D` is separate.** Applying a
  placement transformation updates vertex positions in the BRep + geometry buffer. Before
  assuming vertex updates are sufficient, confirm whether halfedges, edges, loops, faces,
  wires, and shells also need updating to maintain BRep invariants. See
  `.claude/skills/brep-invariants.md`.
- **B-Rep is canonical, triangulation is lazy.** Primitives populate the BRep; the
  Earcut-based triangulation runs on demand inside `get_geometry_buffer()`. Do not
  pre-triangulate or cache triangulated meshes parallel to the BRep.

### Public API discipline

- Public TypeScript imports must use `"opengeometry"` (the published package name), not
  `@opengeometry/kernel-three` and not relative paths into `main/opengeometry/pkg/`.
  Internal SDK files in this repo may import from `pkg/` directly — customer code may not.
- Preferred high-level workflows (use these in examples and docs):
  - `polygon.extrude(height)` → returns `Solid`
  - `Solid.extrude(profile, height, options)` → BRep face extrusion
  - `Opening.subtractFrom(host, options?)` → host-cutting (binary)
  - `shape.subtract([operands], options?)` → shape-level subtraction (array-only)
  - `booleanUnion / booleanIntersection / booleanSubtraction` → standalone helpers
  - `toFreeform()` → wrapper-to-direct-editing handoff
  - `OGSceneManager` (or `OGEntityRegistry` once stabilized) → projection + export bridge
- BRep accessor precedence when serializing wrapper geometry:
  1. `getBrepSerialized()`
  2. `getBrepData()`
  3. `getBrep()`
  Note: `Polygon.getBrepData()` is the exception — it returns serialized BRep JSON, not
  a parsed object.

---

## 7. Engineering Standards

### Scope discipline

- Change only what the request requires. Do not refactor unrelated modules.
- Keep each commit atomic and explainable in one sentence.
- A bug fix does not need surrounding cleanup. A one-shot operation does not need a
  helper. Three similar lines are better than a premature abstraction.

### API & backward compatibility

- Preserve public API behavior by default. The published surface is what `dist/index.js`
  and its `.d.ts` files re-export.
- Breaking changes require explicit sign-off in the prompt or PR description, plus a
  migration note.
- Avoid hidden behavioral changes in existing call paths. If a function used to do X and
  now does Y, that is a breaking change even if the signature is unchanged.

### Security & safety

- No hardcoded secrets, keys, tokens. Validate / sanitize external input at boundaries
  (user input, deserialized JSON, file imports).
- Prefer fail-safe behavior on malformed input. Return structured errors rather than
  panic in recoverable paths.
- Do not introduce eval-like dynamic code paths.

### Reliability & error handling

- No `unwrap()` / `expect()` in code paths reachable from WASM exports unless the panic
  is genuinely unreachable and documented as such.
- Return structured errors with actionable context (`BrepError`, `Result<T, E>`).
- Do not silently swallow failures. If a `Result` is ignored, the reason must be obvious.

### Performance

- No obviously unbounded algorithms in hot paths (projection, triangulation, boolean).
- Reuse existing components; avoid duplicating data transformations between Rust and TS.
- Mind serialization cost — passing large BRep JSON across the WASM boundary is the most
  common perf pitfall.

### Readability

- Default to no comments. Add one only when the *why* is non-obvious (hidden constraint,
  subtle invariant, workaround for a specific bug).
- Don't explain *what* the code does — well-named identifiers do that.
- Don't reference current task / PR / issue numbers in code comments — they belong in
  commit messages.

---

## 8. Documentation Policy

- **No AI-generated handoffs, playbooks, or runbooks in the repo.** The repo previously
  had an `AI-DOCs/` directory; it has been removed because those files went stale faster
  than they were read. Use git history, commit messages, and PR descriptions instead.
- **`knowledge/`** holds long-lived architecture and domain notes (e.g.,
  `opengeometry-architecture.md`, `terminology-and-definitions.md`,
  `technical-drawing-pdf-export.md`). Add to it sparingly, only when the content
  outlives a single task.
- **`docs/`** is user-facing Mintlify content. Do not put internal agent guidance there.
- **Do not create planning, decision, or analysis documents during a task** unless the
  user explicitly asks. Work from conversation context. If a long-form spec is genuinely
  needed and the user agrees, place it in `knowledge/`.

---

## 9. Gotchas

A targeted list of things agents have tripped on. If you've read nothing else, read this.

1. **`Vector3` before `OpenGeometry.create()` will throw.** It is wasm-backed.
2. **`printpdf` (PDF export) is native-only.** Does not compile to WASM. Browser PDF is
   not supported in the kernel today.
3. **Scene insertion is snapshot-based.** Later wrapper edits do not propagate.
4. **Build order:** `npm run build-core` must precede `npm run build-three`. Run
   `npm run build` to do both correctly.
5. **`pkg/` is generated.** Editing files there is overwritten by the next `wasm-pack`
   run.
6. **`earcutr = "=0.3.0"` is intentionally pinned.** Do not bump.
7. **`npm test` does not test TypeScript.** Lint + manual example are the only TS gates.
8. **`cargo test --examples` produces a benign "no targets matched" warning.** There is
   no `examples/` directory in the cargo crate — the binary examples live as
   `cargo run --example <name>` targets defined in `Cargo.toml`'s `[[example]]` blocks.
9. **`OGSceneManager` is being augmented/renamed.** The new `OGEntityRegistry` lives
   alongside it in `scenegraph.rs`. Verify current names with `git grep` before
   recommending an API. Both are wasm-exported during the transition.
10. **Five scaffold packages exist under `main/`** (`-webgl`, `-babylon`, `-ios`,
    `-export-io`, `-export-schema`). They are not implemented. Do not assume they are
    real targets when grepping or building.
11. **`Three.js` is a peer dependency.** Apps install it themselves; Rollup excludes it
    from the bundle. There is no version-mismatch detection — the published package
    requires `three >= 0.168.0`.
12. **Working directory matters for cargo example commands.** Run them from
    `main/opengeometry/`, not the repo root, or use `--manifest-path`.

---

## 10. In-Flight Work

Active multi-phase initiative: **technical drawing PDF export**. Spec at
`knowledge/technical-drawing-pdf-export.md`.

Status (as of writing):
- Phase 1 (kernel-side `EdgeClass`, `ClassifiedSegment` in `export/projection.rs`,
  `OGEntityRegistry` in `scenegraph.rs`) — **partially landed, uncommitted in working
  tree**.
- Phases 2–6 (TS layouts package, sheet composers, PDF/SVG/DXF emitters, OpenPlans
  wiring) — **pending, mostly in OpenPlans repo, not here**.

Implications for agents working in this repo:
- Scene/projection/scenegraph APIs are mid-shift. Always `git grep` for current names
  before suggesting a method. Don't lock examples to symbols that are about to be
  renamed.
- New code in `export/projection.rs` should align with `EdgeClass` / `ClassifiedSegment`
  rather than emitting unclassified `Segment2D` if the path is on the technical-drawing
  flow.
- Browser PDF export is *not* part of this repo's deliverable. Don't add `pdf-lib` here.

---

## 11. Change Management

- Prefer incremental commits with focused intent. One commit per logical change.
- Commit messages: imperative subject, optional body explaining *why* (not what).
- Do not commit generated binaries, `pkg/` regenerations, or `target/` artifacts.
- Do not commit `dist/` unless explicitly building a release artifact.
- No noisy formatting churn (whitespace, import reordering) unrelated to the task.

### When you finish a task

- State what changed, what gates ran, and what residual risk exists, in plain text.
- If a gate could not run, name it and explain.
- Do not write a markdown handoff file. The conversation, the diff, and the commit
  message are the handoff.

---

## 12. Definition of Done

A task is done only when all are true:

- Requested functionality is implemented end-to-end (no half-finished stubs).
- Required gates have passed (per the table in §5), or skipped gates are explicitly
  named with a reason.
- Documentation and examples updated when behavior or public API changed.
- No unintended files staged (`pkg/`, `target/`, `dist/`, `node_modules/`, OS dotfiles).
- Result is reviewable in one read-through without hidden assumptions.

---

## 13. Pointers

- This file: source of truth for agents.
- `CLAUDE.md`: redirects here.
- `.github/copilot-instructions.md`: redirects here.
- `README.md`: user-facing product overview.
- `developer.md`: contributor / release process.
- `knowledge/opengeometry-architecture.md`: deeper architecture notes.
- `knowledge/terminology-and-definitions.md`: domain vocabulary.
- `knowledge/technical-drawing-pdf-export.md`: in-flight technical drawing spec.
- `.claude/skills/wasm-build-flow.md`: trigger-based skill for build-pipeline issues.
- `.claude/skills/brep-invariants.md`: trigger-based skill for BRep editing.
- `.claude/skills/scene-snapshot-rules.md`: trigger-based skill for scene/projection/export.
- `docs/`: published Mintlify documentation source.
- `main/opengeometry-three/examples-vite/`: customer-facing examples.
