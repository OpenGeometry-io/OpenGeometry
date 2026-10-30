use crate::brep::{
    BrepEnvelope, CurveGeometry, GeometryError, GeometryStore, IntersectionSide, PcurveGeometry,
    Surface, SurfaceGeometry,
};
use crate::intersection::{IntersectionBranch, IntersectionGraph};
use crate::math::{cross, dot, Interval};

pub(super) fn remap_closed_loop_pcurve(
    pcurve: &PcurveGeometry,
    curve: u32,
    surface: u32,
) -> Result<PcurveGeometry, GeometryError> {
    Ok(match pcurve {
        PcurveGeometry::Line2 { .. } | PcurveGeometry::Conic2 { .. } => pcurve.clone(),
        PcurveGeometry::ProjectedCurve {
            chart,
            uv_hint,
            uv_rate,
            parameter_origin,
            ..
        } => PcurveGeometry::ProjectedCurve {
            curve,
            surface,
            chart: *chart,
            uv_hint: *uv_hint,
            uv_rate: *uv_rate,
            parameter_origin: *parameter_origin,
        },
        PcurveGeometry::IntersectionSide { .. } => {
            return Err(GeometryError::InvalidGeometry(
                "intersection-side pcurve requires a remapped intersection definition".into(),
            ))
        }
    })
}

pub(super) fn exact_closed_circle_pcurve(
    source_geometry: &GeometryStore,
    target_geometry: &GeometryStore,
    source_pcurve: u32,
    circle: &CurveGeometry,
    surface: u32,
    tolerance: f64,
) -> Result<Option<PcurveGeometry>, GeometryError> {
    if !matches!(
        source_geometry.pcurves.get(source_pcurve as usize),
        Some(PcurveGeometry::ProjectedCurve { .. })
    ) {
        return Ok(None);
    }
    let CurveGeometry::Circle {
        frame: circle,
        radius,
    } = circle
    else {
        return Ok(None);
    };
    let support = target_geometry.surface(surface)?;
    let frame = support.frame();
    let centre = frame.local(circle.origin);
    let axis_x = [dot(circle.x, frame.x), dot(circle.x, frame.y)];
    let axis_y = [dot(circle.y, frame.x), dot(circle.y, frame.y)];
    match support {
        SurfaceGeometry::Plane { .. }
            if centre[2].abs() <= tolerance
                && dot(circle.x, frame.z).abs() * radius <= tolerance
                && dot(circle.y, frame.z).abs() * radius <= tolerance =>
        {
            Ok(Some(PcurveGeometry::Conic2 {
                origin: [centre[0], centre[1]],
                axis_a: axis_x.map(|value| value * radius),
                axis_b: axis_y.map(|value| value * radius),
            }))
        }
        SurfaceGeometry::Cylinder {
            radius: support_radius,
            ..
        } if centre[0].hypot(centre[1]) <= tolerance
            && (radius - support_radius).abs() <= tolerance
            && (dot(circle.z, frame.z).abs() - 1.0).abs() <= 1.0e-10 =>
        {
            let rate = dot(cross(circle.x, circle.y), frame.z).signum();
            let phase = axis_x[1].atan2(axis_x[0]);
            let original = source_geometry.pcurve_at(source_pcurve, 0.0)?;
            let lifted_phase = phase
                + ((original[0] - phase) / std::f64::consts::TAU).round() * std::f64::consts::TAU;
            Ok(Some(PcurveGeometry::Line2 {
                origin: [lifted_phase, centre[2]],
                direction: [rate, 0.0],
            }))
        }
        _ => Ok(None),
    }
}

pub(crate) fn pcurve_winding(
    geometry: &GeometryStore,
    surface: u32,
    pcurve: u32,
    range: Interval,
) -> Result<[i32; 2], GeometryError> {
    let chart = &geometry.surface(surface)?.charts()[0];
    let start = geometry.pcurve_at(pcurve, range.lo)?;
    let end = geometry.pcurve_at(pcurve, range.hi)?;
    Ok(std::array::from_fn(|axis| {
        chart.periods[axis].map_or(0, |period| {
            ((end[axis] - start[axis]) / period).round() as i32
        })
    }))
}

