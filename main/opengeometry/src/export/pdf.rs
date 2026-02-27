/**
 * PDF Vector Export
 * 
 * Exports Scene2D to vector PDF format using the printpdf crate.
 * All output is true vector graphics - no rasterization.
 */

use crate::drawing::{Scene2D, Path2D, Segment2D, Vec2};
use printpdf::*;
use std::fs::File;
use std::io::BufWriter;

/// Conversion factor: meters to millimeters
const METERS_TO_MM: f32 = 1000.0;

/// Default line width in millimeters
const DEFAULT_LINE_WIDTH_MM: f32 = 0.25;

/// Default page margin in millimeters
const DEFAULT_MARGIN_MM: f32 = 10.0;

/// PDF export configuration
#[derive(Clone, Debug)]
pub struct PdfExportConfig {
    /// Page width in millimeters
    pub page_width_mm: f32,
    /// Page height in millimeters
    pub page_height_mm: f32,
    /// Page margin in millimeters
    pub margin_mm: f32,
    /// Default line width in millimeters
    pub line_width_mm: f32,
    /// Whether to auto-fit content to page
    pub auto_fit: bool,
    /// Document title
    pub title: Option<String>,
}

impl Default for PdfExportConfig {
    fn default() -> Self {
        Self {
            page_width_mm: 297.0,  // A4 landscape width
            page_height_mm: 210.0, // A4 landscape height
            margin_mm: DEFAULT_MARGIN_MM,
            line_width_mm: DEFAULT_LINE_WIDTH_MM,
            auto_fit: true,
            title: None,
        }
    }
}

impl PdfExportConfig {
    /// Create A4 portrait configuration
    pub fn a4_portrait() -> Self {
        Self {
            page_width_mm: 210.0,
            page_height_mm: 297.0,
            ..Default::default()
        }
    }

    /// Create A4 landscape configuration
    pub fn a4_landscape() -> Self {
        Self::default()
    }

    /// Create A3 landscape configuration
    pub fn a3_landscape() -> Self {
        Self {
            page_width_mm: 420.0,
            page_height_mm: 297.0,
            ..Default::default()
        }
    }

    /// Create custom page size configuration
    pub fn custom(width_mm: f32, height_mm: f32) -> Self {
        Self {
            page_width_mm: width_mm,
            page_height_mm: height_mm,
            ..Default::default()
        }
    }
}

/// Result type for PDF export operations
pub type PdfExportResult<T> = Result<T, PdfExportError>;

/// Errors that can occur during PDF export
#[derive(Debug)]
pub enum PdfExportError {
    /// Error creating the PDF document
    DocumentCreation(String),
    /// Error writing to file
    FileWrite(String),
    /// Scene is empty
    EmptyScene,
    /// Invalid configuration
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

/// Export a Scene2D to a PDF file with default configuration
pub fn export_scene_to_pdf(scene: &Scene2D, file_path: &str) -> PdfExportResult<()> {
    export_scene_to_pdf_with_config(scene, file_path, &PdfExportConfig::default())
}

/// Export a Scene2D to a PDF file with custom configuration
pub fn export_scene_to_pdf_with_config(
    scene: &Scene2D,
    file_path: &str,
    config: &PdfExportConfig,
) -> PdfExportResult<()> {
    if scene.is_empty() {
        return Err(PdfExportError::EmptyScene);
    }

    // Calculate drawable area
    let drawable_width_mm = config.page_width_mm - 2.0 * config.margin_mm;
    let drawable_height_mm = config.page_height_mm - 2.0 * config.margin_mm;

    if drawable_width_mm <= 0.0 || drawable_height_mm <= 0.0 {
        return Err(PdfExportError::InvalidConfig(
            "Margins too large for page size".to_string(),
        ));
    }

    // Get scene bounds (convert from f64 to f32)
    let scene_bounds = scene.bounding_box().ok_or(PdfExportError::EmptyScene)?;
    let scene_width = (scene_bounds.1.x - scene_bounds.0.x) as f32;
    let scene_height = (scene_bounds.1.y - scene_bounds.0.y) as f32;

    // Calculate scale factor
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

    // Calculate offset to center the drawing
    let scaled_width = scene_width * METERS_TO_MM * scale;
    let scaled_height = scene_height * METERS_TO_MM * scale;
    let offset_x = config.margin_mm + (drawable_width_mm - scaled_width) / 2.0
        - (scene_bounds.0.x as f32) * METERS_TO_MM * scale;
    let offset_y = config.margin_mm + (drawable_height_mm - scaled_height) / 2.0
        - (scene_bounds.0.y as f32) * METERS_TO_MM * scale;

    // Create PDF document
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

    let current_layer = doc.get_page(page1).get_layer(layer1);

    // Set default line properties
    current_layer.set_outline_thickness(config.line_width_mm);
    current_layer.set_outline_color(Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));

