use crate::brep::{
    Accuracy, BrepEnvelope, CurveGeometry, Face, FaceProvenance, FaceRole, GeometryError,
    Orientation, TrimRegion,
};
use crate::intersection::{intersect_breps, IntersectionBranch, IntersectionGraph};
use crate::math::{add, norm, scale, sub, Interval};
use crate::operations::modifying::boolean::assembly::{
    add_closed_intersection_loop, add_hole_loop, append_analytic_input,
    append_closed_branch_geometry, finish_analytic_result, pcurve_support_surface, pcurve_winding,
    retained_definition, reverse_face, topology_loop_bounds,
};
use crate::operations::modifying::boolean::operands::brep_face_source;
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};

struct CutBand {
    face: u32,
    surface: u32,
}

pub(crate) fn generic_two_sided_cutter_band_subtraction(
    host: &BrepEnvelope,
    cutter: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<Option<BooleanResult>, GeometryError> {
    if operation != BooleanOp::Subtraction
        || host.solids.len() != 1
        || cutter.solids.len() != 1
        || host.topology.shells.len() != 1
        || cutter.topology.shells.len() != 1
        || !host.solids[0].cavity_shells.is_empty()
        || !cutter.solids[0].cavity_shells.is_empty()
    {
        return Ok(None);
    }
    let body_graph = intersect_breps(host, cutter)?;
    if body_graph.pairs.len() != 2
        || body_graph.pairs[0].faces[1] != body_graph.pairs[1].faces[1]
        || body_graph.pairs[0].faces[0] == body_graph.pairs[1].faces[0]
    {
        return Ok(None);
    }
    let mut closed_branches = Vec::with_capacity(2);
    for pair in &body_graph.pairs {
        let Some(closed) = single_closed_branch(&pair.graph, host, cutter)? else {
            return Ok(None);
        };
        closed_branches.push(closed);
    }

    let accuracy = Accuracy::combined(host.accuracy, cutter.accuracy);
    let mut out = BrepEnvelope::new(id, accuracy)?;
    append_analytic_input(&mut out, host)?;
    let cutter_surface = out.geometry.surfaces.len() as u32;
    out.geometry
        .surfaces
        .push(body_graph.pairs[0].graph.geometry.surfaces[1].clone());
    let cut_face = out.topology.faces.len() as u32;
    let band = CutBand {
        face: cut_face,
        surface: cutter_surface,
    };
    let mut cut_loops = Vec::with_capacity(2);
    let mut cut_bounds: Option<[Interval; 2]> = None;
    for (pair, branch) in body_graph.pairs.iter().zip(&closed_branches) {
        let (cutter_loop, bounds) = add_band_loops(
            &mut out,
            &pair.graph,
            branch,
            pair.faces[0],
            &band,
            &cut_loops,
            accuracy.intersection,
        )?;
        cut_bounds = Some(widened_cut_bounds(cut_bounds, bounds)?);
        cut_loops.push(cutter_loop);
    }
    add_cut_face(
        &mut out,
        cutter,
        body_graph.pairs[0].faces[1],
        &band,
        cut_bounds,
        &cut_loops,
    )?;
    reverse_face(&mut out, cut_face);
    finish_analytic_result(out, host, cutter, true, operation, false).map(Some)
}

fn single_closed_branch(
    graph: &IntersectionGraph,
    host: &BrepEnvelope,
    cutter: &BrepEnvelope,
) -> Result<Option<IntersectionBranch>, GeometryError> {
    if graph.coincident || !graph.contacts.is_empty() || graph.branches.len() != 1 {
        return Ok(None);
    }
    let branch = &graph.branches[0];
    let closure = norm(sub(branch.endpoints[0], branch.endpoints[1]));
    let intersection = host.accuracy.intersection.max(cutter.accuracy.intersection);
    let geometric = host.accuracy.geometric.max(cutter.accuracy.geometric);
    let closed = if closure <= intersection {
        IntersectionBranch {
            curve: branch.curve,
            pcurves: branch.pcurves,
            range: branch.range,
            endpoints: branch.endpoints,
        }
    } else if let Some(CurveGeometry::Circle { frame, radius }) =
        graph.geometry.curves.get(branch.curve as usize)
    {
        let tau = std::f64::consts::TAU;
        if closure > 4.0 * geometric
            || branch.range.lo.abs() * radius > 4.0 * geometric
            || (branch.range.hi - tau).abs() * radius > 4.0 * geometric
        {
            return Ok(None);
        }
        let point = add(frame.origin, scale(frame.x, *radius));
        IntersectionBranch {
            curve: branch.curve,
            pcurves: branch.pcurves,
            range: Interval::new(0.0, tau)?,
            endpoints: [point, point],
        }
    } else {
        return Ok(None);
    };
    if pcurve_winding(
        &graph.geometry,
        pcurve_support_surface(&graph.geometry, closed.pcurves[0])?,
        closed.pcurves[0],
        closed.range,
    )? != [0, 0]
    {
        return Ok(None);
    }
    Ok(Some(closed))
}

fn add_band_loops(
    out: &mut BrepEnvelope,
    graph: &IntersectionGraph,
    branch: &IntersectionBranch,
    host_face: u32,
    band: &CutBand,
    cut_loops: &[u32],
    tolerance: f64,
) -> Result<(u32, [Interval; 2]), GeometryError> {
    let host_surface = out.topology.faces[host_face as usize].surface;
    let selected_surfaces = [host_surface, band.surface];
    let geometric = out.accuracy.geometric;
    let loop_edge = append_closed_branch_geometry(
        out,
        graph,
        branch,
        geometric,
        |_, definition| retained_definition(graph, definition, selected_surfaces),
        |side| Ok(selected_surfaces[side]),
    )?;
    add_hole_loop(
        out,
        &loop_edge,
        0,
        host_face,
        Orientation::Forward,
        tolerance,
    )?;
    out.topology.faces[host_face as usize].provenance.role = FaceRole::Split;

    let cutter_loop = add_closed_intersection_loop(
        out,
        loop_edge.edge,
        loop_edge.vertex,
        band.face,
        loop_edge.pcurves[1].clone(),
        Orientation::Forward,
        !cut_loops.is_empty(),
    )?;
    let bounds = topology_loop_bounds(out, cutter_loop, tolerance)?;
    Ok((cutter_loop, bounds))
}

fn widened_cut_bounds(
    cut_bounds: Option<[Interval; 2]>,
    bounds: [Interval; 2],
) -> Result<[Interval; 2], GeometryError> {
    Ok(match cut_bounds {
        Some(previous) => [0, 1]
            .map(|axis| {
                Interval::new(
                    previous[axis].lo.min(bounds[axis].lo),
                    previous[axis].hi.max(bounds[axis].hi),
                )
            })
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| GeometryError::InvalidGeometry("cutter band UV bounds".into()))?,
        None => bounds,
    })
}

fn add_cut_face(
    out: &mut BrepEnvelope,
    cutter: &BrepEnvelope,
    cutter_face: u32,
    band: &CutBand,
    cut_bounds: Option<[Interval; 2]>,
    cut_loops: &[u32],
) -> Result<(), GeometryError> {
    let source_face = &cutter.topology.faces[cutter_face as usize];
    out.topology.faces.push(Face {
        id: band.face,
        key: format!("{}:through-cut", source_face.key),
        surface: band.surface,
        sense: source_face.sense,
        trim: TrimRegion {
            chart: source_face.trim.chart,
            uv_bounds: cut_bounds.ok_or_else(|| {
                GeometryError::InvalidTopology("cutter band has no trim bounds".into())
            })?,
            outer: cut_loops[0],
            holes: cut_loops[1..].to_vec(),
        },
        shell_ref: Some(0),
        provenance: FaceProvenance {
            sources: vec![brep_face_source(cutter, source_face.id)],
            role: FaceRole::Cut,
            reversed: true,
        },
    });
    out.topology.shells[0].faces.push(band.face);
    Ok(())
}
