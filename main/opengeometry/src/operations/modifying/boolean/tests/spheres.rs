use super::support::{sphere, volume};
use crate::brep::{
    placed, Accuracy, BrepEnvelope, Frame3, GeometryError, Orientation, Surface, SurfaceGeometry,
};
use crate::math::{add, norm, scale, sub, Point3};
use crate::operations::modifying::boolean::handlers::boolean_spheres;
use crate::operations::modifying::boolean::types::BooleanOp;
use crate::primitives;
use crate::tessellation::tessellate;

#[test]
fn transverse_spheres_preserve_surfaces_shared_edge_and_occupancy_volumes() {
    let a = sphere("a", [0.0; 3], 1.0);
    let b = sphere("b", [1.0, 0.0, 0.0], 1.0);
    let unit_volume = 4.0 * std::f64::consts::PI / 3.0;
    let lens = 5.0 * std::f64::consts::PI / 12.0;
    for (op, expected) in [
        (BooleanOp::Union, 2.0 * unit_volume - lens),
        (BooleanOp::Intersection, lens),
        (BooleanOp::Subtraction, unit_volume - lens),
    ] {
        let r = boolean_spheres(&a, &b, op, "r".into()).unwrap();
        assert_eq!(r.brep.topology.faces.len(), 2);
        let occupied = |point: Point3| {
            let inside_a = norm(sub(point, [0.0; 3])) < 1.0;
            let inside_b = norm(sub(point, [1.0, 0.0, 0.0])) < 1.0;
            match op {
                BooleanOp::Union => inside_a || inside_b,
                BooleanOp::Intersection => inside_a && inside_b,
                BooleanOp::Subtraction => inside_a && !inside_b,
            }
        };
        for face in &r.brep.topology.faces {
            let surface = r.brep.geometry.surface(face.surface).unwrap();
            let v = face.trim.uv_bounds[1];
            for fraction in [0.25, 0.5, 0.75] {
                let uv = [0.9, v.lo + fraction * v.width()];
                let point = surface.point_at(uv).unwrap();
                let outward = scale(surface.normal_at(uv).unwrap(), face.sense.multiplier());
                assert!(occupied(sub(point, scale(outward, 1e-5))));
                assert!(!occupied(add(point, scale(outward, 1e-5))));
            }
        }
        assert!(r
            .brep
            .geometry
            .surfaces
            .iter()
            .all(|s| matches!(s, SurfaceGeometry::Sphere { .. })));
        let shared: Vec<_> = r
            .brep
            .topology
            .edges
            .iter()
            .filter(|e| !e.chart_seam)
            .collect();
        assert_eq!(shared.len(), 1);
        assert!(shared[0].twin_halfedge.is_some());
        assert!((volume(&r.brep) - expected).abs() < 0.04);
        assert_eq!(r.report.face_mappings[0].result_faces, vec![0]);
        assert_eq!(r.report.face_mappings[1].result_faces, vec![1]);
        assert_eq!(
            r.brep.topology.faces[1].provenance.reversed,
            op == BooleanOp::Subtraction
        );
    }
}

#[test]
fn enclosed_subtraction_orients_cavity_and_preserves_ancestry() {
    let a = sphere("host", [0.0; 3], 2.0);
    let b = sphere("cutter", [0.2, 0.0, 0.0], 0.5);
    let r = boolean_spheres(&a, &b, BooleanOp::Subtraction, "r".into()).unwrap();
    assert_eq!(r.brep.solids[0].cavity_shells, vec![1]);
    assert_eq!(r.brep.topology.faces[1].sense, Orientation::Reverse);
    assert_eq!(
        r.brep.topology.faces[1].provenance.sources[0].entity,
        "cutter"
    );
    assert!((volume(&r.brep) - 4.0 * std::f64::consts::PI / 3.0 * (8.0 - 0.125)).abs() < 0.1);
}

#[test]
fn contact_disjoint_containment_and_coincident_regularize_deterministically() {
    let a = sphere("a", [0.0; 3], 1.0);
    for x in [2.0, 2.5] {
        let b = sphere("b", [x, 0.0, 0.0], 1.0);
        let r = boolean_spheres(&a, &b, BooleanOp::Union, "r".into()).unwrap();
        assert_eq!(r.brep.solids.len(), 2);
        assert_eq!(r.report.contacts.len(), usize::from(x == 2.0));
        assert!(boolean_spheres(&a, &b, BooleanOp::Intersection, "r".into())
            .unwrap()
            .brep
            .solids
            .is_empty());
    }
    let b = sphere("b", [0.0; 3], 1.0);
    let r = boolean_spheres(&a, &b, BooleanOp::Union, "r".into()).unwrap();
    assert!(r.report.coincident);
    assert_eq!(r.brep.topology.faces[0].provenance.sources.len(), 2);
    assert!(boolean_spheres(&a, &b, BooleanOp::Subtraction, "r".into())
        .unwrap()
        .brep
        .topology
        .faces
        .is_empty());
    let big = sphere("big", [0.0; 3], 2.0);
    assert!(
        boolean_spheres(&a, &big, BooleanOp::Subtraction, "r".into())
            .unwrap()
            .brep
            .solids
            .is_empty()
    );
    assert_eq!(
        boolean_spheres(&a, &big, BooleanOp::Intersection, "r".into())
            .unwrap()
            .brep
            .topology
            .faces[0]
            .provenance
            .sources[0]
            .entity,
        "a"
    );
}

