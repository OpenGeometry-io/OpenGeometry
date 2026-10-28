use super::support::assert_volume;
use opengeometry::brep::BodyType;
use opengeometry::primitives::{ProfileSegment, SegmentArc};
use opengeometry::world_graph::{
    CreateOptions, EditScope, ErrorCode, ErrorDetails, ModifyingOperation, Plane, Primitive,
    StepOptions, WorldGraph,
};
use opengeometry_test_support::part21::Document;
use opengeometry_test_support::scenes::revolve::{
    polygon, revolve, revolve_scenes, upright, wc_bowl, RevolveScene,
};
use opengeometry_test_support::world_graph::{graph, named};
use std::f64::consts::PI;

fn half_disc(radius: f64, counter_clockwise: bool) -> Primitive {
    Primitive::Profile {
        start: [0.0, -radius],
        segments: vec![
            ProfileSegment {
                to: [0.0, radius],
                arc: Some(SegmentArc {
                    center: [0.0, 0.0],
                    counter_clockwise,
                }),
            },
            ProfileSegment {
                to: [0.0, -radius],
                arc: None,
            },
        ],
        closed: true,
    }
}

fn hemisphere(radius: f64) -> Primitive {
    Primitive::Profile {
        start: [0.0, -radius],
        segments: vec![
            ProfileSegment {
                to: [radius, 0.0],
                arc: Some(SegmentArc {
                    center: [0.0, 0.0],
                    counter_clockwise: true,
                }),
            },
            ProfileSegment {
                to: [0.0, 0.0],
                arc: None,
            },
            ProfileSegment {
                to: [0.0, -radius],
                arc: None,
            },
        ],
        closed: true,
    }
}

fn revolved(profile: Primitive, axis: Option<([f64; 2], [f64; 2])>) -> WorldGraph {
    let mut world = graph();
    world.create_primitive(profile, upright("profile")).unwrap();
    world
        .create_operation(revolve("profile", axis), named("solid"))
        .unwrap();
    world
}

fn scene(name: &str) -> RevolveScene {
    revolve_scenes()
        .into_iter()
        .find(|scene| scene.name == name)
        .unwrap()
}

fn refusal(name: &str) -> (ErrorCode, String) {
    let RevolveScene {
        profile,
        options,
        operation,
        ..
    } = scene(name);
    let mut world = graph();
    world.create_primitive(profile, options).unwrap();
    let error = world
        .create_operation(operation, named("solid"))
        .expect_err(name);
    assert_eq!(world.node_count(), 1, "{name}");
    (error.error_code(), error.message())
}

#[test]
fn a_rectangle_off_the_axis_revolves_into_a_tube() {
    let world = revolved(
        polygon(&[[0.0, 0.0], [1.0, 0.0], [1.0, 2.0], [0.0, 2.0]]),
        Some(([-1.0, 0.0], [0.0, 1.0])),
    );
    assert_volume(&world, "solid", PI * 2.0 * (4.0 - 1.0));
}

#[test]
fn the_default_axis_is_the_profile_planes_v_axis() {
    let mut world = graph();
    let tube = polygon(&[[1.0, 0.0], [2.0, 0.0], [2.0, 3.0], [1.0, 3.0]]);
    world.create_primitive(tube, named("profile")).unwrap();
    world
        .create_operation(revolve("profile", None), named("solid"))
        .unwrap();
    assert_volume(&world, "solid", PI * 3.0 * (4.0 - 1.0));
    let bounds = world.brep("solid").unwrap().bounds().unwrap().unwrap();
    assert!(bounds.contains([2.0, 0.0, -1.5]));
    assert!(bounds.contains([0.0, 2.0, -1.5]));
    assert!(!bounds.contains([0.0, 0.0, 1.0]));
    assert!(!bounds.contains([0.0, 0.0, -3.5]));
}

#[test]
fn a_bowl_of_two_lines_and_one_arc_revolves_and_exports_to_step() {
    let world = revolved(hemisphere(0.2), None);
    assert_volume(&world, "solid", 2.0 / 3.0 * PI * 0.008);
    let (text, _) = world
        .export_step(&["solid".into()], &StepOptions::default())
        .unwrap();
    let document = Document::parse(&text).unwrap();
    assert_eq!(document.count("SPHERICAL_SURFACE"), 1);
    assert_eq!(document.count("PLANE"), 1);
    assert_eq!(document.count("MANIFOLD_SOLID_BREP"), 1);
}

#[test]
fn the_wc_bowl_revolves_into_one_face_per_edge_off_the_axis() {
    let world = revolved(wc_bowl(), None);
    let body = world.brep("solid").unwrap();
    body.validate().unwrap();
    assert_eq!(body.topology.faces.len(), 5);
    let (text, _) = world
        .export_step(&["solid".into()], &StepOptions::default())
        .unwrap();
    let document = Document::parse(&text).unwrap();
    for (surface, count) in [
        ("SPHERICAL_SURFACE", 1),
        ("TOROIDAL_SURFACE", 1),
        ("CONICAL_SURFACE", 1),
        ("PLANE", 2),
    ] {
        assert_eq!(document.count(surface), count, "{surface}");
    }
}

