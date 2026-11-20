use opengeometry::brep::{
    boundary, plane_boundary, uv_conic, uv_line, BrepEnvelope, Builder, CurveGeometry, Frame3,
    GeometryError, Orientation, SurfaceGeometry, Use,
};
use opengeometry::exchange::export_step;
use opengeometry::math::{Interval, Point3};
use opengeometry::query::{classify_point, PointClassification};
use opengeometry_test_support::accuracy::accuracy;
use opengeometry_test_support::volume::estimate;
use std::f64::consts::{PI, TAU};
use Orientation::{Forward as F, Reverse as R};

type BoxFace = (&'static str, [usize; 4], Point3);

const FACES: [BoxFace; 6] = [
    ("bottom", [0, 2, 3, 1], [0.0, 0.0, -1.0]),
    ("top", [4, 5, 7, 6], [0.0, 0.0, 1.0]),
    ("left", [0, 4, 6, 2], [-1.0, 0.0, 0.0]),
    ("right", [1, 3, 7, 5], [1.0, 0.0, 0.0]),
    ("front", [0, 1, 5, 4], [0.0, -1.0, 0.0]),
    ("back", [2, 6, 7, 3], [0.0, 1.0, 0.0]),
];

struct BoxParts {
    builder: Builder,
    points: Vec<Point3>,
    edges: [[u32; 3]; 8],
}

fn sub(a: Point3, b: Point3) -> Point3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: Point3, b: Point3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn box_parts(size: Point3) -> BoxParts {
    let mut builder = Builder::new("box".into(), accuracy(1e-6)).unwrap();
    let points: Vec<Point3> = (0..8)
        .map(|i| {
            std::array::from_fn(|axis| {
                if i & (1 << axis) == 0 {
                    0.0
                } else {
                    size[axis]
                }
            })
        })
        .collect();
    for point in &points {
        builder.vertex(*point);
    }
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
    BoxParts {
        builder,
        points,
        edges,
    }
}

fn face_frame(parts: &BoxParts, (_, corner, normal): BoxFace) -> Frame3 {
    let x = sub(parts.points[corner[1]], parts.points[corner[0]]);
    Frame3::from_axis(parts.points[corner[0]], normal, x).unwrap()
}

fn box_face(
    parts: &mut BoxParts,
    face: BoxFace,
    holes: Vec<Vec<Use>>,
) -> Result<u32, GeometryError> {
    let (key, corner, _) = face;
    let frame = face_frame(parts, face);
    let mut uses = Vec::new();
    let mut uv = Vec::new();
    for i in 0..4 {
        let (from, to) = (corner[i], corner[(i + 1) % 4]);
        let axis = (from ^ to).trailing_zeros() as usize;
        let sense = if from < to { F } else { R };
        let edge = parts.edges[from.min(to)][axis];
        let side = plane_boundary(&parts.builder, frame, edge, from as u32, to as u32, sense)?;
        uses.push(side);
        let local = sub(parts.points[from], frame.origin);
        uv.push([dot(local, frame.x), dot(local, frame.y)]);
    }
    let bounds = std::array::from_fn(|axis| {
        let values = uv.iter().map(|point| point[axis]);
        let lo = values.clone().fold(f64::INFINITY, f64::min);
        [lo - 1e-7, values.fold(f64::NEG_INFINITY, f64::max) + 1e-7]
    });
    let plane = SurfaceGeometry::Plane { frame };
    parts
        .builder
        .face_with_holes(key, plane, bounds, uses, holes)
}

fn box_builder(size: Point3) -> (Builder, Vec<u32>) {
    let mut parts = box_parts(size);
    let faces = FACES
        .map(|face| box_face(&mut parts, face, Vec::new()).unwrap())
        .to_vec();
    (parts.builder, faces)
}

fn assert_faces_point_outward(brep: &BrepEnvelope, on_faces: &[Point3]) {
    assert_eq!(on_faces.len(), brep.topology.faces.len());
    for (face, &point) in on_faces.iter().enumerate() {
        let normal = brep.face_normal_at(face as u32, point).unwrap();
        let off = std::array::from_fn(|axis| point[axis] + 1e-3 * normal[axis]);
        let class = classify_point(brep, off).unwrap();
        assert_eq!(class, PointClassification::Outside, "face {face}");
    }
}

fn box_face_centres(size: Point3) -> [Point3; 6] {
    let [x, y, z] = size.map(|value| value / 2.0);
    let [w, d, h] = size;
    [
        [x, y, 0.0],
        [x, y, h],
        [0.0, y, z],
        [w, y, z],
        [x, 0.0, z],
        [x, d, z],
    ]
}

#[test]
fn a_box_built_through_the_public_builder_is_a_valid_exportable_solid() {
    let (builder, faces) = box_builder([2.0, 3.0, 4.0]);
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
    assert_faces_point_outward(&brep, &box_face_centres([2.0, 3.0, 4.0]));
    let (step, _) = export_step(&brep, "millimetre").unwrap();
    assert_eq!(step.matches("=ADVANCED_FACE(").count(), 6);
}

#[test]
fn a_plate_with_a_reversed_bore_has_holed_faces_and_a_seam_edge() {
    let mut parts = box_parts([4.0, 4.0, 1.0]);
    let builder = &mut parts.builder;
    let [low, high] = [0.0, 1.0].map(|z| builder.vertex([3.0, 2.0, z]));
    let circle_frame = |z| Frame3::from_axis([2.0, 2.0, z], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]);
    let [bottom, top] = [0.0, 1.0].map(|z| {
        let circle = CurveGeometry::Circle {
            frame: circle_frame(z).unwrap(),
            radius: 1.0,
        };
        builder.edge(circle, Interval::new(0.0, TAU).unwrap(), false)
    });
    let line = CurveGeometry::Line {
        origin: [3.0, 2.0, 0.0],
        direction: [0.0, 0.0, 1.0],
    };
    let seam = builder.edge(line, Interval::new(0.0, 1.0).unwrap(), true);
    let lower_hole = plane_boundary(
        &parts.builder,
        face_frame(&parts, FACES[0]),
        bottom,
        low,
        low,
        R,
    );
    box_face(&mut parts, FACES[0], vec![vec![lower_hole.unwrap()]]).unwrap();
    let upper_hole = boundary(
        top,
        high,
        high,
        F,
        uv_conic([2.0, 2.0], [1.0, 0.0], [0.0, 1.0]),
    );
    box_face(&mut parts, FACES[1], vec![vec![upper_hole]]).unwrap();
    for face in &FACES[2..] {
        box_face(&mut parts, *face, Vec::new()).unwrap();
    }
    let bore_uses = vec![
        boundary(bottom, low, low, F, uv_line([0.0, 0.0], [1.0, 0.0])),
        boundary(seam, low, high, F, uv_line([TAU, 0.0], [0.0, 1.0])),
        boundary(top, high, high, R, uv_line([0.0, 1.0], [1.0, 0.0])),
        boundary(seam, high, low, R, uv_line([0.0, 0.0], [0.0, 1.0])),
    ];
    let cylinder = SurfaceGeometry::Cylinder {
        frame: circle_frame(0.0).unwrap(),
        radius: 1.0,
    };
    let bounds = [[0.0, TAU], [0.0, 1.0]];
    let bore = parts
        .builder
        .face("bore", cylinder, bounds, bore_uses)
        .unwrap();
    parts.builder.set_face_sense(bore, R).unwrap();
    assert_eq!(parts.builder.face_sense(bore).unwrap(), R);
    let brep = parts.builder.finish_solid().unwrap();
    brep.validate().unwrap();
    let volume = estimate(&brep, 1e-3);
    assert!((volume.value - (16.0 - PI)).abs() <= volume.error_bound.max(1e-3));
    let hole = classify_point(&brep, [2.0, 2.0, 0.5]).unwrap();
    assert_eq!(hole, PointClassification::Outside);
    let mut on_faces = box_face_centres([4.0, 4.0, 1.0]).to_vec();
    on_faces[..2].copy_from_slice(&[[0.5, 0.5, 0.0], [0.5, 0.5, 1.0]]);
    on_faces.push([2.0, 3.0, 0.5]);
    assert_faces_point_outward(&brep, &on_faces);
}

