use crate::brep::fine_accuracy;
use crate::brep::{BrepEnvelope, Frame3, Orientation, Surface, SurfaceGeometry};
use crate::math::{dot, sub};
use crate::primitives::cuboid::cuboid;
use crate::primitives::cylinder::cylinder;
use crate::primitives::internal::{
    annular_cylinder, cone, cylinder_sector, frustum, sphere, torus,
};
use crate::tessellation::tessellate;

#[test]
fn cuboid_edges_are_shared_and_all_face_normals_point_outward() {
    let frame = Frame3 {
        origin: [1.0, 2.0, 3.0],
        x: [0.0, 1.0, 0.0],
        y: [-1.0, 0.0, 0.0],
        ..Frame3::IDENTITY
    };
    let body = cuboid("box".into(), frame, [2.0, 3.0, 4.0], fine_accuracy()).unwrap();
    assert_eq!(
        (
            body.topology.vertices.len(),
            body.topology.edges.len(),
            body.topology.faces.len()
        ),
        (8, 12, 6)
    );
    let center = frame.point([1.0, 1.5, 2.0]);
    for face in &body.topology.faces {
        let surface = body.geometry.surface(face.surface).unwrap();
        let uv = face.trim.uv_bounds.map(|range| (range.lo + range.hi) / 2.0);
        assert!(
            dot(
                sub(surface.point_at(uv).unwrap(), center),
                surface.normal_at(uv).unwrap()
            ) > 0.0
        );
    }
    let serialized = body.to_json().unwrap();
    assert_eq!(
        BrepEnvelope::from_json(&serialized)
            .unwrap()
            .to_json()
            .unwrap(),
        serialized
    );
    assert!(cuboid("bad".into(), frame, [0.0, 1.0, 1.0], fine_accuracy()).is_err());
}
#[test]
fn curved_topology_is_sparse_and_closed() {
    let shapes = [
        cylinder("c".into(), Frame3::IDENTITY, 1.0, 2.0, fine_accuracy()).unwrap(),
        cone("k".into(), Frame3::IDENTITY, 1.0, 2.0, fine_accuracy()).unwrap(),
        sphere("s".into(), Frame3::IDENTITY, 1.0, fine_accuracy()).unwrap(),
        torus("t".into(), Frame3::IDENTITY, 3.0, 1.0, fine_accuracy()).unwrap(),
    ];
    for (b, faces) in shapes.into_iter().zip([3, 2, 1, 1]) {
        b.validate().unwrap();
        assert_eq!(b.topology.faces.len(), faces);
        assert!(b.topology.vertices.len() <= 2);
        assert_eq!(b.solids.len(), 1);
        let json = b.to_json().unwrap();
        BrepEnvelope::from_json(&json).unwrap();
    }
}
#[test]
fn annular_cylinder_has_exact_inner_outer_supports_and_planar_holes() {
    let body = annular_cylinder(
        "tube".into(),
        Frame3::IDENTITY,
        0.5,
        1.0,
        2.0,
        fine_accuracy(),
    )
    .unwrap();
    body.validate().unwrap();
    assert_eq!(body.topology.faces.len(), 4);
    assert_eq!(body.topology.shells.len(), 1);
    assert_eq!(body.solids.len(), 1);
    assert_eq!(body.topology.faces[0].sense, Orientation::Forward);
    assert_eq!(body.topology.faces[1].sense, Orientation::Reverse);
    assert_eq!(body.topology.faces[2].trim.holes.len(), 1);
    assert_eq!(body.topology.faces[3].trim.holes.len(), 1);
    let mesh = tessellate(&body, 0.01, 100_000).unwrap();
    assert!(!mesh.indices.is_empty());
    assert!(annular_cylinder(
        "thin".into(),
        Frame3::IDENTITY,
        1.0,
        1.0 + fine_accuracy().geometric,
        2.0,
        fine_accuracy(),
    )
    .is_err());
}
#[test]
fn frusta_reduce_to_the_supported_surface_set() {
    for radii in [(1.0, 2.0), (2.0, 1.0), (2.0, 0.0), (1.0, 1.0)] {
        let b = frustum(
            "f".into(),
            Frame3::IDENTITY,
            radii.0,
            radii.1,
            3.0,
            fine_accuracy(),
        )
        .unwrap();
        b.validate().unwrap();
        assert_eq!(b.topology.faces.len(), if radii.1 == 0.0 { 2 } else { 3 });
    }
}
#[test]
fn ring_and_positive_dimensions_are_enforced() {
    assert!(torus("t".into(), Frame3::IDENTITY, 1.0, 1.0, fine_accuracy()).is_err());
    assert!(cylinder("c".into(), Frame3::IDENTITY, 1.0, 0.0, fine_accuracy()).is_err());
}
#[test]
fn body_bounds_cover_curved_extrema_absent_from_vertices() {
    let b = cylinder("c".into(), Frame3::IDENTITY, 2.0, 3.0, fine_accuracy()).unwrap();
    assert!(b.topology.vertices.iter().all(|v| v.position[0] == 2.0));
    let bounds = b.bounds().unwrap().unwrap();
    for p in [[-2.0, 0.0, 1.5], [0.0, -2.0, 1.5], [0.0, 2.0, 3.0]] {
        assert!(bounds.contains(p));
    }
    let empty = BrepEnvelope::new("empty".into(), fine_accuracy()).unwrap();
    assert!(empty.bounds().unwrap().is_none());
}
#[test]
fn cylinder_sector_keeps_one_trimmed_cylinder_face_and_exact_radial_caps() {
    let sector = cylinder_sector(
        "sector".into(),
        Frame3::IDENTITY,
        2.0,
        3.0,
        -1.2,
        1.2,
        fine_accuracy(),
    )
    .unwrap();
    sector.validate().unwrap();
    assert_eq!(sector.topology.faces.len(), 5);
    assert_eq!(sector.topology.edges.len(), 9);
    assert_eq!(sector.topology.vertices.len(), 6);
    assert!(matches!(
        sector.geometry.surfaces[sector.topology.faces[0].surface as usize],
        SurfaceGeometry::Cylinder { radius: 2.0, .. }
    ));
    assert!(sector.topology.faces[1..].iter().all(|face| matches!(
        sector.geometry.surfaces[face.surface as usize],
        SurfaceGeometry::Plane { .. }
    )));
    let coarse = tessellate(&sector, 0.05, 100_000).unwrap();
    let fine = tessellate(&sector, 0.005, 100_000).unwrap();
    assert!(fine.indices.len() > coarse.indices.len());
    BrepEnvelope::from_json(&sector.to_json().unwrap()).unwrap();

    let full = cylinder_sector(
        "full".into(),
        Frame3::IDENTITY,
        2.0,
        3.0,
        0.0,
        std::f64::consts::TAU,
        fine_accuracy(),
    )
    .unwrap();
    assert_eq!(full.topology.faces.len(), 3);
}
