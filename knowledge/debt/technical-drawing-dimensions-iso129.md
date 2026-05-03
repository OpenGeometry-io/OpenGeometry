# Debt: Dimensions And Tolerances Are Not Implemented

## Status

OpenGeometry currently exports projected geometry linework only. It does not implement dimensioning, tolerances, leaders, arrows, or measurement annotations.

## Standards Researched

- ISO 129-1:2018, Technical product documentation — Presentation of dimensions and tolerances — General principles.
- ISO says ISO 129-1:2018 was reviewed and confirmed in 2023 and remains current, with Amendment 1 from 2020.

Sources:

- https://www.iso.org/standard/64007.html
- https://webstore.ansi.org/standards/iso/iso1292018
- https://www.document-center.com/standards/show/ISO-129-1

## What The Code Does Today

The Drawing IR supports:

- line
- arc
- ellipse
- cubic bezier
- simple text

There are no dimension-specific primitives.

## What Is Correct

The existing primitive set could be used to draw dimensions manually, but there is no semantic support yet.

## What Is Missing

The kernel does not have:

- aligned dimensions
- linear horizontal/vertical dimensions
- radial or diameter dimensions
- angular dimensions
- extension/witness lines
- dimension lines
- arrowheads or ticks
- dimension text placement rules
- tolerances
- prefixes/suffixes
- leaders and reference lines
- property indicators
- scale-aware annotation sizing

ISO 129-1 covers presentation rules for dimensions and associated tolerances on 2D technical drawings. None of that exists as code today.

## Recommended Fix

Add semantic annotation primitives rather than flattening dimensions too early:

```rust
pub enum DrawingAnnotation {
    LinearDimension(LinearDimension),
    RadialDimension(RadialDimension),
    AngularDimension(AngularDimension),
    Leader(LeaderAnnotation),
}
```

Exporters may lower these to line/text primitives, but the Drawing IR should preserve the annotation intent long enough to support PDF and DXF differently.

## Acceptance Criteria

- Add at least linear, radial, diameter, and angular dimension primitives.
- PDF output renders extension lines, dimension lines, arrows/ticks, and centered text.
- DXF output emits `DIMENSION` entities if practical, or documented fallback geometry/text.
- Tests verify dimensions are stable in mm paper space independent of model scale.
- Tolerances are either implemented or explicitly rejected with a structured error.

