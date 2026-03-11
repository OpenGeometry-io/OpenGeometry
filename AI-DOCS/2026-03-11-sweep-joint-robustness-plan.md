# Sweep Joint Robustness Plan

## Context

In `sweep_hilbert_profiles`, the profile remains correct in isolation, but the swept solid looks squeezed/distorted around path joints (especially around sharp turns). This is a known behavior when a sweep is built from only one section per polyline vertex and then connected linearly.

## Why this happens in the current implementation

Current sweep generation in `main/opengeometry/src/operations/sweep.rs`:

- Builds one frame per path vertex (`build_path_frames`), with a bisector tangent at interior corners.
- Places one profile ring at each path vertex.
- Connects adjacent rings with side quads.

This creates two geometric problems at corners:

1. **Corner compression ("squeeze")**: the same profile is reused at a corner where the local effective offset distance should be miter-adjusted.
2. **Asymmetric twist near joints**: frame propagation can remain numerically stable but still rotate the section in ways that look visually uneven when the path has abrupt directional changes.

## Goal

Make sweep output robust and predictable at joints while preserving backward compatibility by default.

## Proposed delivery plan

### Phase 1 — Add measurable quality gates (no behavior change)

1. Add sweep-quality tests that measure cross-section distortion around joints:
   - Width/depth preservation for rectangular profiles on L-turn and zig-zag paths.
   - Corner stress cases (e.g., 30°, 60°, 90°, 135°) with unequal segment lengths.
2. Add fixture-style helper(s) to compare local section extents against expected profile extents.
3. Keep current behavior as baseline; tests should initially characterize current error bands.

**Outcome:** objective pass/fail metrics before algorithm changes.

### Phase 2 — Corner-aware section generation

1. Treat each path corner as a join operation instead of a single blended frame sample.
2. Generate either:
   - **dual rings at corners** (end of incoming segment + start of outgoing segment), or
   - **explicit join patch** using miter/bevel/round strategy.
3. For miter joins, compute a bounded miter scale from turn angle and clamp with a configurable miter limit to avoid spikes/self-intersections.

**Outcome:** removes squeezed appearance at joints and produces deterministic geometry.

### Phase 3 — Frame robustness improvements

1. Keep parallel transport as base but add:
   - Explicit handling for near-180° turns.
   - Optional path-level twist minimization for closed loops.
2. Add continuity checks on normal/binormal flips between neighboring frames.

**Outcome:** reduced visual wobble and fewer frame discontinuities.

### Phase 4 — API + migration strategy

1. Introduce non-breaking sweep options (with defaults matching old behavior):
   - `join_style: miter | bevel | round`
   - `miter_limit: f64`
   - `corner_subdivisions: usize` (for round joins)
2. Keep legacy mode as default for one release cycle; expose robust mode behind explicit options first.

**Outcome:** safe rollout with backward compatibility.

### Phase 5 — Validation + rollout

1. Regression tests:
   - Topology expectations (faces/edges/vertices) across join styles.
   - Geometric invariants (section size tolerance, no NaN/degenerate faces).
2. Visual verification cases in sandbox/examples:
   - Hilbert path, sharp polyline turns, closed loops.
3. Document recommended defaults for user-facing UIs.

**Outcome:** production confidence and reduced support churn.

## Implementation notes (important)

- Prioritize **corner-aware ring/join generation** first; this is the primary fix for squeezed joints.
- Avoid silent behavior changes in existing call paths.
- Keep caps behavior independent of join strategy.
- Ensure robust fallbacks when a join becomes numerically degenerate.

## Local verification checklist (for implementation phase)

From `main/opengeometry/`:

- `cargo fmt`
- `cargo check`
- `cargo test operations::sweep -- --nocapture`

Add visual checks in sandbox after algorithm rollout.

## Backward-compatibility notes

- No immediate API break required.
- New options can be additive.
- Existing default sweep behavior can be preserved until robust mode is proven and promoted.

## Known caveats

- Round joins and high corner subdivision can increase face count significantly.
- Miter joins require strict limits to avoid long spikes in acute angles.
- Closed-loop orientation stabilization may require additional loop-wide correction.