pub(crate) fn pcurve_support_surface(
    geometry: &GeometryStore,
    pcurve: u32,
) -> Result<u32, GeometryError> {
    match geometry.pcurves.get(pcurve as usize) {
        Some(PcurveGeometry::ProjectedCurve { surface, .. }) => Ok(*surface),
        Some(PcurveGeometry::IntersectionSide { definition, side }) => {
            let definition = geometry
                .intersections
                .get(*definition as usize)
                .ok_or_else(|| GeometryError::MissingReference {
                    kind: "intersection definition".into(),
                    index: *definition,
                })?;
            Ok(definition.surfaces[if *side == IntersectionSide::A { 0 } else { 1 }])
        }
        Some(_) => Err(GeometryError::InvalidGeometry(
            "periodic pcurve has no support surface reference".into(),
        )),
        None => Err(GeometryError::MissingReference {
            kind: "pcurve".into(),
            index: pcurve,
        }),
    }
}

pub(crate) fn intersection_loop_sample(
    graph: &IntersectionGraph,
    branch: &IntersectionBranch,
    side: usize,
) -> Result<[f64; 2], GeometryError> {
    let subdivisions = 128;
    let mut points = Vec::with_capacity(subdivisions);
    for index in 0..subdivisions {
        let parameter = branch.range.lo + branch.range.width() * index as f64 / subdivisions as f64;
        points.push(graph.geometry.pcurve_at(branch.pcurves[side], parameter)?);
    }
    let mut twice_area = 0.0;
    let mut centroid = [0.0; 2];
    for index in 0..points.len() {
        let a = points[index];
        let b = points[(index + 1) % points.len()];
        let cross = a[0] * b[1] - b[0] * a[1];
        twice_area += cross;
        centroid[0] += (a[0] + b[0]) * cross;
        centroid[1] += (a[1] + b[1]) * cross;
    }
    if twice_area.abs() <= f64::EPSILON {
        return Ok([
            points.iter().map(|point| point[0]).sum::<f64>() / points.len() as f64,
            points.iter().map(|point| point[1]).sum::<f64>() / points.len() as f64,
        ]);
    }
    Ok([
        centroid[0] / (3.0 * twice_area),
        centroid[1] / (3.0 * twice_area),
    ])
}

pub(crate) fn intersection_loop_bounds(
    graph: &IntersectionGraph,
    branch: &IntersectionBranch,
    side: usize,
    tolerance: f64,
) -> Result<[Interval; 2], GeometryError> {
    pcurve_loop_bounds(
        &graph.geometry,
        branch.pcurves[side],
        branch.range,
        tolerance,
    )
}

fn pcurve_loop_bounds(
    geometry: &GeometryStore,
    pcurve: u32,
    range: Interval,
    tolerance: f64,
) -> Result<[Interval; 2], GeometryError> {
    let mut lo = [f64::INFINITY; 2];
    let mut hi = [f64::NEG_INFINITY; 2];
    for index in 0..=256 {
        let parameter = range.lo + range.width() * index as f64 / 256.0;
        let uv = geometry.pcurve_at(pcurve, parameter)?;
        for axis in 0..2 {
            lo[axis] = lo[axis].min(uv[axis]);
            hi[axis] = hi[axis].max(uv[axis]);
        }
    }
    let padding =
        [0, 1].map(|axis| (hi[axis] - lo[axis]).abs().max(tolerance) * 1.0e-6 + tolerance);
    Ok([
        Interval::new(lo[0] - padding[0], hi[0] + padding[0])?,
        Interval::new(lo[1] - padding[1], hi[1] + padding[1])?,
    ])
}

pub(crate) fn topology_loop_bounds(
    brep: &BrepEnvelope,
    loop_id: u32,
    tolerance: f64,
) -> Result<[Interval; 2], GeometryError> {
    let loop_record = brep.topology.loops.get(loop_id as usize).ok_or_else(|| {
        GeometryError::MissingReference {
            kind: "loop".into(),
            index: loop_id,
        }
    })?;
    let halfedge = &brep.topology.halfedges[loop_record.start_halfedge as usize];
    let pcurve = halfedge.geometry_use.pcurve.ok_or_else(|| {
        GeometryError::InvalidTopology("intersection loop is missing its pcurve".into())
    })?;
    pcurve_loop_bounds(
        &brep.geometry,
        pcurve,
        brep.topology.edges[halfedge.edge as usize].geometry.range(),
        tolerance,
    )
}
