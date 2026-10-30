use super::support::extrude;
use opengeometry::brep::SurfaceGeometry;
use opengeometry::primitives::{self, ProfileSegment, SegmentArc};
use opengeometry::world_graph::{
    CreateOptions, CreatingOperation, EditScope, ErrorCode, Plane, Primitive, StepOptions,
    WorldGraph,
};
use opengeometry_test_support::accuracy::accuracy;
use opengeometry_test_support::part21::Document;
use opengeometry_test_support::volume;
use opengeometry_test_support::world_graph::{graph, named};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

fn line(to: [f64; 2]) -> ProfileSegment {
    ProfileSegment { to, arc: None }
}

fn arc(to: [f64; 2], center: [f64; 2], counter_clockwise: bool) -> ProfileSegment {
    ProfileSegment {
        to,
        arc: Some(SegmentArc {
            center,
            counter_clockwise,
        }),
    }
}

fn closed(start: [f64; 2], segments: Vec<ProfileSegment>) -> Primitive {
    Primitive::Profile {
        start,
        segments,
        closed: true,
    }
}

fn ring_sector(inner: f64, outer: f64) -> Primitive {
    closed(
        [inner, 0.0],
        vec![
            line([outer, 0.0]),
            arc([0.0, outer], [0.0, 0.0], true),
            line([0.0, inner]),
            arc([inner, 0.0], [0.0, 0.0], false),
        ],
    )
}

fn extruded(world: &mut WorldGraph, holes: &[&str], distance: f64) -> f64 {
    world
        .create_operation(
            CreatingOperation::Extrude {
                profile: "profile".into(),
                holes: holes.iter().map(|hole| (*hole).into()).collect(),
                distance,
            },
            named("solid"),
        )
        .unwrap();
    let body = world.brep("solid").unwrap();
    body.validate().unwrap();
    volume::estimate(&body, 0.001).value
}

fn assert_volume(measured: f64, expected: f64) {
    assert!(
        (measured - expected).abs() <= 1e-3 * expected,
        "{measured} versus {expected}"
    );
}

fn assert_refused(primitive: Primitive) {
    let mut world = graph();
    let label = format!("{primitive:?}");
    let error = world
        .create_primitive(primitive, named("profile"))
        .expect_err(&label);
    assert_eq!(error.error_code(), ErrorCode::InvalidParameter, "{error:?}");
    assert_eq!(world.node_count(), 0);
}

#[test]
fn ring_sector_profile_extrudes_and_exports_to_step() {
    let mut world = graph();
    world
        .create_primitive(ring_sector(1.0, 2.0), named("profile"))
        .unwrap();
    assert_volume(extruded(&mut world, &[], 3.0), 9.0 * PI / 4.0);
    let body = world.brep("solid").unwrap();
    let cylinders = body
        .topology
        .faces
        .iter()
        .filter(|face| {
            matches!(
                body.geometry.surfaces[face.surface as usize],
                SurfaceGeometry::Cylinder { .. }
            )
        })
        .count();
    assert_eq!(cylinders, 2);
    assert_eq!(body.topology.faces.len(), 6);
    let (text, _) = world
        .export_step(&["solid".into()], &StepOptions::default())
        .unwrap();
    let document = Document::parse(&text).unwrap();
    assert_eq!(document.count("CYLINDRICAL_SURFACE"), 2);
}

#[test]
fn rectangle_with_one_arc_side_extrudes() {
    let mut world = graph();
    let profile = closed(
        [0.0, 0.0],
        vec![
            line([4.0, 0.0]),
            arc([4.0, 2.0], [4.0, 1.0], true),
            line([0.0, 2.0]),
            line([0.0, 0.0]),
        ],
    );
    world.create_primitive(profile, named("profile")).unwrap();
    assert_volume(extruded(&mut world, &[], 2.0), (8.0 + FRAC_PI_2) * 2.0);
}

#[test]
fn curved_wall_with_rectangular_hole_extrudes() {
    let mut world = graph();
    world
        .create_primitive(ring_sector(4.0, 6.0), named("profile"))
        .unwrap();
    let centre = 5.0 * std::f64::consts::FRAC_1_SQRT_2;
    let corner = |du: f64, dv: f64| [centre + du, 0.0, -(centre + dv)];
    let hole = Primitive::Polyline {
        points: vec![
            corner(-0.3, -0.3),
            corner(0.3, -0.3),
            corner(0.3, 0.3),
            corner(-0.3, 0.3),
        ],
        closed: true,
    };
    world.create_primitive(hole, named("hole")).unwrap();
    assert_volume(
        extruded(&mut world, &["hole"], 2.0),
        (5.0 * PI - 0.36) * 2.0,
    );
}

