# Codex Repo Playbook

This playbook defines practical defaults for Codex work in this repository.

## 1) Before changing code

- Read relevant files first.
- Confirm scope and avoid unrelated edits.
- Prefer deterministic commands and explicit outputs.

## 2) During implementation

- Keep patches small and understandable.
- Reuse existing modules/functions before adding new abstractions.
- Add examples for new user-facing capabilities.

### OpenGeometry geometry defaults

- Use one canonical local `brep` per primitive.
- Store placement separately in `Placement3D`.
- We need to apply the placement's world transformation to the BREP and geometry buffer when requested, which will update the vertex positions in those structures according to the placement's translation, rotation and scale. This way we can keep the original geometry data intact and apply transformations on demand without losing fidelity or needing to recompute geometry from scratch after every transformation change.
- But we need to be careful about how we apply the placement transformation to the BREP and geometry buffer, as we want to ensure that all related elements (e.g., halfedges, edges, loops, faces, wires, shells) are updated consistently to maintain the integrity of the BREP structure. We should verify if transforming just the vertices is sufficient or if we also need to update other elements based on how they reference vertex positions.

## 3) Validation

Run, at minimum, relevant checks for touched areas:

```bash
# example baseline commands
cargo fmt --check
cargo check
cargo test
```

If full validation cannot run, document exactly why and what was run instead.

## 4) Commit hygiene

- Keep commit messages clear and specific.
- Do not include generated binaries.
- Include doc updates in `AI-DOCs/` for significant work.

## 5) Handoff checklist

- Summary of changes
- Local verification commands
- Expected artifacts/outputs
- Known caveats
- Any placement-model exceptions or compatibility shims
