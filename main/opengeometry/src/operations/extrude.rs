use super::windingsort;
use crate::brep::{Brep, BrepBuilder};
use crate::operations::triangulate::compute_polygon_normal;
use openmaths::Vector3;
use uuid::Uuid;
use wasm_bindgen::prelude::*;

const EXTRUDE_EPSILON: f64 = crate::tolerance::MODELING_TOLERANCE_FLOOR;

/// How an extrusion grows relative to the sketch/profile plane (D3).
#[derive(Clone, Copy, Debug)]
pub enum ExtrudeExtent {
    /// Grows `distance` along the direction, profile stays on the start cap.
    OneSided { distance: f64 },
    /// Grows `distance/2` on each side of the profile plane.
    Symmetric { distance: f64 },
    /// Grows `plus` along the direction and `minus` against it (distinct).
    TwoSided { plus: f64, minus: f64 },
}

impl ExtrudeExtent {
    /// The (base-plane shift, extrusion vector) for a unit `direction`. The
    /// source cap is built at `profile + shift`; the end cap at `+ vector`.
    fn resolve(self, direction: Vector3) -> (Vector3, Vector3) {
        let scaled = |t: f64| Vector3::new(direction.x * t, direction.y * t, direction.z * t);
        match self {
            ExtrudeExtent::OneSided { distance } => (scaled(0.0), scaled(distance)),
            ExtrudeExtent::Symmetric { distance } => (scaled(-distance / 2.0), scaled(distance)),
            ExtrudeExtent::TwoSided { plus, minus } => (scaled(-minus), scaled(plus + minus)),
        }
    }

    /// Total span; must be finite and non-zero for a valid extrusion.
    fn span(self) -> f64 {
        match self {
            ExtrudeExtent::OneSided { distance } | ExtrudeExtent::Symmetric { distance } => {
                distance
            }
            ExtrudeExtent::TwoSided { plus, minus } => plus + minus,
        }
    }
}

pub fn extrude_profile_loops(
    brep_id: Uuid,
    outer: &[Vector3],
    holes: &[Vec<Vector3>],
    height: f64,
) -> Result<Brep, String> {
    // Backwards-compatible default: blind extrude up the world +Y axis.
    extrude_profile_loops_with(
        brep_id,
        outer,
        holes,
        Vector3::new(0.0, 1.0, 0.0),
        ExtrudeExtent::OneSided { distance: height },
    )
}

