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

