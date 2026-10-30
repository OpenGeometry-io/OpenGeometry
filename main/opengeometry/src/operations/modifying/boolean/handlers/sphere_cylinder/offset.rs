use super::coaxial::coaxial_sphere_cylinder_boolean;
use crate::brep::{
    Accuracy, BrepEnvelope, CurveGeometry, Edge, EdgeGeometry, Face, FaceProvenance, FaceRole,
    GeometryError, Orientation, PcurveGeometry, TrimRegion, Vertex,
};
use crate::intersection::{intersect_faces, FaceView, IntersectionBranch, IntersectionGraph};
use crate::math::{norm, sub};
use crate::operations::modifying::boolean::assembly::{
    add_closed_intersection_loop, add_complement_input, finish_analytic_result,
    intersection_loop_bounds, loop_containment, loop_regions, loop_sense, merge_selected_shells,
    reverse_face, widen_trim_bounds, LoopRegion,
};
use crate::operations::modifying::boolean::operands::{
    brep_face_source, required_id, CylinderInput, SphereInput,
};
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};

struct ClassifiedLoop<'a> {
    graph: &'a IntersectionGraph,
    branch: &'a IntersectionBranch,
    curve_definition: u32,
    regions: [LoopRegion; 2],
    reverse_selected: [bool; 2],
}

struct IntersectionEdge {
    definition: u32,
    vertex: u32,
    edge: u32,
}

pub(super) fn offset_sphere_cylinder_boolean(
    sphere: &SphereInput<'_>,
    cylinder: &CylinderInput<'_>,
    sphere_is_a: bool,
    operation: BooleanOp,
    id: String,
    clearance: f64,
) -> Result<BooleanResult, GeometryError> {
    let local = cylinder.frame.local(sphere.frame.origin);
    let radial = local[0].hypot(local[1]);
    let scale = sphere
        .radius
        .max(cylinder.radius)
        .max(cylinder.height)
        .max(1.0);
    let roundoff = 128.0 * f64::EPSILON * scale;
    if radial <= roundoff {
        return coaxial_sphere_cylinder_boolean(
            sphere,
            cylinder,
            sphere_is_a,
            operation,
            id,
            clearance,
        );
    }
    let sphere_low = local[2] - sphere.radius;
    let sphere_high = local[2] + sphere.radius;
    if sphere_low <= clearance || cylinder.height - sphere_high <= clearance {
        return Err(GeometryError::CoverageGap {
            families: ["sphere".into(), "finite cylinder cap intersection".into()],
        });
    }
    let graph = intersect_faces(
        FaceView {
            brep: sphere.brep,
            face: 0,
        },
        FaceView {
            brep: cylinder.brep,
            face: 0,
        },
    )?;
    let (branch, curve_definition) = single_closed_branch(&graph, clearance)?;
    let loop_inside_other = loop_containment(
        sphere.brep,
        cylinder.brep,
        &graph,
        branch,
        "sphere/cylinder intersection loop interior is unclassified",
    )?;
    let (regions, reverse_selected) = loop_regions(operation, sphere_is_a, loop_inside_other);
    let classified = ClassifiedLoop {
        graph: &graph,
        branch,
        curve_definition,
        regions,
        reverse_selected,
    };
    classified_loop_boolean(sphere, cylinder, sphere_is_a, operation, id, &classified)
}

fn single_closed_branch(
    graph: &IntersectionGraph,
    clearance: f64,
) -> Result<(&IntersectionBranch, u32), GeometryError> {
    if graph.coincident || !graph.contacts.is_empty() || graph.branches.len() != 1 {
        return Err(GeometryError::CoverageGap {
            families: ["sphere".into(), "multi-branch or tangent cylinder".into()],
        });
    }
    let branch = &graph.branches[0];
    if norm(sub(branch.endpoints[0], branch.endpoints[1])) > clearance {
        return Err(GeometryError::CoverageGap {
            families: ["sphere".into(), "open cylinder intersection branch".into()],
        });
    }
    let curve_definition = match graph.geometry.curves[branch.curve as usize] {
        CurveGeometry::Intersection { definition } => definition,
        _ => {
            return Err(GeometryError::CoverageGap {
                families: ["sphere".into(), "non-numerical cylinder branch".into()],
            })
        }
    };
    Ok((branch, curve_definition))
}

fn classified_loop_boolean(
    sphere: &SphereInput<'_>,
    cylinder: &CylinderInput<'_>,
    sphere_is_a: bool,
    operation: BooleanOp,
    id: String,
    classified: &ClassifiedLoop<'_>,
) -> Result<BooleanResult, GeometryError> {
    let inputs = [sphere.brep, cylinder.brep];
    let mut out = BrepEnvelope::new(
        id,
        Accuracy::combined(sphere.brep.accuracy, cylinder.brep.accuracy),
    )?;
    let mut selected_faces = [None; 2];
    let mut selected_surfaces = [None; 2];
    let selected_shells = add_complement_sides(
        &mut out,
        inputs,
        classified.regions,
        &mut selected_faces,
        &mut selected_surfaces,
    )?;
    merge_selected_shells(&mut out, &selected_shells);
    for side in 0..2 {
        if classified.regions[side] == LoopRegion::Patch {
            selected_surfaces[side] = Some(out.geometry.surfaces.len() as u32);
            out.geometry
                .surfaces
                .push(classified.graph.geometry.surfaces[side].clone());
        }
    }
    let loop_edge = add_intersection_edge(&mut out, classified, selected_surfaces)?;
    add_loop_sides(
        &mut out,
        classified,
        inputs,
        &loop_edge,
        &mut selected_faces,
        selected_surfaces,
    )?;
    for side in 0..2 {
        if classified.reverse_selected[side] {
            reverse_face(&mut out, required_id(selected_faces[side])?);
        }
    }
    finish_analytic_result(
        out,
        sphere.brep,
        cylinder.brep,
        sphere_is_a,
        operation,
        false,
    )
}

