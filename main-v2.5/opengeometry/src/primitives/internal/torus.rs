use crate::brep::{
    boundary, uv_line, Accuracy, BrepEnvelope, Builder, CurveGeometry, Frame3, GeometryError,
    Orientation, SurfaceGeometry,
};
use crate::math::{scale, Interval};

pub fn torus(
    id: String,
    frame: Frame3,
    major_radius: f64,
    minor_radius: f64,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    let surface = SurfaceGeometry::Torus {
        frame,
        major_radius,
        minor_radius,
    };
    surface.validate()?;
    let mut b = Builder::new(id, accuracy)?;
    let vertex = b.vertex(frame.point([major_radius + minor_radius, 0.0, 0.0]));
    let tau = std::f64::consts::TAU;
    let ring = b.edge(
        CurveGeometry::Circle {
            frame,
            radius: major_radius + minor_radius,
        },
        Interval::new(0.0, tau)?,
        true,
    );
    let tube_frame = Frame3 {
        origin: frame.point([major_radius, 0.0, 0.0]),
        x: frame.x,
        y: frame.z,
        z: scale(frame.y, -1.0),
    };
    let tube = b.edge(
        CurveGeometry::Circle {
            frame: tube_frame,
            radius: minor_radius,
        },
        Interval::new(0.0, tau)?,
        true,
    );
    b.face(
        "skin",
        surface,
        [[0.0, tau]; 2],
        vec![
            boundary(
                ring,
                vertex,
                vertex,
                Orientation::Forward,
                uv_line([0.0, 0.0], [1.0, 0.0]),
            ),
            boundary(
                tube,
                vertex,
                vertex,
                Orientation::Forward,
                uv_line([tau, 0.0], [0.0, 1.0]),
            ),
            boundary(
                ring,
                vertex,
                vertex,
                Orientation::Reverse,
                uv_line([0.0, tau], [1.0, 0.0]),
            ),
            boundary(
                tube,
                vertex,
                vertex,
                Orientation::Reverse,
                uv_line([0.0, 0.0], [0.0, 1.0]),
            ),
        ],
    )?;
    b.finish_solid()
}
