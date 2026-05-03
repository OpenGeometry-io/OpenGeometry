# Debt: Sheet Layout Is A4-Sized But Not ISO 5457-Compliant

## Status

OpenGeometry currently creates an A4 landscape page with margins and auto-laid-out views. This is not a complete ISO 5457 sheet layout.

## Standards Researched

- ISO 5457:1999, Technical product documentation — Sizes and layout of drawing sheets.
- ISO says ISO 5457:1999 was reviewed and confirmed in 2021 and remains current, with Amendment 1 from 2010.

Sources:

- https://www.iso.org/standard/29017.html
- https://webstore.ansi.org/standards/iso/iso54571999
- https://www.document-center.com/standards/show/ISO-5457

## What The Code Does Today

`main/opengeometry/src/export/drawing.rs` defaults to:

- page width: `297.0 mm`
- page height: `210.0 mm`
- margin: `10.0 mm`
- view gap: `8.0 mm`

This corresponds to A4 landscape dimensions. Views are arranged in an automatically computed grid.

## What Is Correct

- The coordinate system is millimetre-based.
- A4 landscape dimensions are real ISO paper dimensions.
- Multiple views can be placed on one page.

## What Is Missing

ISO 5457 is about sheet sizes and layout of drawing sheets, not only page dimensions. We do not yet implement:

- standard border/frame
- title block placement and reserved zone
- filing margin
- centering marks
- trimming marks
- grid references
- multiple standard sheet sizes A0-A4 as named presets
- portrait/landscape policy per sheet size
- drawing zone separated from title-block zone

The current layout may overlap professional title-block expectations because every view is packed into the drawable area using a simple grid.

## Recommended Fix

Add a real sheet model:

```rust
pub enum SheetStandard {
    Iso5457,
}

pub enum IsoSheetSize {
    A0,
    A1,
    A2,
    A3,
    A4,
}

pub struct DrawingSheetLayout {
    pub outer_size_mm: Vec2,
    pub frame_rect_mm: Rect2,
    pub drawing_area_mm: Rect2,
    pub title_block_rect_mm: Rect2,
}
```

OpenGeometry should expose sheet geometry in the Drawing IR. PDF/DXF exporters should render the frame/title block from the same IR.

## Acceptance Criteria

- A4, A3, A2, A1, A0 are selectable by name.
- Border/frame geometry is emitted into PDF and DXF.
- Title block occupies a reserved region and views cannot overlap it.
- Tests assert sheet dimensions and drawing-area rectangles for each preset.
- Exported DXF includes sheet/frame linework on a distinct layer.

