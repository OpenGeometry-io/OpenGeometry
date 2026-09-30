use crate::brep::{
    boundary, padded_uv_bounds, plane_boundary, uv_line, Builder, Frame3, GeometryError,
    Orientation, SurfaceGeometry, Use,
};
use crate::math::{norm, sub, Interval, Point3};
use Orientation::{Forward as F, Reverse as R};

#[derive(Clone, Copy)]
pub(super) struct CylinderArc {
    pub(super) frame: Frame3,
    pub(super) radius: f64,
    pub(super) range: Interval,
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) reversed: bool,
}

pub(super) struct ArcFaceBoundary {
    pub(super) bottom_edge: u32,
    pub(super) top_edge: u32,
    pub(super) vertical_edges: [u32; 2],
    pub(super) vertices: [[u32; 2]; 2],
}

pub(super) fn selected_cylinder_arc(
    frame: Frame3,
    radius: f64,
    points: [Point3; 2],
    other_center: Point3,
    other_radius: f64,
    keep_inside: bool,
    reversed: bool,
) -> Result<CylinderArc, GeometryError> {
    let angles = points.map(|point| circle_parameter(frame, point));
    let tau = std::f64::consts::TAU;
    let forward_end = if angles[1] > angles[0] {
        angles[1]
    } else {
        angles[1] + tau
    };
    let candidates = [
        (angles[0], forward_end, 0, 1),
        (angles[1], angles[0] + tau, 1, 0),
    ];
    for (lo, hi, start, end) in candidates {
        let midpoint = frame.point([
            radius * ((lo + hi) * 0.5).cos(),
            radius * ((lo + hi) * 0.5).sin(),
            0.0,
        ]);
        let inside = norm(sub(midpoint, other_center)) < other_radius;
        if inside == keep_inside {
            return Ok(CylinderArc {
                frame,
                radius,
                range: Interval::new(lo, hi)?,
                start,
                end,
                reversed,
            });
        }
    }
    Err(GeometryError::UnresolvedIntersection(
        "could not isolate the selected cylinder boundary arc".into(),
    ))
}

fn circle_parameter(frame: Frame3, point: Point3) -> f64 {
    let local = frame.local(point);
    local[1].atan2(local[0]).rem_euclid(std::f64::consts::TAU)
}

pub(super) fn add_cylinder_arc_face(
    builder: &mut Builder,
    key: &str,
    arc: CylinderArc,
    face_boundary: &ArcFaceBoundary,
    height: f64,
) -> Result<(), GeometryError> {
    let ArcFaceBoundary {
        bottom_edge,
        top_edge,
        vertical_edges,
        vertices,
    } = *face_boundary;
    let uses =
        cylinder_arc_face_uses(arc, bottom_edge, top_edge, vertical_edges, vertices, height)?;
    let face = builder.brep.topology.faces.len();
    builder.face(
        key,
        SurfaceGeometry::Cylinder {
            frame: arc.frame,
            radius: arc.radius,
        },
        [[arc.range.lo, arc.range.hi], [0.0, height]],
        uses,
    )?;
    if arc.reversed {
        builder.brep.topology.faces[face].sense = R;
    }
    Ok(())
}

fn cylinder_arc_face_uses(
    arc: CylinderArc,
    bottom_edge: u32,
    top_edge: u32,
    vertical_edges: [u32; 2],
    vertices: [[u32; 2]; 2],
    height: f64,
) -> Result<Vec<Use>, GeometryError> {
    let bottom = vertices[0];
    let top = vertices[1];
    let start = arc.start;
    let end = arc.end;
    let vertical = |point: usize| -> Result<Use, GeometryError> {
        Ok(boundary(
            vertical_edges[point],
            bottom[point],
            top[point],
            F,
            uv_line([arc_angle_at(arc, point)?, 0.0], [0.0, 1.0]),
        ))
    };
    let vertical_reverse = |point: usize| -> Result<Use, GeometryError> {
        Ok(boundary(
            vertical_edges[point],
            top[point],
            bottom[point],
            R,
            uv_line([arc_angle_at(arc, point)?, 0.0], [0.0, 1.0]),
        ))
    };
    let uses = if arc.reversed {
        vec![
            boundary(
                bottom_edge,
                bottom[end],
                bottom[start],
                R,
                uv_line([0.0; 2], [1.0, 0.0]),
            ),
            vertical(start)?,
            boundary(
                top_edge,
                top[start],
                top[end],
                F,
                uv_line([0.0, height], [1.0, 0.0]),
            ),
            vertical_reverse(end)?,
        ]
    } else {
        vec![
            boundary(
                bottom_edge,
                bottom[start],
                bottom[end],
                F,
                uv_line([0.0; 2], [1.0, 0.0]),
            ),
            vertical(end)?,
            boundary(
                top_edge,
                top[end],
                top[start],
                R,
                uv_line([0.0, height], [1.0, 0.0]),
            ),
            vertical_reverse(start)?,
        ]
    };
    Ok(uses)
}

