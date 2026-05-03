# Debt: Projection Method And Drawing Scale Are Not Standards-Aware

## Status

OpenGeometry can export multiple orthographic-like views, but the sheet does not declare or enforce first-angle/third-angle projection conventions or ISO drawing scales.

## Standards Researched

- ISO 5456-2:1996, Technical drawings — Projection methods — Orthographic representations.
- ISO 5455, Technical drawings — Scales. Public ISO metadata was less useful in search results, but this remains the relevant standard family for technical drawing scale declarations.

Sources:

- https://www.iso.org/standard/11502.html
- https://standards.iteh.ai/catalog/standards/iso/26ecbc6f-88d6-468b-bfa0-f86e1c7ff59b/iso-5456-2-1996
- https://webstore.ansi.org/standards/iso/iso54561996-1021905

## What The Code Does Today

`DrawingDocument::from_scenes()` lays out views in a grid. `build_view()` computes a scale to fit projected bounds into the view cell and clamps the scale to at most `1.0`.

The browser example requests views named front, top, right, and isometric, but the layout engine treats them as independent views and does not encode a projection convention.

## What Is Correct

- Multiple views can appear on one sheet.
- Orthographic camera parameters can be passed through the registry.
- The exported coordinates are in paper-space millimetres.

## What Is Missing

The current system does not:

- distinguish first-angle from third-angle layout
- draw first-angle or third-angle projection symbols
- enforce view adjacency rules
- choose a principal/front view according to drawing convention
- support named scales such as `1:1`, `1:2`, `1:5`, `1:10`, `1:20`, `1:50`, `1:100`
- emit scale text in the title block or per view
- prevent arbitrary auto-fit scaling from producing undocumented drawing scales
- handle axonometric/isometric views as a separately labelled projection type

## Recommended Fix

Add a projection/layout profile:

```rust
pub enum ProjectionConvention {
    FirstAngle,
    ThirdAngle,
    Free,
}

pub enum DrawingScale {
    OneToOne,
    Ratio { paper: u32, model: u32 },
}

pub struct DrawingViewPlacement {
    pub projection_convention: ProjectionConvention,
    pub scale: DrawingScale,
    pub label: Option<String>,
}
```

Auto-fit should become an explicit mode, not the silent default for technical drawings.

## Acceptance Criteria

- First-angle and third-angle profiles place top/right/left views differently and predictably.
- Exported sheet includes a projection convention symbol or explicit metadata.
- Each view has a declared scale.
- Tests verify the same model exports at exact `1:1` and `1:10` paper-space dimensions.
- Isometric views are labelled and not confused with orthographic projection views.

