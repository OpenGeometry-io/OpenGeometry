use opengeometry::brep::{BodyType, BrepEnvelope};
use opengeometry::query::validity;
use opengeometry::tessellation::measure;
use opengeometry::world_graph::{
    CreatingOperation, EditScope, ErrorCode, ModifyingOperation, Primitive, Transform, WorldGraph,
};
use opengeometry_test_support::world_graph::{graph, named};
use std::f64::consts::PI;

fn create(world: &mut WorldGraph, id: &str, primitive: Primitive) {
    world.create_primitive(primitive, named(id)).unwrap();
}

fn tube(world: &mut WorldGraph) {
    create(
        world,
        "tube",
        Primitive::Cylinder {
            radius: 1.0,
            height: 2.0,
        },
    );
    create(
        world,
        "bore",
        Primitive::Cylinder {
            radius: 0.4,
            height: 4.0,
        },
    );
    world
        .transform(
            "bore",
            Transform::Translate {
                offset: [0.0, -1.0, 0.0],
            },
        )
        .unwrap();
    world
        .operate(
            "tube",
            ModifyingOperation::Subtract,
            &["bore".into()],
            EditScope::Node,
        )
        .unwrap();
}

fn catalog(world: &mut WorldGraph) -> Vec<(&'static str, f64)> {
    let solids = [
        (
            "box",
            Primitive::Cuboid {
                width: 2.0,
                height: 3.0,
                depth: 4.0,
            },
            24.0,
        ),
        (
            "cylinder",
            Primitive::Cylinder {
                radius: 1.0,
                height: 2.0,
            },
            2.0 * PI,
        ),
        ("sphere", Primitive::Sphere { radius: 1.0 }, 4.0 * PI / 3.0),
        (
            "torus",
            Primitive::Torus {
                major_radius: 2.0,
                minor_radius: 0.5,
            },
            PI * PI,
        ),
        (
            "frustum",
            Primitive::Cone {
                radius_bottom: 1.0,
                radius_top: 0.4,
                height: 2.0,
            },
            2.0 * PI * (1.0 + 0.4 + 0.16) / 3.0,
        ),
    ];
    let mut expected = Vec::new();
    for (id, primitive, volume) in solids {
        create(world, id, primitive);
        expected.push((id, volume));
    }
    tube(world);
    expected.push(("tube", 2.0 * PI * (1.0 - 0.16)));
    expected
}

#[test]
fn measured_volumes_of_graph_solids_lie_within_their_bound_of_the_exact_volume() {
    let mut world = graph();
    let deflection = world.accuracy().tessellation;
    for (id, exact) in catalog(&mut world) {
        let measured = world.measure(id, None).unwrap();
        assert_eq!(
            measured,
            measure(&world.brep(id).unwrap(), deflection).unwrap()
        );
        assert_eq!(measured.deflection, deflection, "{id}");
        assert!(measured.area > 0.0, "{id}");
        assert!(
            (measured.volume - exact).abs() <= measured.error_bound,
            "{id}: {} versus {exact} with bound {}",
            measured.volume,
            measured.error_bound
        );
    }
}

#[test]
fn a_cuboid_measures_exactly_and_a_finer_deflection_tightens_a_curved_bound() {
    let mut world = graph();
    catalog(&mut world);
    let cuboid = measure(&world.brep("box").unwrap(), 0.01).unwrap();
    assert!((cuboid.volume - 24.0).abs() <= 1e-12);
    assert!((cuboid.area - 52.0).abs() <= 1e-12);
    assert!(cuboid.error_bound <= 1e-8);
    let sphere = world.brep("sphere").unwrap();
    let coarse = measure(&sphere, 0.01).unwrap();
    let fine = measure(&sphere, 0.001).unwrap();
    assert!(fine.error_bound < coarse.error_bound);
    assert!((fine.volume - 4.0 * PI / 3.0).abs() <= fine.error_bound);
}

#[test]
fn measure_refuses_a_deflection_the_tessellator_refuses() {
    let mut world = graph();
    catalog(&mut world);
    let body = world.brep("box").unwrap();
    for deflection in [0.0, -1.0, f64::NAN] {
        assert!(measure(&body, deflection).is_err(), "{deflection}");
    }
}

#[test]
fn graph_measure_refuses_a_zero_deflection_with_invalid_parameter() {
    let mut world = graph();
    catalog(&mut world);
    let error = world.measure("box", Some(0.0)).unwrap_err();
    assert_eq!(error.error_code(), ErrorCode::InvalidParameter);
}

fn swept_sheet(world: &mut WorldGraph) {
    create(
        world,
        "profile",
        Primitive::Rectangle {
            width: 1.0,
            breadth: 1.0,
        },
    );
    create(
        world,
        "path",
        Primitive::Polyline {
            points: vec![[0.0, 0.0, 0.0], [0.0, 3.0, 0.0]],
            closed: false,
        },
    );
    world
        .create_operation(
            CreatingOperation::Sweep {
                profile: "profile".into(),
                path: "path".into(),
                caps: Some(false),
            },
            named("sheet"),
        )
        .unwrap();
}

#[test]
fn validity_reports_a_closed_solid_an_open_sheet_and_closed_and_open_wires() {
    let mut world = graph();
    catalog(&mut world);
    swept_sheet(&mut world);
    let cases = [
        ("box", BodyType::Solid, true),
        ("tube", BodyType::Solid, true),
        ("sheet", BodyType::Sheet, false),
        ("profile", BodyType::Wire, true),
        ("path", BodyType::Wire, false),
    ];
    for (id, body_type, closed) in cases {
        let report = world.validity(id).unwrap();
        assert_eq!(report, validity(&world.brep(id).unwrap()), "{id}");
        assert!(report.valid, "{id}");
        assert_eq!(report.body_type, Some(body_type), "{id}");
        assert_eq!(report.closed, closed, "{id}");
        assert!(report.error.is_none(), "{id}");
    }
}

#[test]
fn validity_of_a_broken_envelope_is_invalid_with_its_first_error() {
    let mut world = graph();
    catalog(&mut world);
    let mut json = serde_json::to_value(&*world.brep("box").unwrap()).unwrap();
    json["topology"]["shells"][0]["faces"]
        .as_array_mut()
        .unwrap()
        .pop();
    let broken: BrepEnvelope = serde_json::from_value(json).unwrap();
    let report = validity(&broken);
    assert!(!report.valid);
    assert_eq!(report.body_type, None);
    assert_eq!(
        report.error.unwrap().to_string(),
        broken.validate().unwrap_err().to_string()
    );
}
