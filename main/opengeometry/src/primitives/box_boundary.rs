use crate::brep::{
    padded_uv_bounds, plane_boundary, Accuracy, Builder, CurveGeometry, Frame3, GeometryError,
    Orientation, Use,
};
use crate::math::{scale, sub, Interval, Point3};

pub(crate) struct BoxCorners {
    points: Vec<Point3>,
    pub(crate) vertices: Vec<u32>,
    edges: [[u32; 3]; 8],
}

pub(crate) struct BoxFaceBoundary {
    pub(crate) frame: Frame3,
    pub(crate) uses: Vec<Use>,
    pub(crate) bounds: [[f64; 2]; 2],
}

pub(crate) struct BoxFaceLayout {
    pub(crate) key: &'static str,
    pub(crate) indices: [usize; 4],
    pub(crate) normal: Point3,
}

pub(crate) fn add_box_corners(
    builder: &mut Builder,
    frame: Frame3,
    size: [f64; 3],
    axes: [Point3; 3],
) -> Result<BoxCorners, GeometryError> {
    let points: Vec<_> = (0..8)
        .map(|index| {
            frame.point(std::array::from_fn(|coordinate| {
                if index & (1 << coordinate) == 0 {
                    0.0
                } else {
                    size[coordinate]
                }
            }))
        })
        .collect();
    let vertices = points
        .iter()
        .map(|point| builder.vertex(*point))
        .collect::<Vec<_>>();
    let mut edges = [[u32::MAX; 3]; 8];
    for index in 0..8 {
        for coordinate in 0..3 {
            if index & (1 << coordinate) == 0 {
                edges[index][coordinate] = builder.edge(
                    CurveGeometry::Line {
                        origin: points[index],
                        direction: axes[coordinate],
                    },
                    Interval::new(0.0, size[coordinate])?,
                    false,
                );
            }
        }
    }
    Ok(BoxCorners {
        points,
        vertices,
        edges,
    })
}

pub(crate) fn box_face_layout(frame: Frame3) -> [BoxFaceLayout; 6] {
    [
        BoxFaceLayout {
            key: "bottom",
            indices: [0, 2, 3, 1],
            normal: scale(frame.z, -1.0),
        },
        BoxFaceLayout {
            key: "top",
            indices: [4, 5, 7, 6],
            normal: frame.z,
        },
        BoxFaceLayout {
            key: "left",
            indices: [0, 4, 6, 2],
            normal: scale(frame.x, -1.0),
        },
        BoxFaceLayout {
            key: "right",
            indices: [1, 3, 7, 5],
            normal: frame.x,
        },
        BoxFaceLayout {
            key: "front",
            indices: [0, 1, 5, 4],
            normal: scale(frame.y, -1.0),
        },
        BoxFaceLayout {
            key: "back",
            indices: [2, 6, 7, 3],
            normal: frame.y,
        },
    ]
}

pub(crate) fn box_face_boundary(
    builder: &Builder,
    corners: &BoxCorners,
    indices: [usize; 4],
    normal: Point3,
    accuracy: Accuracy,
    vertex: impl Fn(usize) -> u32,
) -> Result<BoxFaceBoundary, GeometryError> {
    let face_frame = Frame3::from_axis(
        corners.points[indices[0]],
        normal,
        sub(corners.points[indices[1]], corners.points[indices[0]]),
    )?;
    let mut uses = Vec::with_capacity(4);
    for index in 0..4 {
        let from = indices[index];
        let to = indices[(index + 1) % 4];
        let coordinate = (from ^ to).trailing_zeros() as usize;
        uses.push(plane_boundary(
            builder,
            face_frame,
            corners.edges[from.min(to)][coordinate],
            vertex(from),
            vertex(to),
            if from < to {
                Orientation::Forward
            } else {
                Orientation::Reverse
            },
        )?);
    }
    let bounds = padded_uv_bounds(
        indices.map(|from| face_frame.local(corners.points[from])),
        accuracy.geometric,
    );
    Ok(BoxFaceBoundary {
        frame: face_frame,
        uses,
        bounds,
    })
}