    // Draw all paths
    for path in scene.paths() {
        draw_path_to_layer(&current_layer, path, scale, offset_x, offset_y, config);
    }

    // Save to file
    let file = File::create(file_path).map_err(|e| PdfExportError::FileWrite(e.to_string()))?;
    let mut writer = BufWriter::new(file);

    doc.save(&mut writer)
        .map_err(|e| PdfExportError::FileWrite(e.to_string()))?;

    Ok(())
}

/// Draw a single path to a PDF layer
fn draw_path_to_layer(
    layer: &PdfLayerReference,
    path: &Path2D,
    scale: f32,
    offset_x: f32,
    offset_y: f32,
    config: &PdfExportConfig,
) {
    if path.is_empty() {
        return;
    }

    // Apply path-specific styling if present
    if let Some(width) = path.stroke_width {
        layer.set_outline_thickness((width as f32) * METERS_TO_MM * scale);
    }

    if let Some((r, g, b)) = path.stroke_color {
        layer.set_outline_color(Color::Rgb(Rgb::new(r as f32, g as f32, b as f32, None)));
    }

    // Convert and draw each segment
    for segment in &path.segments {
        match segment {
            Segment2D::Line { start, end } => {
                let start_mm = transform_point(start, scale, offset_x, offset_y);
                let end_mm = transform_point(end, scale, offset_x, offset_y);

                // Build a line path using printpdf's Line type
                let line = Line {
                    points: vec![
                        (Point::new(Mm(start_mm.0), Mm(start_mm.1)), false),
                        (Point::new(Mm(end_mm.0), Mm(end_mm.1)), false),
                    ],
                    is_closed: false,
                };

                layer.add_line(line);
            }
        }
    }

    // Reset to default styling
    layer.set_outline_thickness(config.line_width_mm);
    layer.set_outline_color(Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));
}

/// Transform a point from scene coordinates to PDF coordinates (returns f32 tuple)
fn transform_point(point: &Vec2, scale: f32, offset_x: f32, offset_y: f32) -> (f32, f32) {
    (
        (point.x as f32) * METERS_TO_MM * scale + offset_x,
        (point.y as f32) * METERS_TO_MM * scale + offset_y,
    )
}

/// Export a Scene2D to PDF bytes (for WASM/web usage)
pub fn export_scene_to_pdf_bytes(scene: &Scene2D, config: &PdfExportConfig) -> PdfExportResult<Vec<u8>> {
    if scene.is_empty() {
        return Err(PdfExportError::EmptyScene);
    }

    // Calculate drawable area
    let drawable_width_mm = config.page_width_mm - 2.0 * config.margin_mm;
    let drawable_height_mm = config.page_height_mm - 2.0 * config.margin_mm;

    if drawable_width_mm <= 0.0 || drawable_height_mm <= 0.0 {
        return Err(PdfExportError::InvalidConfig(
            "Margins too large for page size".to_string(),
        ));
    }

    let scene_bounds = scene.bounding_box().ok_or(PdfExportError::EmptyScene)?;
    let scene_width = (scene_bounds.1.x - scene_bounds.0.x) as f32;
    let scene_height = (scene_bounds.1.y - scene_bounds.0.y) as f32;

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
        - (scene_bounds.0.x as f32) * METERS_TO_MM * scale;
    let offset_y = config.margin_mm + (drawable_height_mm - scaled_height) / 2.0
        - (scene_bounds.0.y as f32) * METERS_TO_MM * scale;

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

    let current_layer = doc.get_page(page1).get_layer(layer1);

    current_layer.set_outline_thickness(config.line_width_mm);
    current_layer.set_outline_color(Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));

    for path in scene.paths() {
        draw_path_to_layer(&current_layer, path, scale, offset_x, offset_y, config);
    }

    doc.save_to_bytes()
        .map_err(|e| PdfExportError::DocumentCreation(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transform_point() {
        let point = Vec2::new(1.0, 2.0); // 1m, 2m
        let result = transform_point(&point, 1.0, 10.0, 20.0);
        
        // 1m = 1000mm, plus offset
        assert!((result.0 - 1010.0).abs() < 0.001);
        assert!((result.1 - 2020.0).abs() < 0.001);
    }

    #[test]
    fn test_config_defaults() {
        let config = PdfExportConfig::default();
        assert_eq!(config.page_width_mm, 297.0);
        assert_eq!(config.page_height_mm, 210.0);
    }
}
