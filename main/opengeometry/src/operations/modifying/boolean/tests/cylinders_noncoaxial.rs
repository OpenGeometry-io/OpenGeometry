use super::support::{cylinder, volume};
use crate::brep::{Accuracy, FaceRole, Frame3, GeometryError, SurfaceGeometry};
use crate::exchange::export_step;
use crate::operations::modifying::boolean::dispatch::boolean_brep;
use crate::operations::modifying::boolean::types::BooleanOp;
use crate::primitives;
use crate::query::{classify_point, PointClassification};
use crate::tessellation::tessellate;

#[test]
fn nonparallel_cylinder_through_cut_keeps_numerical_ssi_authoritative() {
    let accuracy = Accuracy {
        geometric: 4e-8,
        intersection: 1e-8,
        tessellation: 0.01,
        exchange: 1e-5,
    };
    let host = primitives::cylinder(
        "host".into(),
        Frame3 {
            origin: [0.0, 0.0, -2.0],
            ..Frame3::IDENTITY
        },
        1.0,
        4.0,
        accuracy,
    )
    .unwrap();
    let cutter = primitives::cylinder(
        "cutter".into(),
        Frame3::from_axis([-2.0, 0.0, 0.2], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap(),
        0.6,
        4.0,
        accuracy,
    )
    .unwrap();
    let result =
        boolean_brep(&host, &cutter, BooleanOp::Subtraction, "through-cut".into()).unwrap();
    result.brep.validate().unwrap();
    assert_eq!(result.brep.topology.faces.len(), 4);
    assert_eq!(result.brep.topology.faces[0].trim.holes.len(), 2);
    assert_eq!(result.brep.topology.faces[3].provenance.role, FaceRole::Cut);
    assert!(result.brep.topology.faces[3].provenance.reversed);
    assert_eq!(result.brep.geometry.intersections.len(), 2);
    assert_eq!(result.report.face_mappings.len(), 6);
    assert_eq!(
        classify_point(&result.brep, [0.0, 0.0, 0.2]).unwrap(),
        PointClassification::Outside
    );
    assert_eq!(
        classify_point(&result.brep, [0.0, 0.9, 0.2]).unwrap(),
        PointClassification::Inside
    );
    let mesh = tessellate(&result.brep, 0.0075, 2_000_000).unwrap();
    assert!(
        mesh.triangle_face_ids.len() < 50_000,
        "cross-drill tessellation produced {} triangles",
        mesh.triangle_face_ids.len()
    );
}

#[test]
fn transverse_parallel_cylinders_build_two_arc_caps_and_shared_edges() {
    let accuracy = cylinder("accuracy", 0.0, 1.0, false).accuracy;
    let a = primitives::cylinder("a".into(), Frame3::IDENTITY, 1.0, 2.0, accuracy).unwrap();
    let b = primitives::cylinder(
        "b".into(),
        Frame3 {
            origin: [1.0, 0.0, 0.0],
            ..Frame3::IDENTITY
        },
        1.0,
        2.0,
        accuracy,
    )
    .unwrap();
    let union = boolean_brep(&a, &b, BooleanOp::Union, "union".into()).unwrap();
    let intersection =
        boolean_brep(&a, &b, BooleanOp::Intersection, "intersection".into()).unwrap();
    let subtraction = boolean_brep(&a, &b, BooleanOp::Subtraction, "subtraction".into()).unwrap();
    for (name, result) in [
        ("union", &union),
        ("intersection", &intersection),
        ("subtraction", &subtraction),
    ] {
        assert_eq!(result.brep.solids.len(), 1, "{name}");
        assert_eq!(result.brep.topology.faces.len(), 4, "{name}");
        for cap in 2..4 {
            assert_eq!(
                result
                    .brep
                    .topology
                    .halfedges
                    .iter()
                    .filter(|halfedge| halfedge.face == Some(cap))
                    .count(),
                2,
                "{name} cap {cap}"
            );
        }
        result.brep.validate().unwrap();
        assert!(!tessellate(&result.brep, 0.01, 2_000_000)
            .unwrap_or_else(|error| panic!("{name}: {error}"))
            .indices
            .is_empty());
        export_step(&result.brep, "metre").unwrap_or_else(|error| panic!("{name} STEP: {error}"));
    }
    assert_eq!(union.report.face_mappings.len(), 6);
    assert_eq!(intersection.report.face_mappings.len(), 6);
    assert_eq!(subtraction.report.face_mappings.len(), 6);
    assert_eq!(
        subtraction.brep.topology.faces[1].provenance.role,
        FaceRole::Cut
    );
    assert!(subtraction.brep.topology.faces[1].provenance.reversed);

    let cylinder_volume = 2.0 * std::f64::consts::PI;
    let intersection_volume = volume(&intersection.brep).abs();
    assert!((volume(&union.brep).abs() + intersection_volume - 2.0 * cylinder_volume).abs() < 0.2);
    assert!((volume(&subtraction.brep).abs() + intersection_volume - cylinder_volume).abs() < 0.2);

    let reversed = primitives::cylinder(
        "reversed".into(),
        Frame3 {
            origin: [1.0, 0.0, 2.0],
            x: [1.0, 0.0, 0.0],
            y: [0.0, -1.0, 0.0],
            z: [0.0, 0.0, -1.0],
        },
        1.0,
        2.0,
        accuracy,
    )
    .unwrap();
    let reversed_result = boolean_brep(
        &a,
        &reversed,
        BooleanOp::Intersection,
        "reversed-result".into(),
    )
    .unwrap();
    reversed_result.brep.validate().unwrap();
    assert!((volume(&reversed_result.brep).abs() - intersection_volume).abs() < 0.1);

    let shifted = primitives::cylinder(
        "shifted".into(),
        Frame3 {
            origin: [1.0, 0.0, 0.25],
            ..Frame3::IDENTITY
        },
        1.0,
        2.0,
        accuracy,
    )
    .unwrap();
    let shifted_union = boolean_brep(&a, &shifted, BooleanOp::Union, "gap".into());
    assert!(
        matches!(shifted_union, Err(GeometryError::UnresolvedIntersection(_))),
        "unexpected shifted-cylinder result: {:?}",
        shifted_union.err()
    );
    let tangent = primitives::cylinder(
        "tangent".into(),
        Frame3 {
            origin: [2.0, 0.0, 0.0],
            ..Frame3::IDENTITY
        },
        1.0,
        2.0,
        accuracy,
    )
    .unwrap();
    for (operation, solids) in [
        (BooleanOp::Union, 2),
        (BooleanOp::Intersection, 0),
        (BooleanOp::Subtraction, 1),
    ] {
        let contact =
            boolean_brep(&a, &tangent, operation, format!("contact-{operation:?}")).unwrap();
        assert_eq!(contact.brep.solids.len(), solids);
        assert_eq!(contact.report.contacts.len(), 2);
    }
    let near_tangent = primitives::cylinder(
        "near-tangent".into(),
        Frame3 {
            origin: [2.0 + 2e-9, 0.0, 0.0],
            ..Frame3::IDENTITY
        },
        1.0,
        2.0,
        accuracy,
    )
    .unwrap();
    assert!(matches!(
        boolean_brep(
            &a,
            &near_tangent,
            BooleanOp::Union,
            "near-tangent-result".into()
        ),
        Err(GeometryError::UnresolvedIntersection(_))
    ));
    let diagonal = primitives::cylinder(
        "diagonal".into(),
        Frame3 {
            origin: [1.5, 1.5, 0.0],
            ..Frame3::IDENTITY
        },
        1.0,
        2.0,
        accuracy,
    )
    .unwrap();
    let diagonal_union =
        boolean_brep(&a, &diagonal, BooleanOp::Union, "diagonal-union".into()).unwrap();
    assert_eq!(diagonal_union.brep.solids.len(), 2);
    assert!(diagonal_union.report.contacts.is_empty());
}

#[test]
fn contained_parallel_cylinder_builds_an_eccentric_analytic_through_hole() {
    let accuracy = cylinder("accuracy", 0.0, 1.0, false).accuracy;
    let outer = primitives::cylinder("outer".into(), Frame3::IDENTITY, 2.0, 2.0, accuracy).unwrap();
    let inner = primitives::cylinder(
        "inner".into(),
        Frame3 {
            origin: [0.75, 0.0, 0.0],
            ..Frame3::IDENTITY
        },
        0.5,
        2.0,
        accuracy,
    )
    .unwrap();
    let union = boolean_brep(&outer, &inner, BooleanOp::Union, "union".into()).unwrap();
    let intersection = boolean_brep(
        &outer,
        &inner,
        BooleanOp::Intersection,
        "intersection".into(),
    )
    .unwrap();
    let subtraction =
        boolean_brep(&outer, &inner, BooleanOp::Subtraction, "subtraction".into()).unwrap();
    assert_eq!(union.brep.topology.faces.len(), 3);
    assert_eq!(intersection.brep.topology.faces.len(), 3);
    assert_eq!(subtraction.brep.topology.faces.len(), 4);
    assert_eq!(subtraction.brep.topology.faces[2].trim.holes.len(), 1);
    assert_eq!(subtraction.brep.topology.faces[3].trim.holes.len(), 1);
    assert_eq!(
        subtraction.brep.topology.faces[1].provenance.role,
        FaceRole::Cut
    );
    assert!(subtraction.brep.topology.faces[1].provenance.reversed);
    match subtraction.brep.geometry.surfaces[1] {
        SurfaceGeometry::Cylinder { frame, radius } => {
            assert_eq!(frame.origin, [0.75, 0.0, 0.0]);
            assert_eq!(radius, 0.5);
        }
        _ => panic!("inner cut face lost its cylinder support"),
    }
    subtraction.brep.validate().unwrap();
    tessellate(&subtraction.brep, 0.01, 2_000_000).unwrap();
    export_step(&subtraction.brep, "metre").unwrap();
    let expected = std::f64::consts::PI * (4.0 - 0.25) * 2.0;
    assert!((volume(&subtraction.brep).abs() - expected).abs() < 0.25);

    let touching = primitives::cylinder(
        "touching".into(),
        Frame3 {
            origin: [1.5, 0.0, 0.0],
            ..Frame3::IDENTITY
        },
        0.5,
        2.0,
        accuracy,
    )
    .unwrap();
    assert!(matches!(
        boolean_brep(
            &outer,
            &touching,
            BooleanOp::Subtraction,
            "touching-result".into()
        ),
        Err(GeometryError::UnresolvedIntersection(_))
    ));
}

#[test]
fn noncoextensive_parallel_cylinders_intersect_and_cut_when_the_span_is_bounded() {
    let accuracy = cylinder("accuracy", 0.0, 1.0, false).accuracy;
    let host = primitives::cylinder("host".into(), Frame3::IDENTITY, 1.0, 3.0, accuracy).unwrap();
    let short = primitives::cylinder(
        "short".into(),
        Frame3 {
            origin: [1.0, 0.0, 1.0],
            ..Frame3::IDENTITY
        },
        1.0,
        1.0,
        accuracy,
    )
    .unwrap();
    let intersection = boolean_brep(
        &host,
        &short,
        BooleanOp::Intersection,
        "intersection".into(),
    )
    .unwrap();
    assert_eq!(intersection.brep.topology.faces.len(), 4);
    assert_eq!(intersection.brep.solids.len(), 1);
    assert!(intersection.brep.topology.faces[2..]
        .iter()
        .all(|face| face.provenance.sources.len() == 1
            && face.provenance.sources[0].entity == "short"));
    intersection.brep.validate().unwrap();
    tessellate(&intersection.brep, 0.01, 2_000_000).unwrap();
    let lens_area = 2.0 * std::f64::consts::FRAC_PI_3 - 3.0_f64.sqrt() * 0.5;
    assert!((volume(&intersection.brep).abs() - lens_area).abs() < 0.12);
    let union = boolean_brep(&host, &short, BooleanOp::Union, "partial-union".into()).unwrap();
    let reversed_union = boolean_brep(
        &short,
        &host,
        BooleanOp::Union,
        "reversed-partial-union".into(),
    )
    .unwrap();
    let subtraction = boolean_brep(
        &host,
        &short,
        BooleanOp::Subtraction,
        "partial-subtraction".into(),
    )
    .unwrap();
    for (name, result) in [("union", &union), ("subtraction", &subtraction)] {
        assert_eq!(result.brep.topology.faces.len(), 10, "{name}");
        assert_eq!(result.brep.solids.len(), 1, "{name}");
        result.brep.validate().unwrap();
        tessellate(&result.brep, 0.01, 2_000_000).unwrap_or_else(|error| panic!("{name}: {error}"));
        export_step(&result.brep, "metre").unwrap_or_else(|error| panic!("{name} STEP: {error}"));
    }
    assert_eq!(
        subtraction.brep.topology.faces[3].provenance.role,
        FaceRole::Cut
    );
    assert!(subtraction.brep.topology.faces[3].provenance.reversed);
    assert!(subtraction.brep.topology.faces[7..9]
        .iter()
        .all(|face| face.provenance.role == FaceRole::Cut && face.provenance.reversed));
    let host_volume = 3.0 * std::f64::consts::PI;
    assert!(
        (volume(&union.brep).abs() - (host_volume + std::f64::consts::PI - lens_area)).abs() < 0.2
    );
    assert!((volume(&subtraction.brep).abs() - (host_volume - lens_area)).abs() < 0.2);
    assert!((volume(&reversed_union.brep).abs() - volume(&union.brep).abs()).abs() < 0.1);
    reversed_union.brep.validate().unwrap();

    let through = primitives::cylinder(
        "through".into(),
        Frame3 {
            origin: [1.0, 0.0, -1.0],
            ..Frame3::IDENTITY
        },
        1.0,
        5.0,
        accuracy,
    )
    .unwrap();
    let cut = boolean_brep(
        &host,
        &through,
        BooleanOp::Subtraction,
        "through-cut".into(),
    )
    .unwrap();
    assert_eq!(cut.brep.topology.faces.len(), 4);
    assert_eq!(cut.brep.topology.faces[1].provenance.role, FaceRole::Cut);
    assert!(cut.brep.topology.faces[1].provenance.reversed);
    assert!(!cut.report.coincident);
    cut.brep.validate().unwrap();
    tessellate(&cut.brep, 0.01, 2_000_000).unwrap();
    let expected_cut_volume = (std::f64::consts::PI - lens_area) * 3.0;
    assert!((volume(&cut.brep).abs() - expected_cut_volume).abs() < 0.2);
}
