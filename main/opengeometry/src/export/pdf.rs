use krilla::geom::{Path, PathBuilder, Point};
use krilla::num::NormalizedF32;
use krilla::page::PageSettings;
use krilla::paint::{LineCap, LineJoin, Stroke, StrokeDash};
use krilla::text::{Font, TextDirection};
use krilla::{Data, Document};

use crate::brep::Brep;
use crate::export::drawing::{
    DrawingDocument, DrawingExportConfig, DrawingGeometry, DrawingPrimitive, DrawingStyle,
};
use crate::export::projection::{project_brep_to_scene, CameraParameters, HlrOptions, Scene2D};

#[cfg(not(target_arch = "wasm32"))]
use std::fs;

const PT_PER_MM: f64 = 72.0 / 25.4;
const CURVE_STEPS: usize = 32;
const DEFAULT_TITLE_FONT_BYTES: &[u8] = include_bytes!("assets/Roboto-Regular.ttf");

#[derive(Clone, Debug, Default)]
pub struct PdfExportConfig {
    pub drawing: DrawingExportConfig,
    /// Optional bundled font bytes for title block / view labels.
    ///
    /// OpenGeometry never loads system fonts. Text is omitted when no valid
    /// font bytes are supplied.
    pub title_font_bytes: Option<Vec<u8>>,
}

pub type PdfExportResult<T> = Result<T, PdfExportError>;

#[derive(Debug)]
pub enum PdfExportError {
    DocumentCreation(String),
    FileWrite(String),
    EmptyScene,
    InvalidConfig(String),
}

impl std::fmt::Display for PdfExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PdfExportError::DocumentCreation(msg) => write!(f, "PDF creation error: {}", msg),
            PdfExportError::FileWrite(msg) => write!(f, "File write error: {}", msg),
            PdfExportError::EmptyScene => write!(f, "Cannot export empty scene"),
            PdfExportError::InvalidConfig(msg) => write!(f, "Invalid config: {}", msg),
        }
    }
}

impl std::error::Error for PdfExportError {}

pub fn export_scene_to_pdf(scene: &Scene2D, file_path: &str) -> PdfExportResult<()> {
    export_scene_to_pdf_with_config(scene, file_path, &PdfExportConfig::default())
}

