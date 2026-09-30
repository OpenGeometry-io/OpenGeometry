use crate::brep::{
    boundary, dimensions, unit, uv_line, Accuracy, BrepEnvelope, Builder, CurveGeometry,
    EdgeGeometry, Frame3, GeometryError, Orientation, PcurveGeometry, SurfaceGeometry, Use,
};
use crate::math::{norm, scale, sub, Interval};

pub(super) struct SectionExtent {
    pub(super) r0: f64,
    pub(super) r1: f64,
    pub(super) z0: f64,
    pub(super) z1: f64,
}

struct SectionTopology {
    low: u32,
    high: u32,
    bottom: u32,
    top: u32,
    seam: u32,
}

pub(super) fn section(
    id: String,
    frame: Frame3,
    surface: SurfaceGeometry,
    extent: &SectionExtent,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    let SectionExtent { r0, r1, z0, z1 } = *extent;
    frame.validate()?;
    surface.validate()?;
    dimensions(&[r1, z1 - z0])?;
    if !r0.is_finite() || r0 < 0.0 {
        return Err(GeometryError::InvalidGeometry(
            "negative lower radius".into(),
        ));
    }
    let mut b = Builder::new(id, accuracy)?;
    let low = b.vertex(frame.point([r0, 0.0, z0]));
    let high = b.vertex(frame.point([r1, 0.0, z1]));
    let tau = std::f64::consts::TAU;
    let mut f0 = frame;
    f0.origin = frame.point([0.0, 0.0, z0]);
    let mut f1 = frame;
    f1.origin = frame.point([0.0, 0.0, z1]);
    let bottom = if r0 == 0.0 {
        b.edge_geometry(EdgeGeometry::Collapsed { vertex: low }, true)
    } else {
        b.edge(
            CurveGeometry::Circle {
                frame: f0,
                radius: r0,
            },
            Interval::new(0.0, tau)?,
            false,
        )
    };
    let top = b.edge(
        CurveGeometry::Circle {
            frame: f1,
            radius: r1,
        },
        Interval::new(0.0, tau)?,
        false,
    );
    let p0 = frame.point([r0, 0.0, z0]);
    let p1 = frame.point([r1, 0.0, z1]);
    let length = norm(sub(p1, p0));
    let seam = b.edge(
        CurveGeometry::Line {
            origin: p0,
            direction: unit(sub(p1, p0))?,
        },
        Interval::new(0.0, length)?,
        true,
    );
    let topology = SectionTopology {
        low,
        high,
        bottom,
        top,
        seam,
    };
    b.face(
        "lateral",
        surface,
        [[0.0, tau], [z0, z1]],
        lateral_uses(&topology, r0, z0, z1, length),
    )?;
    if r0 > 0.0 {
        add_lower_cap(&mut b, f0, bottom, low, r0)?;
    }
    add_upper_cap(&mut b, f1, top, high, r1)?;
    b.finish_solid()
}

fn lateral_uses(topology: &SectionTopology, r0: f64, z0: f64, z1: f64, length: f64) -> Vec<Use> {
    let tau = std::f64::consts::TAU;
    let &SectionTopology {
        low,
        high,
        bottom,
        top,
        seam,
    } = topology;
    vec![
        boundary(
            bottom,
            low,
            low,
            Orientation::Forward,
            if r0 == 0.0 {
                uv_line([0.0, z0], [tau, 0.0])
            } else {
                uv_line([0.0, z0], [1.0, 0.0])
            },
        ),
        boundary(
            seam,
            low,
            high,
            Orientation::Forward,
            uv_line([tau, z0], [0.0, (z1 - z0) / length]),
        ),
        boundary(
            top,
            high,
            high,
            Orientation::Reverse,
            uv_line([0.0, z1], [1.0, 0.0]),
        ),
        boundary(
            seam,
            high,
            low,
            Orientation::Reverse,
            uv_line([0.0, z0], [0.0, (z1 - z0) / length]),
        ),
    ]
}

fn add_lower_cap(
    b: &mut Builder,
    mut f0: Frame3,
    bottom: u32,
    low: u32,
    r0: f64,
) -> Result<(), GeometryError> {
    f0.y = scale(f0.y, -1.0);
    f0.z = scale(f0.z, -1.0);
    b.face(
        "lower_cap",
        SurfaceGeometry::Plane { frame: f0 },
        [[-r0, r0]; 2],
        vec![boundary(
            bottom,
            low,
            low,
            Orientation::Reverse,
            PcurveGeometry::Conic2 {
                origin: [0.0; 2],
                axis_a: [r0, 0.0],
                axis_b: [0.0, -r0],
            },
        )],
    )
}

fn add_upper_cap(
    b: &mut Builder,
    f1: Frame3,
    top: u32,
    high: u32,
    r1: f64,
) -> Result<(), GeometryError> {
    b.face(
        "upper_cap",
        SurfaceGeometry::Plane { frame: f1 },
        [[-r1, r1]; 2],
        vec![boundary(
            top,
            high,
            high,
            Orientation::Forward,
            PcurveGeometry::Conic2 {
                origin: [0.0; 2],
                axis_a: [r1, 0.0],
                axis_b: [0.0, r1],
            },
        )],
    )
}
