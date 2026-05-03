# Debt: DXF Output Is Structurally Valid But Not CAD-Standards Complete

## Status

OpenGeometry now writes a focused ASCII DXF file from the Drawing IR. It includes the important table sections and lineweight group codes, but it is not a complete CAD standards implementation.

## Standards And References Researched

- Autodesk DXF reference for `LTYPE` table group codes.
- Autodesk DXF group code reference.
- AIA CAD Layer Guidelines / US National CAD Standard layer naming conventions.

Sources:

- https://help.autodesk.com/cloudhelp/2024/ENU/AutoCAD-DXF/files/GUID-F57A316C-94A2-416C-8280-191E34B182AC.htm
- https://help.autodesk.com/cloudhelp/2018/ENU/AutoCAD-DXF/files/GUID-3F0380A5-1C15-464D-BC66-2C5F094BCFB9.htm
- https://pages.uoregon.edu/arch/landcad/Layer_Standards.htm

## What The Code Does Today

`main/opengeometry/src/export/dxf.rs` emits:

- `HEADER`
- `TABLES`
- `LTYPE`
- `LAYER`
- `ENTITIES`
- `EOF`

The header uses:

- `$ACADVER = AC1021`
- `$INSUNITS = 4` for millimetres

Layer names:

- `OG-VISIBLE-OUTLINE`
- `OG-VISIBLE-CREASE`
- `OG-HIDDEN`
- `OG-SECTION`

Linetypes:

- `CONTINUOUS`
- `HIDDEN`
- `SECTION`

Entity lineweight is emitted through group code `370`.

## What Is Correct

- `LTYPE` entries use recognized DXF group codes such as `2`, `3`, `40`, `49`, `73`, `74`.
- Layer and entity lineweights use group code `370`.
- `$INSUNITS = 4` is the AutoCAD convention for millimetres.
- Hidden and section linetypes are represented in the table and referenced by entities.

## What Is Missing

The DXF writer does not yet implement:

- handles and owner references
- full `OBJECTS` section
- text styles
- dimension styles
- layouts/paper space
- blocks
- title block text/entities
- `TEXT`, `MTEXT`, `DIMENSION`, `HATCH`, `SPLINE`, or native `ELLIPSE` entities
- AIA/NCS layer naming
- semantic layer mapping from entity kind, such as wall -> `A-WALL`, door -> `A-DOOR`, glazing -> `A-GLAZ`
- per-layer plot style/color standards
- validation against common CAD readers beyond basic file structure

Curves are also approximated:

- ellipses are written as line polylines
- cubic beziers are written as line segments

This is acceptable as a first interchange path but not a high-fidelity CAD exchange layer.

## Recommended Fix

Add a DXF profile:

```rust
pub enum DxfProfile {
    MinimalAscii,
    Acad2007PaperSpace,
    AiaNcs,
}
```

Then add:

- entity handles
- paper-space layout support
- native `ELLIPSE` and `SPLINE` output where possible
- `TEXT`/`MTEXT` for title blocks
- semantic layer mapping from `RegisteredEntity.kind`
- optional AIA/NCS layer table

## Acceptance Criteria

- DXF opens cleanly in LibreCAD and ODA File Converter.
- DXF opens in AutoCAD-compatible viewers with expected layer names, linetypes, and lineweights.
- AIA/NCS mode emits `A-WALL`, `A-DOOR`, `A-GLAZ`, etc. when entity kind is available.
- Ellipses are emitted as native DXF `ELLIPSE` or a documented approximation with tolerance.
- Cubic curves are emitted as `SPLINE` or a documented approximation with tolerance.
- Title block and sheet frame appear in DXF, not only PDF.

