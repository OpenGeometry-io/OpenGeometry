# AGENTS.md

Repository-level standards for Codex, Copilot, and other AI coding agents.

This file defines how agents must operate to produce production-ready code.
If a subdirectory has stricter instructions, follow both; if they conflict, use the more restrictive rule.

## Core Outcome

All agent work must be:

- Correct and behaviorally safe
- Testable and reproducible
- Secure by default
- Backward-compatible unless explicitly approved
- Reviewable through small, clear diffs

## Documentation Location Policy (Mandatory)

- All AI-generated docs must be placed under `AI-DOCs/`.
- Do not add AI-generated docs under app/code directories such as `main/`, `src/`, `test/`, or package folders.
- If a task needs documentation, create or update files in `AI-DOCs/` unless the user explicitly asks otherwise.

## Documentation Naming Rules

- Use `kebab-case` file names.
- Prefix date when useful: `YYYY-MM-DD-topic.md`.
- Keep one concern per file (design note, runbook, handoff, postmortem, etc.).

## Engineering Standards

### 1) Scope Discipline

- Only change what is necessary for the request.
- Do not refactor unrelated modules unless explicitly requested.
- Keep each change atomic and explainable.

### 2) API and Compatibility

- Preserve public API behavior by default.
- If a breaking change is required, document it clearly and provide migration guidance.
- Avoid hidden behavioral changes in existing call paths.

### 3) Security and Safety

- Never hardcode secrets, keys, tokens, or credentials.
- Validate and sanitize all external/user-controlled inputs.
- Prefer fail-safe behavior on invalid state or malformed input.
- Minimize attack surface: least privilege, least data exposure, least capability.

### 4) Reliability and Error Handling

- Avoid panics for recoverable runtime conditions.
- Return structured errors with actionable context.
- Do not silently swallow failures.
- Keep retries bounded and deterministic.

### 5) Performance and Scalability

- Avoid obviously unbounded algorithms in hot paths.
- Reuse existing components and avoid duplicate data transformations.
- Consider memory overhead and serialization costs for large scenes/data.

### 6) Readability and Maintainability

- Prefer simple, explicit code over clever shortcuts.
- Keep functions focused and names descriptive.
- Add concise comments only where intent is not obvious from code.

## Testing and Validation Standards (Mandatory)

Before finalizing a change, run relevant checks for touched areas:

- Formatting
- Static/build checks
- Tests
- Example/integration command for user-facing features

Minimum expectations:

- Run project-standard commands where available (for example `cargo fmt`, `cargo check`, `cargo test`).
- If full validation cannot run, report exactly:
  - which command failed or was skipped
  - why
  - residual risk

## Change Management

- Prefer incremental commits with focused intent.
- Do not commit generated binaries or transient artifacts.
- Keep diffs review-friendly and avoid noisy unrelated formatting churn.

## Pull Request / Handoff Standard

For significant changes, add/update a short handoff note in `AI-DOCs/` including:

- What changed
- Why it changed
- How to test locally
- Backward-compatibility notes
- Known caveats and follow-ups

## Definition of Done

A task is done only when all are true:

- Requested functionality is implemented end-to-end.
- Quality gates have been run or limitations are explicitly documented.
- Documentation and examples are updated when behavior/API changed.
- No unintended artifacts are introduced into version control.
- Result is ready for production review without hidden assumptions.