#[test]
fn unsupported_inputs_and_invalid_geometry_never_enter_a_mesh_bridge() {
    let a = sphere("a", [0.0; 3], 1.0);
    let cylinder =
        primitives::cylinder("c".into(), Frame3::IDENTITY, 1.0, 2.0, a.accuracy).unwrap();
    assert!(matches!(
        boolean_spheres(&a, &cylinder, BooleanOp::Union, "r".into()),
        Err(GeometryError::CoverageGap { .. })
    ));
    let mut invalid = a.clone();
    invalid.topology.faces[0].surface = 100;
    assert!(matches!(
        boolean_spheres(&a, &invalid, BooleanOp::Union, "r".into()),
        Err(GeometryError::MissingReference { .. })
    ));
    let mut trimmed = a.clone();
    trimmed.topology.faces[0].trim.uv_bounds[1].hi = 0.5;
    assert!(boolean_spheres(&a, &trimmed, BooleanOp::Union, "r".into()).is_err());
}

#[test]
fn positive_similarity_roundoff_does_not_reject_full_sphere_operands() {
    let rotation = std::f64::consts::PI / 7.0;
    let placement = Frame3 {
        origin: [0.2, 0.1, 0.2],
        x: [rotation.cos(), 0.0, -rotation.sin()],
        y: [0.0, 1.0, 0.0],
        z: [rotation.sin(), 0.0, rotation.cos()],
    };
    let frame = Frame3 {
        origin: [0.0, 1.2, 0.0],
        y: [0.0, 0.0, -1.0],
        z: [0.0, 1.0, 0.0],
        ..Frame3::IDENTITY
    };
    let first = primitives::sphere(
        "a".into(),
        frame,
        1.0,
        sphere("reference", [0.0; 3], 1.0).accuracy,
    )
    .unwrap();
    let second = primitives::sphere(
        "b".into(),
        Frame3 {
            origin: [1.0, 1.2, 0.0],
            ..frame
        },
        1.0,
        first.accuracy,
    )
    .unwrap();
    let first = placed(&first, placement, 1.25).unwrap();
    let second = placed(&second, placement, 1.25).unwrap();
    for op in [
        BooleanOp::Union,
        BooleanOp::Intersection,
        BooleanOp::Subtraction,
    ] {
        let result = boolean_spheres(&first, &second, op, "placed".into()).unwrap();
        result.brep.validate().unwrap();
        assert_eq!(result.brep.topology.faces.len(), 2);
        assert_eq!(result.report.face_mappings[0].source.entity, "a");
    }
}

#[test]
fn internal_tangency_and_unresolved_small_caps_are_errors() {
    let a = sphere("a", [0.0; 3], 2.0);
    let b = sphere("b", [1.0, 0.0, 0.0], 1.0);
    assert!(matches!(
        boolean_spheres(&a, &b, BooleanOp::Subtraction, "r".into()),
        Err(GeometryError::UnresolvedIntersection(_))
    ));
    let a = sphere("a", [0.0; 3], 1.0);
    let b = sphere("b", [2.0 - 1e-10, 0.0, 0.0], 1.0);
    assert!(matches!(
        boolean_spheres(&a, &b, BooleanOp::Intersection, "r".into()),
        Err(GeometryError::UnresolvedIntersection(_))
    ));
}

#[test]
fn rotated_unequal_spheres_roundtrip_and_keep_identical_boundary_positions() {
    let a = sphere("a", [5.0, -3.0, 8.0], 2.0);
    let b = sphere("b", [6.0, -2.0, 9.0], 1.25);
    let r = boolean_spheres(&a, &b, BooleanOp::Subtraction, "r".into()).unwrap();
    let decoded = BrepEnvelope::from_json(&r.brep.to_json().unwrap()).unwrap();
    let mesh = tessellate(&decoded, 0.02, 2_000_000).unwrap();
    let boundary = decoded.geometry.curves[0].elementary_point(0.0).unwrap();
    let count = mesh
        .positions
        .chunks_exact(3)
        .filter(|p| p == &boundary.as_slice())
        .count();
    assert!(count >= 2);
    assert!(mesh.indices.len() > 100);
}

#[test]
fn sphere_booleans_obey_scaled_budgets_and_building_coordinates() {
    for size in [1e-6, 1.0, 1e6] {
        let accuracy = Accuracy {
            geometric: size * 1e-9,
            intersection: size * 1e-10,
            tessellation: size * 0.01,
            exchange: size * 1e-5,
        };
        let a = primitives::sphere("a".into(), Frame3::IDENTITY, size, accuracy).unwrap();
        let b = primitives::sphere(
            "b".into(),
            Frame3 {
                origin: [size, 0.0, 0.0],
                ..Frame3::IDENTITY
            },
            size,
            accuracy,
        )
        .unwrap();
        let r = boolean_spheres(&a, &b, BooleanOp::Subtraction, "r".into()).unwrap();
        assert_eq!(r.brep.topology.faces.len(), 2);
        tessellate(&r.brep, size * 0.02, 2_000_000).unwrap();
    }
    let accuracy = Accuracy {
        geometric: 1e-6,
        intersection: 5e-7,
        tessellation: 0.01,
        exchange: 1e-5,
    };
    let a = primitives::sphere(
        "a".into(),
        Frame3 {
            origin: [1e9; 3],
            ..Frame3::IDENTITY
        },
        1.0,
        accuracy,
    )
    .unwrap();
    let b = primitives::sphere(
        "b".into(),
        Frame3 {
            origin: [1e9 + 1.0, 1e9, 1e9],
            ..Frame3::IDENTITY
        },
        1.0,
        accuracy,
    )
    .unwrap();
    let r = boolean_spheres(&a, &b, BooleanOp::Intersection, "r".into()).unwrap();
    tessellate(&r.brep, 0.01, 2_000_000).unwrap();
}
