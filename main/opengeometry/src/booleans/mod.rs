pub mod error;
pub mod planar;
pub mod rebuild;
pub mod solid;
pub mod types;

use std::collections::HashMap;

use serde_json::Value;
use wasm_bindgen::prelude::*;

use crate::booleans::error::{BooleanError, BooleanErrorKind};
use crate::booleans::planar::{execute_planar_boolean, planar_context_from_brep};
use crate::booleans::rebuild::{build_brep_from_polygons, build_brep_from_triangle_mesh};
use crate::booleans::solid::{brep_to_polygons, execute_solid_boolean};
use crate::booleans::types::{
    BooleanOperandKind, BooleanOperation, BooleanOptions, BooleanOutput, BooleanReport,
};
use crate::brep::Brep;

pub use error::{BooleanError as OGBooleanError, BooleanErrorKind as OGBooleanErrorKind};
pub use types::{BooleanOperation as OGBooleanOperation, BooleanOptions as OGBooleanOptions};

const FEATURE_OUTLINE_CREASE_COS_THRESHOLD: f64 = 0.965;

/// Result payload returned across the wasm boundary.
#[wasm_bindgen]
pub struct OGBooleanResult {
    brep_serialized: String,
    geometry_serialized: String,
    outline_geometry_serialized: String,
    report_json: String,
}

impl OGBooleanResult {
    fn from_output(output: BooleanOutput) -> Result<Self, String> {
        let brep_serialized = serde_json::to_string(&output.brep)
            .map_err(|error| format!("Failed to serialize boolean BRep: {}", error))?;
        let geometry_serialized = serde_json::to_string(&output.brep.get_triangle_vertex_buffer())
            .map_err(|error| format!("Failed to serialize boolean geometry: {}", error))?;
        let outline_geometry_serialized = serde_json::to_string(
            &output
                .brep
                .get_feature_outline_vertex_buffer(FEATURE_OUTLINE_CREASE_COS_THRESHOLD),
        )
        .map_err(|error| format!("Failed to serialize boolean outline: {}", error))?;
        let report_json = serde_json::to_string(&output.report)
            .map_err(|error| format!("Failed to serialize boolean report: {}", error))?;

        Ok(Self {
            brep_serialized,
            geometry_serialized,
            outline_geometry_serialized,
            report_json,
        })
    }
}

#[wasm_bindgen]
impl OGBooleanResult {
    #[wasm_bindgen(getter, js_name = brepSerialized)]
    pub fn brep_serialized(&self) -> String {
        self.brep_serialized.clone()
    }

    #[wasm_bindgen(getter, js_name = geometrySerialized)]
    pub fn geometry_serialized(&self) -> String {
        self.geometry_serialized.clone()
    }

    #[wasm_bindgen(getter, js_name = outlineGeometrySerialized)]
    pub fn outline_geometry_serialized(&self) -> String {
        self.outline_geometry_serialized.clone()
    }

    #[wasm_bindgen(getter, js_name = reportJson)]
    pub fn report_json(&self) -> String {
        self.report_json.clone()
    }
}

/// Applies a boolean union between two compatible BReps.
pub fn boolean_union(
    lhs: &Brep,
    rhs: &Brep,
    options: BooleanOptions,
) -> Result<BooleanOutput, BooleanError> {
    execute_boolean(lhs, rhs, BooleanOperation::Union, options)
}

/// Applies a boolean intersection between two compatible BReps.
pub fn boolean_intersection(
    lhs: &Brep,
    rhs: &Brep,
    options: BooleanOptions,
) -> Result<BooleanOutput, BooleanError> {
    execute_boolean(lhs, rhs, BooleanOperation::Intersection, options)
}

/// Applies a boolean subtraction (`lhs - rhs`) between two compatible BReps.
pub fn boolean_subtraction(
    lhs: &Brep,
    rhs: &Brep,
    options: BooleanOptions,
) -> Result<BooleanOutput, BooleanError> {
    execute_boolean(lhs, rhs, BooleanOperation::Subtraction, options)
}

/// Wasm entry point for union.
#[wasm_bindgen(js_name = booleanUnion)]
pub fn boolean_union_wasm(
    lhs_brep_serialized: String,
    rhs_brep_serialized: String,
    options_json: Option<String>,
) -> Result<OGBooleanResult, JsValue> {
    boolean_wasm_entry(
        lhs_brep_serialized,
        rhs_brep_serialized,
        options_json,
        BooleanOperation::Union,
    )
}

