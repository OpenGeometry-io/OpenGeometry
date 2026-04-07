use super::windingsort;
use crate::brep::{Brep, BrepBuilder};
use openmaths::Vector3;
use uuid::Uuid;
use wasm_bindgen::prelude::*;

const EXTRUDE_EPSILON: f64 = 1.0e-9;

pub fn extrude_profile_loops(
    brep_id: Uuid,
    outer: &[Vector3],
    holes: &[Vec<Vector3>],
    height: f64,
) -> Result<Brep, String> {
    if !height.is_finite() || height.abs() <= EXTRUDE_EPSILON {
        return Err("Extrude height must be a finite non-zero number".to_string());
    }

    let outer = sanitize_loop_points(outer);
    if outer.len() < 3 {
        return Ok(Brep::new(brep_id));
    }

    let holes: Vec<Vec<Vector3>> = holes
        .iter()
        .map(|hole| sanitize_loop_points(hole))
        .filter(|hole| hole.len() >= 3)
        .collect();

    let Some(profile_normal) = compute_loop_normal(&outer) else {
        return Err("Failed to compute a stable normal from the extrusion profile".to_string());
    };

    let extrusion = Vector3::new(0.0, height, 0.0);
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

fn compute_loop_normal(points: &[Vector3]) -> Option<Vector3> {
    if points.len() < 3 {
        return None;
    }

    let origin = points[0];
    for index in 1..(points.len() - 1) {
        let edge_a = vector_difference(points[index], origin);
        let edge_b = vector_difference(points[index + 1], origin);
        let cross = edge_a.cross(&edge_b);
        let length_sq = cross.dot(&cross);
        if length_sq > EXTRUDE_EPSILON * EXTRUDE_EPSILON {
            let inv_length = length_sq.sqrt().recip();
            return Some(Vector3::new(
                cross.x * inv_length,
                cross.y * inv_length,
                cross.z * inv_length,
            ));
        }
    }

    None
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
    fn try_extrude_brep_face_rejects_zero_height() {
        let outer = rectangle(0.0, 0.0, 2.0, 2.0);
        let source = build_face_brep(&outer, &[]);

        let error = try_extrude_brep_face(source, 0.0)
            .err()
            .expect("zero height should fail");

        assert!(error.contains("finite non-zero"));
    }
}
