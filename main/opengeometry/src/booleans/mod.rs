pub mod error;
pub mod planar;
pub mod rebuild;
pub mod solid;
pub mod types;

use std::collections::HashMap;

use serde_json::Value;
use wasm_bindgen::prelude::*;

use crate::booleans::error::{BooleanError, BooleanErrorKind};
use crate::booleans::planar::{
    execute_planar_boolean, planar_context_from_brep, planar_input_triangle_count,
};
use crate::booleans::rebuild::{build_brep_from_polygons, build_brep_from_triangle_mesh};
use crate::booleans::solid::{brep_to_polygons, execute_solid_boolean};
use crate::booleans::types::{
    BooleanOperandKind, BooleanOperation, BooleanOptions, BooleanOutput, BooleanReport,
};
use crate::brep::Brep;

pub use error::{
    BooleanError as OGBooleanError, BooleanErrorKind as OGBooleanErrorKind,
    BooleanErrorPhase as OGBooleanErrorPhase,
};
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

/// Applies repeated boolean subtraction `host - {c1, c2, ..., cN}`.
///
/// For solid operands with two or more cutters this first folds the cutter
/// list through `boolean_union` to produce a single non-overlapping cutter
/// shell, then runs one subtract against that shell. The identity
/// `host - {c1..cN} == host - union(c1..cN)` holds for both overlapping and
/// non-overlapping cutter sets, but the unioned form sidesteps cumulative
/// numerical drift across N sequential subtractions and merges any
/// overlapping cutters into a manifold cut volume.
///
/// If the union pre-pass fails for any reason (e.g. boolmesh rejects an
/// intermediate union), this falls back to the legacy sequential subtract
/// loop so the failure surface is identical to today's behavior — no
/// regression for callers whose inputs already worked.
///
/// Cutter indices reported in errors are **0-based**.
pub fn boolean_subtraction_many(
    lhs: &Brep,
    cutters: &[Brep],
    options: BooleanOptions,
) -> Result<BooleanOutput, BooleanError> {
    if cutters.is_empty() {
        return Err(BooleanError::new(
            BooleanErrorKind::InvalidOperand,
            "Boolean subtraction requires at least one cutter in the operand array",
        ));
    }

    lhs.validate_topology().map_err(BooleanError::from)?;
    for (index, cutter) in cutters.iter().enumerate() {
        cutter
            .validate_topology()
            .map_err(BooleanError::from)
            .map_err(|error| indexed_subtraction_error(index, error))?;
    }

    let mut operands = Vec::with_capacity(cutters.len() + 1);
    operands.push(lhs);
    operands.extend(cutters.iter());

    let working_tolerance = options.resolve_tolerance_many(&operands).max(1.0e-6);
    let operand_kind = detect_operand_kind(lhs, working_tolerance)?;
    let mut input_triangle_count =
        count_operand_input_triangles(lhs, operand_kind, working_tolerance)?;
    let input_face_count = lhs.faces.len()
        + cutters
            .iter()
            .map(|cutter| cutter.faces.len())
            .sum::<usize>();

    for (index, cutter) in cutters.iter().enumerate() {
        let cutter_kind = detect_operand_kind(cutter, working_tolerance)
            .map_err(|error| indexed_subtraction_error(index, error))?;
        if cutter_kind != operand_kind {
            return Err(indexed_subtraction_error(
                index,
                BooleanError::new(
                    BooleanErrorKind::MixedOperandKinds,
                    "Boolean operands must both be closed solids or both be coplanar planar faces",
                ),
            ));
        }

        input_triangle_count +=
            count_operand_input_triangles(cutter, operand_kind, working_tolerance)
                .map_err(|error| indexed_subtraction_error(index, error))?;
    }

    let mut current = lhs.clone();
    let mut handled_via_union = false;

    if cutters.len() >= 2 && operand_kind == BooleanOperandKind::ClosedSolid {
        if let Some(merged) = try_union_cutters(cutters, working_tolerance) {
            if let Ok(output) = execute_boolean_with_tolerance(
                &current,
                &merged,
                BooleanOperation::Subtraction,
                working_tolerance,
            ) {
                current = output.brep;
                handled_via_union = true;
            }
        }
    }

    if !handled_via_union {
        for (index, cutter) in cutters.iter().enumerate() {
            if current.faces.is_empty() {
                break;
            }

            current = execute_boolean_with_tolerance(
                &current,
                cutter,
                BooleanOperation::Subtraction,
                working_tolerance,
            )
            .map(|output| output.brep)
            .map_err(|error| indexed_subtraction_error(index, error))?;
        }
    }

    Ok(BooleanOutput {
        report: BooleanReport {
            operation: BooleanOperation::Subtraction,
            operand_kind,
            input_face_count,
            input_triangle_count,
            output_face_count: current.faces.len(),
            output_shell_count: current.shells.len(),
            empty: current.faces.is_empty(),
        },
        brep: current,
    })
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

/// Wasm entry point for array-backed repeated subtraction.
#[wasm_bindgen(js_name = booleanSubtractionMany)]
pub fn boolean_subtraction_many_wasm(
    lhs_brep_serialized: String,
    cutters_brep_serialized: String,
    options_json: Option<String>,
) -> Result<OGBooleanResult, JsValue> {
    let lhs: Brep = serde_json::from_str(&lhs_brep_serialized)
        .map_err(|error| JsValue::from_str(&format!("Invalid lhs BRep JSON payload: {}", error)))?;
    let cutters: Vec<Brep> = serde_json::from_str(&cutters_brep_serialized).map_err(|error| {
        JsValue::from_str(&format!(
            "Invalid subtraction cutter BRep JSON payload: {}",
            error
        ))
    })?;
    let options = parse_options_json(options_json).map_err(|error| JsValue::from_str(&error))?;
    let output = boolean_subtraction_many(&lhs, &cutters, options)
        .map_err(|error| JsValue::from_str(&error.to_wasm_json()))?;
    OGBooleanResult::from_output(output).map_err(|error| JsValue::from_str(&error))
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
        .map_err(|error| JsValue::from_str(&error.to_wasm_json()))?;
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
    execute_boolean_with_tolerance(lhs, rhs, operation, working_tolerance)
}

/// Validates operands, routes to the solid or planar pipeline, and packages
/// the rebuilt BRep plus operation report using a caller-supplied tolerance.
///
/// For `Subtraction` against `ClosedSolid` operands, this enforces an AABB
/// pre-check on the cutter (`#3` fix):
///
/// * Cutter fully disjoint from host → result is the host unchanged
///   (subtracting nothing is identity).
/// * Cutter fits inside host's AABB → use as-is (current happy path).
/// * Cutter overshoots host's AABB → auto-clip via `cutter ∩ host_aabb_cuboid`
///   and use the clipped shell as the effective cutter.
/// * Auto-clip itself fails → typed `BooleanErrorKind::CutterExceedsHost`
///   surfaced through the WS-0 structured error infrastructure.
fn execute_boolean_with_tolerance(
    lhs: &Brep,
    rhs: &Brep,
    operation: BooleanOperation,
    working_tolerance: f64,
) -> Result<BooleanOutput, BooleanError> {
    let lhs_kind = detect_operand_kind(lhs, working_tolerance)?;
    let rhs_kind = detect_operand_kind(rhs, working_tolerance)?;

    if lhs_kind != rhs_kind {
        return Err(BooleanError::new(
            BooleanErrorKind::MixedOperandKinds,
            "Boolean operands must both be closed solids or both be coplanar planar faces",
        ));
    }

    // Subtraction-only AABB enforcement. Union and Intersection are
    // mathematically well-defined for any operand pair, so the bounds check
    // does not apply.
    // Subtraction-only AABB enforcement. Union and Intersection are
    // mathematically well-defined for any operand pair, so the bounds check
    // does not apply.
    let clipped_storage: Brep;
    let rhs_for_dispatch: &Brep = if operation == BooleanOperation::Subtraction
        && lhs_kind == BooleanOperandKind::ClosedSolid
    {
        match enforce_host_bounds_for_subtraction(lhs, rhs, working_tolerance)? {
            BoundsAction::UseAsIs => rhs,
            BoundsAction::Identity => {
                return identity_subtraction_output(lhs, rhs, lhs_kind, working_tolerance);
            }
            BoundsAction::ClipTo(clipped) => {
                clipped_storage = clipped;
                &clipped_storage
            }
        }
    } else {
        rhs
    };

    let (brep, input_triangle_count) = match lhs_kind {
        BooleanOperandKind::ClosedSolid => {
            let (lhs_polygons, lhs_triangles) = brep_to_polygons(lhs, working_tolerance)?;
            let (rhs_polygons, rhs_triangles) =
                brep_to_polygons(rhs_for_dispatch, working_tolerance)?;
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
                execute_planar_boolean(lhs, rhs_for_dispatch, operation, working_tolerance)?;
            let result_brep = build_brep_from_polygons(&result_polygons, working_tolerance, false)?;
            (result_brep, triangle_count)
        }
    };

    // Report uses the *original* operands' face counts so callers see
    // pre-clip metrics.
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

fn count_operand_input_triangles(
    brep: &Brep,
    operand_kind: BooleanOperandKind,
    tolerance: f64,
) -> Result<usize, BooleanError> {
    match operand_kind {
        BooleanOperandKind::ClosedSolid => {
            brep_to_polygons(brep, tolerance).map(|(_, count)| count)
        }
        BooleanOperandKind::PlanarFace => planar_input_triangle_count(brep, tolerance),
    }
}

/// Decision returned by the AABB pre-check for a `Subtraction` cutter.
enum BoundsAction {
    /// Cutter fits within the host AABB; pass it to the boolean dispatch
    /// unchanged.
    UseAsIs,
    /// Cutter is fully disjoint from the host; subtraction is the identity
    /// (host returned unchanged).
    Identity,
    /// Cutter overshoots the host AABB but auto-clipping succeeded; use the
    /// clipped shell as the effective cutter.
    ClipTo(Brep),
}

/// Computes the axis-aligned bounding box of a BRep. Returns `None` if the
/// BRep has no vertices or any vertex is non-finite.
fn compute_brep_aabb(brep: &Brep) -> Option<(openmaths::Vector3, openmaths::Vector3)> {
    if brep.vertices.is_empty() {
        return None;
    }
    let mut min = openmaths::Vector3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
    let mut max = openmaths::Vector3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for vertex in &brep.vertices {
        let p = vertex.position;
        min.x = min.x.min(p.x);
        min.y = min.y.min(p.y);
        min.z = min.z.min(p.z);
        max.x = max.x.max(p.x);
        max.y = max.y.max(p.y);
        max.z = max.z.max(p.z);
    }
    if !min.x.is_finite() || !max.x.is_finite() {
        return None;
    }
    Some((min, max))
}

/// True if the cutter AABB lies fully outside the host AABB along at least
/// one axis (with `tolerance` slack).
fn aabb_disjoint(
    host: &(openmaths::Vector3, openmaths::Vector3),
    cutter: &(openmaths::Vector3, openmaths::Vector3),
    tolerance: f64,
) -> bool {
    let (h_min, h_max) = host;
    let (c_min, c_max) = cutter;
    c_max.x < h_min.x - tolerance
        || c_min.x > h_max.x + tolerance
        || c_max.y < h_min.y - tolerance
        || c_min.y > h_max.y + tolerance
        || c_max.z < h_min.z - tolerance
        || c_min.z > h_max.z + tolerance
}

/// If the cutter overshoots the host along any axis by more than `tolerance`,
/// returns the dominant `(axis, overshoot)` pair so a typed
/// `CutterExceedsHost` error can carry the diagnostic. `None` means the
/// cutter is fully contained.
fn aabb_overshoot(
    host: &(openmaths::Vector3, openmaths::Vector3),
    cutter: &(openmaths::Vector3, openmaths::Vector3),
    tolerance: f64,
) -> Option<(char, f64)> {
    let (h_min, h_max) = host;
    let (c_min, c_max) = cutter;
    let candidates = [
        ('x', (h_min.x - c_min.x).max(c_max.x - h_max.x)),
        ('y', (h_min.y - c_min.y).max(c_max.y - h_max.y)),
        ('z', (h_min.z - c_min.z).max(c_max.z - h_max.z)),
    ];
    let mut worst: Option<(char, f64)> = None;
    for (axis, overshoot) in candidates {
        if overshoot > tolerance {
            match worst {
                Some((_, w)) if w >= overshoot => {}
                _ => worst = Some((axis, overshoot)),
            }
        }
    }
    worst
}

/// Builds a closed cuboid BRep that fully encloses the given AABB, inflated
/// by `inflate` units on each side so the subsequent intersection's coplanar
/// face cases stay clear of the host AABB faces.
fn build_aabb_cuboid(
    aabb: &(openmaths::Vector3, openmaths::Vector3),
    inflate: f64,
) -> Result<Brep, BooleanError> {
    let (min, max) = aabb;
    let center = openmaths::Vector3::new(
        (min.x + max.x) * 0.5,
        (min.y + max.y) * 0.5,
        (min.z + max.z) * 0.5,
    );
    let width = (max.x - min.x).max(0.0) + 2.0 * inflate;
    let height = (max.y - min.y).max(0.0) + 2.0 * inflate;
    let depth = (max.z - min.z).max(0.0) + 2.0 * inflate;

    let mut cuboid = crate::primitives::cuboid::OGCuboid::new("aabb-clip-volume".to_string());
    cuboid
        .set_config(center, width, height, depth)
        .map_err(|err| {
            BooleanError::new(
                BooleanErrorKind::KernelFailure,
                format!("Failed to build AABB clip volume: {:?}", err),
            )
        })?;
    Ok(cuboid.world_brep())
}

/// AABB-based bounds enforcement for a `Subtraction` cutter against a
/// closed-solid host. See `execute_boolean_with_tolerance` for the full
/// behavior matrix.
fn enforce_host_bounds_for_subtraction(
    host: &Brep,
    cutter: &Brep,
    tolerance: f64,
) -> Result<BoundsAction, BooleanError> {
    let Some(host_aabb) = compute_brep_aabb(host) else {
        return Ok(BoundsAction::UseAsIs);
    };
    let Some(cutter_aabb) = compute_brep_aabb(cutter) else {
        return Ok(BoundsAction::UseAsIs);
    };

    if aabb_disjoint(&host_aabb, &cutter_aabb, tolerance) {
        return Ok(BoundsAction::Identity);
    }

    let Some((axis, overshoot)) = aabb_overshoot(&host_aabb, &cutter_aabb, tolerance) else {
        return Ok(BoundsAction::UseAsIs);
    };

    // Auto-clip: cutter ∩ inflated_host_aabb_cuboid. The inflate keeps the
    // intersection clear of coincident-face cases between the cutter and the
    // host's actual faces. The intersection itself is a non-Subtraction
    // boolean so the recursion through `execute_boolean_with_tolerance` does
    // not re-enter this enforce path.
    let inflate = (tolerance * 4.0).max(1.0e-4);
    let aabb_cuboid = build_aabb_cuboid(&host_aabb, inflate)?;

    match execute_boolean_with_tolerance(
        cutter,
        &aabb_cuboid,
        BooleanOperation::Intersection,
        tolerance,
    ) {
        Ok(output) if !output.brep.faces.is_empty() => Ok(BoundsAction::ClipTo(output.brep)),
        Ok(_) => Ok(BoundsAction::Identity),
        Err(_) => Err(BooleanError::input_validation(
            BooleanErrorKind::CutterExceedsHost { axis, overshoot },
            format!(
                "Cutter extends past host along {} by {:.6e} and auto-clip failed",
                axis, overshoot
            ),
        )),
    }
}

/// Returns the host unchanged with a properly-populated `BooleanReport`.
/// Used when the cutter is fully outside the host so subtraction is the
/// identity operation.
fn identity_subtraction_output(
    host: &Brep,
    cutter: &Brep,
    operand_kind: BooleanOperandKind,
    tolerance: f64,
) -> Result<BooleanOutput, BooleanError> {
    let host_triangles = count_operand_input_triangles(host, operand_kind, tolerance)?;
    let cutter_triangles =
        count_operand_input_triangles(cutter, operand_kind, tolerance).unwrap_or(0);
    Ok(BooleanOutput {
        report: BooleanReport {
            operation: BooleanOperation::Subtraction,
            operand_kind,
            input_face_count: host.faces.len() + cutter.faces.len(),
            input_triangle_count: host_triangles + cutter_triangles,
            output_face_count: host.faces.len(),
            output_shell_count: host.shells.len(),
            empty: host.faces.is_empty(),
        },
        brep: host.clone(),
    })
}

/// Folds `boolean_union` over a slice of cutter BReps to produce a single
/// merged cutter shell. Returns `None` if any pairwise union fails or if the
/// merged shell collapses to empty — the caller falls back to sequential
/// subtraction in either case so the failure mode is unchanged from the
/// pre-union behavior.
fn try_union_cutters(cutters: &[Brep], tolerance: f64) -> Option<Brep> {
    if cutters.is_empty() {
        return None;
    }

    let mut iter = cutters.iter();
    let first = iter.next()?.clone();
    let mut accumulator = first;

    for next in iter {
        match execute_boolean_with_tolerance(&accumulator, next, BooleanOperation::Union, tolerance)
        {
            Ok(output) => {
                if output.brep.faces.is_empty() {
                    return None;
                }
                accumulator = output.brep;
            }
            Err(_) => return None,
        }
    }

    Some(accumulator)
}

/// Wraps an inner boolean error with the 0-based cutter index that triggered
/// it. Phase becomes `SubtractStep`. The original error's `kind`, `details`,
/// and `edge_samples` are preserved so the wrapper does not lose diagnostics.
fn indexed_subtraction_error(index: usize, error: BooleanError) -> BooleanError {
    let inner_message = error.message.clone();
    let mut wrapped = BooleanError::subtract_step(
        error.kind.clone(),
        index,
        format!("Failed to subtract cutter #{}: {}", index, inner_message),
    );
    if let Some(details) = error.details {
        wrapped = wrapped.with_details(details);
    } else if !inner_message.is_empty() {
        wrapped = wrapped.with_details(inner_message);
    }
    if let Some(samples) = error.edge_samples {
        wrapped = wrapped.with_edge_samples(samples);
    }
    wrapped
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
    use crate::booleans::error::BooleanErrorPhase;
    use crate::operations::extrude::try_extrude_brep_face;
    use crate::primitives::cuboid::OGCuboid;
    use crate::primitives::polygon::OGPolygon;
    use crate::primitives::polyline::OGPolyline;
    use crate::primitives::sphere::OGSphere;
    use openmaths::Vector3;

    /// Reproduces the wall topology built by
    /// `main/opengeometry-three/src/examples/wall-from-offsets.ts`: take a
    /// centerline polyline, compute left + right offsets with bevel
    /// (acute_threshold_degrees = 35), join them into a single closed
    /// outline polygon, then extrude. This is the actual PolyWall topology
    /// OpenPlans uses — bevels at acute corners give it 8+ vertex outlines
    /// which is exactly the regime the debt sheet's `#6a` / `#6b` complain
    /// about.
    fn build_wall_from_offsets(
        centerline_points: Vec<Vector3>,
        wall_thickness: f64,
        height: f64,
    ) -> Brep {
        let half = wall_thickness * 0.5;
        let acute_threshold = 35.0;
        let bevel = true;

        let mut polyline = OGPolyline::new("wall-centerline".to_string());
        polyline.set_config(centerline_points).expect("polyline config");

        let left = polyline.get_offset_points(half, acute_threshold, bevel);
        let right = polyline.get_offset_points(-half, acute_threshold, bevel);

        // Match the TS buildWallOutline: left forward + right reversed,
        // dedup the closing vertex if it equals the opener.
        let mut outline: Vec<Vector3> = left.clone();
        let mut right_rev = right.clone();
        right_rev.reverse();
        outline.extend(right_rev);
        if outline.len() > 2 {
            let first = outline[0];
            let last = outline[outline.len() - 1];
            let dx = first.x - last.x;
            let dy = first.y - last.y;
            let dz = first.z - last.z;
            if dx * dx + dy * dy + dz * dz <= 1.0e-18 {
                outline.pop();
            }
        }

        let polygon_brep = build_polygon(outline);
        try_extrude_brep_face(polygon_brep, height)
            .expect("offset wall polygon should extrude")
    }

    /// Original simple U-shape PolyWall (8 vertices, hand-constructed).
    /// Kept for the simpler `polywall_u_shape_extrusion_is_a_valid_closed_solid`
    /// sanity test.
    fn build_polywall_u_shape(arm_length: f64, thickness: f64, height: f64) -> Brep {
        // Outer ring: traversed counter-clockwise when viewed from +Y.
        // The U opens toward +z.
        let outer = [
            Vector3::new(-arm_length, 0.0, -arm_length),                 // outer back-left
            Vector3::new(arm_length, 0.0, -arm_length),                  // outer back-right
            Vector3::new(arm_length, 0.0, 0.0),                          // outer right-inner-corner-top
            Vector3::new(arm_length - thickness, 0.0, 0.0),              // inner right-inner-corner-top
            Vector3::new(arm_length - thickness, 0.0, -arm_length + thickness), // inner back-right
            Vector3::new(-arm_length + thickness, 0.0, -arm_length + thickness), // inner back-left
            Vector3::new(-arm_length + thickness, 0.0, 0.0),             // inner left-inner-corner-top
            Vector3::new(-arm_length, 0.0, 0.0),                         // outer left-inner-corner-top
        ];

        let polygon_brep = build_polygon(outer.to_vec());
        try_extrude_brep_face(polygon_brep, height)
            .expect("U-shape polygon should extrude successfully")
    }

    fn build_cuboid(center: Vector3, width: f64, height: f64, depth: f64) -> Brep {
        let mut cuboid = OGCuboid::new("boolean-cuboid".to_string());
        cuboid
            .set_config(center, width, height, depth)
            .expect("cuboid config");
        cuboid.world_brep()
    }

    fn build_polygon(points: Vec<Vector3>) -> Brep {
        let mut polygon = OGPolygon::new("boolean-polygon".to_string());
        polygon.set_config(points).expect("polygon config");
        polygon.world_brep()
    }

    fn build_sphere(center: Vector3, radius: f64) -> Brep {
        let mut sphere = OGSphere::new("boolean-sphere".to_string());
        sphere
            .set_config(center, radius, 18, 12)
            .expect("sphere config");
        sphere.world_brep()
    }

    fn build_placed_cuboid(
        center: Vector3,
        width: f64,
        height: f64,
        depth: f64,
        translation: Vector3,
        rotation: Vector3,
        scale: Vector3,
    ) -> Brep {
        let mut cuboid = OGCuboid::new("placed-boolean-cuboid".to_string());
        cuboid
            .set_config(center, width, height, depth)
            .expect("cuboid config");
        cuboid
            .set_transform(translation, rotation, scale)
            .expect("placement transform");
        cuboid.world_brep()
    }

    fn build_placed_sphere(
        center: Vector3,
        radius: f64,
        translation: Vector3,
        rotation: Vector3,
        scale: Vector3,
    ) -> Brep {
        let mut sphere = OGSphere::new("placed-boolean-sphere".to_string());
        sphere
            .set_config(center, radius, 18, 12)
            .expect("sphere config");
        sphere
            .set_transform(translation, rotation, scale)
            .expect("placement transform");
        sphere.world_brep()
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
    fn subtraction_of_placed_sphere_from_rotated_scaled_cuboid_is_watertight() {
        let lhs = build_placed_cuboid(
            Vector3::new(0.0, 0.0, 0.0),
            2.4,
            1.8,
            1.6,
            Vector3::new(0.15, 0.0, 0.1),
            Vector3::new(0.25, 0.4, -0.15),
            Vector3::new(1.1, 1.1, 1.1),
        );
        let rhs = build_placed_sphere(
            Vector3::new(0.0, 0.0, 0.0),
            0.8,
            Vector3::new(0.45, 0.05, 0.15),
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(0.95, 0.95, 0.95),
        );

        let output = boolean_subtraction(&lhs, &rhs, BooleanOptions::default())
            .expect("placed boolean subtraction");

        assert_closed_solid(&output.brep);
        assert!(!output
            .brep
            .get_feature_outline_vertex_buffer(FEATURE_OUTLINE_CREASE_COS_THRESHOLD)
            .is_empty());
    }

    #[test]
    fn multi_subtraction_of_solid_cutters_preserves_closed_shell() {
        let lhs = build_cuboid(Vector3::new(0.0, 0.0, 0.0), 3.2, 2.2, 1.4);
        let cutters = vec![
            build_cuboid(Vector3::new(-0.55, 0.0, 0.0), 0.95, 1.4, 0.9),
            build_cuboid(Vector3::new(0.75, 0.1, 0.0), 0.85, 1.2, 0.9),
        ];

        let output = boolean_subtraction_many(&lhs, &cutters, BooleanOptions::default())
            .expect("multi subtraction");

        assert_closed_solid(&output.brep);
        assert_eq!(
            output.report.input_face_count,
            lhs.faces.len()
                + cutters
                    .iter()
                    .map(|cutter| cutter.faces.len())
                    .sum::<usize>()
        );
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
    fn multi_subtraction_of_planar_faces_returns_planar_output() {
        let lhs = build_polygon(vec![
            Vector3::new(-2.4, 0.0, -1.0),
            Vector3::new(2.4, 0.0, -1.0),
            Vector3::new(2.4, 0.0, 1.0),
            Vector3::new(-2.4, 0.0, 1.0),
        ]);
        let cutters = vec![
            build_polygon(vec![
                Vector3::new(-1.4, 0.0, -0.45),
                Vector3::new(-0.35, 0.0, -0.45),
                Vector3::new(-0.35, 0.0, 0.45),
                Vector3::new(-1.4, 0.0, 0.45),
            ]),
            build_polygon(vec![
                Vector3::new(0.45, 0.0, -0.45),
                Vector3::new(1.45, 0.0, -0.45),
                Vector3::new(1.45, 0.0, 0.45),
                Vector3::new(0.45, 0.0, 0.45),
            ]),
        ];

        let output = boolean_subtraction_many(&lhs, &cutters, BooleanOptions::default())
            .expect("planar multi subtraction");

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
        assert_eq!(error.kind(), &BooleanErrorKind::MixedOperandKinds);
    }

    #[test]
    fn multi_subtraction_with_one_cutter_matches_binary_subtraction() {
        let lhs = build_cuboid(Vector3::new(0.0, 0.0, 0.0), 2.2, 1.6, 1.2);
        let cutter = build_cuboid(Vector3::new(0.45, 0.0, 0.0), 0.9, 1.0, 0.8);

        let binary = boolean_subtraction(&lhs, &cutter, BooleanOptions::default())
            .expect("binary subtraction");
        let multi = boolean_subtraction_many(&lhs, &[cutter], BooleanOptions::default())
            .expect("multi subtraction");

        assert_eq!(multi.brep.faces.len(), binary.brep.faces.len());
        assert_eq!(multi.brep.shells.len(), binary.brep.shells.len());
        assert_eq!(
            multi.report.output_face_count,
            binary.report.output_face_count
        );
        assert_eq!(
            multi.report.output_shell_count,
            binary.report.output_shell_count
        );
        assert_eq!(multi.report.empty, binary.report.empty);
    }

    #[test]
    fn multi_subtraction_requires_at_least_one_cutter() {
        let lhs = build_cuboid(Vector3::new(0.0, 0.0, 0.0), 2.2, 1.6, 1.2);
        let error = match boolean_subtraction_many(&lhs, &[], BooleanOptions::default()) {
            Ok(_) => panic!("empty cutter list should fail"),
            Err(error) => error,
        };

        assert_eq!(error.kind(), &BooleanErrorKind::InvalidOperand);
        assert_eq!(
            error.to_string(),
            "Boolean subtraction requires at least one cutter in the operand array"
        );
    }

    #[test]
    fn multi_subtraction_reports_later_cutter_index_on_failure() {
        let lhs = build_cuboid(Vector3::new(0.0, 0.0, 0.0), 2.8, 1.8, 1.4);
        let cutters = vec![
            build_cuboid(Vector3::new(-0.5, 0.0, 0.0), 0.9, 1.1, 0.8),
            build_polygon(vec![
                Vector3::new(-0.3, 0.0, -0.3),
                Vector3::new(0.8, 0.0, -0.3),
                Vector3::new(0.8, 0.0, 0.3),
                Vector3::new(-0.3, 0.0, 0.3),
            ]),
        ];

        let error = match boolean_subtraction_many(&lhs, &cutters, BooleanOptions::default()) {
            Ok(_) => panic!("mixed dimensional cutter should fail"),
            Err(error) => error,
        };

        assert_eq!(error.kind(), &BooleanErrorKind::MixedOperandKinds);
        assert!(error.to_string().contains("cutter #1"));
        assert_eq!(error.cutter_index(), Some(1));
        assert_eq!(error.phase(), BooleanErrorPhase::SubtractStep);
    }

    /// WS-1 — issue #2: two cutters that overlap each other should produce a
    /// manifold result instead of crashing the next consumer with a non-manifold
    /// edge error. Pre-WS-1, the second cutter's overlap with the first leaves
    /// duplicated boundary geometry along the shared edges; the union pre-pass
    /// merges them into a single cutter shell first.
    #[test]
    fn subtract_many_with_two_overlapping_cuboid_cutters_succeeds() {
        let host = build_cuboid(Vector3::new(0.0, 0.0, 0.0), 4.0, 2.0, 2.0);
        let cutter_a = build_placed_cuboid(
            Vector3::new(0.0, 0.0, 0.0),
            1.0,
            1.0,
            3.0,
            Vector3::new(-0.4, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
        );
        let cutter_b = build_placed_cuboid(
            Vector3::new(0.0, 0.0, 0.0),
            1.0,
            1.0,
            3.0,
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
        );

        let output =
            boolean_subtraction_many(&host, &[cutter_a, cutter_b], BooleanOptions::default())
                .expect("overlapping cutters should succeed via union pre-pass");

        assert_closed_solid(&output.brep);
    }

    /// WS-1 — issue #2 follow-up: three cutters where two overlap each other
    /// and the third is disjoint. Validates the union pre-pass folds correctly
    /// across more than a pair.
    #[test]
    fn subtract_many_with_three_partially_overlapping_cuboid_cutters_succeeds() {
        let host = build_cuboid(Vector3::new(0.0, 0.0, 0.0), 6.0, 2.0, 2.0);
        let cutter_a = build_placed_cuboid(
            Vector3::new(0.0, 0.0, 0.0),
            1.0,
            1.0,
            3.0,
            Vector3::new(-1.6, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
        );
        let cutter_b = build_placed_cuboid(
            Vector3::new(0.0, 0.0, 0.0),
            1.0,
            1.0,
            3.0,
            Vector3::new(-1.2, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
        );
        let cutter_c = build_placed_cuboid(
            Vector3::new(0.0, 0.0, 0.0),
            1.0,
            1.0,
            3.0,
            Vector3::new(1.6, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
        );

        let output = boolean_subtraction_many(
            &host,
            &[cutter_a, cutter_b, cutter_c],
            BooleanOptions::default(),
        )
        .expect("partially overlapping cutters should succeed via union pre-pass");

        assert_closed_solid(&output.brep);
    }

    /// WS-1 — semantic equivalence regression: the union pre-pass must not
    /// change behavior on the existing non-overlapping happy path. Two
    /// disjoint cutters subtracted together should produce the same shell
    /// counts as subtracting them sequentially.
    #[test]
    fn subtract_many_with_non_overlapping_cutters_matches_sequential_baseline() {
        let host = build_cuboid(Vector3::new(0.0, 0.0, 0.0), 6.0, 2.0, 2.0);
        let cutter_a = build_placed_cuboid(
            Vector3::new(0.0, 0.0, 0.0),
            0.8,
            1.0,
            3.0,
            Vector3::new(-2.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
        );
        let cutter_b = build_placed_cuboid(
            Vector3::new(0.0, 0.0, 0.0),
            0.8,
            1.0,
            3.0,
            Vector3::new(2.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
        );

        let multi = boolean_subtraction_many(
            &host,
            &[cutter_a.clone(), cutter_b.clone()],
            BooleanOptions::default(),
        )
        .expect("multi subtraction (union pre-pass)");

        let sequential = {
            let intermediate = boolean_subtraction(&host, &cutter_a, BooleanOptions::default())
                .expect("first subtraction");
            boolean_subtraction(&intermediate.brep, &cutter_b, BooleanOptions::default())
                .expect("second subtraction")
        };

        assert_eq!(multi.brep.shells.len(), sequential.brep.shells.len());
        assert_eq!(
            multi.brep.faces.is_empty(),
            sequential.brep.faces.is_empty()
        );
        assert_closed_solid(&multi.brep);
    }

    /// WS-1 — issue #6b convergence test: a non-trivial extruded prism (the
    /// PolyWall pattern that fails today on the 3rd cutter) succeeds under
    /// the union pre-pass. Uses three cuboid cutters at distinct positions to
    /// mirror the OpenPlans "Add 3 (one per segment)" reproducer.
    #[test]
    fn polywall_pattern_with_three_non_overlapping_cutters_succeeds() {
        let host = build_cuboid(Vector3::new(0.0, 0.0, 0.0), 9.0, 3.0, 0.3);
        let cutter_a = build_placed_cuboid(
            Vector3::new(0.0, 0.0, 0.0),
            1.0,
            2.1,
            1.0,
            Vector3::new(-3.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
        );
        let cutter_b = build_placed_cuboid(
            Vector3::new(0.0, 0.0, 0.0),
            1.0,
            2.1,
            1.0,
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
        );
        let cutter_c = build_placed_cuboid(
            Vector3::new(0.0, 0.0, 0.0),
            1.0,
            2.1,
            1.0,
            Vector3::new(3.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
        );

        let output = boolean_subtraction_many(
            &host,
            &[cutter_a, cutter_b, cutter_c],
            BooleanOptions::default(),
        )
        .expect("three-cutter PolyWall pattern should succeed under union pre-pass");

        assert_closed_solid(&output.brep);
    }

    /// WS-1 — `try_union_cutters` returns `None` for an empty cutter slice so
    /// the caller can fall back. Tested directly to guard the contract.
    #[test]
    fn try_union_cutters_returns_none_for_empty_slice() {
        let merged = try_union_cutters(&[], 1.0e-6);
        assert!(merged.is_none());
    }

    /// WS-1 — `try_union_cutters` returns the lone cutter unchanged when the
    /// slice has exactly one element (no fold work to do).
    #[test]
    fn try_union_cutters_returns_single_cutter_unchanged() {
        let cutter = build_cuboid(Vector3::new(0.0, 0.0, 0.0), 1.0, 1.0, 1.0);
        let merged = try_union_cutters(std::slice::from_ref(&cutter), 1.0e-6)
            .expect("single cutter should pass through");
        assert_eq!(merged.faces.len(), cutter.faces.len());
        assert_eq!(merged.shells.len(), cutter.shells.len());
    }

    /// WS-2 — issue #3 primary case: a cutter whose AABB extends past the
    /// host along one axis is auto-clipped to the host's AABB and the
    /// subtraction succeeds with a watertight result.
    #[test]
    fn subtract_with_cutter_extending_past_host_clips_to_host_aabb() {
        let host = build_cuboid(Vector3::new(0.0, 0.0, 0.0), 4.0, 2.0, 2.0);
        // Cutter centered well past the host's +x face, extending well past it
        let cutter = build_placed_cuboid(
            Vector3::new(0.0, 0.0, 0.0),
            2.0,
            1.0,
            3.0,
            Vector3::new(2.5, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
        );

        let output = boolean_subtraction(&host, &cutter, BooleanOptions::default())
            .expect("out-of-bounds cutter should auto-clip and succeed");

        assert_closed_solid(&output.brep);
    }

    /// WS-2 — issue #3 disjoint case: a cutter whose AABB is fully outside
    /// the host returns the host unchanged (subtracting nothing is identity).
    #[test]
    fn subtract_with_cutter_disjoint_from_host_returns_identity() {
        let host = build_cuboid(Vector3::new(0.0, 0.0, 0.0), 2.0, 2.0, 2.0);
        let cutter = build_placed_cuboid(
            Vector3::new(0.0, 0.0, 0.0),
            1.0,
            1.0,
            1.0,
            Vector3::new(10.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
        );

        let output = boolean_subtraction(&host, &cutter, BooleanOptions::default())
            .expect("disjoint subtraction should be identity");

        assert_closed_solid(&output.brep);
        assert_eq!(output.brep.faces.len(), host.faces.len());
        assert_eq!(output.brep.shells.len(), host.shells.len());
        assert_eq!(output.report.output_face_count, host.faces.len());
        assert!(!output.report.empty);
    }

    /// WS-2 — issue #3 corner overshoot: a cutter that overshoots the host
    /// along two axes simultaneously still auto-clips successfully.
    #[test]
    fn subtract_with_cutter_corner_overshooting_succeeds() {
        let host = build_cuboid(Vector3::new(0.0, 0.0, 0.0), 4.0, 4.0, 4.0);
        let cutter = build_placed_cuboid(
            Vector3::new(0.0, 0.0, 0.0),
            2.0,
            2.0,
            5.0,
            Vector3::new(2.5, 2.5, 0.0),
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
        );

        let output = boolean_subtraction(&host, &cutter, BooleanOptions::default())
            .expect("corner-overshooting cutter should auto-clip and succeed");

        assert_closed_solid(&output.brep);
    }

    /// WS-2 — regression: cutter fully inside the host (the existing happy
    /// path) is unchanged by the AABB enforcement.
    #[test]
    fn subtract_with_cutter_fully_inside_host_unchanged_by_bounds_check() {
        let host = build_cuboid(Vector3::new(0.0, 0.0, 0.0), 4.0, 4.0, 4.0);
        let cutter = build_placed_cuboid(
            Vector3::new(0.0, 0.0, 0.0),
            1.0,
            1.0,
            5.0,
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
        );

        let output = boolean_subtraction(&host, &cutter, BooleanOptions::default())
            .expect("inside-cutter happy path");

        assert_closed_solid(&output.brep);
    }

    /// WS-2 — many-subtract over a list mixing in-bounds and out-of-bounds
    /// cutters succeeds. Validates that the AABB enforcement composes
    /// correctly with WS-1's union pre-pass.
    #[test]
    fn subtract_many_with_one_in_bounds_one_out_of_bounds_cutter_succeeds() {
        let host = build_cuboid(Vector3::new(0.0, 0.0, 0.0), 4.0, 2.0, 2.0);
        let inside = build_placed_cuboid(
            Vector3::new(0.0, 0.0, 0.0),
            1.0,
            1.0,
            3.0,
            Vector3::new(-1.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
        );
        let outside = build_placed_cuboid(
            Vector3::new(0.0, 0.0, 0.0),
            2.0,
            1.0,
            3.0,
            Vector3::new(2.5, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
        );

        let output = boolean_subtraction_many(&host, &[inside, outside], BooleanOptions::default())
            .expect("mixed in-bounds + out-of-bounds cutters should succeed");

        assert_closed_solid(&output.brep);
    }

    /// WS-2 — `aabb_overshoot` returns the dominant axis when the cutter
    /// overshoots along multiple axes. Tested directly to lock the contract.
    #[test]
    fn aabb_overshoot_picks_dominant_axis() {
        let host = (Vector3::new(-1.0, -1.0, -1.0), Vector3::new(1.0, 1.0, 1.0));
        let cutter = (Vector3::new(-0.5, -0.5, -0.5), Vector3::new(2.0, 1.5, 1.1));

        let result = aabb_overshoot(&host, &cutter, 1.0e-6);
        let (axis, overshoot) = result.expect("cutter overshoots host");
        assert_eq!(
            axis, 'x',
            "x overshoot (1.0) should dominate y (0.5) and z (0.1)"
        );
        assert!((overshoot - 1.0).abs() < 1.0e-9);
    }

    /// WS-2 — `aabb_disjoint` true positive when cutter is fully outside.
    #[test]
    fn aabb_disjoint_detects_separated_aabbs() {
        let host = (Vector3::new(0.0, 0.0, 0.0), Vector3::new(1.0, 1.0, 1.0));
        let cutter = (Vector3::new(5.0, 5.0, 5.0), Vector3::new(6.0, 6.0, 6.0));
        assert!(aabb_disjoint(&host, &cutter, 1.0e-6));

        let touching = (Vector3::new(1.0, 0.0, 0.0), Vector3::new(2.0, 1.0, 1.0));
        assert!(!aabb_disjoint(&host, &touching, 1.0e-6));
    }

    /// WS-3 — issue #6a regression: a cutter that passes cleanly through the
    /// host (top + bottom faces coplanar with host top + bottom) must still
    /// succeed. The post-mortem coincident-face detection only fires on
    /// kernel failure, never pre-emptively, so this case is unaffected.
    #[test]
    fn subtract_cutter_passing_cleanly_through_host_succeeds() {
        let host = build_cuboid(Vector3::new(0.0, 0.0, 0.0), 4.0, 2.0, 2.0);
        // Cutter z extent matches host exactly (passes cleanly through top
        // and bottom). Width and depth fit inside.
        let cutter = build_placed_cuboid(
            Vector3::new(0.0, 0.0, 0.0),
            1.0,
            2.0,
            1.0,
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
        );

        let output = boolean_subtraction(&host, &cutter, BooleanOptions::default())
            .expect("clean pass-through subtraction should succeed");

        assert_closed_solid(&output.brep);
    }

    /// Pre-condition: the wall-from-offsets pipeline (centerline →
    /// bilateral offset with bevel → outline polygon → extrude) produces a
    /// closed solid. This mirrors `examples/wall-from-offsets.ts` exactly,
    /// including the 6-point zig-zag centerline + 35° bevel threshold that
    /// produces the bevelled-corner topology the debt sheet's `#6a`/`#6b`
    /// describe.
    #[test]
    fn wall_from_offsets_extrusion_is_a_valid_closed_solid() {
        let centerline = vec![
            Vector3::new(-2.6, 0.0, -1.9),
            Vector3::new(-1.2, 0.0, -1.0),
            Vector3::new(-0.2, 0.0, 0.1),
            Vector3::new(0.6, 0.0, 0.2),
            Vector3::new(0.1, 0.0, 1.0),
            Vector3::new(2.6, 0.0, 2.0),
        ];
        let wall = build_wall_from_offsets(centerline, 0.45, 3.0);
        assert_closed_solid(&wall);
        // 6-point centerline with bevels: outline should have substantially
        // more than 12 vertices because of the bevel insertions on acute
        // corners.
        assert!(
            wall.vertices.len() >= 24,
            "wall-from-offsets vertex count expected ≥ 24 (6+6 base + bevels), got {}",
            wall.vertices.len()
        );
    }

    /// **#6b debt-sheet reproducer using the actual wall-from-offsets
    /// pipeline**: 6-point centerline, 0.45m thickness, 3m height (matches
    /// `examples/wall-from-offsets.ts` exactly), then subtract three door
    /// cutters at three of the centerline midpoints. This is the topology
    /// that triggers the original `#6b` symptom in OpenPlans.
    #[test]
    fn wall_from_offsets_with_three_doors_succeeds() {
        let centerline = vec![
            Vector3::new(-2.6, 0.0, -1.9),
            Vector3::new(-1.2, 0.0, -1.0),
            Vector3::new(-0.2, 0.0, 0.1),
            Vector3::new(0.6, 0.0, 0.2),
            Vector3::new(0.1, 0.0, 1.0),
            Vector3::new(2.6, 0.0, 2.0),
        ];
        let wall_thickness = 0.45;
        let wall_height = 3.0;
        let wall = build_wall_from_offsets(centerline, wall_thickness, wall_height);

        // Three cutters at three centerline midpoints. Each cutter spans
        // the wall thickness with the 1mm overshoot pattern (the `#6a`
        // precision regime). Door dimensions roughly match a real door.
        let door_height = 2.1;
        let door_width = 0.8;
        let across_overshoot = 0.001; // 1 mm — exactly the debt-sheet symptom
        let depth = wall_thickness + 2.0 * across_overshoot;

        let door_a = build_placed_cuboid(
            Vector3::new(0.0, 0.0, 0.0),
            door_width,
            door_height,
            depth,
            Vector3::new(-1.9, door_height * 0.5, -1.45),
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
        );
        let door_b = build_placed_cuboid(
            Vector3::new(0.0, 0.0, 0.0),
            door_width,
            door_height,
            depth,
            Vector3::new(0.2, door_height * 0.5, 0.15),
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
        );
        let door_c = build_placed_cuboid(
            Vector3::new(0.0, 0.0, 0.0),
            door_width,
            door_height,
            depth,
            Vector3::new(1.35, door_height * 0.5, 0.6),
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
        );

        let result = boolean_subtraction_many(
            &wall,
            &[door_a, door_b, door_c],
            BooleanOptions::default(),
        );

        let output = result.expect(
            "wall-from-offsets + 3 doors must succeed (the actual #6b debt-sheet reproducer)",
        );
        assert_closed_solid(&output.brep);
    }

    /// **#6a debt-sheet reproducer using the actual wall-from-offsets
    /// pipeline**: same wall, single cutter offset by exactly 1 mm past
    /// one wall face — the precise condition the debt sheet says triggers
    /// "Boolean kernel produced a degenerate result triangle".
    #[test]
    fn wall_from_offsets_with_single_1mm_overshoot_cutter_succeeds() {
        let centerline = vec![
            Vector3::new(-2.6, 0.0, -1.9),
            Vector3::new(-1.2, 0.0, -1.0),
            Vector3::new(-0.2, 0.0, 0.1),
            Vector3::new(0.6, 0.0, 0.2),
            Vector3::new(0.1, 0.0, 1.0),
            Vector3::new(2.6, 0.0, 2.0),
        ];
        let wall_thickness = 0.45;
        let wall_height = 3.0;
        let wall = build_wall_from_offsets(centerline, wall_thickness, wall_height);

        let door_height = 2.1;
        let door_width = 0.8;
        let across_overshoot = 0.001;
        let depth = wall_thickness + 2.0 * across_overshoot;

        let door = build_placed_cuboid(
            Vector3::new(0.0, 0.0, 0.0),
            door_width,
            door_height,
            depth,
            Vector3::new(-1.9, door_height * 0.5, -1.45),
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
        );

        let output = boolean_subtraction(&wall, &door, BooleanOptions::default())
            .expect("wall-from-offsets + 1mm overshoot cutter must succeed (#6a)");
        assert_closed_solid(&output.brep);
    }

    /// Sanity check: the 8-vertex U-shape extrusion produces a closed solid
    /// the boolean kernel can consume. Pre-condition for the `#6b` /
    /// `#6a` debt-sheet reproducers below.
    #[test]
    fn polywall_u_shape_extrusion_is_a_valid_closed_solid() {
        let polywall = build_polywall_u_shape(2.0, 0.3, 3.0);
        assert_closed_solid(&polywall);
        // 8 side walls (one per outer-ring edge) + top + bottom = 10 faces.
        assert!(
            polywall.faces.len() >= 10,
            "U-shape extrusion expected ≥10 faces, got {}",
            polywall.faces.len()
        );
    }

    /// **#6b debt-sheet reproducer (the actual one).** OpenPlans
    /// `examples/src/elements/solids/poly-wall-openings.html` "Add 3 (one per
    /// segment)": U-shape PolyWall (8-vertex base, 0.3 m thickness, 3 m
    /// height), three door cutters at the three segment midpoints. The
    /// debt sheet specifies cutters with ~1 mm overshoot (the `#6a` precision
    /// regime), arms ≥ 6 m, doors at midpoint with ≥ 3 m clearance from
    /// corners. Pre-WS-1 this fails on the third cutter with "Triangle mesh
    /// is not closed or contains non-manifold edges".
    #[test]
    fn polywall_u_shape_with_three_segment_doors_succeeds() {
        let arm_length = 6.0; // matches debt-sheet "≥ 3 m clearance from corners"
        let thickness = 0.3;
        let height = 3.0;
        let polywall = build_polywall_u_shape(arm_length, thickness, height);

        // Three door cutters: left arm midpoint, back wall midpoint, right
        // arm midpoint. 1 mm overshoot on each across-wall side — exactly
        // the `#6a` snap-tolerance regime that the debt sheet says triggers
        // the cumulative-drift class of failures.
        let door_width = 0.8;
        let door_height = 2.1;
        let across_overshoot = 0.001; // 1 mm
        let door_depth = thickness + 2.0 * across_overshoot;
        let half = arm_length - thickness * 0.5;

        let left_door = build_placed_cuboid(
            Vector3::new(0.0, 0.0, 0.0),
            door_depth,                                    // along x (perpendicular to left wall)
            door_height,                                   // along y (vertical)
            door_width,                                    // along z (along the arm)
            Vector3::new(-half, door_height * 0.5, -arm_length * 0.5),
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
        );
        let back_door = build_placed_cuboid(
            Vector3::new(0.0, 0.0, 0.0),
            door_width,                                    // along x (along back wall)
            door_height,                                   // along y
            door_depth,                                    // along z (perpendicular to back wall)
            Vector3::new(0.0, door_height * 0.5, -arm_length + thickness * 0.5),
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
        );
        let right_door = build_placed_cuboid(
            Vector3::new(0.0, 0.0, 0.0),
            door_depth,
            door_height,
            door_width,
            Vector3::new(half, door_height * 0.5, -arm_length * 0.5),
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
        );

        let result = boolean_subtraction_many(
            &polywall,
            &[left_door, back_door, right_door],
            BooleanOptions::default(),
        );

        let output = result.expect("U-shape PolyWall + 3 segment doors should succeed (#6b)");
        assert_closed_solid(&output.brep);
    }

    /// WS-3 — confirms structured `BooleanErrorKind::DegenerateTriangle` /
    /// `BooleanErrorKind::CoincidentFaces` variants exist post-WS-0 so apps
    /// can `match` on them. Constructed directly because reproducing a
    /// degenerate boolmesh failure deterministically across boolmesh versions
    /// is fragile.
    #[test]
    fn degenerate_triangle_and_coincident_faces_variants_are_distinct() {
        let degenerate = BooleanError::new(
            BooleanErrorKind::DegenerateTriangle,
            "boolmesh degenerate output",
        )
        .with_phase(BooleanErrorPhase::OutputValidation);
        let coincident = BooleanError::new(
            BooleanErrorKind::CoincidentFaces,
            "coincident faces detected",
        )
        .with_details("lhs face #2 coplanar with rhs face #5 within 9.876e-4");

        assert_eq!(degenerate.kind(), &BooleanErrorKind::DegenerateTriangle);
        assert_eq!(coincident.kind(), &BooleanErrorKind::CoincidentFaces);
        assert_eq!(degenerate.phase(), BooleanErrorPhase::OutputValidation);
        assert!(coincident
            .details()
            .map(|d| d.contains("coplanar"))
            .unwrap_or(false));
    }
}