fn add_complement_sides(
    out: &mut BrepEnvelope,
    inputs: [&BrepEnvelope; 2],
    regions: [LoopRegion; 2],
    selected_faces: &mut [Option<u32>; 2],
    selected_surfaces: &mut [Option<u32>; 2],
) -> Result<Vec<u32>, GeometryError> {
    let mut selected_shells = Vec::new();
    for side in 0..2 {
        if regions[side] != LoopRegion::Complement {
            continue;
        }
        add_complement_input(
            out,
            inputs[side],
            0,
            &mut selected_surfaces[side],
            &mut selected_shells,
            |face| selected_faces[side] = Some(face),
        )?;
    }
    Ok(selected_shells)
}

fn add_intersection_edge(
    out: &mut BrepEnvelope,
    classified: &ClassifiedLoop<'_>,
    selected_surfaces: [Option<u32>; 2],
) -> Result<IntersectionEdge, GeometryError> {
    let mut definition_geometry =
        classified.graph.geometry.intersections[classified.curve_definition as usize].clone();
    definition_geometry.surfaces = [
        selected_surfaces[definition_geometry.surfaces[0] as usize].ok_or_else(|| {
            GeometryError::InvalidTopology(
                "sphere-cylinder first support surface was not retained".into(),
            )
        })?,
        selected_surfaces[definition_geometry.surfaces[1] as usize].ok_or_else(|| {
            GeometryError::InvalidTopology(
                "sphere-cylinder second support surface was not retained".into(),
            )
        })?,
    ];
    let definition = out.geometry.intersections.len() as u32;
    out.geometry.intersections.push(definition_geometry);
    let curve = out.geometry.curves.len() as u32;
    out.geometry
        .curves
        .push(CurveGeometry::Intersection { definition });
    let vertex = out.topology.vertices.len() as u32;
    out.topology.vertices.push(Vertex {
        id: vertex,
        position: classified.branch.endpoints[0],
        outgoing_halfedge: None,
        tolerance: out.accuracy.geometric,
    });
    let edge = out.topology.edges.len() as u32;
    out.topology.edges.push(Edge {
        id: edge,
        geometry: EdgeGeometry::Curve {
            curve,
            range: classified.branch.range,
        },
        halfedge: u32::MAX,
        twin_halfedge: None,
        tolerance: out.accuracy.geometric,
        chart_seam: false,
    });
    Ok(IntersectionEdge {
        definition,
        vertex,
        edge,
    })
}

fn add_loop_sides(
    out: &mut BrepEnvelope,
    classified: &ClassifiedLoop<'_>,
    inputs: [&BrepEnvelope; 2],
    loop_edge: &IntersectionEdge,
    selected_faces: &mut [Option<u32>; 2],
    selected_surfaces: [Option<u32>; 2],
) -> Result<(), GeometryError> {
    for side in 0..2 {
        let (pcurve, sense) = side_pcurve(classified, side, loop_edge.definition)?;
        if classified.regions[side] == LoopRegion::Complement {
            let face = required_id(selected_faces[side])?;
            let loop_bounds = intersection_loop_bounds(
                classified.graph,
                classified.branch,
                side,
                out.accuracy.intersection,
            )?;
            widen_trim_bounds(out, face, loop_bounds)?;
            let loop_id = add_closed_intersection_loop(
                out,
                loop_edge.edge,
                loop_edge.vertex,
                face,
                pcurve,
                sense,
                true,
            )?;
            out.topology.faces[face as usize].trim.holes.push(loop_id);
        } else {
            let face = out.topology.faces.len() as u32;
            let loop_id = add_closed_intersection_loop(
                out,
                loop_edge.edge,
                loop_edge.vertex,
                face,
                pcurve,
                sense,
                false,
            )?;
            out.topology.faces.push(Face {
                id: face,
                key: format!("{}:intersection-patch", inputs[side].topology.faces[0].key),
                surface: required_id(selected_surfaces[side])?,
                sense: Orientation::Forward,
                trim: TrimRegion {
                    chart: 0,
                    uv_bounds: intersection_loop_bounds(
                        classified.graph,
                        classified.branch,
                        side,
                        out.accuracy.intersection,
                    )?,
                    outer: loop_id,
                    holes: Vec::new(),
                },
                shell_ref: Some(0),
                provenance: FaceProvenance {
                    sources: vec![brep_face_source(inputs[side], 0)],
                    role: if classified.reverse_selected[side] {
                        FaceRole::Cut
                    } else {
                        FaceRole::Split
                    },
                    reversed: classified.reverse_selected[side],
                },
            });
            out.topology.shells[0].faces.push(face);
            selected_faces[side] = Some(face);
        }
    }
    Ok(())
}

fn side_pcurve(
    classified: &ClassifiedLoop<'_>,
    side: usize,
    definition: u32,
) -> Result<(PcurveGeometry, Orientation), GeometryError> {
    let pcurve = match classified.graph.geometry.pcurves[classified.branch.pcurves[side] as usize] {
        PcurveGeometry::IntersectionSide {
            side: intersection_side,
            ..
        } => PcurveGeometry::IntersectionSide {
            definition,
            side: intersection_side,
        },
        _ => {
            return Err(GeometryError::InvalidGeometry(
                "numerical sphere-cylinder curve is missing an intersection-side pcurve".into(),
            ))
        }
    };
    Ok((pcurve, loop_sense(side, classified.reverse_selected[side])))
}
