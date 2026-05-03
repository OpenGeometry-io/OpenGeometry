use crate::export::drawing::{
    DrawingDocument, DrawingExportConfig, DrawingGeometry, DrawingPrimitive, DrawingStyle,
};
use crate::export::projection::{EdgeClass, Scene2D};

const CURVE_STEPS: usize = 32;

#[derive(Clone, Debug, Default)]
pub struct DxfExportConfig {
    pub drawing: DrawingExportConfig,
}

pub type DxfExportResult<T> = Result<T, DxfExportError>;

#[derive(Debug)]
pub enum DxfExportError {
    EmptyDrawing,
    InvalidConfig(String),
}

impl std::fmt::Display for DxfExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DxfExportError::EmptyDrawing => write!(f, "Cannot export empty drawing"),
            DxfExportError::InvalidConfig(msg) => write!(f, "Invalid DXF config: {}", msg),
        }
    }
}

impl std::error::Error for DxfExportError {}

pub fn export_scene_to_dxf_string(
    scene: &Scene2D,
    config: &DxfExportConfig,
) -> DxfExportResult<String> {
    let drawing = DrawingDocument::from_scene(scene, &config.drawing)
        .map_err(|err| DxfExportError::InvalidConfig(err.to_string()))?;
    export_dxf_string(&drawing)
}

pub fn export_dxf_string(drawing: &DrawingDocument) -> DxfExportResult<String> {
    if drawing.views.is_empty() {
        return Err(DxfExportError::EmptyDrawing);
    }

    let mut out = DxfWriter::new();
    write_header(&mut out);
    write_tables(&mut out);
    write_entities(&mut out, drawing);
    out.pair(0, "EOF");
    Ok(out.finish())
}

fn write_header(out: &mut DxfWriter) {
    out.section("HEADER");
    out.pair(9, "$ACADVER");
    out.pair(1, "AC1021");
    out.pair(9, "$INSUNITS");
    out.pair(70, 4);
    out.end_section();
}

fn write_tables(out: &mut DxfWriter) {
    out.section("TABLES");
    write_ltype_table(out);
    write_layer_table(out);
    out.end_section();
}

fn write_ltype_table(out: &mut DxfWriter) {
    out.pair(0, "TABLE");
    out.pair(2, "LTYPE");
    out.pair(70, 4);
    ltype(out, "CONTINUOUS", "Solid line", &[]);
    ltype(out, "HIDDEN", "Hidden __ __ __", &[3.0, -1.5]);
    ltype(
        out,
        "SECTION",
        "Section ____ _ ____",
        &[12.0, -3.0, 2.0, -3.0],
    );
    ltype(out, "BYLAYER", "", &[]);
    out.pair(0, "ENDTAB");
}

fn ltype(out: &mut DxfWriter, name: &str, description: &str, elements: &[f64]) {
    out.pair(0, "LTYPE");
    out.pair(100, "AcDbSymbolTableRecord");
    out.pair(100, "AcDbLinetypeTableRecord");
    out.pair(2, name);
    out.pair(70, 0);
    out.pair(3, description);
    out.pair(72, 65);
    out.pair(73, elements.len());
    out.pair(40, elements.iter().map(|v| v.abs()).sum::<f64>());
    for value in elements {
        out.pair(49, *value);
        out.pair(74, 0);
    }
}

fn write_layer_table(out: &mut DxfWriter) {
    out.pair(0, "TABLE");
    out.pair(2, "LAYER");
    out.pair(70, 5);
    layer(out, "0", 7, "CONTINUOUS", 25);
    layer(out, "OG-VISIBLE-OUTLINE", 7, "CONTINUOUS", 50);
    layer(out, "OG-VISIBLE-CREASE", 7, "CONTINUOUS", 25);
    layer(out, "OG-HIDDEN", 8, "HIDDEN", 18);
    layer(out, "OG-SECTION", 1, "SECTION", 70);
    out.pair(0, "ENDTAB");
}

fn layer(out: &mut DxfWriter, name: &str, color: i32, ltype: &str, lineweight: i32) {
    out.pair(0, "LAYER");
    out.pair(100, "AcDbSymbolTableRecord");
    out.pair(100, "AcDbLayerTableRecord");
    out.pair(2, name);
    out.pair(70, 0);
    out.pair(62, color);
    out.pair(6, ltype);
    out.pair(370, lineweight);
}

fn write_entities(out: &mut DxfWriter, drawing: &DrawingDocument) {
    out.section("ENTITIES");
    for view in &drawing.views {
        for primitive in &view.primitives {
            write_primitive(out, primitive);
        }
    }
    out.end_section();
}