#[test]
fn a_face_sense_is_set_by_face_id() {
    let (mut builder, faces) = box_builder([1.0, 1.0, 1.0]);
    let top = [0.5, 0.5, 1.0];
    let normal = |builder: &Builder| builder.brep().face_normal_at(faces[1], top).unwrap();
    assert!((normal(&builder)[2] - 1.0).abs() < 1e-12);
    assert_eq!(builder.face_sense(faces[1]).unwrap(), F);
    builder.set_face_sense(faces[1], R).unwrap();
    assert_eq!(builder.face_sense(faces[1]).unwrap(), R);
    assert!((normal(&builder)[2] + 1.0).abs() < 1e-12);
}

#[test]
fn the_builder_leaves_orientation_to_the_caller() {
    let (mut builder, faces) = box_builder([1.0, 1.0, 1.0]);
    for face in faces {
        builder.set_face_sense(face, R).unwrap();
    }
    let inside_out = builder.finish_solid().unwrap();
    for (face, point) in box_face_centres([1.0; 3]).into_iter().enumerate() {
        let normal = inside_out.face_normal_at(face as u32, point).unwrap();
        let off = std::array::from_fn(|axis| point[axis] + 1e-3 * normal[axis]);
        let class = classify_point(&inside_out, off).unwrap();
        assert_eq!(class, PointClassification::Inside, "face {face}");
    }
}

