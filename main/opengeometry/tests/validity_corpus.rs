//! Labelled validity corpus (debt item D5, NFR-3).
//!
//! NFR-3 requires the validity checker to classify a labelled set of valid /
//! invalid B-reps with ≥99% precision on "invalid" — i.e. it must never wave a
//! genuinely-invalid B-rep through as valid. This corpus pairs known-good
//! primitives with deliberately-broken B-reps and asserts the checker's verdict
//! matches the label for every case (so: zero false "valid").

use opengeometry::brep::validity::check_validity;
use opengeometry::brep::{Brep, BrepBuilder, HalfEdge};
use opengeometry::primitives::cuboid::OGCuboid;
use opengeometry::primitives::cylinder::OGCylinder;
use opengeometry::primitives::sphere::OGSphere;
use openmaths::Vector3;
use uuid::Uuid;

fn cuboid() -> Brep {
    let mut c = OGCuboid::new("v".into());
    c.set_config(Vector3::new(0.0, 0.0, 0.0), 2.0, 2.0, 2.0)
        .unwrap();
    c.world_brep()
}

fn cylinder() -> Brep {
    let mut c = OGCylinder::new("v".into());
    c.set_config(
        Vector3::new(0.0, 0.0, 0.0),
        1.0,
        2.0,
        2.0 * std::f64::consts::PI,
        24,
    )
    .unwrap();
    c.world_brep()
}

fn sphere() -> Brep {
    let mut s = OGSphere::new("v".into());
    s.set_config(Vector3::new(0.0, 0.0, 0.0), 1.0, 18, 12)
        .unwrap();
    s.world_brep()
}

fn open_triangle() -> Brep {
    let mut b = BrepBuilder::new(Uuid::new_v4());
    b.add_vertices(&[
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(1.0, 0.0, 0.0),
        Vector3::new(0.0, 0.0, 1.0),
    ]);
    b.add_face(&[0, 1, 2], &[]).unwrap();
    b.build().unwrap()
}

fn non_finite_vertex() -> Brep {
    let mut brep = cuboid();
    if let Some(v) = brep.vertices.get_mut(0) {
        v.position = Vector3::new(f64::NAN, 0.0, 0.0);
    }
    brep
}

fn non_manifold_edge() -> Brep {
    // A closed cuboid with a third half-edge grafted onto an existing edge:
    // that edge is now shared by three half-edges, which is non-manifold and
    // must be flagged.
    let mut brep = cuboid();
    let edge_id = brep.edges[0].id;
    let he_id = brep.halfedges.len() as u32;
    brep.halfedges
        .push(HalfEdge::new(he_id, 0, 1, edge_id, None, None, None));
    brep
}

#[test]
fn checker_never_passes_an_invalid_brep() {
    // (brep, expected_valid)
    let cases: Vec<(&str, Brep, bool)> = vec![
        ("cuboid", cuboid(), true),
        ("cylinder", cylinder(), true),
        ("sphere", sphere(), true),
        ("open-triangle", open_triangle(), false),
        ("non-finite-vertex", non_finite_vertex(), false),
        ("non-manifold-edge", non_manifold_edge(), false),
    ];

    let mut false_valids = Vec::new();
    let mut false_invalids = Vec::new();

    for (name, brep, expected_valid) in &cases {
        let verdict = check_validity(brep).is_valid();
        if *expected_valid && !verdict {
            false_invalids.push(*name);
        }
        if !*expected_valid && verdict {
            // The cardinal sin (NFR-3): an invalid B-rep waved through as valid.
            false_valids.push(*name);
        }
        println!(
            "{}: expected_valid={} got_valid={}",
            name, expected_valid, verdict
        );
    }

    assert!(
        false_valids.is_empty(),
        "checker passed invalid B-reps as valid (NFR-3 violation): {:?}",
        false_valids
    );
    // Valids must also be recognized; otherwise the checker is uselessly strict.
    assert!(
        false_invalids.is_empty(),
        "checker rejected valid B-reps: {:?}",
        false_invalids
    );
}
