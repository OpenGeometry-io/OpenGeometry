use opengeometry::primitives::{ProfileSegment, SegmentArc};
use opengeometry::query::{classify_point, PointClassification};
use opengeometry::world_graph::{
    CreateOptions, CreatingOperation, EditScope, ModifyingOperation, Plane, Primitive, StepOptions,
    Transform, WorldGraph,
};
use opengeometry_test_support::part21::Document;
use opengeometry_test_support::volume;
use opengeometry_test_support::world_graph::{graph, named};

fn arched_profile() -> Primitive {
    let edge = |to: [f64; 2]| ProfileSegment { to, arc: None };
    Primitive::Profile {
        start: [-0.45, 0.2],
        segments: vec![
            edge([0.45, 0.2]),
            edge([0.45, 2.2]),
            ProfileSegment {
                to: [-0.45, 2.2],
                arc: Some(SegmentArc {
                    center: [0.0, 2.2],
                    counter_clockwise: true,
                }),
            },
            edge([-0.45, 0.2]),
        ],
        closed: true,
    }
}

fn add_arch(world: &mut WorldGraph, angle: f64) {
    let (sin, cos) = angle.sin_cos();
    let plane = Plane {
        origin: Some([2.55 * cos, 0.0, 2.55 * sin]),
        normal: Some([cos, 0.0, sin]),
        x_direction: Some([sin, 0.0, -cos]),
    };
    let options = CreateOptions {
        plane: Some(plane),
        ..named("arch-profile")
    };
    world.create_primitive(arched_profile(), options).unwrap();
    world
        .create_operation(
            CreatingOperation::Extrude {
                profile: "arch-profile".into(),
                holes: Vec::new(),
                distance: 0.6,
            },
            named("arch"),
        )
        .unwrap();
}

fn add_ring(world: &mut WorldGraph) {
    world
        .create_primitive(Primitive::Circle { radius: 3.0 }, named("outer"))
        .unwrap();
    world
        .create_primitive(Primitive::Circle { radius: 2.7 }, named("inner"))
        .unwrap();
    world
        .create_operation(
            CreatingOperation::Extrude {
                profile: "outer".into(),
                holes: vec!["inner".into()],
                distance: 3.0,
            },
            named("wall"),
        )
        .unwrap();
}

fn cut_arch(world: &mut WorldGraph) {
    world
        .operate(
            "wall",
            ModifyingOperation::Subtract,
            &["arch".into()],
            EditScope::Node,
        )
        .unwrap();
    assert_eq!(
        world.report("wall").unwrap().unwrap()["handlers"],
        serde_json::json!(["generic_two_sided_cutter_band_subtraction"])
    );
    let (text, _) = world
        .export_step(&["wall".into()], &StepOptions::default())
        .unwrap();
    assert_eq!(
        Document::parse(&text).unwrap().count("MANIFOLD_SOLID_BREP"),
        1
    );
}

#[test]
fn curved_wall_minus_one_arched_profile_extrusion_is_a_valid_body() {
    let mut world = graph();
    add_ring(&mut world);
    add_arch(&mut world, 0.7);
    cut_arch(&mut world);
    let body = world.brep("wall").unwrap();
    body.validate().unwrap();
    let (sin, cos) = 0.7_f64.sin_cos();
    let opening = [2.85 * cos, 1.2, 2.85 * sin];
    assert_eq!(
        classify_point(&body, opening).unwrap(),
        PointClassification::Outside
    );
}

#[test]
fn straight_wall_minus_one_arched_profile_extrusion_is_a_valid_body() {
    let mut world = graph();
    world
        .create_primitive(
            Primitive::Cuboid {
                width: 0.3,
                height: 3.0,
                depth: 4.0,
            },
            named("wall"),
        )
        .unwrap();
    world
        .transform(
            "wall",
            Transform::Translate {
                offset: [2.85, 0.0, 0.0],
            },
        )
        .unwrap();
    add_arch(&mut world, 0.0);
    cut_arch(&mut world);
}

#[test]
fn curved_wall_minus_an_arched_profile_extrusion_over_the_seam_keeps_the_off_seam_volume() {
    let volumes = [0.0, 0.7].map(|angle| {
        let mut world = graph();
        add_ring(&mut world);
        add_arch(&mut world, angle);
        cut_arch(&mut world);
        let body = world.brep("wall").unwrap();
        body.validate().unwrap();
        volume::estimate(&body, 0.001)
    });
    let tolerance = 1e-3 * volumes[1].value;
    assert!(
        (volumes[0].value - volumes[1].value).abs() <= tolerance,
        "{} versus {} within {tolerance}",
        volumes[0].value,
        volumes[1].value
    );
}