#[test]
fn unknown_ids_are_missing_references() {
    let (mut builder, _) = box_builder([1.0, 1.0, 1.0]);
    let missing = |result: Result<_, GeometryError>, kind: &str| match result {
        Err(GeometryError::MissingReference { kind: found, index }) => {
            assert_eq!((found.as_str(), index), (kind, 99))
        }
        other => panic!("{kind}: {:?}", other.map(|_| ())),
    };
    let frame = Frame3::from_axis([0.0; 3], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]).unwrap();
    missing(
        plane_boundary(&builder, frame, 99, 0, 1, F).map(|_| ()),
        "edge",
    );
    let plane = SurfaceGeometry::Plane { frame };
    let bounds = [[-1.0, 1.0]; 2];
    for (edge, from, to, kind) in [
        (99, 0, 1, "edge"),
        (0, 99, 1, "vertex"),
        (0, 0, 99, "vertex"),
    ] {
        let uses = vec![boundary(edge, from, to, F, uv_line([0.0; 2], [1.0, 0.0]))];
        missing(
            builder.face("bad", plane.clone(), bounds, uses).map(|_| ()),
            kind,
        );
    }
    missing(builder.face_sense(99).map(|_| ()), "face");
    missing(builder.set_face_sense(99, R), "face");
}

#[test]
fn a_rejected_face_leaves_the_builder_unchanged() {
    let mut parts = box_parts([1.0, 1.0, 1.0]);
    for face in &FACES[..5] {
        box_face(&mut parts, *face, Vec::new()).unwrap();
    }
    let before = format!("{:?}", parts.builder.brep());
    let back_frame = face_frame(&parts, FACES[5]);
    let third_use = plane_boundary(&parts.builder, back_frame, 0, 0, 1, F).unwrap();
    let rejected = box_face(&mut parts, FACES[5], vec![vec![third_use]]);
    assert!(matches!(rejected, Err(GeometryError::InvalidTopology(_))));
    let bad_vertex = boundary(0, 0, 99, F, uv_line([0.0; 2], [1.0, 0.0]));
    let rejected = box_face(&mut parts, FACES[5], vec![vec![bad_vertex]]);
    assert!(matches!(
        rejected,
        Err(GeometryError::MissingReference { .. })
    ));
    assert_eq!(format!("{:?}", parts.builder.brep()), before);
    assert_eq!(box_face(&mut parts, FACES[5], Vec::new()).unwrap(), 5);
    let brep = parts.builder.finish_solid().unwrap();
    assert_faces_point_outward(&brep, &box_face_centres([1.0; 3]));
}
