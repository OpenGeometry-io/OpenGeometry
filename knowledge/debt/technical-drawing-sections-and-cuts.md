# Debt: Section And Cut Standards Are Not Implemented

## Status

OpenGeometry has a `SectionCut` edge class and a section line style, but it does not yet generate real section geometry from a section plane.

## Standards Researched

- ISO 128-3:2022, Technical product documentation — General principles of representation — Views, sections and cuts.
- ISO 5456-2:1996, Orthographic representations, referenced by ISO 128-3 for projection methods.

Sources:

- https://www.iso.org/standard/83356.html
- https://standards.iteh.ai/catalog/standards/cen/b1e7976f-ddcf-425c-80f5-097c4cbd8439/en-iso-128-3-2022
- https://www.iso.org/standard/11502.html

## What The Code Does Today

`EdgeClass::SectionCut` exists in `main/opengeometry/src/export/projection.rs`.

`iso128_style()` maps it to:

- `0.70 mm`
- dash pattern `[12.0, 3.0, 2.0, 3.0]`

`OGEntityRegistry` accepts a `section_plane` field in view requests, but the current code path marks it as future wiring and does not apply the plane.

## What Is Correct

- The data model has a place to carry section-cut semantics.
- PDF and DXF can already style a segment as section/cut once the projection code emits one.

## What Is Missing

The kernel does not yet:

- intersect BReps with a section plane
- classify intersection curves as `SectionCut`
- clip geometry on one side of the section plane
- generate hatch/fill patterns for cut faces
- label section views
- generate cutting-plane indicators, arrows, or references
- distinguish section view linework from cutting-plane linework

This means current "section" support is only a style slot, not a standards-compliant section drawing feature.

## Recommended Fix

Add a section projection mode:

```rust
pub struct SectionViewRequest {
    pub camera: CameraParameters,
    pub hlr: HlrOptions,
    pub section_plane: SectionPlane,
    pub show_cut_hatch: bool,
}
```

Pipeline:

1. Clip or slice BRep by section plane.
2. Generate intersection loops.
3. Emit intersection loops as `SectionCut`.
4. Project remaining visible geometry normally.
5. Optionally emit hatch primitives for cut faces.

## Acceptance Criteria

- A cuboid cut through the middle emits a rectangular `SectionCut` loop.
- A cuboid with opening cut through the opening emits correct inner and outer cut loops.
- SectionCut segments appear at `0.70 mm` in PDF and DXF.
- Hatch primitives are represented in the Drawing IR or deliberately marked as out of scope.
- Section plane arrows/labels have a separate Drawing IR primitive or annotation type.