fn write_primitive(out: &mut DxfWriter, primitive: &DrawingPrimitive) {
    match &primitive.geometry {
        DrawingGeometry::Line { start, end } => {
            write_line(out, primitive, start.x, start.y, end.x, end.y);
        }
        DrawingGeometry::Arc {
            center,
            radius_mm,
            start_angle,
            end_angle,
        } => {
            write_arc(
                out,
                primitive,
                center.x,
                center.y,
                *radius_mm,
                start_angle.to_degrees(),
                end_angle.to_degrees(),
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
            write_ellipse_polyline(
                out,
                primitive,
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
            let mut prev = *p0;
            for i in 1..=CURVE_STEPS {
                let t = i as f64 / CURVE_STEPS as f64;
                let next = cubic_point(*p0, *p1, *p2, *p3, t);
                write_line(out, primitive, prev.x, prev.y, next.x, next.y);
                prev = next;
            }
        }
    }
}

fn write_line(
    out: &mut DxfWriter,
    primitive: &DrawingPrimitive,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
) {
    write_entity_common(out, "LINE", &primitive.style);
    out.pair(10, x1);
    out.pair(20, y1);
    out.pair(30, 0.0);
    out.pair(11, x2);
    out.pair(21, y2);
    out.pair(31, 0.0);
}

fn write_arc(
    out: &mut DxfWriter,
    primitive: &DrawingPrimitive,
    cx: f64,
    cy: f64,
    radius: f64,
    start_degrees: f64,
    end_degrees: f64,
) {
    write_entity_common(out, "ARC", &primitive.style);
    out.pair(10, cx);
    out.pair(20, cy);
    out.pair(30, 0.0);
    out.pair(40, radius);
    out.pair(50, start_degrees);
    out.pair(51, end_degrees);
}

fn write_ellipse_polyline(
    out: &mut DxfWriter,
    primitive: &DrawingPrimitive,
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
    let mut prev: Option<crate::export::projection::Vec2> = None;
    for i in 0..=CURVE_STEPS {
        let t = i as f64 / CURVE_STEPS as f64;
        let angle = start_angle + (end_angle - start_angle) * t;
        let local_x = rx * angle.cos();
        let local_y = ry * angle.sin();
        let next = crate::export::projection::Vec2::new(
            cx + local_x * cos_r - local_y * sin_r,
            cy + local_x * sin_r + local_y * cos_r,
        );
        if let Some(prev) = prev {
            write_line(out, primitive, prev.x, prev.y, next.x, next.y);
        }
        prev = Some(next);
    }
}

fn write_entity_common(out: &mut DxfWriter, entity_type: &str, style: &DrawingStyle) {
    out.pair(0, entity_type);
    out.pair(8, layer_for_edge_class(style.edge_class));
    out.pair(6, ltype_for_edge_class(style.edge_class));
    out.pair(370, lineweight_100th_mm(style.stroke_width_mm));
}

fn layer_for_edge_class(class: EdgeClass) -> &'static str {
    match class {
        EdgeClass::VisibleOutline => "OG-VISIBLE-OUTLINE",
        EdgeClass::VisibleCrease | EdgeClass::VisibleSmooth => "OG-VISIBLE-CREASE",
        EdgeClass::Hidden => "OG-HIDDEN",
        EdgeClass::SectionCut => "OG-SECTION",
    }
}

fn ltype_for_edge_class(class: EdgeClass) -> &'static str {
    match class {
        EdgeClass::Hidden => "HIDDEN",
        EdgeClass::SectionCut => "SECTION",
        _ => "CONTINUOUS",
    }
}

fn lineweight_100th_mm(width_mm: f64) -> i32 {
    (width_mm * 100.0).round() as i32
}

fn cubic_point(
    p0: crate::export::projection::Vec2,
    p1: crate::export::projection::Vec2,
    p2: crate::export::projection::Vec2,
    p3: crate::export::projection::Vec2,
    t: f64,
) -> crate::export::projection::Vec2 {
    let mt = 1.0 - t;
    crate::export::projection::Vec2::new(
        mt.powi(3) * p0.x
            + 3.0 * mt.powi(2) * t * p1.x
            + 3.0 * mt * t.powi(2) * p2.x
            + t.powi(3) * p3.x,
        mt.powi(3) * p0.y
            + 3.0 * mt.powi(2) * t * p1.y
            + 3.0 * mt * t.powi(2) * p2.y
            + t.powi(3) * p3.y,
    )
}

struct DxfWriter {
    lines: Vec<String>,
}

impl DxfWriter {
    fn new() -> Self {
        Self { lines: Vec::new() }
    }

    fn section(&mut self, name: &str) {
        self.pair(0, "SECTION");
        self.pair(2, name);
    }

    fn end_section(&mut self) {
        self.pair(0, "ENDSEC");
    }

    fn pair(&mut self, code: impl ToString, value: impl ToString) {
        self.lines.push(code.to_string());
        self.lines.push(value.to_string());
    }

    fn finish(self) -> String {
        let mut out = self.lines.join("\n");
        out.push('\n');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::projection::{ClassifiedSegment, Segment2D, Vec2};

    #[test]
    fn dxf_contains_tables_linetypes_and_lineweights() {
        let mut scene = Scene2D::with_name("dxf-proof");
        scene.add_segment(ClassifiedSegment {
            geometry: Segment2D::Line {
                start: Vec2::new(0.0, 0.0),
                end: Vec2::new(1.0, 0.0),
            },
            class: EdgeClass::Hidden,
            layer: None,
            source_entity_id: None,
        });

        let dxf = export_scene_to_dxf_string(&scene, &DxfExportConfig::default())
            .expect("DXF export should pass");

        assert!(dxf.contains("HEADER"));
        assert!(dxf.contains("LTYPE"));
        assert!(dxf.contains("LAYER"));
        assert!(dxf.contains("ENTITIES"));
        assert!(dxf.contains("\n370\n18\n"));
        assert!(dxf.contains("HIDDEN"));
    }
}
