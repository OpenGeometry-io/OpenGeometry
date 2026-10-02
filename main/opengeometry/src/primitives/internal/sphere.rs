use crate::brep::{
    boundary, dimensions, uv_line, Accuracy, BrepEnvelope, Builder, CurveGeometry, EdgeGeometry,
    Frame3, GeometryError, Orientation, SurfaceGeometry,
};
use crate::math::{scale, Interval};

pub fn sphere(
    id: String,
    frame: Frame3,
    radius: f64,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    frame.validate()?;
    dimensions(&[radius])?;
    let mut b = Builder::new(id, accuracy)?;
    let pi2 = std::f64::consts::FRAC_PI_2;
    let tau = std::f64::consts::TAU;
    let south = b.vertex(frame.point([0.0, 0.0, -radius]));
    let north = b.vertex(frame.point([0.0, 0.0, radius]));
    let bottom = b.edge_geometry(EdgeGeometry::Collapsed { vertex: south }, true);
    let top = b.edge_geometry(EdgeGeometry::Collapsed { vertex: north }, true);
    let meridian_frame = Frame3 {
        origin: frame.origin,
        x: frame.x,
        y: frame.z,
        z: scale(frame.y, -1.0),
    };
    let seam = b.edge(
        CurveGeometry::Circle {
            frame: meridian_frame,
            radius,
        },
        Interval::new(-pi2, pi2)?,
        true,
    );
    b.face(
        "skin",
        SurfaceGeometry::Sphere { frame, radius },
        [[0.0, tau], [-pi2, pi2]],
        vec![
            boundary(
                bottom,
                south,
                south,
                Orientation::Forward,
                uv_line([0.0, -pi2], [tau, 0.0]),
            ),
            boundary(
                seam,
                south,
                north,
                Orientation::Forward,
                uv_line([tau, 0.0], [0.0, 1.0]),
            ),
            boundary(
                top,
                north,
                north,
                Orientation::Forward,
                uv_line([tau, pi2], [-tau, 0.0]),
            ),
            boundary(
                seam,
                north,
                south,
                Orientation::Reverse,
                uv_line([0.0, 0.0], [0.0, 1.0]),
            ),
        ],
    )?;
    b.finish_solid()
}
