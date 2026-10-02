use super::boundaries::NumericalCylinderBoundary;
use crate::brep::{
    boundary, uv_line, Builder, CurveGeometry, Frame3, GeometryError, IntersectionSide,
    Orientation, PcurveGeometry, SurfaceGeometry,
};
use crate::math::{scale, Interval};
use crate::operations::modifying::boolean::operands::CylinderInput;

pub(super) struct HostRims {
    low: u32,
    high: u32,
    bottom: u32,
    top_frame: Frame3,
    top: u32,
    host_seam: u32,
}

pub(super) fn add_host_rims(
    builder: &mut Builder,
    host: &CylinderInput<'_>,
    host_frame: Frame3,
) -> Result<HostRims, GeometryError> {
    let low = builder.vertex(host_frame.point([host.radius, 0.0, 0.0]));
    let high = builder.vertex(host_frame.point([host.radius, 0.0, host.height]));
    let tau = std::f64::consts::TAU;
    let bottom = builder.edge(
        CurveGeometry::Circle {
            frame: host_frame,
            radius: host.radius,
        },
        Interval::new(0.0, tau)?,
        false,
    );
    let mut top_frame = host_frame;
    top_frame.origin = host_frame.point([0.0, 0.0, host.height]);
    let top = builder.edge(
        CurveGeometry::Circle {
            frame: top_frame,
            radius: host.radius,
        },
        Interval::new(0.0, tau)?,
        false,
    );
    let host_seam = builder.edge(
        CurveGeometry::Line {
            origin: host_frame.point([host.radius, 0.0, 0.0]),
            direction: host_frame.z,
        },
        Interval::new(0.0, host.height)?,
        true,
    );
    Ok(HostRims {
        low,
        high,
        bottom,
        top_frame,
        top,
        host_seam,
    })
}

pub(super) fn add_host_lateral_face(
    builder: &mut Builder,
    host: &CylinderInput<'_>,
    host_frame: Frame3,
    rims: &HostRims,
    numerical: &[NumericalCylinderBoundary],
) -> Result<(), GeometryError> {
    let tau = std::f64::consts::TAU;
    let host_holes = numerical
        .iter()
        .map(|numeric_boundary| {
            vec![boundary(
                numeric_boundary.edge,
                numeric_boundary.vertex,
                numeric_boundary.vertex,
                Orientation::Forward,
                PcurveGeometry::IntersectionSide {
                    definition: numeric_boundary.definition,
                    side: IntersectionSide::A,
                },
            )]
        })
        .collect();
    builder.face_with_holes(
        "host:lateral",
        SurfaceGeometry::Cylinder {
            frame: host_frame,
            radius: host.radius,
        },
        [[0.0, tau], [0.0, host.height]],
        vec![
            boundary(
                rims.bottom,
                rims.low,
                rims.low,
                Orientation::Forward,
                uv_line([0.0, 0.0], [1.0, 0.0]),
            ),
            boundary(
                rims.host_seam,
                rims.low,
                rims.high,
                Orientation::Forward,
                uv_line([tau, 0.0], [0.0, 1.0]),
            ),
            boundary(
                rims.top,
                rims.high,
                rims.high,
                Orientation::Reverse,
                uv_line([0.0, host.height], [1.0, 0.0]),
            ),
            boundary(
                rims.host_seam,
                rims.high,
                rims.low,
                Orientation::Reverse,
                uv_line([0.0, 0.0], [0.0, 1.0]),
            ),
        ],
        host_holes,
    )
}

pub(super) fn add_host_caps(
    builder: &mut Builder,
    host: &CylinderInput<'_>,
    host_frame: Frame3,
    rims: &HostRims,
) -> Result<(), GeometryError> {
    let mut lower_cap_frame = host_frame;
    lower_cap_frame.y = scale(lower_cap_frame.y, -1.0);
    lower_cap_frame.z = scale(lower_cap_frame.z, -1.0);
    builder.face(
        "host:lower_cap",
        SurfaceGeometry::Plane {
            frame: lower_cap_frame,
        },
        [[-host.radius, host.radius]; 2],
        vec![boundary(
            rims.bottom,
            rims.low,
            rims.low,
            Orientation::Reverse,
            PcurveGeometry::Conic2 {
                origin: [0.0; 2],
                axis_a: [host.radius, 0.0],
                axis_b: [0.0, -host.radius],
            },
        )],
    )?;
    builder.face(
        "host:upper_cap",
        SurfaceGeometry::Plane {
            frame: rims.top_frame,
        },
        [[-host.radius, host.radius]; 2],
        vec![boundary(
            rims.top,
            rims.high,
            rims.high,
            Orientation::Forward,
            PcurveGeometry::Conic2 {
                origin: [0.0; 2],
                axis_a: [host.radius, 0.0],
                axis_b: [0.0, host.radius],
            },
        )],
    )
}
