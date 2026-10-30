use opengeometry::primitives::{ProfileSegment, SegmentArc};
use opengeometry::world_graph::{
    CreatingOperation, EditScope, ErrorCode, ErrorDetails, ModifyingOperation, Primitive,
    Transform, WorldGraph,
};
use opengeometry_test_support::scenes::revolve::{polygon, revolve, upright, wc_bowl};
use opengeometry_test_support::world_graph::{graph, named};

const REVOLVED: [&str; 2] = ["restricted analytic shell", "revolved profile"];
const EXTRUDED: [&str; 2] = ["restricted analytic shell", "arc-edged profile extrusion"];

fn d_pan() -> Primitive {
    let line = |to| ProfileSegment { to, arc: None };
    Primitive::Profile {
        start: [0.0, 0.0],
        segments: vec![
            line([0.325, 0.0]),
            ProfileSegment {
                to: [0.325, 0.35],
                arc: Some(SegmentArc {
                    center: [0.325, 0.175],
                    counter_clockwise: true,
                }),
            },
            line([0.0, 0.35]),
            line([0.0, 0.0]),
        ],
        closed: true,
    }
}

fn revolved(world: &mut WorldGraph, id: &str, profile: Primitive) {
    let outline = format!("{id}-profile");
    world.create_primitive(profile, upright(&outline)).unwrap();
    world
        .create_operation(revolve(&outline, None), named(id))
        .unwrap();
}

fn disc() -> Primitive {
    let arc = |to| ProfileSegment {
        to,
        arc: Some(SegmentArc {
            center: [0.0, 0.0],
            counter_clockwise: true,
        }),
    };
    Primitive::Profile {
        start: [0.2, 0.0],
        segments: vec![arc([-0.2, 0.0]), arc([0.2, 0.0])],
        closed: true,
    }
}

fn extruded(world: &mut WorldGraph, id: &str, profile: Primitive) {
    let outline = format!("{id}-profile");
    world.create_primitive(profile, named(&outline)).unwrap();
    let extrude = CreatingOperation::Extrude {
        profile: outline,
        holes: Vec::new(),
        distance: 0.1,
    };
    world.create_operation(extrude, named(id)).unwrap();
}

fn refused_families(world: &mut WorldGraph, id: &str) -> Vec<String> {
    let error = world.shell(id, 0.005, EditScope::Node).unwrap_err();
    assert_eq!(error.error_code(), ErrorCode::CoverageGap, "{id}");
    let ErrorDetails::CoverageGap { families } = error.details() else {
        panic!("{id}: {:?}", error.details());
    };
    families.to_vec()
}

#[test]
fn a_full_turn_revolve_shell_names_the_revolved_profile_family() {
    let mut world = graph();
    revolved(&mut world, "bowl", wc_bowl());
    revolved(
        &mut world,
        "frustum",
        polygon(&[[0.0, 0.0], [1.0, 0.0], [0.5, 1.0], [0.0, 1.0]]),
    );
    revolved(
        &mut world,
        "stepped",
        polygon(&[
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 0.5],
            [0.5, 0.5],
            [0.5, 1.0],
            [0.0, 1.0],
        ]),
    );
    let tube = polygon(&[[1.0, 0.0], [2.0, 0.0], [2.0, 3.0], [1.0, 3.0]]);
    revolved(&mut world, "tube", tube);
    for id in ["bowl", "frustum", "stepped", "tube"] {
        assert_eq!(refused_families(&mut world, id), REVOLVED, "{id}");
    }
}

#[test]
fn an_arc_edged_extrusion_shell_keeps_its_family_and_coaxial_extrusions_still_shell() {
    let mut world = graph();
    extruded(&mut world, "notched", d_pan());
    world
        .create_primitive(
            Primitive::Cuboid {
                width: 0.1,
                height: 0.1,
                depth: 0.1,
            },
            named("notch"),
        )
        .unwrap();
    let up = Transform::Translate {
        offset: [0.0, 0.05, 0.0],
    };
    world.transform("notch", up).unwrap();
    let tools = ["notch".to_string()];
    let subtract = ModifyingOperation::Subtract;
    world
        .operate("notched", subtract, &tools, EditScope::Node)
        .unwrap();
    assert_eq!(refused_families(&mut world, "notched"), EXTRUDED);
    extruded(&mut world, "pan", d_pan());
    world.shell("pan", 0.005, EditScope::Node).unwrap();
    extruded(&mut world, "disc", disc());
    world.shell("disc", 0.005, EditScope::Node).unwrap();
}