/// General extrude: arbitrary `direction` (need not be unit; it is normalized)
/// and an [`ExtrudeExtent`] (one-sided, symmetric, or two-sided).
pub fn extrude_profile_loops_with(
    brep_id: Uuid,
    outer: &[Vector3],
    holes: &[Vec<Vector3>],
    direction: Vector3,
    extent: ExtrudeExtent,
) -> Result<Brep, String> {
    let span = extent.span();
    if !span.is_finite() || span.abs() <= EXTRUDE_EPSILON {
        return Err("Extrude extent must be a finite non-zero distance".to_string());
    }

    let dir_len =
        (direction.x * direction.x + direction.y * direction.y + direction.z * direction.z).sqrt();
    if !dir_len.is_finite() || dir_len <= EXTRUDE_EPSILON {
        return Err("Extrude direction must be a non-zero vector".to_string());
    }
    let direction = Vector3::new(
        direction.x / dir_len,
        direction.y / dir_len,
        direction.z / dir_len,
    );
    let (base_shift, extrusion) = extent.resolve(direction);

    let outer = sanitize_loop_points(outer);
    if outer.len() < 3 {
        return Ok(Brep::new(brep_id));
    }

    let holes: Vec<Vec<Vector3>> = holes
        .iter()
        .map(|hole| sanitize_loop_points(hole))
        .filter(|hole| hole.len() >= 3)
        .collect();

    // Shift the profile onto the start cap (no-op for one-sided extents).
    let shift =
        |p: &Vector3| Vector3::new(p.x + base_shift.x, p.y + base_shift.y, p.z + base_shift.z);
    let outer: Vec<Vector3> = outer.iter().map(shift).collect();
    let holes: Vec<Vec<Vector3>> = holes
        .iter()
        .map(|hole| hole.iter().map(shift).collect())
        .collect();

    let Some(profile_normal) = compute_polygon_normal(&outer) else {
        return Err("Failed to compute a stable normal from the extrusion profile".to_string());
    };

    let flip_source_face = profile_normal.dot(&extrusion) > 0.0;

    let mut builder = BrepBuilder::new(brep_id);
    let mut source_positions = Vec::new();

    source_positions.extend(outer.iter().copied());
    for hole in &holes {
        source_positions.extend(hole.iter().copied());
    }

    let end_positions: Vec<Vector3> = source_positions
        .iter()
        .map(|point| {
            Vector3::new(
                point.x + extrusion.x,
                point.y + extrusion.y,
                point.z + extrusion.z,
            )
        })
        .collect();

    let mut all_positions = source_positions.clone();
    all_positions.extend(end_positions.iter().copied());
    builder.add_vertices(&all_positions);

    let mut next_index = 0u32;
    let source_outer = range_indices(next_index, outer.len());
    next_index += outer.len() as u32;

    let mut source_holes = Vec::with_capacity(holes.len());
    for hole in &holes {
        let indices = range_indices(next_index, hole.len());
        next_index += hole.len() as u32;
        source_holes.push(indices);
    }

    let end_offset = source_positions.len() as u32;
    let end_outer: Vec<u32> = source_outer
        .iter()
        .map(|index| index + end_offset)
        .collect();
    let end_holes: Vec<Vec<u32>> = source_holes
        .iter()
        .map(|hole| hole.iter().map(|index| index + end_offset).collect())
        .collect();

    builder
        .add_face(
            &orient_loop_indices(&source_outer, flip_source_face),
            &orient_hole_indices(&source_holes, flip_source_face),
        )
        .map_err(|error| format!("Failed to build source extrusion cap: {}", error))?;

    builder
        .add_face(
            &orient_loop_indices(&end_outer, !flip_source_face),
            &orient_hole_indices(&end_holes, !flip_source_face),
        )
        .map_err(|error| format!("Failed to build extruded cap: {}", error))?;

    add_side_faces(&mut builder, &source_outer, &end_outer, flip_source_face)?;

    for (source_hole, end_hole) in source_holes.iter().zip(end_holes.iter()) {
        add_side_faces(&mut builder, source_hole, end_hole, flip_source_face)?;
    }

    builder
        .add_shell_from_all_faces(true)
        .map_err(|error| format!("Failed to build extrusion shell: {}", error))?;

    builder
        .build()
        .map_err(|error| format!("Failed to finalize extrusion BREP: {}", error))
}

pub fn try_extrude_brep_face(brep_face: Brep, height: f64) -> Result<Brep, String> {
    if let Some(face) = brep_face.faces.first() {
        let (outer, holes) = brep_face.get_vertices_and_holes_by_face_id(face.id);
        return extrude_profile_loops(brep_face.id, &outer, &holes, height);
    }

    if let Some(wire) = brep_face.wires.first() {
        let outer = brep_face
            .get_wire_vertex_indices(wire.id)
            .into_iter()
            .filter_map(|vertex_id| brep_face.vertices.get(vertex_id as usize))
            .map(|vertex| vertex.position)
            .collect::<Vec<_>>();

        return extrude_profile_loops(brep_face.id, &outer, &[], height);
    }

    let base_points = windingsort::ccw_test(brep_face.get_flattened_vertices());
    extrude_profile_loops(brep_face.id, &base_points, &[], height)
}

pub fn extrude_brep_face(brep_face: Brep, height: f64) -> Brep {
    let brep_id = brep_face.id;
    try_extrude_brep_face(brep_face, height).unwrap_or_else(|_| Brep::new(brep_id))
}

#[wasm_bindgen(js_name = extrudeBrepFace)]
pub fn extrude_brep_face_wasm(
    local_brep_serialized: String,
    height: f64,
) -> Result<String, JsValue> {
    let brep: Brep = serde_json::from_str(&local_brep_serialized).map_err(|error| {
        JsValue::from_str(&format!("Invalid local BRep JSON payload: {}", error))
    })?;
    let extruded = try_extrude_brep_face(brep, height)
        .map_err(|error| JsValue::from_str(&format!("Failed to extrude BRep face: {}", error)))?;
    extruded.validate_topology().map_err(|error| {
        JsValue::from_str(&format!("Extruded BRep topology is invalid: {}", error))
    })?;
    serde_json::to_string(&extruded).map_err(|error| {
        JsValue::from_str(&format!("Failed to serialize extruded BRep: {}", error))
    })
}

