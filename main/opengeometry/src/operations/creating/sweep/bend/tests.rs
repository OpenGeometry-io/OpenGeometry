use super::{
    add_bend_arcs, add_polygon_bend, add_torus_bend, bend_frame, Bend, BendFrame, CircleBend,
    CircleEnds, PolygonBend,
};
use crate::brep::{
    coarse_accuracy, plane_boundary, unit, Accuracy, BrepEnvelope, Builder, CurveGeometry, Frame3,
    Orientation, PcurveGeometry, SurfaceGeometry,
};
use crate::math::{add, cross, dot, norm, sub, Interval, Point3};
use crate::operations::OperationError;
use crate::tessellation::tessellate;
use std::f64::consts::{FRAC_PI_2, PI, TAU};

fn accuracy() -> Accuracy {
    coarse_accuracy(1e-6)
}

const UP: Point3 = [0.0, 0.0, 1.0];
const DUCT: [Point3; 4] = [
    [0.4, 0.0, -0.125],
    [0.4, 0.0, 0.125],
    [0.8, 0.0, 0.125],
    [0.8, 0.0, -0.125],
];
const TRAPEZOID: [Point3; 4] = [
    [0.4, 0.0, -0.1],
    [0.5, 0.0, 0.1],
    [0.7, 0.0, 0.1],
    [0.8, 0.0, -0.1],
];

const DOWN: Point3 = [0.0, 0.0, -1.0];
const INVERTED_TRAPEZOID: [Point3; 4] = [
    [0.5, 0.0, -0.1],
    [0.4, 0.0, 0.1],
    [0.8, 0.0, 0.1],
    [0.7, 0.0, -0.1],
];

fn turn(axis: Point3, angle: f64, radius: f64, reach: f64) -> Result<BendFrame, OperationError> {
    bend_frame(
        Bend {
            centre: [0.0; 3],
            axis,
            angle,
        },
        [radius, 0.0, 0.0],
        reach,
        accuracy(),
    )
}

fn quarter_bend(radius: f64, reach: f64) -> Result<BendFrame, OperationError> {
    turn(UP, FRAC_PI_2, radius, reach)
}

fn turned(frame: BendFrame, point: Point3) -> Point3 {
    add(frame.centre, frame.rotated(sub(point, frame.centre)))
}

fn line(builder: &mut Builder, from: u32, to: u32) -> u32 {
    let [a, b] = [from, to].map(|id| builder.brep.topology.vertices[id as usize].position);
    builder.edge(
        CurveGeometry::Line {
            origin: a,
            direction: unit(sub(b, a)).unwrap(),
        },
        Interval::new(0.0, norm(sub(b, a))).unwrap(),
        false,
    )
}

fn cap(
    builder: &mut Builder,
    key: &str,
    plane: Frame3,
    ring: (&[u32], &[u32]),
    sense: Orientation,
) {
    let (vertices, edges) = ring;
    let count = vertices.len();
    let mut order = (0..count).collect::<Vec<_>>();
    if sense == Orientation::Reverse {
        order.reverse();
    }
    let uses = order
        .into_iter()
        .map(|edge| {
            let [from, to] = [vertices[edge], vertices[(edge + 1) % count]];
            let [from, to] = if sense == Orientation::Forward {
                [from, to]
            } else {
                [to, from]
            };
            plane_boundary(builder, plane, edges[edge], from, to, sense).unwrap()
        })
        .collect();
    builder
        .face(
            key,
            SurfaceGeometry::Plane { frame: plane },
            [[-2.0, 2.0]; 2],
            uses,
        )
        .unwrap();
}

fn caps(builder: &mut Builder, frame: BendFrame, origin: Point3, rings: [(&[u32], &[u32]); 2]) {
    let tangent = cross(frame.axis, frame.radial);
    let start = Frame3::from_axis(
        origin,
        [-tangent[0], -tangent[1], -tangent[2]],
        frame.radial,
    );
    let end = Frame3::from_axis(
        turned(frame, origin),
        frame.rotated(tangent),
        frame.rotated(frame.radial),
    );
    cap(
        builder,
        "cap-start",
        start.unwrap(),
        rings[0],
        Orientation::Reverse,
    );
    cap(
        builder,
        "cap-end",
        end.unwrap(),
        rings[1],
        Orientation::Forward,
    );
}