/// Wasm entry point for intersection.
#[wasm_bindgen(js_name = booleanIntersection)]
pub fn boolean_intersection_wasm(
    lhs_brep_serialized: String,
    rhs_brep_serialized: String,
    options_json: Option<String>,
) -> Result<OGBooleanResult, JsValue> {
    boolean_wasm_entry(
        lhs_brep_serialized,
        rhs_brep_serialized,
        options_json,
        BooleanOperation::Intersection,
    )
}

/// Wasm entry point for subtraction.
#[wasm_bindgen(js_name = booleanSubtraction)]
pub fn boolean_subtraction_wasm(
    lhs_brep_serialized: String,
    rhs_brep_serialized: String,
    options_json: Option<String>,
) -> Result<OGBooleanResult, JsValue> {
    boolean_wasm_entry(
        lhs_brep_serialized,
        rhs_brep_serialized,
        options_json,
        BooleanOperation::Subtraction,
    )
}

fn boolean_wasm_entry(
    lhs_brep_serialized: String,
    rhs_brep_serialized: String,
    options_json: Option<String>,
    operation: BooleanOperation,
) -> Result<OGBooleanResult, JsValue> {
    let lhs: Brep = serde_json::from_str(&lhs_brep_serialized)
        .map_err(|error| JsValue::from_str(&format!("Invalid lhs BRep JSON payload: {}", error)))?;
    let rhs: Brep = serde_json::from_str(&rhs_brep_serialized)
        .map_err(|error| JsValue::from_str(&format!("Invalid rhs BRep JSON payload: {}", error)))?;
    let options = parse_options_json(options_json).map_err(|error| JsValue::from_str(&error))?;
    let output = execute_boolean(&lhs, &rhs, operation, options)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    OGBooleanResult::from_output(output).map_err(|error| JsValue::from_str(&error))
}

/// Validates operands, routes to the solid or planar pipeline, and packages
/// the rebuilt BRep plus operation report.
fn execute_boolean(
    lhs: &Brep,
    rhs: &Brep,
    operation: BooleanOperation,
    options: BooleanOptions,
) -> Result<BooleanOutput, BooleanError> {
    lhs.validate_topology().map_err(BooleanError::from)?;
    rhs.validate_topology().map_err(BooleanError::from)?;

    let tolerance = options.resolve_tolerance(lhs, rhs);
    let working_tolerance = tolerance.max(1.0e-6);
    let lhs_kind = detect_operand_kind(lhs, working_tolerance)?;
    let rhs_kind = detect_operand_kind(rhs, working_tolerance)?;

    if lhs_kind != rhs_kind {
        return Err(BooleanError::new(
            BooleanErrorKind::MixedOperandKinds,
            "Boolean operands must both be closed solids or both be coplanar planar faces",
        ));
    }

    let _ = options.merge_coplanar_faces;

    let (brep, input_triangle_count) = match lhs_kind {
        BooleanOperandKind::ClosedSolid => {
            let (lhs_polygons, lhs_triangles) = brep_to_polygons(lhs, working_tolerance)?;
            let (rhs_polygons, rhs_triangles) = brep_to_polygons(rhs, working_tolerance)?;
            let result_mesh =
                execute_solid_boolean(lhs_polygons, rhs_polygons, operation, working_tolerance)?;
            let result_brep = build_brep_from_triangle_mesh(&result_mesh, working_tolerance, true)?;

            if !result_brep.faces.is_empty() && !is_closed_solid_operand(&result_brep) {
                return Err(BooleanError::new(
                    BooleanErrorKind::TopologyError,
                    "Solid boolean output is not watertight",
                ));
            }

            (result_brep, lhs_triangles + rhs_triangles)
        }
        BooleanOperandKind::PlanarFace => {
            let (result_polygons, triangle_count) =
                execute_planar_boolean(lhs, rhs, operation, working_tolerance)?;
            let result_brep = build_brep_from_polygons(&result_polygons, working_tolerance, false)?;
            (result_brep, triangle_count)
        }
    };

    let report = BooleanReport {
        operation,
        operand_kind: lhs_kind,
        input_face_count: lhs.faces.len() + rhs.faces.len(),
        input_triangle_count,
        output_face_count: brep.faces.len(),
        output_shell_count: brep.shells.len(),
        empty: brep.faces.is_empty(),
    };

    Ok(BooleanOutput { brep, report })
}

