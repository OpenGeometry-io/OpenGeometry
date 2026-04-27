use crate::brep::Brep;
use crate::export::projection::{
    project_brep_to_scene, CameraParameters, ClassifiedSegment, EdgeClass, HlrOptions, Scene2D,
    Segment2D, Vec2,
};
use printpdf::*;
use std::fs::File;
use std::io::BufWriter;

const METERS_TO_MM: f64 = 1000.0;
const DEFAULT_MARGIN_MM: f64 = 10.0;

/// ISO 128 line weight in mm per edge class.
fn iso128_line_width_mm(class: EdgeClass) -> f64 {
    match class {
        EdgeClass::VisibleOutline => 0.50,
        EdgeClass::VisibleCrease => 0.25,
        EdgeClass::VisibleSmooth => 0.18,
        EdgeClass::Hidden => 0.18,
        EdgeClass::SectionCut => 0.70,
    }
}

#[derive(Clone, Debug)]
pub struct PdfExportConfig {
    pub page_width_mm: f64,
    pub page_height_mm: f64,
    pub margin_mm: f64,
    /// Ignored — line widths are now determined by ISO 128 edge class.
    #[deprecated(note = "Line width is now set per-class via ISO 128. Use EdgeClass instead.")]
    pub line_width_mm: f64,
    pub auto_fit: bool,
    pub title: Option<String>,
}

#[allow(deprecated)]
impl Default for PdfExportConfig {
    fn default() -> Self {
        Self {
            page_width_mm: 297.0,
            page_height_mm: 210.0,
            margin_mm: DEFAULT_MARGIN_MM,
            line_width_mm: 0.25,
            auto_fit: true,
            title: None,
        }
    }
}

#[allow(deprecated)]
impl PdfExportConfig {
    pub fn a4_portrait() -> Self {
        Self {
            page_width_mm: 210.0,
            page_height_mm: 297.0,
            ..Default::default()
        }
    }

    pub fn a4_landscape() -> Self {
        Self::default()
    }

    pub fn a3_landscape() -> Self {
        Self {
            page_width_mm: 420.0,
            page_height_mm: 297.0,
            ..Default::default()
        }
    }

