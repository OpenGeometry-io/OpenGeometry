use crate::brep::{Accuracy, BrepEnvelope, GeometryError, Surface};
use crate::intersection::{intersect_breps, IntersectionBranch, IntersectionGraph};
use crate::math::{norm, sub};
use crate::operations::modifying::boolean::assembly::{
    add_complement_input, add_hole_loop, add_patch_face, add_patch_loop,
    append_closed_branch_geometry, finish_analytic_result, intersection_loop_sample,
    loop_selection, loop_sense, merge_selected_shells, pcurve_support_surface, pcurve_winding,
    retained_definition, reverse_face, LoopRegion, LoopSides,
};
use crate::operations::modifying::boolean::operands::required_id;
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};
use crate::query::{classify_point, PointClassification};

pub(crate) fn generic_multiple_closed_loops_boolean(
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
    if pair.graph.coincident || !pair.graph.contacts.is_empty() || pair.graph.branches.len() < 2 {
        return Ok(None);
    }
    let graph = &pair.graph;
    let accuracy = Accuracy::combined(a.accuracy, b.accuracy);
    if !branches_closed_and_unwound(graph, accuracy)? {
        return Ok(None);
    }

    let inputs = [a, b];
    let face_ids = pair.faces;
    let (want_inside, reverse_selected) = loop_selection(operation, true);
    let Some(selected_regions) = selected_loop_regions(graph, inputs, want_inside)? else {
        return Ok(None);
    };

    let mut out = BrepEnvelope::new(id, accuracy)?;
    let (mut selected_faces, selected_surfaces) =
        add_selected_sides(&mut out, graph, inputs, face_ids, selected_regions)?;
    let sides = LoopSides {
        inputs,
        face_ids,
        regions: selected_regions,
        reverse_selected,
        surfaces: selected_surfaces,
    };
    for (branch_index, branch) in graph.branches.iter().enumerate() {
        add_branch_loops(
            &mut out,
            graph,
            branch,
            branch_index,
            &sides,
            &mut selected_faces,
            accuracy.intersection,
        )?;
    }
    for side in 0..2 {
        if reverse_selected[side] {
            for face in selected_faces[side].clone() {
                reverse_face(&mut out, face);
            }
        }
    }
    finish_analytic_result(out, a, b, true, operation, false).map(Some)
}

fn branches_closed_and_unwound(
    graph: &IntersectionGraph,
    accuracy: Accuracy,
) -> Result<bool, GeometryError> {
    for branch in &graph.branches {
        if norm(sub(branch.endpoints[0], branch.endpoints[1])) > accuracy.intersection {
            return Ok(false);
        }
        for side in 0..2 {
            if pcurve_winding(
                &graph.geometry,
                pcurve_support_surface(&graph.geometry, branch.pcurves[side])?,
                branch.pcurves[side],
                branch.range,
            )? != [0, 0]
            {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn selected_loop_regions(
    graph: &IntersectionGraph,
    inputs: [&BrepEnvelope; 2],
    want_inside: [bool; 2],
) -> Result<Option<[LoopRegion; 2]>, GeometryError> {
    let mut loop_regions = [Vec::new(), Vec::new()];
    for branch in &graph.branches {
        for side in 0..2 {
            let uv = intersection_loop_sample(graph, branch, side)?;
            let point = graph.geometry.surfaces[side].point_at(uv)?;
            let inside = match classify_point(inputs[1 - side], point)? {
                PointClassification::Inside => true,
                PointClassification::Outside => false,
                PointClassification::Unknown | PointClassification::Boundary => {
                    return Err(GeometryError::UnresolvedIntersection(
                        "closed SSI loop interior is unclassified".into(),
                    ))
                }
            };
            loop_regions[side].push(if inside == want_inside[side] {
                LoopRegion::Patch
            } else {
                LoopRegion::Complement
            });
        }
    }
    if [0, 1].into_iter().any(|side| {
        loop_regions[side]
            .iter()
            .any(|region| *region != loop_regions[side][0])
    }) {
        return Ok(None);
    }
    let selected_regions = [loop_regions[0][0], loop_regions[1][0]];
    for side in 0..2 {
        if selected_regions[side] == LoopRegion::Patch && inputs[side].topology.faces.len() != 1 {
            return Ok(None);
        }
    }
    Ok(Some(selected_regions))
}

fn add_selected_sides(
    out: &mut BrepEnvelope,
    graph: &IntersectionGraph,
    inputs: [&BrepEnvelope; 2],
    face_ids: [u32; 2],
    selected_regions: [LoopRegion; 2],
) -> Result<([Vec<u32>; 2], [u32; 2]), GeometryError> {
    let mut selected_faces = [Vec::new(), Vec::new()];
    let mut selected_surfaces = [None, None];
    let mut selected_shells = Vec::new();
    for side in 0..2 {
        if selected_regions[side] == LoopRegion::Complement {
            add_complement_input(
                out,
                inputs[side],
                face_ids[side],
                &mut selected_surfaces[side],
                &mut selected_shells,
                |face| selected_faces[side].push(face),
            )?;
        } else {
            selected_surfaces[side] = Some(out.geometry.surfaces.len() as u32);
            out.geometry
                .surfaces
                .push(graph.geometry.surfaces[side].clone());
        }
    }
    merge_selected_shells(out, &selected_shells);

    let selected_surfaces = [
        required_id(selected_surfaces[0])?,
        required_id(selected_surfaces[1])?,
    ];
    Ok((selected_faces, selected_surfaces))
}

fn add_branch_loops(
    out: &mut BrepEnvelope,
    graph: &IntersectionGraph,
    branch: &IntersectionBranch,
    branch_index: usize,
    sides: &LoopSides<'_, u32>,
    selected_faces: &mut [Vec<u32>; 2],
    tolerance: f64,
) -> Result<(), GeometryError> {
    let geometric = out.accuracy.geometric;
    let loop_edge = append_closed_branch_geometry(
        out,
        graph,
        branch,
        geometric,
        |_, definition| retained_definition(graph, definition, sides.surfaces),
        |side| Ok(sides.surfaces[side]),
    )?;
    for side in 0..2 {
        let sense = loop_sense(side, sides.reverse_selected[side]);
        if sides.regions[side] == LoopRegion::Complement {
            let face = selected_faces[side][0];
            add_hole_loop(out, &loop_edge, side, face, sense, tolerance)?;
        } else {
            let patch = add_patch_loop(out, &loop_edge, side, sense, tolerance)?;
            add_patch_face(out, sides, side, &patch, Some(branch_index), Ok)?;
            selected_faces[side].push(patch.face);
        }
    }
    Ok(())
}
