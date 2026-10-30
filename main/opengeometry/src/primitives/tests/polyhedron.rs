use crate::brep::{coarse_accuracy, fine_accuracy};
use crate::brep::{BrepEnvelope, CurveGeometry, GeometryError, Surface, SurfaceGeometry};
use crate::math::{dot, sub, Point3};
use crate::primitives::polyhedron::polyhedron;
use crate::tessellation::tessellate;

const CUBE_FACES: [[u32; 4]; 6] = [
    [0, 3, 2, 1],
    [4, 5, 6, 7],
    [0, 1, 5, 4],
    [3, 7, 6, 2],
    [0, 4, 7, 3],
    [1, 2, 6, 5],
];

const TETRAHEDRON_FACES: [[u32; 3]; 4] = [[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]];

fn cube_points() -> Vec<Point3> {
    (0..8)
        .map(|index| {
            [
                f64::from(u8::from(matches!(index, 1 | 2 | 5 | 6))),
                f64::from(u8::from(matches!(index, 2 | 3 | 6 | 7))),
                f64::from(u8::from(index >= 4)),
            ]
        })
        .collect()
}

fn cube_faces() -> Vec<Vec<u32>> {
    CUBE_FACES.iter().map(|ring| ring.to_vec()).collect()
}

fn tetrahedron(offset: Point3, first: u32) -> (Vec<Point3>, Vec<Vec<u32>>) {
    let points = [[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
        .map(|point| {
            [
                point[0] + offset[0],
                point[1] + offset[1],
                point[2] + offset[2],
            ]
        })
        .to_vec();
    let faces = TETRAHEDRON_FACES
        .iter()
        .map(|ring| ring.iter().map(|index| index + first).collect())
        .collect();
    (points, faces)
}

fn built(points: &[Point3], faces: &[Vec<u32>]) -> Result<BrepEnvelope, GeometryError> {
    polyhedron("polyhedron".into(), points, faces, fine_accuracy())
}

fn topology_error(points: &[Point3], faces: &[Vec<u32>]) -> String {
    match built(points, faces) {
        Err(GeometryError::InvalidTopology(message)) => message,
        other => panic!("expected InvalidTopology, got {other:?}"),
    }
}

fn geometry_error(points: &[Point3], faces: &[Vec<u32>]) -> String {
    match built(points, faces) {
        Err(GeometryError::InvalidGeometry(message)) => message,
        other => panic!("expected InvalidGeometry, got {other:?}"),
    }
}

#[test]
fn cube_polyhedron_is_a_valid_planar_solid_with_outward_faces() {
    let body = built(&cube_points(), &cube_faces()).unwrap();
    body.validate().unwrap();
    let topology = &body.topology;
    assert_eq!(
        (
            topology.vertices.len(),
            topology.edges.len(),
            topology.faces.len()
        ),
        (8, 12, 6)
    );
    assert!(body
        .geometry
        .curves
        .iter()
        .all(|curve| matches!(curve, CurveGeometry::Line { .. })));
    for (index, face) in topology.faces.iter().enumerate() {
        assert_eq!(face.key, format!("face-{index}"));
        assert!(matches!(
            body.geometry.surfaces[face.surface as usize],
            SurfaceGeometry::Plane { .. }
        ));
        let surface = body.geometry.surface(face.surface).unwrap();
        let uv = face.trim.uv_bounds.map(|range| (range.lo + range.hi) / 2.0);
        let outward = dot(
            sub(surface.point_at(uv).unwrap(), [0.5; 3]),
            surface.normal_at(uv).unwrap(),
        );
        assert!(outward > 0.0, "face {index}");
    }
    assert!(!tessellate(&body, 0.01, 100_000).unwrap().indices.is_empty());
}

#[test]
fn face_off_its_plane_by_more_than_one_tolerance_is_refused() {
    let mut points = cube_points();
    points[6][2] += 6e-9;
    assert!(geometry_error(&points, &cube_faces()).contains("off the plane of face"));
    let mut points = cube_points();
    points[6][2] += 2e-9;
    built(&points, &cube_faces()).unwrap().validate().unwrap();
}

#[test]
fn open_shell_is_refused() {
    let mut faces = cube_faces();
    faces.pop();
    assert!(topology_error(&cube_points(), &faces).contains("open"));
}

#[test]
fn edge_run_twice_one_way_is_refused() {
    let mut faces = cube_faces();
    faces[1].reverse();
    assert!(topology_error(&cube_points(), &faces).contains("both run"));
}

#[test]
fn inward_winding_is_refused() {
    let mut faces = cube_faces();
    for ring in &mut faces {
        ring.reverse();
    }
    assert!(topology_error(&cube_points(), &faces).contains("inward"));
}

#[test]
fn coincident_points_are_refused() {
    let mut points = cube_points();
    points[6] = [points[0][0] + 1e-9, points[0][1], points[0][2]];
    assert!(geometry_error(&points, &cube_faces()).contains("points 0 and 6"));
}

#[test]
fn unused_point_is_refused() {
    let mut points = cube_points();
    points.push([5.0, 5.0, 5.0]);
    assert!(geometry_error(&points, &cube_faces()).contains("point 8"));
}

#[test]
fn malformed_rings_points_and_counts_are_refused_without_panic() {
    let mut faces = cube_faces();
    faces[0][1] = 99;
    assert!(geometry_error(&cube_points(), &faces).contains("99"));
    let mut faces = cube_faces();
    faces[0][1] = 0;
    assert!(geometry_error(&cube_points(), &faces).contains("twice"));
    let mut faces = cube_faces();
    faces[0].truncate(2);
    assert!(geometry_error(&cube_points(), &faces).contains("three"));
    let mut points = cube_points();
    points[3][0] = f64::NAN;
    assert!(geometry_error(&points, &cube_faces()).contains("finite"));
    let (points, faces) = tetrahedron([0.0; 3], 0);
    assert!(geometry_error(&points[..3], &faces[..3]).contains("four"));
}

#[test]
fn two_disjoint_tetrahedra_are_refused() {
    let (mut points, mut faces) = tetrahedron([0.0; 3], 0);
    let (more_points, more_faces) = tetrahedron([5.0, 0.0, 0.0], 4);
    points.extend(more_points);
    faces.extend(more_faces);
    assert!(topology_error(&points, &faces).contains("shell"));
}

#[test]
fn pinched_vertex_is_refused() {
    let (mut points, mut faces) = tetrahedron([0.0; 3], 0);
    let (upper_points, upper_faces) = tetrahedron([0.0, 0.0, 1.0], 3);
    points.extend_from_slice(&upper_points[1..]);
    faces.extend(upper_faces);
    assert!(topology_error(&points, &faces).contains("vertex 3"));
}

#[test]
fn flat_self_overlapping_shell_is_refused() {
    let (mut points, faces) = tetrahedron([0.0; 3], 0);
    points[3] = [0.25, 0.25, 5e-10];
    assert!(topology_error(&points, &faces).contains("enclose no volume"));
}

#[test]
fn collinear_ring_points_and_coplanar_neighbour_faces_are_accepted() {
    let mut points = cube_points();
    points.extend([[0.5, 0.0, 1.0], [0.5, 1.0, 1.0]]);
    let mut faces = cube_faces();
    faces[1] = vec![4, 8, 9, 7];
    faces[2] = vec![0, 1, 5, 8, 4];
    faces[3] = vec![3, 7, 9, 6, 2];
    faces.push(vec![8, 5, 6, 9]);
    let body = built(&points, &faces).unwrap();
    body.validate().unwrap();
    assert_eq!(
        (body.topology.vertices.len(), body.topology.faces.len()),
        (10, 7)
    );
}

#[test]
fn polyhedron_ten_thousand_kilometres_from_the_origin_builds() {
    let points = cube_points()
        .iter()
        .map(|point| [point[0] + 1e7, point[1] * 3.0, point[2] + 1e7])
        .collect::<Vec<_>>();
    let body = polyhedron("far".into(), &points, &cube_faces(), coarse_accuracy(1e-5)).unwrap();
    body.validate().unwrap();
}
