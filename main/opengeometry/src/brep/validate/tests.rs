use crate::brep::{
    coarse_accuracy, BrepEnvelope, CurveGeometry, EdgeGeometry, Frame3, GeometryError, Orientation,
};
use crate::primitives;

fn tube() -> BrepEnvelope {
    primitives::annular_cylinder(
        "tube".into(),
        Frame3::IDENTITY,
        0.4,
        1.0,
        2.0,
        coarse_accuracy(1e-6),
    )
    .unwrap()
}

fn tube_with_old_winding() -> BrepEnvelope {
    let mut brep = tube();
    let mut loops = Vec::new();
    for face in &brep.topology.faces {
        loops.extend(&face.trim.holes);
        if face.key == "inner" {
            loops.push(face.trim.outer);
        }
    }
    brep.reverse_loops(&loops).unwrap();
    brep
}

fn circular_turn(brep: &BrepEnvelope, face: u32, loop_id: u32) -> f64 {
    let use_ =
        &brep.topology.halfedges[brep.topology.loops[loop_id as usize].start_halfedge as usize];
    assert_eq!(use_.next, Some(use_.id), "one edge per cap loop");
    let edge = &brep.topology.edges[use_.edge as usize];
    let EdgeGeometry::Curve { curve, .. } = edge.geometry else {
        panic!("cap loop edge is a curve")
    };
    let CurveGeometry::Circle { frame, .. } = brep.geometry.curves[curve as usize] else {
        panic!("cap loop edge is a circle")
    };
    let point = brep.topology.vertices[use_.from as usize].position;
    let normal = brep.face_normal_at(face, point).unwrap();
    let along: f64 = (0..3).map(|i| frame.z[i] * normal[i]).sum();
    along.signum() * use_.geometry_use.sense.multiplier()
}

#[test]
fn cap_holes_run_clockwise_and_outer_loops_counter_clockwise() {
    let brep = tube();
    brep.validate().unwrap();
    for face in brep
        .topology
        .faces
        .iter()
        .filter(|f| f.key.ends_with("cap"))
    {
        assert_eq!(circular_turn(&brep, face.id, face.trim.outer), 1.0);
        for &hole in &face.trim.holes {
            assert_eq!(circular_turn(&brep, face.id, hole), -1.0, "{}", face.key);
        }
    }
}

#[test]
fn validate_rejects_loops_against_the_material_normal() {
    let err = tube_with_old_winding().validate().unwrap_err();
    assert!(
        matches!(&err, GeometryError::InvalidTopology(m) if m.contains("material normal")),
        "{err:?}"
    );
}

#[test]
fn a_flipped_face_sense_with_its_loop_unturned_is_rejected() {
    let mut brep = primitives::cuboid(
        "box".into(),
        Frame3::IDENTITY,
        [1.0, 1.0, 1.0],
        coarse_accuracy(1e-6),
    )
    .unwrap();
    for face in &mut brep.topology.faces {
        face.sense = Orientation::Reverse;
    }
    assert!(brep.validate().is_err());
    brep.orient_loops().unwrap();
    brep.validate().unwrap();
}

#[test]
fn bodies_with_the_old_winding_load_with_their_loops_turned() {
    let old = serde_json::to_string(&tube_with_old_winding()).unwrap();
    let loaded = BrepEnvelope::from_json(&old).unwrap();
    assert_eq!(loaded.topology.halfedges, tube().topology.halfedges);
}