/// Detects which boolean pipeline can legally consume the operand.
fn detect_operand_kind(brep: &Brep, tolerance: f64) -> Result<BooleanOperandKind, BooleanError> {
    if is_closed_solid_operand(brep) {
        return Ok(BooleanOperandKind::ClosedSolid);
    }

    if planar_context_from_brep(brep, tolerance).is_ok() {
        return Ok(BooleanOperandKind::PlanarFace);
    }

    if !brep.wires.is_empty() && brep.faces.is_empty() {
        return Err(BooleanError::new(
            BooleanErrorKind::UnsupportedOperandKind,
            "Boolean operands do not support wire-only BReps",
        ));
    }

    Err(BooleanError::new(
        BooleanErrorKind::UnsupportedOperandKind,
        "Boolean operands must be closed solids or coplanar planar faces",
    ))
}

/// Verifies that the operand is a closed shell solid with exactly two face uses
/// per topological edge.
fn is_closed_solid_operand(brep: &Brep) -> bool {
    if brep.faces.is_empty() || brep.edges.is_empty() || brep.shells.is_empty() {
        return false;
    }

    if brep.shells.iter().any(|shell| !shell.is_closed) {
        return false;
    }

    let mut edge_to_faces: HashMap<u32, usize> = HashMap::new();
    for halfedge in &brep.halfedges {
        if halfedge.face.is_some() {
            *edge_to_faces.entry(halfedge.edge).or_insert(0) += 1;
        }
    }

    brep.edges
        .iter()
        .all(|edge| edge_to_faces.get(&edge.id).copied().unwrap_or(0) == 2)
}

