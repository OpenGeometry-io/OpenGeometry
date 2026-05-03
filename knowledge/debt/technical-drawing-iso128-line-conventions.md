# Debt: ISO 128 Line Conventions Are Only Partially Implemented

## Status

OpenGeometry currently implements an ISO 128-inspired line-style mapping, but not a complete ISO 128 line convention.

## Standards Researched

- ISO 128-2:2022, Technical product documentation — General principles of representation — Basic conventions for lines.
- ISO 128-2:2020 public metadata and previews. ISO lists the 2020 edition as withdrawn and revised by ISO 128-2:2022, but public previews of the 2020 text expose the same important line-width series and relative line-dimension rules that matter to this implementation.

Sources:

- https://www.iso.org/standard/69129.html
- https://www.iso.org/es/contents/data/standard/06/91/69129.html
- https://standards.iteh.ai/catalog/standards/cen/f56e654f-f6f4-4d33-8920-4b35f45efe2f/en-iso-128-2-2020
- https://standards.iteh.ai/catalog/standards/iso/a0c6a7e3-b7fa-4ea2-8d4b-6858cf97df24/iso-128-2-2020

## What The Code Does Today

The current mapping lives in `main/opengeometry/src/export/drawing.rs`:

- `VisibleOutline` -> continuous `0.50 mm`
- `VisibleCrease` -> continuous `0.25 mm`
- `VisibleSmooth` -> continuous `0.18 mm`
- `Hidden` -> dashed `0.18 mm`, dash pattern `[3.0, 1.5]`
- `SectionCut` -> chain-style `0.70 mm`, dash pattern `[12.0, 3.0, 2.0, 3.0]`

PDF export converts these values from mm to PDF points in `main/opengeometry/src/export/pdf.rs`.

DXF export writes equivalent layer/entity lineweights as group code `370` in `main/opengeometry/src/export/dxf.rs`.

## What Is Correct

- The chosen widths are from the common ISO line-width series: `0.13, 0.18, 0.25, 0.35, 0.50, 0.70, 1.00, 1.40, 2.00 mm`.
- The implementation differentiates wide, narrow, hidden, and section/cutting line intent.
- The stroke width remains constant along each primitive.
- PDF and DXF both preserve physical lineweight rather than rendering everything as display pixels.

## What Is Missing

The dash patterns are fixed literal millimetre values, not generated from ISO 128's line-element rules.

Public previews of ISO 128-2 describe line element lengths in relation to line width `d`, including concepts such as gaps, dashes, short dashes, and dots. Our code does not derive dash lengths from `d`; it hardcodes:

- hidden: `3.0 mm` dash, `1.5 mm` gap
- section: `12.0 mm` dash, `3.0 mm` gap, `2.0 mm` dash, `3.0 mm` gap

That may look reasonable visually, but it is not yet a standards-backed implementation.

The code also does not implement:

- configurable line families by discipline, such as mechanical vs construction drawing annex choices
- explicit line-type names matching ISO line designations
- dot semantics for chain lines; the current section pattern uses a short dash, not a mathematically dot-sized element
- scale-aware line pattern adjustment for very small details
- validation that neighbouring line widths remain visually distinguishable
- centerline or dimension-line classes

## Recommended Fix

Add a standards-aware line-style module:

```rust
pub enum DrawingStandard {
    Iso128,
}

pub enum IsoLineType {
    Continuous,
    Dashed,
    LongDashDot,
    ChainThin,
    ChainThick,
}

pub struct StandardLineStyle {
    pub width_mm: f64,
    pub pattern: LinePattern,
}
```

Dash patterns should be generated from the selected `d` width and named line type instead of being hardcoded per `EdgeClass`.

## Acceptance Criteria

- Unit tests assert the ISO line-width series is used.
- Unit tests assert hidden/chain patterns are generated from `d`.
- PDF test confirms krilla receives converted dash lengths.
- DXF test confirms LTYPE definitions match the generated pattern.
- Documentation clearly states whether the implementation targets ISO 128-2:2022 or a named internal OpenGeometry profile inspired by ISO 128.

