use serde::{Deserialize, Serialize};

use crate::export::projection::{ClassifiedSegment, EdgeClass, Scene2D, Segment2D, Vec2};

const METERS_TO_MM: f64 = 1000.0;
const DEFAULT_PAGE_WIDTH_MM: f64 = 297.0;
const DEFAULT_PAGE_HEIGHT_MM: f64 = 210.0;
const DEFAULT_MARGIN_MM: f64 = 10.0;
const DEFAULT_VIEW_GAP_MM: f64 = 8.0;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DrawingExportConfig {
    pub page_width_mm: f64,
    pub page_height_mm: f64,
    pub margin_mm: f64,
    pub view_gap_mm: f64,
    pub title: Option<String>,
    pub show_view_labels: bool,
}

impl Default for DrawingExportConfig {
    fn default() -> Self {
        Self {
            page_width_mm: DEFAULT_PAGE_WIDTH_MM,
            page_height_mm: DEFAULT_PAGE_HEIGHT_MM,
            margin_mm: DEFAULT_MARGIN_MM,
            view_gap_mm: DEFAULT_VIEW_GAP_MM,
            title: Some("OpenGeometry Technical Drawing".to_string()),
            show_view_labels: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DrawingDocument {
    pub page_width_mm: f64,
    pub page_height_mm: f64,
    pub title: Option<String>,
    pub views: Vec<DrawingView>,
    pub text: Vec<DrawingText>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DrawingView {
    pub id: String,
    pub origin_mm: Vec2,
    pub size_mm: Vec2,
    pub primitives: Vec<DrawingPrimitive>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DrawingPrimitive {
    pub geometry: DrawingGeometry,
    pub style: DrawingStyle,
    pub layer: Option<String>,
    pub source_entity_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DrawingGeometry {
    Line {
        start: Vec2,
        end: Vec2,
    },
    Arc {
        center: Vec2,
        radius_mm: f64,
        start_angle: f64,
        end_angle: f64,
    },
    Ellipse {
        center: Vec2,
        rx_mm: f64,
        ry_mm: f64,
        rotation: f64,
        start_angle: f64,
        end_angle: f64,
    },
    CubicBezier {
        p0: Vec2,
        p1: Vec2,
        p2: Vec2,
        p3: Vec2,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DrawingStyle {
    pub edge_class: EdgeClass,
    pub stroke_width_mm: f64,
    pub dash_pattern_mm: Vec<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DrawingText {
    pub position_mm: Vec2,
    pub text: String,
    pub size_mm: f64,
}

#[derive(Debug)]
pub enum DrawingBuildError {
    EmptyInput,
    InvalidConfig(String),
}

impl std::fmt::Display for DrawingBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DrawingBuildError::EmptyInput => write!(f, "Cannot build drawing from empty input"),
            DrawingBuildError::InvalidConfig(msg) => write!(f, "Invalid drawing config: {}", msg),
        }
    }
}

impl std::error::Error for DrawingBuildError {}

impl DrawingDocument {
    pub fn from_scenes(
        scenes: &[(String, Scene2D)],
        config: &DrawingExportConfig,
    ) -> Result<Self, DrawingBuildError> {
        validate_config(config)?;
        if scenes.is_empty() {
            return Err(DrawingBuildError::EmptyInput);
        }

        let drawable_width = config.page_width_mm - 2.0 * config.margin_mm;
        let drawable_height = config.page_height_mm - 2.0 * config.margin_mm;
        let cols = (scenes.len() as f64).sqrt().ceil().max(1.0) as usize;
        let rows = ((scenes.len() + cols - 1) / cols).max(1);
        let cell_width =
            (drawable_width - config.view_gap_mm * (cols.saturating_sub(1) as f64)) / cols as f64;
        let cell_height =
            (drawable_height - config.view_gap_mm * (rows.saturating_sub(1) as f64)) / rows as f64;

        if cell_width <= 0.0 || cell_height <= 0.0 {
            return Err(DrawingBuildError::InvalidConfig(
                "View gap and margins leave no drawable area".to_string(),
            ));
        }

        let mut views = Vec::with_capacity(scenes.len());
        let mut text = Vec::new();

        for (index, (id, scene)) in scenes.iter().enumerate() {
            let col = index % cols;
            let row = index / cols;
            let origin = Vec2::new(
                config.margin_mm + col as f64 * (cell_width + config.view_gap_mm),
                config.margin_mm + row as f64 * (cell_height + config.view_gap_mm),
            );
            let size = Vec2::new(cell_width, cell_height);

            let view = build_view(id, scene, origin, size);
            if config.show_view_labels {
                text.push(DrawingText {
                    position_mm: Vec2::new(origin.x, origin.y + cell_height - 4.0),
                    text: id.clone(),
                    size_mm: 3.5,
                });
            }
            views.push(view);
        }

        if let Some(title) = &config.title {
            text.push(DrawingText {
                position_mm: Vec2::new(config.margin_mm, config.page_height_mm - 5.0),
                text: title.clone(),
                size_mm: 4.0,
            });
        }

        Ok(Self {
            page_width_mm: config.page_width_mm,
            page_height_mm: config.page_height_mm,
            title: config.title.clone(),
            views,
            text,
        })
    }

    pub fn from_scene(
        scene: &Scene2D,
        config: &DrawingExportConfig,
    ) -> Result<Self, DrawingBuildError> {
        let id = scene.name.clone().unwrap_or_else(|| "view-1".to_string());
        Self::from_scenes(&[(id, scene.clone())], config)
    }
}

pub fn iso128_style(edge_class: EdgeClass) -> DrawingStyle {
    match edge_class {
        EdgeClass::VisibleOutline => DrawingStyle {
            edge_class,
            stroke_width_mm: 0.50,
            dash_pattern_mm: Vec::new(),
        },
        EdgeClass::VisibleCrease => DrawingStyle {
            edge_class,
            stroke_width_mm: 0.25,
            dash_pattern_mm: Vec::new(),
        },
        EdgeClass::VisibleSmooth => DrawingStyle {
            edge_class,
            stroke_width_mm: 0.18,
            dash_pattern_mm: Vec::new(),
        },
        EdgeClass::Hidden => DrawingStyle {
            edge_class,
            stroke_width_mm: 0.18,
            dash_pattern_mm: vec![3.0, 1.5],
        },
        EdgeClass::SectionCut => DrawingStyle {
            edge_class,
            stroke_width_mm: 0.70,
            dash_pattern_mm: vec![12.0, 3.0, 2.0, 3.0],
        },
    }
}

fn validate_config(config: &DrawingExportConfig) -> Result<(), DrawingBuildError> {
    if config.page_width_mm <= 0.0 || config.page_height_mm <= 0.0 {
        return Err(DrawingBuildError::InvalidConfig(
            "Page dimensions must be positive".to_string(),
        ));
    }
    if config.margin_mm < 0.0 || config.view_gap_mm < 0.0 {
        return Err(DrawingBuildError::InvalidConfig(
            "Margins and view gaps cannot be negative".to_string(),
        ));
    }
    if config.page_width_mm <= 2.0 * config.margin_mm
        || config.page_height_mm <= 2.0 * config.margin_mm
    {
        return Err(DrawingBuildError::InvalidConfig(
            "Margins are too large for page size".to_string(),
        ));
    }
    Ok(())
}

fn build_view(id: &str, scene: &Scene2D, origin: Vec2, size: Vec2) -> DrawingView {
    let bounds = scene.bounding_box();
    let scale = bounds
        .map(|(min, max)| {
            let width_mm = ((max.x - min.x) * METERS_TO_MM).abs();
            let height_mm = ((max.y - min.y) * METERS_TO_MM).abs();
            let sx = if width_mm > 0.0 {
                size.x / width_mm
            } else {
                1.0
            };
            let sy = if height_mm > 0.0 {
                size.y / height_mm
            } else {
                1.0
            };
            sx.min(sy).min(1.0)
        })
        .unwrap_or(1.0);

    let (offset_x, offset_y) = bounds
        .map(|(min, max)| {
            let width_mm = (max.x - min.x) * METERS_TO_MM * scale;
            let height_mm = (max.y - min.y) * METERS_TO_MM * scale;
            (
                origin.x + (size.x - width_mm) / 2.0 - min.x * METERS_TO_MM * scale,
                origin.y + (size.y - height_mm) / 2.0 - min.y * METERS_TO_MM * scale,
            )
        })
        .unwrap_or((origin.x, origin.y));

    let primitives = scene
        .segments()
        .iter()
        .filter_map(|seg| transform_segment(seg, scale, offset_x, offset_y))
        .collect();

    DrawingView {
        id: id.to_string(),
        origin_mm: origin,
        size_mm: size,
        primitives,
    }
}

fn transform_segment(
    seg: &ClassifiedSegment,
    scale: f64,
    offset_x: f64,
    offset_y: f64,
) -> Option<DrawingPrimitive> {
    let geometry = match &seg.geometry {
        Segment2D::Line { start, end } => DrawingGeometry::Line {
            start: transform_point(start, scale, offset_x, offset_y),
            end: transform_point(end, scale, offset_x, offset_y),
        },
        Segment2D::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => DrawingGeometry::Arc {
            center: transform_point(center, scale, offset_x, offset_y),
            radius_mm: radius * METERS_TO_MM * scale,
            start_angle: *start_angle,
            end_angle: *end_angle,
        },
        Segment2D::Ellipse {
            center,
            rx,
            ry,
            rotation,
            start_angle,
            end_angle,
        } => DrawingGeometry::Ellipse {
            center: transform_point(center, scale, offset_x, offset_y),
            rx_mm: rx * METERS_TO_MM * scale,
            ry_mm: ry * METERS_TO_MM * scale,
            rotation: *rotation,
            start_angle: *start_angle,
            end_angle: *end_angle,
        },
        Segment2D::CubicBezier { p0, p1, p2, p3 } => DrawingGeometry::CubicBezier {
            p0: transform_point(p0, scale, offset_x, offset_y),
            p1: transform_point(p1, scale, offset_x, offset_y),
            p2: transform_point(p2, scale, offset_x, offset_y),
            p3: transform_point(p3, scale, offset_x, offset_y),
        },
    };

    Some(DrawingPrimitive {
        geometry,
        style: iso128_style(seg.class),
        layer: seg.layer.clone(),
        source_entity_id: seg.source_entity_id.clone(),
    })
}

fn transform_point(point: &Vec2, scale: f64, offset_x: f64, offset_y: f64) -> Vec2 {
    Vec2::new(
        point.x * METERS_TO_MM * scale + offset_x,
        point.y * METERS_TO_MM * scale + offset_y,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_edges_use_iso_dash_pattern() {
        let style = iso128_style(EdgeClass::Hidden);
        assert_eq!(style.stroke_width_mm, 0.18);
        assert_eq!(style.dash_pattern_mm, vec![3.0, 1.5]);
    }

    #[test]
    fn builds_multiview_document_from_projected_scenes() {
        let mut scene = Scene2D::with_name("front");
        scene.add_segment(ClassifiedSegment {
            geometry: Segment2D::Line {
                start: Vec2::new(0.0, 0.0),
                end: Vec2::new(1.0, 0.0),
            },
            class: EdgeClass::VisibleOutline,
            layer: Some("A-WALL".to_string()),
            source_entity_id: Some("wall".to_string()),
        });

        let doc = DrawingDocument::from_scenes(
            &[
                ("front".to_string(), scene.clone()),
                ("top".to_string(), scene),
            ],
            &DrawingExportConfig::default(),
        )
        .expect("drawing should build");

        assert_eq!(doc.views.len(), 2);
        assert_eq!(doc.views[0].primitives.len(), 1);
    }
}
