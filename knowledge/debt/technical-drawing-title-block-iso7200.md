# Debt: Title Block Is Plain Text, Not ISO 7200 Metadata

## Status

OpenGeometry currently writes a simple title string and view labels using bundled font bytes. It does not implement ISO 7200 title-block fields.

## Standards Researched

- ISO 7200:2004, Technical product documentation — Data fields in title blocks and document headers.
- ISO says ISO 7200:2004 was reviewed and confirmed in 2025 and remains current.

Sources:

- https://www.iso.org/es/contents/data/standard/03/54/35446.html
- https://webstore.ansi.org/standards/iso/iso72002004
- https://standards.iteh.ai/catalog/standards/iso/cda0743a-d12c-45c6-87fd-4e96ffda6041/iso-7200-2004

## What The Code Does Today

`DrawingExportConfig` supports:

- `title: Option<String>`
- `show_view_labels: bool`

`DrawingDocument` has:

- `title: Option<String>`
- `text: Vec<DrawingText>`

The PDF exporter draws text using bundled Roboto font bytes from `main/opengeometry/src/export/assets/Roboto-Regular.ttf`.

## What Is Correct

- Text is embedded from bundled bytes rather than loaded from system fonts.
- The output can include a basic drawing title.
- The implementation works in native and WASM contexts.

## What Is Missing

ISO 7200 defines data fields for title blocks and document headers. The current implementation has no structured fields for:

- owner/legal owner
- document number
- title
- supplementary title
- revision
- date of issue
- creator
- approver
- sheet number
- scale
- document status
- language
- responsible department

It also has no title-block geometry:

- no table/grid lines
- no field layout
- no field length validation
- no metadata-to-visual mapping
- no DXF text export for title-block fields

## Recommended Fix

Add a structured title-block model:

```rust
pub struct DrawingTitleBlock {
    pub owner: Option<String>,
    pub document_id: Option<String>,
    pub title: String,
    pub revision: Option<String>,
    pub issue_date: Option<String>,
    pub creator: Option<String>,
    pub approver: Option<String>,
    pub sheet_number: Option<String>,
    pub sheet_count: Option<String>,
    pub scale: Option<String>,
    pub status: Option<String>,
}
```

Then add title-block primitives to the Drawing IR so PDF and DXF share the same geometry and text layout.

## Acceptance Criteria

- Title-block fields are structured, not free-form text only.
- PDF renders title-block grid and text.
- DXF emits title-block lines and `TEXT` or `MTEXT` entities.
- Tests verify required fields and maximum field lengths if we choose to enforce ISO 7200-compatible limits.
- Export config can intentionally choose a profile: minimal, ISO 7200, or application-defined.

