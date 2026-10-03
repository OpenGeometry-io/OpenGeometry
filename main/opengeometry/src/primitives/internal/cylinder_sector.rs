use crate::brep::{
    boundary, dimensions, plane_boundary, uv_line, Accuracy, BrepEnvelope, Builder, CurveGeometry,
    Frame3, GeometryError, Orientation, SurfaceGeometry,
};
use crate::math::{scale, Interval, Point3};
use crate::primitives::cylinder::cylinder;
use Orientation::{Forward as F, Reverse as R};

struct SectorTopology {
    bottom_center: u32,
    bottom_start: u32,
    bottom_end: u32,
    top_center: u32,
    top_start: u32,
    top_end: u32,
    bottom_start_edge: u32,
    bottom_end_edge: u32,
    top_start_edge: u32,
    top_end_edge: u32,
    center_edge: u32,
    start_edge: u32,
    end_edge: u32,
    bottom_arc: u32,
    top_arc: u32,
    top_frame: Frame3,
}

pub fn cylinder_sector(
    id: String,
    frame: Frame3,
    radius: f64,
    height: f64,
    start_angle: f64,
    sweep_angle: f64,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    frame.validate()?;
    accuracy.validate()?;
    dimensions(&[radius, height])?;
    if !start_angle.is_finite()
        || !sweep_angle.is_finite()
        || sweep_angle == 0.0
        || sweep_angle.abs() > std::f64::consts::TAU
    {
        return Err(GeometryError::InvalidGeometry(
            "cylinder-sector angles require a finite, nonzero sweep of at most one turn".into(),
        ));
    }
    if (sweep_angle.abs() - std::f64::consts::TAU).abs() <= 64.0 * f64::EPSILON {
        return cylinder(id, frame, radius, height, accuracy);
    }
    if radius * sweep_angle.abs() <= 4.0 * accuracy.geometric
        || radius <= 4.0 * accuracy.geometric
        || height <= 4.0 * accuracy.geometric
    {
        return Err(GeometryError::InvalidGeometry(
            "cylinder-sector features are below geometric resolution".into(),
        ));
    }

    let end_angle = start_angle + sweep_angle;
    let lo = start_angle.min(end_angle);
    let hi = start_angle.max(end_angle);
    let range = Interval::new(lo, hi)?;
    let mut b = Builder::new(id, accuracy)?;
    let topology = add_sector_topology(&mut b, frame, radius, height, lo, hi, range)?;
    add_lateral_face(&mut b, &topology, frame, radius, height, lo, hi)?;
    add_sector_caps(&mut b, &topology, frame, radius)?;
    add_radial_faces(&mut b, &topology, frame, radius, height, lo, hi)?;
    b.finish_solid()
}

fn add_sector_topology(
    b: &mut Builder,
    frame: Frame3,
    radius: f64,
    height: f64,
    lo: f64,
    hi: f64,
    range: Interval,
) -> Result<SectorTopology, GeometryError> {
    let point = |angle: f64, elevation: f64| {
        frame.point([radius * angle.cos(), radius * angle.sin(), elevation])
    };
    let bottom_center = b.vertex(frame.origin);
    let bottom_start = b.vertex(point(lo, 0.0));
    let bottom_end = b.vertex(point(hi, 0.0));
    let top_center = b.vertex(frame.point([0.0, 0.0, height]));
    let top_start = b.vertex(point(lo, height));
    let top_end = b.vertex(point(hi, height));

    let line_edge = |builder: &mut Builder,
                     origin: Point3,
                     direction: Point3,
                     length: f64|
     -> Result<u32, GeometryError> {
        Ok(builder.edge(
            CurveGeometry::Line { origin, direction },
            Interval::new(0.0, length)?,
            false,
        ))
    };
    let bottom_start_edge = line_edge(b, frame.origin, radial(frame, lo), radius)?;
    let bottom_end_edge = line_edge(b, frame.origin, radial(frame, hi), radius)?;
    let top_origin = frame.point([0.0, 0.0, height]);
    let top_start_edge = line_edge(b, top_origin, radial(frame, lo), radius)?;
    let top_end_edge = line_edge(b, top_origin, radial(frame, hi), radius)?;
    let center_edge = line_edge(b, frame.origin, frame.z, height)?;
    let start_edge = line_edge(b, point(lo, 0.0), frame.z, height)?;
    let end_edge = line_edge(b, point(hi, 0.0), frame.z, height)?;
    let bottom_arc = b.edge(CurveGeometry::Circle { frame, radius }, range, false);
    let mut top_frame = frame;
    top_frame.origin = top_origin;
    let top_arc = b.edge(
        CurveGeometry::Circle {
            frame: top_frame,
            radius,
        },
        range,
        false,
    );
    Ok(SectorTopology {
        bottom_center,
        bottom_start,
        bottom_end,
        top_center,
        top_start,
        top_end,
        bottom_start_edge,
        bottom_end_edge,
        top_start_edge,
        top_end_edge,
        center_edge,
        start_edge,
        end_edge,
        bottom_arc,
        top_arc,
        top_frame,
    })
}

fn radial(frame: Frame3, angle: f64) -> Point3 {
    frame.vector([angle.cos(), angle.sin(), 0.0])
}