/// Parses optional wasm-side boolean options and falls back to kernel defaults
/// when the payload is absent.
fn parse_options_json(options_json: Option<String>) -> Result<BooleanOptions, String> {
    match options_json {
        Some(payload) if !payload.trim().is_empty() => {
            let value: Value = serde_json::from_str(&payload)
                .map_err(|error| format!("Invalid boolean options JSON payload: {}", error))?;

            serde_json::from_value(value)
                .map_err(|error| format!("Failed to parse boolean options: {}", error))
        }
        _ => Ok(BooleanOptions::default()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::cuboid::OGCuboid;
    use crate::primitives::polygon::OGPolygon;
    use crate::primitives::sphere::OGSphere;
    use openmaths::Vector3;

    fn build_cuboid(center: Vector3, width: f64, height: f64, depth: f64) -> Brep {
        let mut cuboid = OGCuboid::new("boolean-cuboid".to_string());
        cuboid
            .set_config(center, width, height, depth)
            .expect("cuboid config");
        cuboid.brep().clone()
    }

    fn build_polygon(points: Vec<Vector3>) -> Brep {
        let mut polygon = OGPolygon::new("boolean-polygon".to_string());
        polygon.set_config(points).expect("polygon config");
        polygon.brep().clone()
    }

    fn build_sphere(center: Vector3, radius: f64) -> Brep {
        let mut sphere = OGSphere::new("boolean-sphere".to_string());
        sphere
            .set_config(center, radius, 18, 12)
            .expect("sphere config");
        sphere.brep().clone()
    }

    fn assert_closed_solid(brep: &Brep) {
        assert!(!brep.faces.is_empty());
        assert!(!brep.shells.is_empty());
        assert!(brep.validate_topology().is_ok());
        assert!(is_closed_solid_operand(brep));
        assert!(brep.shells.iter().all(|shell| shell.is_closed));
    }

    #[test]
    fn union_of_overlapping_solid_primitives_produces_closed_shell() {
        let lhs = build_cuboid(Vector3::new(-0.1, 0.0, 0.0), 1.8, 1.2, 1.4);
        let rhs = build_sphere(Vector3::new(0.35, 0.1, 0.15), 0.75);

        let output = boolean_union(&lhs, &rhs, BooleanOptions::default()).expect("boolean union");

        assert_closed_solid(&output.brep);
        assert!(!output
            .brep
            .get_feature_outline_vertex_buffer(FEATURE_OUTLINE_CREASE_COS_THRESHOLD)
            .is_empty());
    }

    #[test]
    fn intersection_of_disjoint_solids_returns_empty_brep() {
        let lhs = build_cuboid(Vector3::new(-4.0, 0.0, 0.0), 1.0, 1.0, 1.0);
        let rhs = build_cuboid(Vector3::new(4.0, 0.0, 0.0), 1.0, 1.0, 1.0);

        let output = boolean_intersection(&lhs, &rhs, BooleanOptions::default())
            .expect("empty intersection");

        assert!(output.brep.faces.is_empty());
        assert!(output.report.empty);
    }

    #[test]
    fn subtraction_of_overlapping_cuboids_preserves_closed_shell() {
        let lhs = build_cuboid(Vector3::new(0.0, 0.0, 0.0), 2.2, 1.2, 1.2);
        let rhs = build_cuboid(Vector3::new(0.55, 0.0, 0.0), 1.2, 1.0, 0.8);

        let output = boolean_subtraction(&lhs, &rhs, BooleanOptions::default())
            .expect("boolean subtraction");

        assert_closed_solid(&output.brep);
    }

    #[test]
    fn intersection_of_overlapping_sphere_and_cuboid_is_watertight() {
        let lhs = build_cuboid(Vector3::new(0.0, 0.0, 0.0), 1.9, 1.9, 1.9);
        let rhs = build_sphere(Vector3::new(0.35, 0.1, 0.15), 0.9);

        let output = boolean_intersection(&lhs, &rhs, BooleanOptions::default())
            .expect("boolean intersection");

        assert_closed_solid(&output.brep);
    }

    #[test]
    fn subtraction_of_overlapping_sphere_from_cuboid_is_watertight() {
        let lhs = build_cuboid(Vector3::new(0.0, 0.0, 0.0), 2.2, 2.2, 2.2);
        let rhs = build_sphere(Vector3::new(0.35, 0.1, 0.15), 0.9);

        let output =
            boolean_subtraction(&lhs, &rhs, BooleanOptions::default()).expect("sphere subtraction");

        assert_closed_solid(&output.brep);
    }

    #[test]
    fn union_of_face_touching_cuboids_stays_closed() {
        let lhs = build_cuboid(Vector3::new(-0.5, 0.0, 0.0), 1.0, 1.0, 1.0);
        let rhs = build_cuboid(Vector3::new(0.5, 0.0, 0.0), 1.0, 1.0, 1.0);

        let output = boolean_union(&lhs, &rhs, BooleanOptions::default()).expect("touching union");

        assert_closed_solid(&output.brep);
    }

    #[test]
    fn planar_union_of_rectangles_returns_planar_faces_without_shells() {
        let lhs = build_polygon(vec![
            Vector3::new(-1.5, 0.0, -0.5),
            Vector3::new(0.2, 0.0, -0.5),
            Vector3::new(0.2, 0.0, 0.8),
            Vector3::new(-1.5, 0.0, 0.8),
        ]);
        let rhs = build_polygon(vec![
            Vector3::new(-0.3, 0.0, -0.8),
            Vector3::new(1.4, 0.0, -0.8),
            Vector3::new(1.4, 0.0, 0.5),
            Vector3::new(-0.3, 0.0, 0.5),
        ]);

        let output = boolean_union(&lhs, &rhs, BooleanOptions::default()).expect("planar union");

        assert!(!output.brep.faces.is_empty());
        assert!(output.brep.shells.is_empty());
        assert!(output.brep.validate_topology().is_ok());
    }

    #[test]
    fn mixed_dimensional_operands_are_rejected() {
        let solid = build_cuboid(Vector3::new(0.0, 0.0, 0.0), 1.0, 1.0, 1.0);
        let planar = build_polygon(vec![
            Vector3::new(-1.0, 0.0, -1.0),
            Vector3::new(1.0, 0.0, -1.0),
            Vector3::new(1.0, 0.0, 1.0),
            Vector3::new(-1.0, 0.0, 1.0),
        ]);

        let error = match boolean_union(&solid, &planar, BooleanOptions::default()) {
            Ok(_) => panic!("mixed dimensional operands should be rejected"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), BooleanErrorKind::MixedOperandKinds);
    }
}
