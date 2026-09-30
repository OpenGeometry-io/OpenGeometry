use crate::brep::{
    Accuracy, BrepEnvelope, GeometryError, GeometryStore, IntersectionDefinition, IntersectionSide,
    PcurveGeometry, SurfaceGeometry,
};
use crate::intersection::{intersect_breps, IntersectionBranch, IntersectionGraph};
use crate::math::{norm, sub};
use crate::operations::modifying::boolean::assembly::{
    add_complement_input, add_hole_loop, add_patch_face, add_patch_loop,
    append_closed_branch_geometry, finish_analytic_result, intersection_definition,
    loop_containment, loop_regions, loop_sense, merge_selected_shells, pcurve_support_surface,
    pcurve_winding, reframe_sphere_intersection, reverse_face, ClosedBranchEdge, LoopRegion,
    LoopSides,
};
use crate::operations::modifying::boolean::operands::required_id;
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};

pub(crate) fn generic_single_closed_loop_boolean(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<Option<BooleanResult>, GeometryError> {
    if a.solids.len() != 1
        || b.solids.len() != 1
        || a.topology.shells.len() != 1
        || b.topology.shells.len() != 1
        || !a.solids[0].cavity_shells.is_empty()
        || !b.solids[0].cavity_shells.is_empty()
    {
        return Ok(None);
    }
    let body_graph = intersect_breps(a, b)?;
    if body_graph.pairs.len() != 1 {
        return Ok(None);
    }
    let pair = &body_graph.pairs[0];
    if pair.graph.coincident || !pair.graph.contacts.is_empty() || pair.graph.branches.len() != 1 {
        return Ok(None);
    }
    let graph = &pair.graph;
    let branch = &graph.branches[0];
    let accuracy = Accuracy::combined(a.accuracy, b.accuracy);
    if norm(sub(branch.endpoints[0], branch.endpoints[1])) > accuracy.intersection {
        return Ok(None);
    }
    let Some(windings) = supported_windings(graph, branch)? else {
        return Ok(None);
    };
    let inputs = [a, b];
    let face_ids = pair.faces;
    let loop_inside_other = loop_containment(
        a,
        b,
        graph,
        branch,
        "closed SSI loop interior is unclassified",
    )?;
    let (regions, reverse_selected) = loop_regions(operation, true, loop_inside_other);

    let mut out = BrepEnvelope::new(id, accuracy)?;
    let (mut selected_faces, selected_surfaces) =
        add_selected_sides(&mut out, graph, inputs, face_ids, regions)?;
    let sides = LoopSides {
        inputs,
        face_ids,
        regions,
        reverse_selected,
        surfaces: selected_surfaces,
    };
    let loop_edge = add_intersection_edge(
        &mut out,
        graph,
        branch,
        windings,
        sides.surfaces,
        accuracy.geometric,
    )?;
    add_loop_sides(
        &mut out,
        &sides,
        &loop_edge,
        &mut selected_faces,
        accuracy.intersection,
    )?;
    for side in 0..2 {
        if reverse_selected[side] {
            reverse_face(&mut out, required_id(selected_faces[side])?);
        }
    }
    finish_analytic_result(out, a, b, true, operation, false).map(Some)
}

fn supported_windings(
    graph: &IntersectionGraph,
    branch: &IntersectionBranch,
) -> Result<Option<[[i32; 2]; 2]>, GeometryError> {
    let windings = [
        pcurve_winding(
            &graph.geometry,
            pcurve_support_surface(&graph.geometry, branch.pcurves[0])?,
            branch.pcurves[0],
            branch.range,
        )?,
        pcurve_winding(
            &graph.geometry,
            pcurve_support_surface(&graph.geometry, branch.pcurves[1])?,
            branch.pcurves[1],
            branch.range,
        )?,
    ];
    for side in 0..2 {
        if windings[side] != [0, 0]
            && !matches!(
                graph.geometry.surface(pcurve_support_surface(
                    &graph.geometry,
                    branch.pcurves[side],
                )?)?,
                SurfaceGeometry::Sphere { .. }
            )
        {
            return Ok(None);
        }
    }
    Ok(Some(windings))
}

fn add_selected_sides(
    out: &mut BrepEnvelope,
    graph: &IntersectionGraph,
    inputs: [&BrepEnvelope; 2],
    face_ids: [u32; 2],
    regions: [LoopRegion; 2],
) -> Result<([Option<u32>; 2], [Option<u32>; 2]), GeometryError> {
    let mut selected_faces = [None; 2];
    let mut selected_surfaces = [None; 2];
    let mut selected_shells = Vec::new();
    for side in 0..2 {
        if regions[side] != LoopRegion::Complement {
            continue;
        }
        add_complement_input(
            out,
            inputs[side],
            face_ids[side],
            &mut selected_surfaces[side],
            &mut selected_shells,
            |face| selected_faces[side] = Some(face),
        )?;
    }
    merge_selected_shells(out, &selected_shells);
    for side in 0..2 {
        if regions[side] == LoopRegion::Patch {
            selected_surfaces[side] = Some(out.geometry.surfaces.len() as u32);
            out.geometry
                .surfaces
                .push(graph.geometry.surfaces[side].clone());
        }
    }
    Ok((selected_faces, selected_surfaces))
}

fn add_intersection_edge(
    out: &mut BrepEnvelope,
    graph: &IntersectionGraph,
    branch: &IntersectionBranch,
    windings: [[i32; 2]; 2],
    selected_surfaces: [Option<u32>; 2],
    geometric: f64,
) -> Result<ClosedBranchEdge, GeometryError> {
    append_closed_branch_geometry(
        out,
        graph,
        branch,
        geometric,
        |geometry, definition| {
            retarget_loop_definition(
                geometry,
                graph,
                branch,
                definition,
                windings,
                selected_surfaces,
            )
        },
        |side| required_id(selected_surfaces[side]),
    )
}

fn retarget_loop_definition(
    geometry: &mut GeometryStore,
    graph: &IntersectionGraph,
    branch: &IntersectionBranch,
    definition: u32,
    windings: [[i32; 2]; 2],
    selected_surfaces: [Option<u32>; 2],
) -> Result<IntersectionDefinition, GeometryError> {
    let mut definition_geometry = intersection_definition(graph, definition)?;
    definition_geometry.surfaces = [
        selected_surfaces[definition_geometry.surfaces[0] as usize].ok_or_else(|| {
            GeometryError::InvalidTopology(
                "closed-loop first support surface was not retained".into(),
            )
        })?,
        selected_surfaces[definition_geometry.surfaces[1] as usize].ok_or_else(|| {
            GeometryError::InvalidTopology(
                "closed-loop second support surface was not retained".into(),
            )
        })?,
    ];
    for side in 0..2 {
        if windings[side] == [0, 0] {
            continue;
        }
        let definition_side = match graph.geometry.pcurves[branch.pcurves[side] as usize] {
            PcurveGeometry::IntersectionSide {
                side: IntersectionSide::A,
                ..
            } => 0,
            PcurveGeometry::IntersectionSide {
                side: IntersectionSide::B,
                ..
            } => 1,
            _ => {
                return Err(GeometryError::InvalidGeometry(
                    "winding intersection loop is missing its support pcurve".into(),
                ))
            }
        };
        reframe_sphere_intersection(geometry, &mut definition_geometry, definition_side)?;
    }
    Ok(definition_geometry)
}

fn add_loop_sides(
    out: &mut BrepEnvelope,
    sides: &LoopSides<'_, Option<u32>>,
    loop_edge: &ClosedBranchEdge,
    selected_faces: &mut [Option<u32>; 2],
    tolerance: f64,
) -> Result<(), GeometryError> {
    for side in 0..2 {
        let sense = loop_sense(side, sides.reverse_selected[side]);
        if sides.regions[side] == LoopRegion::Complement {
            let face = required_id(selected_faces[side])?;
            add_hole_loop(out, loop_edge, side, face, sense, tolerance)?;
        } else {
            let patch = add_patch_loop(out, loop_edge, side, sense, tolerance)?;
            add_patch_face(out, sides, side, &patch, None, required_id)?;
            selected_faces[side] = Some(patch.face);
        }
    }
    Ok(())
}
