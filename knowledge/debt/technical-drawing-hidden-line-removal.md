# Debt: HLR Visibility Is Not Full Technical Drawing Hidden-Line Removal

## Status

OpenGeometry currently classifies edge visibility using face orientation and edge adjacency. This is useful, but it is not a complete hidden-line removal algorithm for technical drawings.

## Standards Researched

- ISO 128-3:2022 covers general principles for presenting views, sections and cuts.
- ISO 5456-2:1996 covers orthographic projection methods.

Sources:

- https://www.iso.org/standard/83356.html
- https://standards.iteh.ai/catalog/standards/iso/75d888a5-6e9a-4fac-9120-8f2dffc20899/iso-128-3-2022
- https://www.iso.org/standard/11502.html
- https://standards.iteh.ai/catalog/standards/iso/26ecbc6f-88d6-468b-bfa0-f86e1c7ff59b/iso-5456-2-1996

## What The Code Does Today

`main/opengeometry/src/export/projection.rs` defines:

- `VisibleOutline`
- `VisibleCrease`
- `VisibleSmooth`
- `Hidden`
- `SectionCut`

The classifier:

- marks an edge hidden when no adjacent faces are front-facing
- marks an outline when some adjacent faces are front-facing and some are not
- marks a crease when front-facing adjacent faces diverge
- suppresses `VisibleSmooth` by default

The projection loop emits a full edge segment when its class should be visible according to `HlrOptions`.

## What Is Correct

- Silhouette vs crease classification is a useful first step.
- The exporter does not invent special boolean/export logic; it projects final BRep edges.
- Hidden segments can be styled differently when `hide_hidden_edges` is false.

## What Is Missing

This is not true HLR because it does not split projected edges by occluding faces.

Examples that can be wrong:

- an edge partly visible and partly hidden behind another face
- an edge hidden by a non-adjacent face
- overlapping projected edges at different depths
- boolean-created internal edges hidden behind newly created faces
- curved or circular edges whose visibility changes along the curve

The current code classifies an entire topological edge as one class. Technical drawing output usually needs visibility at the segment/span level after projection and occlusion.

## Recommended Fix

Replace the edge-level visibility classifier with a span-level HLR pipeline:

1. Project all candidate edges.
2. Project all potentially occluding front-facing faces.
3. Split each projected edge at intersections with occluder boundaries and with overlapping edges.
4. Depth-test representative samples per span.
5. Emit `ClassifiedSegment` spans with correct `Visible*` or `Hidden` class.

This can stay kernel-side and still feed the existing Drawing IR.

## Acceptance Criteria

- A box behind another box exports the rear box edges as hidden or suppressed.
- A long edge crossing behind a foreground face is split into visible-hidden-visible spans.
- Boolean subtraction edges behind front faces are hidden.
- Curved edges are split or sampled with bounded error.
- Tests compare exact segment class counts for controlled fixtures.

