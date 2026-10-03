use crate::brep::{
    unit, Accuracy, BrepEnvelope, CurveGeometry, Edge, EdgeGeometry, Face, FaceProvenance,
    FaceSource, Frame3, GeometryError, GeometryQuality, GeometryStore, HalfEdge,
    HalfEdgeGeometryUse, IntersectionDefinition, Loop, PatchBounds, PcurveGeometry, Shell,
    SolidRegion, SurfaceGeometry, Topology, TraceAnchor, TrimRegion, Vertex, Wire,
};
use crate::math::{cross, dot, norm, sub, Interval};

pub(super) fn preflight(brep: &BrepEnvelope, scale: f64) -> Result<f64, GeometryError> {
    brep.validate()?;
    if brep.solids.is_empty()
        || !brep.topology.wires.is_empty()
        || brep.topology.shells.iter().any(|shell| !shell.is_closed)
    {
        return Err(GeometryError::UnsupportedGeometry(
            "analytic analytic exchange export requires nonempty closed solid regions without wires".into(),
        ));
    }
    check_item_count(brep)?;
    if brep.id.len() > 4096 || brep.topology.faces.iter().any(|face| face.key.len() > 4096) {
        return Err(GeometryError::LimitExceeded(
            "analytic exchange body and face labels must be at most 4 KiB".into(),
        ));
    }
    let bounds = brep.bounds()?.ok_or_else(|| {
        GeometryError::InvalidTopology("analytic exchange solid has no bounds".into())
    })?;
    let mut magnitude = bounds
        .axes
        .iter()
        .flat_map(|axis| [axis.lo.abs(), axis.hi.abs()])
        .fold(0.0_f64, f64::max);
    let mut frame_bound = 0.0_f64;
    for face in &brep.topology.faces {
        frame_bound = frame_bound.max(frame_error(
            *brep.geometry.surface(face.surface)?.frame(),
            bounds,
        )?);
        magnitude = brep
            .geometry
            .surface(face.surface)?
            .frame()
            .origin
            .iter()
            .fold(magnitude, |m, v| m.max(v.abs()));
    }
    for edge in &brep.topology.edges {
        if let EdgeGeometry::Curve { curve, .. } = edge.geometry {
            let origin = match &brep.geometry.curves[curve as usize] {
                CurveGeometry::Line { origin, .. } => origin,
                CurveGeometry::Circle { frame, .. } | CurveGeometry::Ellipse { frame, .. } => {
                    &frame.origin
                }
                CurveGeometry::Intersection { .. } => continue,
            };
            magnitude = origin.iter().fold(magnitude, |m, v| m.max(v.abs()));
            if let CurveGeometry::Circle { frame, .. } | CurveGeometry::Ellipse { frame, .. } =
                &brep.geometry.curves[curve as usize]
            {
                frame_bound = frame_bound.max(frame_error(*frame, bounds)?);
            }
        }
    }
    let exchange_bound =
        brep.accuracy.geometric + 2.0 * frame_bound + 64.0 * f64::EPSILON * magnitude;
    if !exchange_bound.is_finite()
        || exchange_bound > brep.accuracy.exchange
        || !(exchange_bound * scale).is_finite()
    {
        return Err(GeometryError::LimitExceeded(
            "analytic exchange exchange budget is below geometric tolerance or coordinate precision".into(),
        ));
    }
    if !scalable(brep, scale) {
        return Err(GeometryError::LimitExceeded(
            "analytic exchange unit conversion exceeds floating-point range".into(),
        ));
    }
    Ok(exchange_bound)
}

fn check_item_count(brep: &BrepEnvelope) -> Result<(), GeometryError> {
    if brep.topology.vertices.len()
        + brep.topology.halfedges.len()
        + brep.topology.faces.len()
        + brep.geometry.curves.len()
        + brep.geometry.surfaces.len()
        + brep.geometry.pcurves.len()
        + brep.geometry.intersections.len()
        + brep.topology.edges.len()
        + brep.topology.loops.len()
        + brep.topology.shells.len()
        + brep.solids.len()
        > 100_000
    {
        return Err(GeometryError::LimitExceeded(
            "analytic exchange body exceeds 100,000 geometry/topology items".into(),
        ));
    }
    Ok(())
}