#[test]
fn a_revolve_minus_a_cuboid_is_a_coverage_gap_naming_the_handlers_tried() {
    let mut world = revolved(hemisphere(0.2), None);
    let tool = Primitive::Cuboid {
        width: 0.1,
        height: 0.1,
        depth: 0.1,
    };
    let sunk = CreateOptions {
        plane: Some(Plane {
            origin: Some([0.0, -0.05, 0.0]),
            ..Plane::default()
        }),
        ..named("tool")
    };
    world.create_primitive(tool, sunk).unwrap();
    let error = world
        .operate(
            "solid",
            ModifyingOperation::Subtract,
            &["tool".into()],
            EditScope::Node,
        )
        .unwrap_err();
    assert_eq!(error.error_code(), ErrorCode::CoverageGap, "{error:?}");
    let ErrorDetails::Operate {
        handlers,
        attempted,
        families,
        ..
    } = error.details()
    else {
        panic!("{error:?}");
    };
    assert!(handlers.is_empty(), "{handlers:?}");
    assert_eq!(
        families.map(|pair| pair.to_vec()),
        Some(vec![
            "full sphere input".to_string(),
            "full sphere input".to_string()
        ])
    );
    assert_eq!(
        attempted,
        [
            "separate_boolean",
            "sphere_box_boolean",
            "subtract_vertical_arc_extrusion",
            "generic_two_sided_cutter_band_subtraction",
            "generic_multiple_closed_loops_boolean",
            "generic_single_closed_loop_boolean",
            "generic_non_intersecting_boolean",
            "prismatic_curved_boolean",
        ]
    );
}

#[test]
fn crossing_or_touching_the_axis_and_a_zero_direction_are_invalid_parameters() {
    for (name, message) in [
        ("crossing-axis", "revolved profile crosses the revolve axis"),
        (
            "touching-axis",
            "revolved profile touches the revolve axis at a point",
        ),
        (
            "zero-direction",
            "revolve axis needs a finite point and a nonzero direction",
        ),
    ] {
        assert_eq!(
            refusal(name),
            (ErrorCode::InvalidParameter, message.to_string()),
            "{name}"
        );
    }
}

#[test]
fn an_open_wire_is_an_invalid_parameter() {
    let mut world = graph();
    let open = Primitive::Profile {
        start: [1.0, 0.0],
        segments: vec![
            ProfileSegment {
                to: [2.0, 0.0],
                arc: None,
            },
            ProfileSegment {
                to: [2.0, 1.0],
                arc: None,
            },
        ],
        closed: false,
    };
    world.create_primitive(open, upright("profile")).unwrap();
    let error = world
        .create_operation(revolve("profile", None), named("solid"))
        .unwrap_err();
    assert_eq!(error.error_code(), ErrorCode::InvalidParameter, "{error:?}");
}

#[test]
fn rebuild_with_a_changed_profile_keeps_a_solid() {
    let mut world = revolved(
        polygon(&[[1.0, 0.0], [2.0, 0.0], [2.0, 1.0], [1.0, 1.0]]),
        None,
    );
    let wider = polygon(&[[1.0, 0.0], [3.0, 0.0], [3.0, 1.0], [1.0, 1.0]]);
    world
        .rebuild_primitive("profile", wider, EditScope::Node)
        .unwrap();
    world
        .rebuild_operation("solid", revolve("profile", None), EditScope::Node)
        .unwrap();
    assert_volume(&world, "solid", PI * (9.0 - 1.0));
    assert_eq!(
        world.brep("solid").unwrap().body_type().unwrap(),
        BodyType::Solid
    );
}

#[test]
fn a_half_disc_on_either_side_of_the_axis_revolves_into_a_ball() {
    for counter_clockwise in [true, false] {
        let world = revolved(half_disc(0.5, counter_clockwise), None);
        assert_volume(&world, "solid", 4.0 / 3.0 * PI * 0.125);
    }
}

#[test]
fn a_subnormal_axis_direction_is_an_invalid_parameter() {
    let mut world = graph();
    let profile = polygon(&[[0.0, 0.0], [1.0, 0.0], [1.0, 2.0], [0.0, 2.0]]);
    world.create_primitive(profile, upright("profile")).unwrap();
    let error = world
        .create_operation(
            revolve("profile", Some(([-1.0, 0.0], [1e-320, 0.0]))),
            named("solid"),
        )
        .unwrap_err();
    assert_eq!(error.error_code(), ErrorCode::InvalidParameter, "{error:?}");
}

#[test]
fn the_solid_follows_a_profile_moved_by_its_plane_origin_through_rebuild() {
    let mut world = graph();
    let moved = CreateOptions {
        plane: Some(Plane {
            origin: Some([5.0, 0.0, 0.0]),
            normal: Some([0.0, 0.0, 1.0]),
            x_direction: Some([1.0, 0.0, 0.0]),
        }),
        ..named("profile")
    };
    let ring = polygon(&[[1.0, 0.0], [2.0, 0.0], [2.0, 1.0], [1.0, 1.0]]);
    world.create_primitive(ring, moved).unwrap();
    world
        .create_operation(revolve("profile", None), named("solid"))
        .unwrap();
    let centred = |world: &WorldGraph, reach: f64| {
        let bounds = world.bounds("solid").unwrap().unwrap();
        assert!((bounds[0] - (5.0 - reach)).abs() < 0.05, "{bounds:?}");
        assert!((bounds[3] - (5.0 + reach)).abs() < 0.05, "{bounds:?}");
    };
    centred(&world, 2.0);
    let wider = polygon(&[[1.0, 0.0], [3.0, 0.0], [3.0, 1.0], [1.0, 1.0]]);
    world
        .rebuild_primitive("profile", wider, EditScope::Node)
        .unwrap();
    world
        .rebuild_operation("solid", revolve("profile", None), EditScope::Node)
        .unwrap();
    centred(&world, 3.0);
}