    pub fn custom(width_mm: f64, height_mm: f64) -> Self {
        Self {
            page_width_mm: width_mm,
            page_height_mm: height_mm,
            ..Default::default()
        }
    }
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

pub fn export_scene_to_pdf_with_config(
    scene: &Scene2D,
    file_path: &str,
    config: &PdfExportConfig,
) -> PdfExportResult<()> {
    let (doc, page1, layer1, scale, offset_x, offset_y) = build_pdf_document(scene, config)?;

    let layer = doc.get_page(page1).get_layer(layer1);
    draw_classified_segments(&layer, scene.segments(), scale, offset_x, offset_y);

    let file = File::create(file_path).map_err(|e| PdfExportError::FileWrite(e.to_string()))?;
    let mut writer = BufWriter::new(file);
    doc.save(&mut writer)
        .map_err(|e| PdfExportError::FileWrite(e.to_string()))?;

    Ok(())
}

pub fn export_scene_to_pdf_bytes(
    scene: &Scene2D,
    config: &PdfExportConfig,
) -> PdfExportResult<Vec<u8>> {
    let (doc, page1, layer1, scale, offset_x, offset_y) = build_pdf_document(scene, config)?;

    let layer = doc.get_page(page1).get_layer(layer1);
    draw_classified_segments(&layer, scene.segments(), scale, offset_x, offset_y);

    doc.save_to_bytes()
        .map_err(|e| PdfExportError::DocumentCreation(e.to_string()))
}

#[allow(clippy::type_complexity)]
fn build_pdf_document(
    scene: &Scene2D,
    config: &PdfExportConfig,
) -> PdfExportResult<(
    PdfDocumentReference,
    PdfPageIndex,
    PdfLayerIndex,
    f64,
    f64,
    f64,
)> {
    if scene.is_empty() {
        return Err(PdfExportError::EmptyScene);
    }

    let drawable_width_mm = config.page_width_mm - 2.0 * config.margin_mm;
    let drawable_height_mm = config.page_height_mm - 2.0 * config.margin_mm;

    if drawable_width_mm <= 0.0 || drawable_height_mm <= 0.0 {
        return Err(PdfExportError::InvalidConfig(
            "Margins too large for page size".to_string(),
        ));
    }

    let scene_bounds = scene.bounding_box().ok_or(PdfExportError::EmptyScene)?;
    let scene_width = scene_bounds.1.x - scene_bounds.0.x;
    let scene_height = scene_bounds.1.y - scene_bounds.0.y;

    let scale = if config.auto_fit && (scene_width > 0.0 || scene_height > 0.0) {
        let scale_x = if scene_width > 0.0 {
            drawable_width_mm / (scene_width * METERS_TO_MM)
        } else {
            1.0
        };
        let scale_y = if scene_height > 0.0 {
            drawable_height_mm / (scene_height * METERS_TO_MM)
        } else {
            1.0
        };
        scale_x.min(scale_y)
    } else {
        1.0
    };

    let scaled_width = scene_width * METERS_TO_MM * scale;
    let scaled_height = scene_height * METERS_TO_MM * scale;
    let offset_x = config.margin_mm + (drawable_width_mm - scaled_width) / 2.0
        - scene_bounds.0.x * METERS_TO_MM * scale;
    let offset_y = config.margin_mm + (drawable_height_mm - scaled_height) / 2.0
        - scene_bounds.0.y * METERS_TO_MM * scale;

    let title = config
        .title
        .clone()
        .or_else(|| scene.name.clone())
        .unwrap_or_else(|| "OpenGeometry Export".to_string());

    let (doc, page1, layer1) = PdfDocument::new(
        &title,
        Mm(config.page_width_mm),
        Mm(config.page_height_mm),
        "Layer 1",
    );

    Ok((doc, page1, layer1, scale, offset_x, offset_y))
}

fn draw_classified_segments(
    layer: &PdfLayerReference,
    segments: &[ClassifiedSegment],
    scale: f64,
    offset_x: f64,
    offset_y: f64,
) {
    layer.set_outline_color(Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));

    for seg in segments {
        let line_width = iso128_line_width_mm(seg.class);
        layer.set_outline_thickness(line_width);

        match &seg.geometry {
            Segment2D::Line { start, end } => {
                let start_mm = transform_point(start, scale, offset_x, offset_y);
                let end_mm = transform_point(end, scale, offset_x, offset_y);

                let line = Line {
                    points: vec![
                        (Point::new(Mm(start_mm.0), Mm(start_mm.1)), false),
                        (Point::new(Mm(end_mm.0), Mm(end_mm.1)), false),
                    ],
                    is_closed: false,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };

                layer.add_shape(line);
            }
            // Arc, Ellipse, CubicBezier segments are produced in Phase 2+.
            _ => {}
        }
    }
}

fn transform_point(point: &Vec2, scale: f64, offset_x: f64, offset_y: f64) -> (f64, f64) {
    (
        point.x * METERS_TO_MM * scale + offset_x,
        point.y * METERS_TO_MM * scale + offset_y,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transform_point() {
        let point = Vec2::new(1.0, 2.0);
        let result = transform_point(&point, 1.0, 10.0, 20.0);
        assert!((result.0 - 1010.0).abs() < 0.001);
        assert!((result.1 - 2020.0).abs() < 0.001);
    }

    #[test]
    fn test_config_defaults() {
        #[allow(deprecated)]
        let config = PdfExportConfig::default();
        assert_eq!(config.page_width_mm, 297.0);
        assert_eq!(config.page_height_mm, 210.0);
    }

    #[test]
    fn iso128_line_widths_are_within_standard() {
        let widths = [
            iso128_line_width_mm(EdgeClass::VisibleOutline),
            iso128_line_width_mm(EdgeClass::VisibleCrease),
            iso128_line_width_mm(EdgeClass::Hidden),
            iso128_line_width_mm(EdgeClass::SectionCut),
        ];
        for w in widths {
            assert!(w >= 0.13 && w <= 2.0, "width {w} outside ISO 128 range");
        }
    }
}
