use super::frame::BendFrame;
use crate::brep::{
    boundary, uv_line, Builder, CurveGeometry, Frame3, GeometryError, Orientation, SurfaceGeometry,
};
use crate::math::{add, dot, scale, Interval, Point3};
use crate::operations::OperationError;
use std::f64::consts::PI;

#[derive(Clone, Copy)]
pub(crate) struct CircleEnds {
    pub(crate) vertices: [u32; 2],
    pub(crate) edges: [u32; 2],
}

pub(crate) struct CircleBend {
    pub(crate) frame: BendFrame,
    pub(crate) segment: usize,
    pub(crate) centre: Point3,
    pub(crate) x: Point3,
    pub(crate) radius: f64,
    pub(crate) start: CircleEnds,
    pub(crate) end: CircleEnds,
}

struct Tube {
    frame: Frame3,
    major: f64,
    seam_angle: f64,
}

pub(crate) fn add_torus_bend(
    builder: &mut Builder,
    bend: &CircleBend,
) -> Result<[u32; 2], OperationError> {
    let frame = bend.frame;
    let [major, height] = frame.section(bend.centre);
    let tube = Tube {
        frame: Frame3::from_axis(
            add(frame.centre, scale(frame.axis, height)),
            frame.axis,
            frame.radial,
        )?,
        major,
        seam_angle: dot(bend.x, frame.axis).atan2(dot(bend.x, frame.radial)),
    };
    let seams = [
        seam_arc(builder, bend, &tube, tube.seam_angle)?,
        seam_arc(builder, bend, &tube, tube.seam_angle - PI)?,
    ];
    for half in 0..2 {
        add_half(builder, bend, &tube, seams, half)?;
    }
    Ok(seams)
}

fn seam_arc(
    builder: &mut Builder,
    bend: &CircleBend,
    tube: &Tube,
    angle: f64,
) -> Result<u32, OperationError> {
    let (sin, cos) = angle.sin_cos();
    let circle = Frame3::from_axis(
        add(tube.frame.origin, scale(tube.frame.z, bend.radius * sin)),
        tube.frame.z,
        tube.frame.x,
    )?;
    Ok(builder.edge(
        CurveGeometry::Circle {
            frame: circle,
            radius: tube.major + bend.radius * cos,
        },
        Interval::new(0.0, bend.frame.angle).map_err(GeometryError::from)?,
        false,
    ))
}

fn add_half(
    builder: &mut Builder,
    bend: &CircleBend,
    tube: &Tube,
    seams: [u32; 2],
    half: usize,
) -> Result<(), OperationError> {
    let next = (half + 1) % 2;
    let angle = bend.frame.angle;
    let high = tube.seam_angle - half as f64 * PI;
    let low = tube.seam_angle - (half + 1) as f64 * PI;
    let (start, end) = (bend.start, bend.end);
    let uses = vec![
        boundary(
            start.edges[half],
            start.vertices[half],
            start.vertices[next],
            Orientation::Forward,
            uv_line([0.0, tube.seam_angle], [0.0, -1.0]),
        ),
        boundary(
            seams[next],
            start.vertices[next],
            end.vertices[next],
            Orientation::Forward,
            uv_line([0.0, low], [1.0, 0.0]),
        ),
        boundary(
            end.edges[half],
            end.vertices[next],
            end.vertices[half],
            Orientation::Reverse,
            uv_line([angle, tube.seam_angle], [0.0, -1.0]),
        ),
        boundary(
            seams[half],
            end.vertices[half],
            start.vertices[half],
            Orientation::Reverse,
            uv_line([0.0, high], [1.0, 0.0]),
        ),
    ];
    builder.face(
        &format!("seg-{}:side-0-{half}", bend.segment),
        SurfaceGeometry::Torus {
            frame: tube.frame,
            major_radius: tube.major,
            minor_radius: bend.radius,
        },
        [[0.0, angle], [low, high]],
        uses,
    )?;
    Ok(())
}
