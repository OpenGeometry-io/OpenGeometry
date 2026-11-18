use opengeometry::brep::{
    plane_boundary, Builder, CurveGeometry, Frame3, GeometryError, Orientation, SurfaceGeometry,
};
use opengeometry::exchange::export_step;
use opengeometry::math::{Interval, Point3};
use opengeometry::query::{classify_point, PointClassification};
use opengeometry_test_support::accuracy::accuracy;
use opengeometry_test_support::volume::estimate;

const FACES: [(&str, [usize; 4], Point3); 6] = [
    ("bottom", [0, 2, 3, 1], [0.0, 0.0, -1.0]),
    ("top", [4, 5, 7, 6], [0.0, 0.0, 1.0]),
    ("left", [0, 4, 6, 2], [-1.0, 0.0, 0.0]),
    ("right", [1, 3, 7, 5], [1.0, 0.0, 0.0]),
    ("front", [0, 1, 5, 4], [0.0, -1.0, 0.0]),
    ("back", [2, 6, 7, 3], [0.0, 1.0, 0.0]),
];

fn sub(a: Point3, b: Point3) -> Point3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: Point3, b: Point3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn corners(size: Point3) -> Vec<Point3> {
    (0..8)
        .map(|i| {
            std::array::from_fn(|axis| {
                if i & (1 << axis) == 0 {
                    0.0
                } else {
                    size[axis]
                }
            })
        })
        .collect()
}

fn box_edges(builder: &mut Builder, points: &[Point3], size: Point3) -> [[u32; 3]; 8] {
    let axes = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let mut edges = [[u32::MAX; 3]; 8];
    for i in 0..8 {
        for axis in (0..3).filter(|axis| i & (1 << axis) == 0) {
            let line = CurveGeometry::Line {
                origin: points[i],
                direction: axes[axis],
            };
            edges[i][axis] = builder.edge(line, Interval::new(0.0, size[axis]).unwrap(), false);
        }
    }
    edges
}

fn box_builder(size: Point3) -> Result<(Builder, Vec<u32>), GeometryError> {
    let mut builder = Builder::new("box".into(), accuracy(1e-6))?;
    let points = corners(size);
    for point in &points {
        builder.vertex(*point);
    }
    let edges = box_edges(&mut builder, &points, size);
    let mut faces = Vec::new();
    for (key, corner, normal) in FACES {
        let x = sub(points[corner[1]], points[corner[0]]);
        let frame = Frame3::from_axis(points[corner[0]], normal, x)?;
        let mut uses = Vec::new();
        let mut uv = Vec::new();
        for i in 0..4 {
            let (from, to) = (corner[i], corner[(i + 1) % 4]);
            let axis = (from ^ to).trailing_zeros() as usize;
            let sense = if from < to {
                Orientation::Forward
            } else {
                Orientation::Reverse
            };
            let edge = edges[from.min(to)][axis];
            uses.push(plane_boundary(
                &builder,
                frame,
                edge,
                from as u32,
                to as u32,
                sense,
            )?);
            let local = sub(points[from], frame.origin);
            uv.push([dot(local, frame.x), dot(local, frame.y)]);
        }
        let bounds = std::array::from_fn(|axis| {
            let values = uv.iter().map(|point| point[axis]);
            let lo = values.clone().fold(f64::INFINITY, f64::min);
            [lo - 1e-7, values.fold(f64::NEG_INFINITY, f64::max) + 1e-7]
        });
        faces.push(builder.face(key, SurfaceGeometry::Plane { frame }, bounds, uses)?);
    }
    Ok((builder, faces))
}

#[test]
fn a_box_built_through_the_public_builder_is_a_valid_exportable_solid() {
    let (builder, faces) = box_builder([2.0, 3.0, 4.0]).unwrap();
    assert_eq!(faces, [0, 1, 2, 3, 4, 5]);
    assert_eq!(builder.brep().topology.faces.len(), 6);
    let brep = builder.finish_solid().unwrap();
    brep.validate().unwrap();
    assert_eq!(brep.topology.edges.len(), 12);
    assert_eq!(brep.topology.halfedges.len(), 24);
    let volume = estimate(&brep, 1e-3);
    assert!((volume.value - 24.0).abs() <= volume.error_bound.max(1e-9));
    let inside = classify_point(&brep, [1.0, 1.5, 2.0]).unwrap();
    assert_eq!(inside, PointClassification::Inside);
    let outside = classify_point(&brep, [3.0, 1.5, 2.0]).unwrap();
    assert_eq!(outside, PointClassification::Outside);
    let (step, _) = export_step(&brep, "millimetre").unwrap();
    assert_eq!(step.matches("=ADVANCED_FACE(").count(), 6);
}

#[test]
fn a_face_sense_is_set_by_face_id() {
    let (mut builder, faces) = box_builder([1.0, 1.0, 1.0]).unwrap();
    let top = [0.5, 0.5, 1.0];
    let normal = |builder: &Builder| builder.brep().face_normal_at(faces[1], top).unwrap();
    assert!((normal(&builder)[2] - 1.0).abs() < 1e-12);
    builder
        .set_face_sense(faces[1], Orientation::Reverse)
        .unwrap();
    assert!((normal(&builder)[2] + 1.0).abs() < 1e-12);
    assert!(matches!(
        builder.set_face_sense(6, Orientation::Reverse),
        Err(GeometryError::MissingReference { .. })
    ));
}

#[test]
fn an_edge_used_by_a_third_loop_is_rejected() {
    let (mut builder, _) = box_builder([1.0, 1.0, 1.0]).unwrap();
    let frame = Frame3::from_axis([0.0; 3], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]).unwrap();
    let reuse = plane_boundary(&builder, frame, 0, 0, 1, Orientation::Forward).unwrap();
    let plane = SurfaceGeometry::Plane { frame };
    let result = builder.face("extra", plane, [[-1.0, 1.0], [-1.0, 1.0]], vec![reuse]);
    assert!(matches!(result, Err(GeometryError::InvalidTopology(_))));
}