#[test]
fn tilted_profile_extrudes_along_its_plane_normal() {
    let mut world = graph();
    let options = CreateOptions {
        plane: Some(Plane {
            origin: Some([1.0, 2.0, 3.0]),
            normal: Some([1.0, 0.0, 0.0]),
            x_direction: Some([0.0, 0.0, 1.0]),
        }),
        ..named("profile")
    };
    world
        .create_primitive(ring_sector(1.0, 2.0), options)
        .unwrap();
    assert_volume(extruded(&mut world, &[], 3.0), 9.0 * PI / 4.0);
    let placement = world.world_placement("solid").unwrap();
    let xs = world
        .brep("solid")
        .unwrap()
        .topology
        .vertices
        .iter()
        .map(|vertex| placement.apply_point(vertex.position)[0])
        .collect::<Vec<_>>();
    assert!(xs.iter().all(|x| (1.0 - 1e-9..=4.0 + 1e-9).contains(x)));
    assert!(xs.iter().any(|x| (x - 4.0).abs() <= 1e-9));
}

#[test]
fn profile_without_arcs_matches_polyline_wire() {
    let tolerance = accuracy(1e-6);
    let (profile, profile_keys) = primitives::profile_wire_with_keys(
        "wire".into(),
        [0.0, 0.0],
        &[line([2.0, 0.0]), line([2.0, 1.0]), line([0.0, 0.0])],
        true,
        tolerance,
    )
    .unwrap();
    let (polyline, polyline_keys) = primitives::polyline_with_keys(
        "wire".into(),
        &[[0.0; 3], [2.0, 0.0, 0.0], [2.0, 0.0, -1.0]],
        true,
        tolerance,
    )
    .unwrap();
    assert_eq!(profile.to_json().unwrap(), polyline.to_json().unwrap());
    assert_eq!(profile_keys, polyline_keys);
}

#[test]
fn profile_edge_keys_follow_segment_order() {
    let (brep, keys) = primitives::profile_wire_with_keys(
        "wire".into(),
        [0.0, 0.0],
        &[
            line([1.0, 0.0]),
            arc([2.0, 0.0], [1.5, 0.0], true),
            line([3.0, 0.0]),
        ],
        false,
        accuracy(1e-6),
    )
    .unwrap();
    assert_eq!(keys, ["seg-0", "seg-1", "seg-2"]);
    assert_eq!(brep.topology.edges.len(), 3);
    assert!(!brep.to_json().unwrap().contains("seg-"));
}

#[test]
fn arc_wire_has_one_signed_edge() {
    let mut world = graph();
    let partial = Primitive::Arc {
        radius: 2.0,
        start_angle: FRAC_PI_2,
        sweep_angle: -FRAC_PI_2,
    };
    world.create_primitive(partial, named("arc")).unwrap();
    let brep = world.brep("arc").unwrap();
    assert_eq!(brep.topology.edges.len(), 1);
    let halfedge = &brep.topology.halfedges[0];
    let from = brep.topology.vertices[halfedge.from as usize].position;
    let to = brep.topology.vertices[halfedge.to as usize].position;
    assert!((from[2] + 2.0).abs() < 1e-12 && from[0].abs() < 1e-12);
    assert!((to[0] - 2.0).abs() < 1e-12 && to[2].abs() < 1e-12);
    assert_eq!(world.shape("arc").unwrap().edge_keys, ["edge-0"]);
    assert!(brep.to_json().unwrap().contains("\"sense\":\"Reverse\""));
}

#[test]
fn arc_wire_refuses_empty_and_full_sweeps() {
    for (radius, sweep_angle) in [(1.0, 0.0), (1.0, TAU), (1.0, -TAU), (1e-9, 1.0)] {
        assert_refused(Primitive::Arc {
            radius,
            start_angle: 0.0,
            sweep_angle,
        });
    }
}

#[test]
fn invalid_profiles_are_invalid_parameter() {
    let square = |last: [f64; 2]| {
        closed(
            [0.0, 0.0],
            vec![
                line([1.0, 0.0]),
                line([1.0, 1.0]),
                line([0.0, 1.0]),
                line(last),
            ],
        )
    };
    assert_refused(square([0.5, 0.0]));
    assert_refused(Primitive::Profile {
        start: [2.0, 0.0],
        segments: vec![line([1.0, 0.0]), arc([1.0, -2e-8], [0.0, 0.0], true)],
        closed: false,
    });
    assert_refused(closed([0.0, 0.0], vec![line([1.0, 0.0]), line([1.0, 0.0])]));
    assert_refused(closed([0.0, 0.0], vec![line([0.0, 0.0])]));
    assert_refused(closed(
        [0.0, 0.0],
        vec![
            line([2.0, 0.0]),
            arc([0.0, 2.5], [0.0, 0.0], true),
            line([0.0, 0.0]),
        ],
    ));
    assert_refused(closed(
        [0.0, 0.0],
        vec![
            line([2.0, 2.0]),
            line([2.0, 0.0]),
            line([0.0, 2.0]),
            line([0.0, 0.0]),
        ],
    ));
    assert_refused(closed(
        [0.0, 0.0],
        vec![
            line([2.0, 0.0]),
            line([2.0, 2.0]),
            line([0.0, 2.0]),
            arc([0.0, 0.0], [0.9, 1.0], false),
        ],
    ));
}