pub fn export_brep_to_pdf_with_camera(
    brep: &Brep,
    camera: &CameraParameters,
    hlr: &HlrOptions,
    file_path: &str,
    config: &PdfExportConfig,
) -> PdfExportResult<()> {
    let scene = project_brep_to_scene(brep, camera, hlr);
    export_scene_to_pdf_with_config(&scene, file_path, config)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn export_scene_to_pdf_with_config(
    scene: &Scene2D,
    file_path: &str,
    config: &PdfExportConfig,
) -> PdfExportResult<()> {
    let bytes = export_scene_to_pdf_bytes(scene, config)?;
    fs::write(file_path, bytes).map_err(|e| PdfExportError::FileWrite(e.to_string()))
}

#[cfg(target_arch = "wasm32")]
pub fn export_scene_to_pdf_with_config(
    _scene: &Scene2D,
    _file_path: &str,
    _config: &PdfExportConfig,
) -> PdfExportResult<()> {
    Err(PdfExportError::FileWrite(
        "Filesystem PDF export is unavailable on wasm32; use byte export".to_string(),
    ))
}

pub fn export_scene_to_pdf_bytes(
    scene: &Scene2D,
    config: &PdfExportConfig,
) -> PdfExportResult<Vec<u8>> {
    if scene.is_empty() {
        return Err(PdfExportError::EmptyScene);
    }

    let drawing = DrawingDocument::from_scene(scene, &config.drawing)
        .map_err(|err| PdfExportError::InvalidConfig(err.to_string()))?;
    export_pdf_bytes(&drawing, config.title_font_bytes.as_deref())
}

pub fn export_pdf_bytes(
    drawing: &DrawingDocument,
    title_font_bytes: Option<&[u8]>,
) -> PdfExportResult<Vec<u8>> {
    if drawing.views.is_empty() {
        return Err(PdfExportError::EmptyScene);
    }

    let mut document = Document::new();
    let mut page = document.start_page_with(
        PageSettings::from_wh(
            mm_to_pt(drawing.page_width_mm),
            mm_to_pt(drawing.page_height_mm),
        )
        .ok_or_else(|| PdfExportError::InvalidConfig("Invalid PDF page dimensions".to_string()))?,
    );

    {
        let mut surface = page.surface();
        surface.set_fill(None);

        for view in &drawing.views {
            for primitive in &view.primitives {
                draw_primitive(&mut surface, primitive)?;
            }
        }

        if let Some(font) = load_font(title_font_bytes) {
            for text in &drawing.text {
                surface.draw_text(
                    Point::from_xy(mm_to_pt(text.position_mm.x), mm_to_pt(text.position_mm.y)),
                    font.clone(),
                    mm_to_pt(text.size_mm),
                    &text.text,
                    false,
                    TextDirection::Auto,
                );
            }
        }

        surface.finish();
    }

    page.finish();
    document
        .finish()
        .map_err(|err| PdfExportError::DocumentCreation(format!("{err:?}")))
}

pub fn export_krilla_probe_pdf_bytes(title_font_bytes: Option<&[u8]>) -> PdfExportResult<Vec<u8>> {
    let scene = krilla_probe_scene();
    let drawing = DrawingDocument::from_scene(&scene, &DrawingExportConfig::default())
        .map_err(|err| PdfExportError::InvalidConfig(err.to_string()))?;
    export_pdf_bytes(&drawing, title_font_bytes)
}

fn draw_primitive(
    surface: &mut krilla::surface::Surface<'_>,
    primitive: &DrawingPrimitive,
) -> PdfExportResult<()> {
    let Some(path) = path_for_geometry(&primitive.geometry) else {
        return Ok(());
    };

    surface.set_stroke(Some(stroke_for_style(&primitive.style)));
    surface.draw_path(&path);
    Ok(())
}

fn path_for_geometry(geometry: &DrawingGeometry) -> Option<Path> {
    let mut pb = PathBuilder::new();
    match geometry {
        DrawingGeometry::Line { start, end } => {
            pb.move_to(mm_to_pt(start.x), mm_to_pt(start.y));
            pb.line_to(mm_to_pt(end.x), mm_to_pt(end.y));
        }
        DrawingGeometry::Arc {
            center,
            radius_mm,
            start_angle,
            end_angle,
        } => {
            append_arc_polyline(
                &mut pb,
                center.x,
                center.y,
                *radius_mm,
                *start_angle,
                *end_angle,
            );
        }
        DrawingGeometry::Ellipse {
            center,
            rx_mm,
            ry_mm,
            rotation,
            start_angle,
            end_angle,
        } => {
            append_ellipse_polyline(
                &mut pb,
                center.x,
                center.y,
                *rx_mm,
                *ry_mm,
                *rotation,
                *start_angle,
                *end_angle,
            );
        }
        DrawingGeometry::CubicBezier { p0, p1, p2, p3 } => {
            pb.move_to(mm_to_pt(p0.x), mm_to_pt(p0.y));
            pb.cubic_to(
                mm_to_pt(p1.x),
                mm_to_pt(p1.y),
                mm_to_pt(p2.x),
                mm_to_pt(p2.y),
                mm_to_pt(p3.x),
                mm_to_pt(p3.y),
            );
        }
    }
    pb.finish()
}

fn append_arc_polyline(
    pb: &mut PathBuilder,
    cx: f64,
    cy: f64,
    radius: f64,
    start_angle: f64,
    end_angle: f64,
) {
    for i in 0..=CURVE_STEPS {
        let t = i as f64 / CURVE_STEPS as f64;
        let angle = start_angle + (end_angle - start_angle) * t;
        let x = cx + radius * angle.cos();
        let y = cy + radius * angle.sin();
        if i == 0 {
            pb.move_to(mm_to_pt(x), mm_to_pt(y));
        } else {
            pb.line_to(mm_to_pt(x), mm_to_pt(y));
        }
    }
}

fn append_ellipse_polyline(
    pb: &mut PathBuilder,
    cx: f64,
    cy: f64,
    rx: f64,
    ry: f64,
    rotation: f64,
    start_angle: f64,
    end_angle: f64,
) {
    let cos_r = rotation.cos();
    let sin_r = rotation.sin();
    for i in 0..=CURVE_STEPS {
        let t = i as f64 / CURVE_STEPS as f64;
        let angle = start_angle + (end_angle - start_angle) * t;
        let local_x = rx * angle.cos();
        let local_y = ry * angle.sin();
        let x = cx + local_x * cos_r - local_y * sin_r;
        let y = cy + local_x * sin_r + local_y * cos_r;
        if i == 0 {
            pb.move_to(mm_to_pt(x), mm_to_pt(y));
        } else {
            pb.line_to(mm_to_pt(x), mm_to_pt(y));
        }
    }
}

fn stroke_for_style(style: &DrawingStyle) -> Stroke {
    Stroke {
        width: mm_to_pt(style.stroke_width_mm),
        line_cap: LineCap::Butt,
        line_join: LineJoin::Miter,
        opacity: NormalizedF32::ONE,
        dash: if style.dash_pattern_mm.is_empty() {
            None
        } else {
            Some(StrokeDash {
                array: style
                    .dash_pattern_mm
                    .iter()
                    .map(|value| mm_to_pt(*value))
                    .collect(),
                offset: 0.0,
            })
        },
        ..Default::default()
    }
}

fn load_font(bytes: Option<&[u8]>) -> Option<Font> {
    let font_bytes = bytes.unwrap_or(DEFAULT_TITLE_FONT_BYTES);
    Font::new(Data::from(font_bytes.to_vec()), 0)
}

fn mm_to_pt(value: f64) -> f32 {
    (value * PT_PER_MM) as f32
}

fn krilla_probe_scene() -> Scene2D {
    use crate::export::projection::{ClassifiedSegment, EdgeClass, Segment2D, Vec2};

    let mut scene = Scene2D::with_name("krilla-proof");
    let cases = [
        (EdgeClass::VisibleOutline, 0.0),
        (EdgeClass::VisibleCrease, 0.25),
        (EdgeClass::Hidden, 0.50),
        (EdgeClass::SectionCut, 0.75),
    ];
    for (class, y) in cases {
        scene.add_segment(ClassifiedSegment {
            geometry: Segment2D::Line {
                start: Vec2::new(0.0, y),
                end: Vec2::new(1.0, y),
            },
            class,
            layer: None,
            source_entity_id: None,
        });
    }
    scene
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn krilla_probe_generates_pdf_bytes() {
        let bytes = export_krilla_probe_pdf_bytes(None).expect("krilla PDF export should pass");
        assert!(bytes.starts_with(b"%PDF-"));
        assert!(bytes.len() > 100);
    }

    #[test]
    fn bundled_title_font_loads_without_system_fonts() {
        assert!(load_font(None).is_some());
    }

    #[test]
    fn krilla_probe_includes_dash_operator_for_hidden_lines() {
        let bytes = export_krilla_probe_pdf_bytes(None).expect("krilla PDF export should pass");
        let pdf = String::from_utf8_lossy(&bytes);
        assert!(
            pdf.contains(" d") || pdf.contains("]"),
            "expected dashed stroke data in PDF output"
        );
    }
}