/// Wasm entry point for general extrude. `direction` is `[x,y,z]`; `extent_kind`
/// is `"one_sided" | "symmetric" | "two_sided"`; `d_plus`/`d_minus` are the
/// extent distances (`d_minus` ignored unless two-sided, `d_plus` is the
/// distance for one-sided/symmetric).
#[wasm_bindgen(js_name = extrudeBrepFaceDirectional)]
pub fn extrude_brep_face_directional_wasm(
    local_brep_serialized: String,
    direction: Vec<f64>,
    extent_kind: String,
    d_plus: f64,
    d_minus: f64,
) -> Result<String, JsValue> {
    if direction.len() != 3 {
        return Err(JsValue::from_str(
            "direction must be a 3-element [x,y,z] array",
        ));
    }
    let dir = Vector3::new(direction[0], direction[1], direction[2]);
    let extent = match extent_kind.as_str() {
        "one_sided" => ExtrudeExtent::OneSided { distance: d_plus },
        "symmetric" => ExtrudeExtent::Symmetric { distance: d_plus },
        "two_sided" => ExtrudeExtent::TwoSided {
            plus: d_plus,
            minus: d_minus,
        },
        other => {
            return Err(JsValue::from_str(&format!(
                "Unknown extent kind '{}': expected one_sided | symmetric | two_sided",
                other
            )))
        }
    };

    let brep: Brep = serde_json::from_str(&local_brep_serialized).map_err(|error| {
        JsValue::from_str(&format!("Invalid local BRep JSON payload: {}", error))
    })?;

    let (outer, holes) = if let Some(face) = brep.faces.first() {
        brep.get_vertices_and_holes_by_face_id(face.id)
    } else {
        (
            windingsort::ccw_test(brep.get_flattened_vertices()),
            Vec::new(),
        )
    };

    let extruded = extrude_profile_loops_with(brep.id, &outer, &holes, dir, extent)
        .map_err(|error| JsValue::from_str(&format!("Failed to extrude BRep face: {}", error)))?;
    extruded.validate_topology().map_err(|error| {
        JsValue::from_str(&format!("Extruded BRep topology is invalid: {}", error))
    })?;
    serde_json::to_string(&extruded).map_err(|error| {
        JsValue::from_str(&format!("Failed to serialize extruded BRep: {}", error))
    })
}

fn sanitize_loop_points(points: &[Vector3]) -> Vec<Vector3> {
    let mut cleaned = Vec::with_capacity(points.len());
    for point in points {
        let is_duplicate = cleaned.last().copied().map_or(false, |last| {
            let delta = vector_difference(*point, last);
            delta.dot(&delta) <= EXTRUDE_EPSILON * EXTRUDE_EPSILON
        });
        if !is_duplicate {
            cleaned.push(*point);
        }
    }

    if cleaned.len() > 2 {
        let first = cleaned[0];
        let last = cleaned[cleaned.len() - 1];
        let delta = vector_difference(first, last);
        if delta.dot(&delta) <= EXTRUDE_EPSILON * EXTRUDE_EPSILON {
            cleaned.pop();
        }
    }

    cleaned
}

fn range_indices(start: u32, count: usize) -> Vec<u32> {
    (0..count as u32).map(|offset| start + offset).collect()
}

fn orient_loop_indices(loop_indices: &[u32], reverse: bool) -> Vec<u32> {
    if reverse {
        loop_indices.iter().rev().copied().collect()
    } else {
        loop_indices.to_vec()
    }
}

fn orient_hole_indices(hole_indices: &[Vec<u32>], reverse: bool) -> Vec<Vec<u32>> {
    hole_indices
        .iter()
        .map(|hole| orient_loop_indices(hole, reverse))
        .collect()
}

fn add_side_faces(
    builder: &mut BrepBuilder,
    source_loop: &[u32],
    end_loop: &[u32],
    flip_source_face: bool,
) -> Result<(), String> {
    for index in 0..source_loop.len() {
        let next = (index + 1) % source_loop.len();
        let source_current = source_loop[index];
        let source_next = source_loop[next];
        let end_next = end_loop[next];
        let end_current = end_loop[index];

        let quad = if flip_source_face {
            vec![source_current, source_next, end_next, end_current]
        } else {
            vec![source_next, source_current, end_current, end_next]
        };

        builder
            .add_face(&quad, &[])
            .map_err(|error| format!("Failed to build extrusion side face: {}", error))?;
    }

    Ok(())
}

