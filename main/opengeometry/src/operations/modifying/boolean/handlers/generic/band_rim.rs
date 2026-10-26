use crate::brep::{
    BrepEnvelope, CurveGeometry, EdgeGeometry, GeometryError, GeometryStore,
    IntersectionDefinition, SurfaceGeometry, TraceAnchor,
};
use crate::math::{cross, dot, norm, sub, Interval, Point3};

pub(super) fn touching_faces(host: &BrepEnvelope, cutter: &BrepEnvelope, reach: f64) -> Vec<u32> {
    let planes = |brep: &BrepEnvelope| {
        brep.topology
            .faces
            .iter()
            .filter_map(|face| match brep.geometry.surfaces[face.surface as usize] {
                SurfaceGeometry::Plane { frame } => Some((face.id, frame)),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let caps = planes(host);
    planes(cutter)
        .into_iter()
        .filter(|(_, frame)| {
            caps.iter().any(|(_, cap)| {
                norm(cross(cap.z, frame.z)) <= 1e-10
                    && dot(sub(frame.origin, cap.origin), cap.z).abs() <= reach
            })
        })
        .map(|(face, _)| face)
        .collect()
}

pub(super) fn rim_edge(
    host: &BrepEnvelope,
    face: u32,
    point: Point3,
    reach: f64,
) -> Result<Option<(u32, &CurveGeometry, f64, u32)>, GeometryError> {
    let outer = host.topology.faces[face as usize].trim.outer;
    for halfedge in &host.topology.halfedges {
        let edge = &host.topology.edges[halfedge.edge as usize];
        let (true, false, &EdgeGeometry::Curve { curve, range }, Some(twin)) = (
            halfedge.loop_ref == Some(outer),
            edge.chart_seam,
            &edge.geometry,
            halfedge.twin,
        ) else {
            continue;
        };
        let geometry = &host.geometry.curves[curve as usize];
        let Some(parameter) = rim_parameter(geometry, point, range) else {
            continue;
        };
        let other = host.topology.halfedges[twin as usize].face;
        if norm(sub(geometry.elementary_point(parameter)?, point)) <= reach && other != Some(face) {
            return Ok(other.map(|other| (edge.id, geometry, parameter, other)));
        }
    }
    Ok(None)
}

pub(super) fn rim_parameter(curve: &CurveGeometry, point: Point3, range: Interval) -> Option<f64> {
    let parameter = match curve {
        CurveGeometry::Line { origin, direction } => dot(sub(point, *origin), *direction),
        CurveGeometry::Circle { frame, .. } => {
            let local = frame.local(point);
            range.lo + (local[1].atan2(local[0]) - range.lo).rem_euclid(std::f64::consts::TAU)
        }
        _ => return None,
    };
    (range.lo < parameter && parameter < range.hi).then_some(parameter)
}

pub(super) fn trimmed_definition(
    store: &GeometryStore,
    definition: u32,
    lo: f64,
    hi: f64,
) -> Result<Option<IntersectionDefinition>, GeometryError> {
    let source = &store.intersections[definition as usize];
    let anchor = |parameter: f64| -> Result<TraceAnchor, GeometryError> {
        let point = source.evaluate(parameter, store)?;
        Ok(TraceAnchor {
            parameter,
            point: point.point,
            uv_a: point.uv_a,
            uv_b: point.uv_b,
        })
    };
    let first = source
        .anchors
        .partition_point(|candidate| candidate.parameter <= lo)
        .saturating_sub(1);
    let last = source
        .anchors
        .partition_point(|candidate| candidate.parameter < hi);
    if last <= first || last >= source.anchors.len() {
        return Ok(None);
    }
    let mut anchors = vec![anchor(lo)?];
    anchors.extend(source.anchors[first + 1..last].iter().cloned());
    anchors.push(anchor(hi)?);
    Ok(Some(IntersectionDefinition {
        surfaces: source.surfaces,
        anchors,
        uv_tubes: source.uv_tubes[first..last].to_vec(),
        residual_tolerance: source.residual_tolerance,
    }))
}