fn frame_error(frame: Frame3, bounds: PatchBounds) -> Result<f64, GeometryError> {
    let z = unit(frame.z)?;
    let x = unit(sub(frame.x, z.map(|v| v * dot(frame.x, z))))?;
    let y = cross(z, x);
    let deviation = norm(sub(x, frame.x))
        .max(norm(sub(y, frame.y)))
        .max(norm(sub(z, frame.z)));
    if deviation == 0.0 {
        return Ok(0.0);
    }
    let distance = norm(std::array::from_fn(|i| {
        (bounds.axes[i].lo - frame.origin[i])
            .abs()
            .max((bounds.axes[i].hi - frame.origin[i]).abs())
    }));
    Ok(4.0 * distance * deviation)
}

fn scalable(brep: &BrepEnvelope, scale: f64) -> bool {
    brep_numbers(brep)
        .into_iter()
        .filter(|value| value.is_finite())
        .all(|value| (value * scale).is_finite())
}

fn brep_numbers(brep: &BrepEnvelope) -> Vec<f64> {
    let BrepEnvelope {
        schema_version,
        id: _,
        revision,
        geometry,
        topology,
        solids,
        accuracy,
        quality,
    } = brep;
    let Accuracy {
        geometric,
        intersection,
        tessellation,
        exchange,
    } = accuracy;
    let mut numbers = vec![f64::from(*schema_version), *revision as f64];
    numbers.extend([geometric, intersection, tessellation, exchange]);
    match quality {
        GeometryQuality::Analytic => {}
        GeometryQuality::Approximate { max_error } => numbers.push(*max_error),
    }
    geometry_numbers(geometry, &mut numbers);
    topology_numbers(topology, &mut numbers);
    for SolidRegion {
        outer_shell,
        cavity_shells,
    } in solids
    {
        index_numbers([outer_shell].into_iter().chain(cavity_shells), &mut numbers);
    }
    numbers
}

fn geometry_numbers(geometry: &GeometryStore, numbers: &mut Vec<f64>) {
    let GeometryStore {
        surfaces,
        curves,
        pcurves,
        intersections,
    } = geometry;
    for surface in surfaces {
        surface_numbers(surface, numbers);
    }
    for curve in curves {
        curve_numbers(curve, numbers);
    }
    for pcurve in pcurves {
        pcurve_numbers(pcurve, numbers);
    }
    for IntersectionDefinition {
        surfaces: pair,
        anchors,
        uv_tubes,
        residual_tolerance,
    } in intersections
    {
        index_numbers(pair, numbers);
        for TraceAnchor {
            parameter,
            point,
            uv_a,
            uv_b,
        } in anchors
        {
            numbers.extend([parameter].into_iter().chain(point).chain(uv_a).chain(uv_b));
        }
        for interval in uv_tubes.iter().flatten() {
            interval_numbers(*interval, numbers);
        }
        numbers.push(*residual_tolerance);
    }
}

fn surface_numbers(surface: &SurfaceGeometry, numbers: &mut Vec<f64>) {
    match surface {
        SurfaceGeometry::Plane { frame } => frame_numbers(*frame, numbers),
        SurfaceGeometry::Sphere { frame, radius } | SurfaceGeometry::Cylinder { frame, radius } => {
            frame_numbers(*frame, numbers);
            numbers.push(*radius);
        }
        SurfaceGeometry::Cone { frame, semi_angle } => {
            frame_numbers(*frame, numbers);
            numbers.push(*semi_angle);
        }
        SurfaceGeometry::Torus {
            frame,
            major_radius,
            minor_radius,
        } => {
            frame_numbers(*frame, numbers);
            numbers.extend([major_radius, minor_radius]);
        }
    }
}

fn frame_numbers(frame: Frame3, numbers: &mut Vec<f64>) {
    let Frame3 { origin, x, y, z } = frame;
    numbers.extend(origin.into_iter().chain(x).chain(y).chain(z));
}

fn curve_numbers(curve: &CurveGeometry, numbers: &mut Vec<f64>) {
    match curve {
        CurveGeometry::Line { origin, direction } => numbers.extend(origin.iter().chain(direction)),
        CurveGeometry::Circle { frame, radius } => {
            frame_numbers(*frame, numbers);
            numbers.push(*radius);
        }
        CurveGeometry::Ellipse {
            frame,
            major_radius,
            minor_radius,
        } => {
            frame_numbers(*frame, numbers);
            numbers.extend([major_radius, minor_radius]);
        }
        CurveGeometry::Intersection { definition } => index_numbers([definition], numbers),
    }
}

fn index_numbers<'a>(indices: impl IntoIterator<Item = &'a u32>, numbers: &mut Vec<f64>) {
    numbers.extend(indices.into_iter().map(|&index| f64::from(index)));
}