fn polygon_elbow(frame: BendFrame, profile: &[Point3]) -> Result<BrepEnvelope, OperationError> {
    let mut builder = Builder::new("elbow".into(), accuracy())?;
    let [start, end] = [0, 1].map(|ring| {
        profile
            .iter()
            .map(|point| {
                builder.vertex(if ring == 0 {
                    *point
                } else {
                    turned(frame, *point)
                })
            })
            .collect::<Vec<_>>()
    });
    let [start_edges, end_edges] = [&start, &end].map(|ring| {
        (0..ring.len())
            .map(|edge| line(&mut builder, ring[edge], ring[(edge + 1) % ring.len()]))
            .collect::<Vec<_>>()
    });
    let arcs = add_bend_arcs(&mut builder, frame, &start)?;
    let bend = PolygonBend {
        frame,
        segment: 0,
        start: &start,
        end: &end,
        start_edges: &start_edges,
        end_edges: &end_edges,
        arcs: &arcs,
    };
    add_polygon_bend(&mut builder, &bend, accuracy())?;
    let origin = builder.brep.topology.vertices[start[0] as usize].position;
    caps(
        &mut builder,
        frame,
        [origin[0], 0.0, 0.0],
        [(&start, &start_edges), (&end, &end_edges)],
    );
    Ok(builder.finish_solid()?)
}

fn circle_ends(
    builder: &mut Builder,
    centre: Point3,
    tangent: Point3,
    x: Point3,
    radius: f64,
) -> CircleEnds {
    let circle = CurveGeometry::Circle {
        frame: Frame3::from_axis(centre, tangent, x).unwrap(),
        radius,
    };
    let vertices = [0.0, PI].map(|t| builder.vertex(circle.elementary_point(t).unwrap()));
    let edges = [[0.0, PI], [PI, TAU]]
        .map(|[lo, hi]| builder.edge(circle.clone(), Interval::new(lo, hi).unwrap(), false));
    CircleEnds { vertices, edges }
}

fn pipe_elbow(
    frame: BendFrame,
    centre: Point3,
    x: Point3,
    radius: f64,
) -> Result<BrepEnvelope, OperationError> {
    let mut builder = Builder::new("elbow".into(), accuracy())?;
    let tangent = cross(frame.axis, frame.radial);
    let start = circle_ends(&mut builder, centre, tangent, x, radius);
    let end = circle_ends(
        &mut builder,
        turned(frame, centre),
        frame.rotated(tangent),
        frame.rotated(x),
        radius,
    );
    let bend = CircleBend {
        frame,
        segment: 0,
        centre,
        x,
        radius,
        start,
        end,
    };
    add_torus_bend(&mut builder, &bend)?;
    let rings = [&start, &end].map(|ring| (ring.vertices.to_vec(), ring.edges.to_vec()));
    caps(
        &mut builder,
        frame,
        centre,
        [(&rings[0].0, &rings[0].1), (&rings[1].0, &rings[1].1)],
    );
    Ok(builder.finish_solid()?)
}

fn volume(body: &BrepEnvelope) -> f64 {
    let mesh = tessellate(body, 1e-4, 2_000_000).unwrap();
    let point =
        |id: u32| -> Point3 { std::array::from_fn(|axis| mesh.positions[id as usize * 3 + axis]) };
    let six = mesh
        .indices
        .chunks_exact(3)
        .map(|ids| dot(point(ids[0]), cross(point(ids[1]), point(ids[2]))));
    six.sum::<f64>() / 6.0
}

fn surfaces(body: &BrepEnvelope) -> Vec<&SurfaceGeometry> {
    body.topology
        .faces
        .iter()
        .map(|face| &body.geometry.surfaces[face.surface as usize])
        .collect()
}

fn every_pcurve_is_exact(body: &BrepEnvelope) -> bool {
    body.topology.halfedges.iter().all(|halfedge| {
        halfedge.geometry_use.pcurve.is_some_and(|id| {
            matches!(
                body.geometry.pcurves[id as usize],
                PcurveGeometry::Line2 { .. } | PcurveGeometry::Conic2 { .. }
            )
        })
    })
}

#[test]
fn bend_tighter_than_the_profile_reach_self_intersects() {
    assert_eq!(
        quarter_bend(0.1, 0.1),
        Err(OperationError::SweepSelfIntersection(
            "bend radius is within the profile reach".into()
        ))
    );
    assert!(quarter_bend(0.1, 0.09).is_ok());
}

