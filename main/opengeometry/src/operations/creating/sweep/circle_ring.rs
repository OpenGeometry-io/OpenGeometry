use super::path::Course;
use crate::brep::{unit, Builder, CurveGeometry, Frame3, GeometryError};
use crate::math::{add, cross, dot, scale, sub, Interval, Point3};
use crate::operations::OperationError;

#[derive(Clone, Copy)]
pub(super) struct Ring {
    pub(super) centre: Point3,
    pub(super) frame: Frame3,
    pub(super) major: f64,
    pub(super) minor: f64,
    pub(super) vertices: [u32; 2],
    pub(super) edges: [u32; 2],
    pub(super) perpendicular: bool,
}

fn ring(
    builder: &mut Builder,
    centre: Point3,
    axis: Point3,
    radial: Point3,
    major_axis: Point3,
    major: f64,
    minor: f64,
) -> Result<Ring, OperationError> {
    let y_axis = unit(cross(axis, major_axis))?;
    let frame = Frame3::from_axis(centre, axis, major_axis)?;
    let start = (dot(radial, y_axis) / minor).atan2(dot(radial, major_axis) / major);
    let curve = if (major - minor).abs() <= 1e-14 * major {
        CurveGeometry::Circle {
            frame,
            radius: major,
        }
    } else {
        CurveGeometry::Ellipse {
            frame,
            major_radius: major,
            minor_radius: minor,
        }
    };
    let vertices = [
        builder.vertex(curve.elementary_point(start)?),
        builder.vertex(curve.elementary_point(start + std::f64::consts::PI)?),
    ];
    let edges = [
        builder.edge(
            curve.clone(),
            Interval::new(start, start + std::f64::consts::PI).map_err(GeometryError::from)?,
            false,
        ),
        builder.edge(
            curve,
            Interval::new(start + std::f64::consts::PI, start + std::f64::consts::TAU)
                .map_err(GeometryError::from)?,
            false,
        ),
    ];
    Ok(Ring {
        centre,
        frame,
        major,
        minor,
        vertices,
        edges,
        perpendicular: false,
    })
}

pub(super) fn section(
    builder: &mut Builder,
    centre: Point3,
    axis: Point3,
    x: Point3,
    radius: f64,
) -> Result<Ring, OperationError> {
    let ring = ring(builder, centre, axis, scale(x, radius), x, radius, radius)?;
    Ok(Ring {
        perpendicular: true,
        ..ring
    })
}

pub(super) fn mitre_ring(
    builder: &mut Builder,
    point: Point3,
    offset: Point3,
    axis: Point3,
    bisector: Point3,
    x: Point3,
    radius: f64,
) -> Result<Ring, OperationError> {
    let denominator = dot(axis, bisector);
    let centre = add(
        add(point, offset),
        scale(axis, -dot(offset, bisector) / denominator),
    );
    let projection = |vector: Point3| sub(vector, scale(axis, dot(vector, bisector) / denominator));
    let radial = scale(projection(x), radius);
    let major_axis = unit(sub(axis, scale(bisector, denominator)))?;
    ring(
        builder,
        centre,
        bisector,
        radial,
        major_axis,
        radius / denominator,
        radius,
    )
}

pub(super) fn turned(course: &Course, segment: usize, vectors: [Point3; 2]) -> [Point3; 2] {
    match course.bends[segment] {
        Some(frame) => vectors.map(|vector| frame.rotated(vector)),
        None => vectors,
    }
}

pub(super) fn reflection(vector: Point3, normal: Point3) -> Point3 {
    sub(vector, scale(normal, 2.0 * dot(vector, normal)))
}