fn pcurve_numbers(pcurve: &PcurveGeometry, numbers: &mut Vec<f64>) {
    match pcurve {
        PcurveGeometry::Line2 { origin, direction } => {
            numbers.extend(origin.iter().chain(direction));
        }
        PcurveGeometry::Conic2 {
            origin,
            axis_a,
            axis_b,
        } => numbers.extend(origin.iter().chain(axis_a).chain(axis_b)),
        PcurveGeometry::ProjectedCurve {
            curve,
            surface,
            chart,
            uv_hint,
            uv_rate,
            parameter_origin,
        } => {
            index_numbers([curve, surface, chart], numbers);
            numbers.extend(uv_hint.iter().chain(uv_rate).chain([parameter_origin]));
        }
        PcurveGeometry::IntersectionSide {
            definition,
            side: _,
        } => index_numbers([definition], numbers),
    }
}

fn interval_numbers(interval: Interval, numbers: &mut Vec<f64>) {
    let Interval { lo, hi } = interval;
    numbers.extend([lo, hi]);
}

fn topology_numbers(topology: &Topology, numbers: &mut Vec<f64>) {
    let Topology {
        vertices,
        edges,
        halfedges,
        loops,
        faces,
        wires,
        shells,
    } = topology;
    for Vertex {
        id,
        position,
        outgoing_halfedge,
        tolerance,
    } in vertices
    {
        index_numbers([id].into_iter().chain(outgoing_halfedge), numbers);
        numbers.extend(position.iter().chain([tolerance]));
    }
    for edge in edges {
        edge_numbers(edge, numbers);
    }
    for halfedge in halfedges {
        halfedge_numbers(halfedge, numbers);
    }
    for Loop {
        id,
        start_halfedge,
        face_ref,
        is_hole: _,
    } in loops
    {
        index_numbers([id, start_halfedge, face_ref], numbers);
    }
    for face in faces {
        face_numbers(face, numbers);
    }
    for Wire {
        id,
        start_halfedge,
        is_closed: _,
    } in wires
    {
        index_numbers([id, start_halfedge], numbers);
    }
    for Shell {
        id,
        faces: shell_faces,
        is_closed: _,
    } in shells
    {
        index_numbers([id].into_iter().chain(shell_faces), numbers);
    }
}

fn edge_numbers(edge: &Edge, numbers: &mut Vec<f64>) {
    let Edge {
        id,
        geometry,
        halfedge,
        twin_halfedge,
        tolerance,
        chart_seam: _,
    } = edge;
    index_numbers([id, halfedge].into_iter().chain(twin_halfedge), numbers);
    match geometry {
        EdgeGeometry::Curve { curve, range } => {
            index_numbers([curve], numbers);
            interval_numbers(*range, numbers);
        }
        EdgeGeometry::Collapsed { vertex } => index_numbers([vertex], numbers),
    }
    numbers.push(*tolerance);
}

fn halfedge_numbers(halfedge: &HalfEdge, numbers: &mut Vec<f64>) {
    let HalfEdge {
        id,
        from,
        to,
        twin,
        next,
        prev,
        edge,
        face,
        loop_ref,
        wire_ref,
        geometry_use,
    } = halfedge;
    let HalfEdgeGeometryUse {
        sense: _,
        pcurve,
        periodic_lift,
    } = geometry_use;
    index_numbers([id, from, to, edge], numbers);
    let links = [twin, next, prev, face, loop_ref, wire_ref, pcurve];
    index_numbers(links.into_iter().flatten(), numbers);
    numbers.extend(periodic_lift.map(f64::from));
}

fn face_numbers(face: &Face, numbers: &mut Vec<f64>) {
    let Face {
        id,
        key: _,
        surface,
        sense: _,
        trim,
        shell_ref,
        provenance,
    } = face;
    let TrimRegion {
        chart,
        uv_bounds,
        outer,
        holes,
    } = trim;
    let FaceProvenance {
        sources,
        role: _,
        reversed: _,
    } = provenance;
    index_numbers([id, surface, chart, outer], numbers);
    for interval in uv_bounds {
        interval_numbers(*interval, numbers);
    }
    index_numbers(holes.iter().chain(shell_ref), numbers);
    for FaceSource {
        entity: _,
        body: _,
        key: _,
        face: source_face,
    } in sources
    {
        index_numbers([source_face], numbers);
    }
}
