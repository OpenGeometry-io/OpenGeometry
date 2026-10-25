use opengeometry::world_graph::{
    CreateOptions, EditScope, ErrorCode, ModifyingOperation, Plane, Primitive, StepOptions,
    Transform, WorldGraph,
};
use opengeometry_test_support::part21::Document;
use opengeometry_test_support::volume;
use opengeometry_test_support::world_graph::{graph, named};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

fn sector(size: [f64; 2], angles: [f64; 2]) -> Primitive {
    Primitive::CylinderSector {
        radius: size[0],
        height: size[1],
        start_angle: angles[0],
        sweep_angle: angles[1],
    }
}

fn assert_volume(world: &WorldGraph, og_id: &str, expected: f64) {
    let body = world.brep(og_id).unwrap();
    body.validate().unwrap();
    let measured = volume::estimate(&body, 0.001);
    assert!(
        (measured.value - expected).abs() <= measured.error_bound,
        "{} versus {expected} with bound {}",
        measured.value,
        measured.error_bound
    );
}

#[test]
fn cylinder_sector_volume_is_half_radius_squared_times_sweep_times_height() {
    let mut world = graph();
    for (index, sweep) in [1.2, -1.2, 5.0].into_iter().enumerate() {
        let og_id = format!("sector-{index}");
        world
            .create_primitive(sector([2.0, 3.0], [0.3, sweep]), named(&og_id))
            .unwrap();
        assert_volume(&world, &og_id, 4.0 * sweep.abs() / 2.0 * 3.0);
    }
}

#[test]
fn cylinder_sector_exports_one_cylindrical_face_to_step() {
    let mut world = graph();
    world
        .create_primitive(sector([2.0, 3.0], [0.0, FRAC_PI_2]), named("sector"))
        .unwrap();
    let (text, _) = world
        .export_step(&["sector".into()], &StepOptions::default())
        .unwrap();
    let document = Document::parse(&text).unwrap();
    assert_eq!(document.count("CYLINDRICAL_SURFACE"), 1);
    assert_eq!(document.count("MANIFOLD_SOLID_BREP"), 1);
}

#[test]
fn cylinder_sector_minus_a_cuboid_through_operate() {
    let mut world = graph();
    world
        .create_primitive(sector([2.0, 3.0], [0.0, FRAC_PI_2]), named("sector"))
        .unwrap();
    world
        .create_primitive(
            Primitive::Cuboid {
                width: 0.6,
                height: 1.0,
                depth: 0.6,
            },
            named("tool"),
        )
        .unwrap();
    world
        .transform(
            "tool",
            Transform::Translate {
                offset: [0.9, 2.5, -0.9],
            },
        )
        .unwrap();
    world
        .operate(
            "sector",
            ModifyingOperation::Subtract,
            &["tool".into()],
            EditScope::Node,
        )
        .unwrap();
    assert_volume(&world, "sector", 3.0 * PI - 0.18);
}

#[test]
fn invalid_cylinder_sectors_are_invalid_parameter() {
    for primitive in [
        sector([2.0, 3.0], [0.0, 0.0]),
        sector([2.0, 3.0], [0.0, f64::NAN]),
        sector([2.0, 3.0], [f64::INFINITY, 1.0]),
        sector([2.0, 3.0], [0.0, TAU]),
        sector([2.0, 3.0], [0.0, -7.0]),
        sector([1e-9, 3.0], [0.0, 1.0]),
        sector([2.0, 1e-9], [0.0, 1.0]),
    ] {
        let mut world = graph();
        let label = format!("{primitive:?}");
        let error = world
            .create_primitive(primitive, named("sector"))
            .expect_err(&label);
        assert_eq!(error.error_code(), ErrorCode::InvalidParameter, "{label}");
        assert_eq!(world.node_count(), 0);
    }
}

#[test]
fn cylinder_sector_json_uses_camel_case_angles() {
    let json = r#"{"kind":"CylinderSector","radius":1,"height":2,"startAngle":0,"sweepAngle":1}"#;
    assert!(serde_json::from_str::<Primitive>(json).is_ok());
    let json = r#"{"kind":"CylinderSector","radius":1,"height":2,"start_angle":0,"sweep_angle":1}"#;
    assert!(serde_json::from_str::<Primitive>(json).is_err());
}

#[test]
fn refused_sector_sweep_names_the_sector() {
    let mut world = graph();
    let error = world
        .create_primitive(sector([2.0, 3.0], [0.0, TAU]), named("sector"))
        .unwrap_err();
    assert_eq!(
        error.message(),
        "sector sweep must be nonzero and less than one turn"
    );
}

#[test]
fn cylinder_sector_on_a_plane_is_placed_and_keeps_its_volume() {
    let mut world = graph();
    let options = CreateOptions {
        plane: Some(Plane {
            origin: Some([1.0, 2.0, 3.0]),
            normal: Some([1.0, 0.0, 0.0]),
            x_direction: Some([0.0, 0.0, 1.0]),
        }),
        ..named("sector")
    };
    world
        .create_primitive(sector([2.0, 3.0], [0.0, FRAC_PI_2]), options)
        .unwrap();
    assert_volume(&world, "sector", 3.0 * PI);
    let placement = world.world_placement("sector").unwrap();
    let points = world
        .brep("sector")
        .unwrap()
        .topology
        .vertices
        .iter()
        .map(|vertex| placement.apply_point(vertex.position))
        .collect::<Vec<_>>();
    let near = |a: [f64; 3], b: [f64; 3]| (0..3).all(|axis| (a[axis] - b[axis]).abs() <= 1e-9);
    for expected in [
        [1.0, 2.0, 3.0],
        [1.0, 2.0, 5.0],
        [1.0, 0.0, 3.0],
        [4.0, 0.0, 3.0],
    ] {
        assert!(
            points.iter().any(|point| near(*point, expected)),
            "{expected:?} in {points:?}"
        );
    }
}