fn vector_difference(lhs: Vector3, rhs: Vector3) -> Vector3 {
    Vector3::new(lhs.x - rhs.x, lhs.y - rhs.y, lhs.z - rhs.z)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_face_brep(outer: &[Vector3], holes: &[Vec<Vector3>]) -> Brep {
        let mut builder = BrepBuilder::new(Uuid::new_v4());
        let mut vertices = outer.to_vec();
        let mut hole_indices = Vec::with_capacity(holes.len());

        for hole in holes {
            let start = vertices.len() as u32;
            vertices.extend(hole.iter().copied());
            hole_indices.push(range_indices(start, hole.len()));
        }

        builder.add_vertices(&vertices);
        builder
            .add_face(&range_indices(0, outer.len()), &hole_indices)
            .expect("build source face");
        builder.build().expect("build source brep")
    }

    fn rectangle(min_x: f64, min_z: f64, max_x: f64, max_z: f64) -> Vec<Vector3> {
        vec![
            Vector3::new(min_x, 0.0, min_z),
            Vector3::new(max_x, 0.0, min_z),
            Vector3::new(max_x, 0.0, max_z),
            Vector3::new(min_x, 0.0, max_z),
        ]
    }

    fn wall_outline_with_reflex_start() -> Vec<Vector3> {
        vec![
            Vector3::new(-2.721670458045537, 0.0, -1.7107348430402753),
            Vector3::new(-1.3465727086811485, 0.0, -0.826743432734597),
            Vector3::new(-0.3093165466574247, 0.0, 0.3142383454914993),
            Vector3::new(0.2836567956860831, 0.0, 0.38012427241855573),
            Vector3::new(-0.2561533171082996, 0.0, 1.0998710894777328),
            Vector3::new(2.5164370978203268, 0.0, 2.2089072554491835),
            Vector3::new(2.6835629021796734, 0.0, 1.7910927445508167),
            Vector3::new(0.45615331710829965, 0.0, 0.9001289105222672),
            Vector3::new(1.1163432043139168, 0.0, 0.01987572758144427),
            Vector3::new(-0.09068345334257541, 0.0, -0.11423834549149932),
            Vector3::new(-1.0534272913188514, 0.0, -1.173256567265403),
            Vector3::new(-2.478329541954463, 0.0, -2.0892651569597245),
        ]
    }

    #[test]
    fn extrude_profile_loops_preserves_holes() {
        let outer = rectangle(0.0, 0.0, 6.0, 4.0);
        let hole = vec![
            Vector3::new(2.0, 0.0, 1.0),
            Vector3::new(2.0, 0.0, 3.0),
            Vector3::new(4.0, 0.0, 3.0),
            Vector3::new(4.0, 0.0, 1.0),
        ];

        let brep = extrude_profile_loops(Uuid::new_v4(), &outer, &[hole], 2.0)
            .expect("profile extrusion should succeed");

        brep.validate_topology().expect("extrusion topology");
        assert_eq!(brep.shells.len(), 1);
        assert_eq!(brep.faces.len(), 10);
        assert_eq!(
            brep.faces
                .iter()
                .filter(|face| face.inner_loops.len() == 1)
                .count(),
            2
        );
    }

    #[test]
    fn extrude_profile_loops_preserves_concave_profiles() {
        let concave = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(4.0, 0.0, 0.0),
            Vector3::new(4.0, 0.0, 1.0),
            Vector3::new(2.0, 0.0, 1.0),
            Vector3::new(2.0, 0.0, 3.0),
            Vector3::new(0.0, 0.0, 3.0),
        ];

        let brep = extrude_profile_loops(Uuid::new_v4(), &concave, &[], 1.5)
            .expect("concave extrusion should succeed");

        brep.validate_topology()
            .expect("concave extrusion topology");
        assert_eq!(brep.shells.len(), 1);
        assert_eq!(brep.faces.len(), concave.len() + 2);
    }

    #[test]
    fn extrude_profile_loops_uses_global_winding_for_reflex_first_corner() {
        let wall_outline = wall_outline_with_reflex_start();

        let brep = extrude_profile_loops(Uuid::new_v4(), &wall_outline, &[], 2.6)
            .expect("wall extrusion should succeed");

        brep.validate_topology().expect("wall extrusion topology");
        assert_eq!(brep.faces.len(), wall_outline.len() + 2);
        assert!(brep.faces[0].normal.y < -0.999);
        assert!(brep.faces[1].normal.y > 0.999);
    }

    #[test]
    fn try_extrude_brep_face_preserves_holes_from_face_input() {
        let outer = rectangle(0.0, 0.0, 6.0, 4.0);
        let hole = vec![
            Vector3::new(2.0, 0.0, 1.0),
            Vector3::new(2.0, 0.0, 3.0),
            Vector3::new(4.0, 0.0, 3.0),
            Vector3::new(4.0, 0.0, 1.0),
        ];
        let source = build_face_brep(&outer, &[hole]);

        let brep = try_extrude_brep_face(source, 2.0).expect("face extrusion should succeed");

        brep.validate_topology().expect("face extrusion topology");
        assert_eq!(brep.shells.len(), 1);
        assert_eq!(brep.faces.len(), 10);
        assert_eq!(
            brep.faces
                .iter()
                .filter(|face| face.inner_loops.len() == 1)
                .count(),
            2
        );
    }

    #[test]
    fn symmetric_extrude_spans_both_sides_of_profile() {
        let square = rectangle(-1.0, -1.0, 1.0, 1.0);
        let brep = extrude_profile_loops_with(
            Uuid::new_v4(),
            &square,
            &[],
            Vector3::new(0.0, 1.0, 0.0),
            ExtrudeExtent::Symmetric { distance: 2.0 },
        )
        .expect("symmetric extrude");
        brep.validate_topology().expect("topology");

        let (min_y, max_y) = brep
            .vertices
            .iter()
            .fold((f64::MAX, f64::MIN), |(lo, hi), v| {
                (lo.min(v.position.y), hi.max(v.position.y))
            });
        assert!((min_y + 1.0).abs() < 1.0e-9, "spans to -1, got {}", min_y);
        assert!((max_y - 1.0).abs() < 1.0e-9, "spans to +1, got {}", max_y);
    }

    #[test]
    fn two_sided_extrude_uses_distinct_distances() {
        let square = rectangle(-0.5, -0.5, 0.5, 0.5);
        let brep = extrude_profile_loops_with(
            Uuid::new_v4(),
            &square,
            &[],
            Vector3::new(0.0, 1.0, 0.0),
            ExtrudeExtent::TwoSided {
                plus: 3.0,
                minus: 1.0,
            },
        )
        .expect("two sided extrude");
        brep.validate_topology().expect("topology");

        let (min_y, max_y) = brep
            .vertices
            .iter()
            .fold((f64::MAX, f64::MIN), |(lo, hi), v| {
                (lo.min(v.position.y), hi.max(v.position.y))
            });
        assert!((min_y + 1.0).abs() < 1.0e-9, "minus side -1, got {}", min_y);
        assert!((max_y - 3.0).abs() < 1.0e-9, "plus side +3, got {}", max_y);
    }

    #[test]
    fn arbitrary_direction_extrude_follows_direction() {
        // Non-axis direction: profile in XZ plane extruded along (1,1,0).
        let square = rectangle(-0.5, -0.5, 0.5, 0.5);
        let dir = Vector3::new(1.0, 1.0, 0.0);
        let dist = (2.0_f64).sqrt();
        let brep = extrude_profile_loops_with(
            Uuid::new_v4(),
            &square,
            &[],
            dir,
            ExtrudeExtent::OneSided { distance: dist },
        )
        .expect("directional extrude");
        brep.validate_topology().expect("topology");

        // The end cap is offset by the unit direction × distance = (1,1,0).
        let max_x = brep
            .vertices
            .iter()
            .fold(f64::MIN, |hi, v| hi.max(v.position.x));
        let max_y = brep
            .vertices
            .iter()
            .fold(f64::MIN, |hi, v| hi.max(v.position.y));
        assert!(
            (max_x - 1.5).abs() < 1.0e-9,
            "x shifted by 1, got {}",
            max_x
        );
        assert!(
            (max_y - 1.0).abs() < 1.0e-9,
            "y shifted by 1, got {}",
            max_y
        );
    }

    #[test]
    fn try_extrude_brep_face_rejects_zero_height() {
        let outer = rectangle(0.0, 0.0, 2.0, 2.0);
        let source = build_face_brep(&outer, &[]);

        let error = try_extrude_brep_face(source, 0.0)
            .err()
            .expect("zero height should fail");

        assert!(error.contains("finite non-zero"));
    }
}