fn add_lateral_face(
    b: &mut Builder,
    topology: &SectorTopology,
    frame: Frame3,
    radius: f64,
    height: f64,
    lo: f64,
    hi: f64,
) -> Result<(), GeometryError> {
    let &SectorTopology {
        bottom_start,
        bottom_end,
        top_start,
        top_end,
        start_edge,
        end_edge,
        bottom_arc,
        top_arc,
        ..
    } = topology;
    b.face(
        "lateral",
        SurfaceGeometry::Cylinder { frame, radius },
        [[lo, hi], [0.0, height]],
        vec![
            boundary(
                bottom_arc,
                bottom_start,
                bottom_end,
                F,
                uv_line([0.0, 0.0], [1.0, 0.0]),
            ),
            boundary(
                end_edge,
                bottom_end,
                top_end,
                F,
                uv_line([hi, 0.0], [0.0, 1.0]),
            ),
            boundary(
                top_arc,
                top_end,
                top_start,
                R,
                uv_line([0.0, height], [1.0, 0.0]),
            ),
            boundary(
                start_edge,
                top_start,
                bottom_start,
                R,
                uv_line([lo, 0.0], [0.0, 1.0]),
            ),
        ],
    )
}

fn add_sector_caps(
    b: &mut Builder,
    topology: &SectorTopology,
    frame: Frame3,
    radius: f64,
) -> Result<(), GeometryError> {
    let &SectorTopology {
        bottom_center,
        bottom_start,
        bottom_end,
        top_center,
        top_start,
        top_end,
        bottom_start_edge,
        bottom_end_edge,
        top_start_edge,
        top_end_edge,
        bottom_arc,
        top_arc,
        top_frame,
        ..
    } = topology;
    let bottom_frame = Frame3 {
        origin: frame.origin,
        x: frame.x,
        y: scale(frame.y, -1.0),
        z: scale(frame.z, -1.0),
    };
    let bottom_uses = [
        (bottom_end_edge, bottom_center, bottom_end, F),
        (bottom_arc, bottom_end, bottom_start, R),
        (bottom_start_edge, bottom_start, bottom_center, R),
    ]
    .into_iter()
    .map(|(edge, from, to, sense)| plane_boundary(b, bottom_frame, edge, from, to, sense))
    .collect::<Result<Vec<_>, _>>()?;
    b.face(
        "lower_cap",
        SurfaceGeometry::Plane {
            frame: bottom_frame,
        },
        [[-radius, radius]; 2],
        bottom_uses,
    )?;

    let top_uses = [
        (top_start_edge, top_center, top_start, F),
        (top_arc, top_start, top_end, F),
        (top_end_edge, top_end, top_center, R),
    ]
    .into_iter()
    .map(|(edge, from, to, sense)| plane_boundary(b, top_frame, edge, from, to, sense))
    .collect::<Result<Vec<_>, _>>()?;
    b.face(
        "upper_cap",
        SurfaceGeometry::Plane { frame: top_frame },
        [[-radius, radius]; 2],
        top_uses,
    )
}

fn add_radial_faces(
    b: &mut Builder,
    topology: &SectorTopology,
    frame: Frame3,
    radius: f64,
    height: f64,
    lo: f64,
    hi: f64,
) -> Result<(), GeometryError> {
    let &SectorTopology {
        bottom_center,
        bottom_start,
        bottom_end,
        top_center,
        top_start,
        top_end,
        bottom_start_edge,
        bottom_end_edge,
        top_start_edge,
        top_end_edge,
        center_edge,
        start_edge,
        end_edge,
        ..
    } = topology;
    let start_frame = Frame3 {
        origin: frame.origin,
        x: radial(frame, lo),
        y: frame.z,
        z: scale(tangent(frame, lo), -1.0),
    };
    let start_uses = [
        (bottom_start_edge, bottom_center, bottom_start, F),
        (start_edge, bottom_start, top_start, F),
        (top_start_edge, top_start, top_center, R),
        (center_edge, top_center, bottom_center, R),
    ]
    .into_iter()
    .map(|(edge, from, to, sense)| plane_boundary(b, start_frame, edge, from, to, sense))
    .collect::<Result<Vec<_>, _>>()?;
    b.face(
        "start_radial",
        SurfaceGeometry::Plane { frame: start_frame },
        [[0.0, radius], [0.0, height]],
        start_uses,
    )?;

    let end_frame = Frame3 {
        origin: frame.origin,
        x: frame.z,
        y: radial(frame, hi),
        z: tangent(frame, hi),
    };
    let end_uses = [
        (center_edge, bottom_center, top_center, F),
        (top_end_edge, top_center, top_end, F),
        (end_edge, top_end, bottom_end, R),
        (bottom_end_edge, bottom_end, bottom_center, R),
    ]
    .into_iter()
    .map(|(edge, from, to, sense)| plane_boundary(b, end_frame, edge, from, to, sense))
    .collect::<Result<Vec<_>, _>>()?;
    b.face(
        "end_radial",
        SurfaceGeometry::Plane { frame: end_frame },
        [[0.0, height], [0.0, radius]],
        end_uses,
    )
}

fn tangent(frame: Frame3, angle: f64) -> Point3 {
    frame.vector([-angle.sin(), angle.cos(), 0.0])
}