#[test]
fn profile_with_unknown_segment_field_is_refused() {
    let json = r#"{"kind":"Profile","start":[0,0],"closed":true,
        "segments":[{"to":[1,0],"bulge":1},{"to":[0,1]},{"to":[0,0]}]}"#;
    assert!(serde_json::from_str::<Primitive>(json).is_err());
    let json = r#"{"kind":"Arc","radius":1,"startAngle":0,"sweepAngle":1}"#;
    assert!(serde_json::from_str::<Primitive>(json).is_ok());
}

#[test]
fn open_profile_is_not_an_extrude_profile() {
    let mut world = graph();
    let open = Primitive::Profile {
        start: [0.0, 0.0],
        segments: vec![line([1.0, 0.0]), arc([2.0, 0.0], [1.5, 0.0], true)],
        closed: false,
    };
    world.create_primitive(open, named("profile")).unwrap();
    let error = world
        .create_operation(extrude("profile", 1.0), named("solid"))
        .unwrap_err();
    assert_eq!(error.error_code(), ErrorCode::InvalidParameter);
    assert_eq!(error.message(), "profile must be one closed wire");
}

#[test]
fn multi_segment_sweep_of_a_curved_profile_is_unsupported() {
    let mut world = graph();
    world
        .create_primitive(ring_sector(0.1, 0.2), named("profile"))
        .unwrap();
    let path = Primitive::Polyline {
        points: vec![[0.0; 3], [0.0, 3.0, 0.0], [3.0, 3.0, 0.0]],
        closed: false,
    };
    world.create_primitive(path, named("path")).unwrap();
    let error = world
        .create_operation(
            CreatingOperation::Sweep {
                profile: "profile".into(),
                path: "path".into(),
                caps: None,
            },
            named("solid"),
        )
        .unwrap_err();
    assert_eq!(error.error_code(), ErrorCode::UnsupportedGeometry);
}

fn refusal(primitive: Primitive) -> String {
    let mut world = graph();
    let error = world
        .create_primitive(primitive, named("profile"))
        .unwrap_err();
    assert_eq!(error.error_code(), ErrorCode::InvalidParameter);
    assert_eq!(world.node_count(), 0);
    error.message().to_string()
}

#[test]
fn profile_refusals_name_their_cause() {
    let open = |start: [f64; 2], segments: Vec<ProfileSegment>| Primitive::Profile {
        start,
        segments,
        closed: false,
    };
    let tiny_arc = vec![line([1.0, 0.0]), arc([1.0, 6e-8], [1.0, 3e-8], true)];
    assert_eq!(
        refusal(open([0.0, 0.0], tiny_arc)),
        "profile arc does not end on its circle"
    );
    let loop_back = vec![line([1.0, 0.0]), line([1.0, 1.0]), line([0.0, 0.0])];
    assert_eq!(
        refusal(open([0.0, 0.0], loop_back)),
        "open profile must not end at its start"
    );
    let infinite = vec![line([f64::INFINITY, 0.0]), line([0.0, 1.0])];
    assert_eq!(
        refusal(open([0.0, 0.0], infinite)),
        "profile points must be finite"
    );
    let sliver = vec![line([1.0, 0.0]), line([0.5, 1e-15]), line([0.0, 0.0])];
    assert_eq!(
        refusal(closed([0.0, 0.0], sliver)),
        "profile self-intersects"
    );
}

#[test]
fn profile_rebuild_keeps_placement() {
    let mut world = graph();
    let options = CreateOptions {
        plane: Some(Plane {
            origin: Some([5.0, 1.0, 0.0]),
            ..Plane::default()
        }),
        ..named("profile")
    };
    world
        .create_primitive(ring_sector(1.0, 2.0), options)
        .unwrap();
    let before = world.world_matrix("profile").unwrap();
    world
        .rebuild_primitive("profile", ring_sector(1.0, 3.0), EditScope::Node)
        .unwrap();
    assert_eq!(world.world_matrix("profile").unwrap(), before);
    assert_eq!(world.shape("profile").unwrap().edge_keys.len(), 4);
    assert_volume(extruded(&mut world, &[], 1.0), 2.0 * PI);
}