#[test]
fn bend_with_a_centre_off_the_path_plane_is_invalid() {
    let bend = Bend {
        centre: [0.0, 0.0, 0.01],
        axis: UP,
        angle: FRAC_PI_2,
    };
    assert!(matches!(
        bend_frame(bend, [0.6, 0.0, 0.0], 0.1, accuracy()),
        Err(OperationError::InvalidParameter(_))
    ));
}

#[test]
fn rectangular_duct_bend_gives_two_cylinders_and_four_planes() {
    let body = polygon_elbow(quarter_bend(0.6, 0.24).unwrap(), &DUCT).unwrap();
    let radii = surfaces(&body)
        .into_iter()
        .filter_map(|surface| match surface {
            SurfaceGeometry::Cylinder { radius, .. } => Some(*radius),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(radii.len(), 2);
    assert!(
        radii.iter().any(|r| (r - 0.4).abs() < 1e-12)
            && radii.iter().any(|r| (r - 0.8).abs() < 1e-12)
    );
    let planes = surfaces(&body)
        .iter()
        .filter(|surface| matches!(surface, SurfaceGeometry::Plane { .. }))
        .count();
    assert_eq!((body.topology.faces.len(), planes), (6, 4));
    assert!(every_pcurve_is_exact(&body));
    assert!((volume(&body) - 0.1 * 0.6 * FRAC_PI_2).abs() < 1e-4);
}

#[test]
fn duct_bend_cylinder_faces_have_exact_uv_rectangles() {
    let body = polygon_elbow(quarter_bend(0.6, 0.24).unwrap(), &DUCT).unwrap();
    for face in &body.topology.faces {
        if matches!(
            body.geometry.surfaces[face.surface as usize],
            SurfaceGeometry::Cylinder { .. }
        ) {
            let [u, v] = face.trim.uv_bounds;
            assert_eq!([u.lo, u.hi], [0.0, FRAC_PI_2]);
            assert!((v.lo + 0.125).abs() < 1e-15 && (v.hi - 0.125).abs() < 1e-15);
        }
    }
}

#[test]
fn trapezoid_bend_gives_two_cones_with_their_apexes_on_either_side() {
    let body = polygon_elbow(quarter_bend(0.6, 0.23).unwrap(), &TRAPEZOID).unwrap();
    let cones = body
        .topology
        .faces
        .iter()
        .filter_map(|face| match body.geometry.surfaces[face.surface as usize] {
            SurfaceGeometry::Cone { frame, semi_angle } => {
                let u = face.trim.uv_bounds[0];
                Some((semi_angle, (frame.z[2], [u.lo, u.hi], face.sense)))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let angle = 0.5_f64.atan();
    assert_eq!(cones.len(), 2);
    assert!(cones.iter().all(|cone| (cone.0 - angle).abs() < 1e-12));
    let sides = cones.iter().map(|cone| cone.1).collect::<Vec<_>>();
    assert!(sides.contains(&(1.0, [0.0, FRAC_PI_2], Orientation::Reverse)));
    assert!(sides.contains(&(-1.0, [-FRAC_PI_2, 0.0], Orientation::Forward)));
    assert_eq!(body.topology.faces.len(), 6);
    assert!(every_pcurve_is_exact(&body));
    assert!((volume(&body) - 0.06 * 0.6 * FRAC_PI_2).abs() < 1e-4);
}

#[test]
fn round_pipe_bend_gives_two_half_torus_faces() {
    for x in [[1.0, 0.0, 0.0], UP, [0.6, 0.0, 0.8]] {
        let body = pipe_elbow(quarter_bend(0.3, 0.1).unwrap(), [0.3, 0.0, 0.0], x, 0.1).unwrap();
        let tori = surfaces(&body)
            .into_iter()
            .filter(|surface| {
                matches!(surface, SurfaceGeometry::Torus { major_radius, minor_radius, .. }
                    if (major_radius - 0.3).abs() < 1e-12 && *minor_radius == 0.1)
            })
            .count();
        assert_eq!((body.topology.faces.len(), tori), (4, 2));
        assert!(every_pcurve_is_exact(&body));
        assert!((volume(&body) - PI * 0.01 * 0.3 * FRAC_PI_2).abs() < 1e-4);
    }
}

#[test]
fn pipe_bend_torus_faces_span_half_a_turn_of_the_tube() {
    let body = pipe_elbow(quarter_bend(0.3, 0.1).unwrap(), [0.3, 0.0, 0.0], UP, 0.1).unwrap();
    let spans = body
        .topology
        .faces
        .iter()
        .filter(|face| {
            matches!(
                body.geometry.surfaces[face.surface as usize],
                SurfaceGeometry::Torus { .. }
            )
        })
        .map(|face| {
            let [u, v] = face.trim.uv_bounds;
            [u.lo, u.hi, v.lo, v.hi]
        })
        .collect::<Vec<_>>();
    assert_eq!(
        spans,
        [
            [0.0, FRAC_PI_2, FRAC_PI_2 - PI, FRAC_PI_2],
            [0.0, FRAC_PI_2, FRAC_PI_2 - TAU, FRAC_PI_2 - PI]
        ]
    );
}

fn reversed(profile: &[Point3]) -> Vec<Point3> {
    profile.iter().rev().copied().collect()
}

fn assert_elbow(body: &BrepEnvelope, faces: usize, expected: f64) {
    assert_eq!(body.topology.faces.len(), faces);
    assert!(every_pcurve_is_exact(body));
    assert!(
        (volume(body) - expected).abs() < 1e-4,
        "{} against {expected}",
        volume(body)
    );
}

fn cone_pairs(body: &BrepEnvelope, axis: Point3) -> Vec<(f64, Orientation)> {
    body.topology
        .faces
        .iter()
        .filter_map(|face| match body.geometry.surfaces[face.surface as usize] {
            SurfaceGeometry::Cone { frame, .. } => Some((dot(frame.z, axis).round(), face.sense)),
            _ => None,
        })
        .collect()
}

#[test]
fn bend_rule_keeps_a_margin_of_four_geometric_tolerances() {
    let g = accuracy().geometric;
    assert_eq!(
        turn(UP, FRAC_PI_2, 0.1, 0.1 - 3.0 * g),
        Err(OperationError::SweepSelfIntersection(
            "bend radius is within the profile reach".into()
        ))
    );
    assert!(turn(UP, FRAC_PI_2, 0.1, 0.1 - 5.0 * g).is_ok());
}

#[test]
fn bend_angle_outside_zero_to_half_a_turn_is_invalid() {
    for angle in [0.0, 3.2, f64::NAN] {
        assert_eq!(
            turn(UP, angle, 0.6, 0.24),
            Err(OperationError::InvalidParameter(
                "sweep bend is invalid".into()
            ))
        );
    }
}

#[test]
fn half_turn_duct_and_pipe_elbows_build() {
    let duct = polygon_elbow(turn(UP, PI, 0.6, 0.24).unwrap(), &DUCT).unwrap();
    assert_elbow(&duct, 6, 0.1 * 0.6 * PI);
    let pipe = pipe_elbow(turn(UP, PI, 0.3, 0.1).unwrap(), [0.3, 0.0, 0.0], UP, 0.1).unwrap();
    assert_elbow(&pipe, 4, PI * 0.01 * 0.3 * PI);
}

#[test]
fn bends_about_a_downward_axis_give_the_same_elbows() {
    let frame = |reach| turn(DOWN, FRAC_PI_2, 0.6, reach).unwrap();
    let duct = polygon_elbow(frame(0.24), &reversed(&DUCT)).unwrap();
    assert_elbow(&duct, 6, 0.1 * 0.6 * FRAC_PI_2);
    let trapezoid = polygon_elbow(frame(0.23), &reversed(&TRAPEZOID)).unwrap();
    assert_elbow(&trapezoid, 6, 0.06 * 0.6 * FRAC_PI_2);
    assert_eq!(cone_pairs(&trapezoid, DOWN).len(), 2);
    let pipe = pipe_elbow(
        turn(DOWN, FRAC_PI_2, 0.3, 0.1).unwrap(),
        [0.3, 0.0, 0.0],
        UP,
        0.1,
    );
    assert_elbow(&pipe.unwrap(), 4, PI * 0.01 * 0.3 * FRAC_PI_2);
}

#[test]
fn inverted_trapezoid_gives_the_other_two_cone_pairs() {
    let body = polygon_elbow(quarter_bend(0.6, 0.23).unwrap(), &INVERTED_TRAPEZOID).unwrap();
    assert_elbow(&body, 6, 0.06 * 0.6 * FRAC_PI_2);
    let pairs = cone_pairs(&body, UP);
    assert_eq!(pairs.len(), 2);
    assert!(pairs.contains(&(-1.0, Orientation::Reverse)));
    assert!(pairs.contains(&(1.0, Orientation::Forward)));
}