fn arc_angle_at(arc: CylinderArc, point: usize) -> Result<f64, GeometryError> {
    if point == arc.start {
        Ok(arc.range.lo)
    } else if point == arc.end {
        Ok(arc.range.hi)
    } else {
        Err(GeometryError::InvalidTopology(
            "cylinder arc endpoint is not part of the intersection".into(),
        ))
    }
}

pub(super) fn shifted_cylinder_arc(mut arc: CylinderArc, axis: Point3, offset: f64) -> CylinderArc {
    arc.frame.origin = std::array::from_fn(|i| arc.frame.origin[i] + axis[i] * offset);
    arc
}

pub(super) fn two_arc_plane_uses(
    builder: &Builder,
    frame: Frame3,
    first: (CylinderArc, u32),
    second: (CylinderArc, u32),
    vertices: [u32; 2],
) -> Result<Vec<Use>, GeometryError> {
    for first_sense in [Orientation::Forward, Orientation::Reverse] {
        let (first_from, first_to) = arc_endpoints(first.0, first_sense);
        let second_sense = if second.0.start == first_to && second.0.end == first_from {
            Orientation::Forward
        } else if second.0.end == first_to && second.0.start == first_from {
            Orientation::Reverse
        } else {
            continue;
        };
        let mut samples = Vec::with_capacity(34);
        for (arc, sense) in [(first.0, first_sense), (second.0, second_sense)] {
            for sample in 0..=16 {
                if !samples.is_empty() && sample == 0 {
                    continue;
                }
                let fraction = sample as f64 / 16.0;
                let parameter = if sense == Orientation::Forward {
                    arc.range.lo + arc.range.width() * fraction
                } else {
                    arc.range.hi - arc.range.width() * fraction
                };
                samples.push(frame.local(arc.frame.point([
                    arc.radius * parameter.cos(),
                    arc.radius * parameter.sin(),
                    0.0,
                ])));
            }
        }
        let signed_area = samples
            .iter()
            .zip(samples.iter().cycle().skip(1))
            .take(samples.len())
            .map(|(a, b)| a[0] * b[1] - b[0] * a[1])
            .sum::<f64>()
            * 0.5;
        if signed_area > 0.0 {
            return Ok(vec![
                plane_boundary(
                    builder,
                    frame,
                    first.1,
                    vertices[first_from],
                    vertices[first_to],
                    first_sense,
                )?,
                plane_boundary(
                    builder,
                    frame,
                    second.1,
                    vertices[arc_endpoints(second.0, second_sense).0],
                    vertices[arc_endpoints(second.0, second_sense).1],
                    second_sense,
                )?,
            ]);
        }
    }
    Err(GeometryError::InvalidTopology(
        "two-arc planar boundary does not form a positive loop".into(),
    ))
}

fn arc_endpoints(arc: CylinderArc, sense: Orientation) -> (usize, usize) {
    if sense == Orientation::Forward {
        (arc.start, arc.end)
    } else {
        (arc.end, arc.start)
    }
}

pub(super) fn two_arc_plane_bounds(
    frame: Frame3,
    arcs: [CylinderArc; 2],
    tolerance: f64,
) -> [[f64; 2]; 2] {
    padded_uv_bounds(
        arcs.into_iter()
            .flat_map(|arc| {
                [
                    arc.frame.origin,
                    std::array::from_fn(|i| arc.frame.origin[i] + arc.frame.x[i] * arc.radius),
                    std::array::from_fn(|i| arc.frame.origin[i] - arc.frame.x[i] * arc.radius),
                    std::array::from_fn(|i| arc.frame.origin[i] + arc.frame.y[i] * arc.radius),
                    std::array::from_fn(|i| arc.frame.origin[i] - arc.frame.y[i] * arc.radius),
                ]
            })
            .map(|point| frame.local(point)),
        tolerance,
    )
}
