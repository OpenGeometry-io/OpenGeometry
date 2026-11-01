use super::append::append_analytic_input;
use super::loop_pcurves::{
    exact_closed_circle_pcurve, intersection_loop_sample, remap_closed_loop_pcurve,
    topology_loop_bounds,
};
use super::provenance::reverse;
use super::sphere_loop_chart::SphereLoopChart;
use crate::brep::{
    BrepEnvelope, CurveGeometry, Edge, EdgeGeometry, Face, FaceProvenance, FaceRole, GeometryError,
    GeometryStore, HalfEdge, HalfEdgeGeometryUse, IntersectionDefinition, Loop, Orientation,
    PcurveGeometry, Shell, SolidRegion, Surface, TrimRegion, Vertex,
};
use crate::intersection::{IntersectionBranch, IntersectionGraph};
use crate::math::Interval;
use crate::operations::modifying::boolean::operands::brep_face_source;
use crate::operations::modifying::boolean::types::BooleanOp;
use crate::query::{classify_point, PointClassification};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LoopRegion {
    Patch,
    Complement,
}

pub(crate) struct LoopSides<'a, S> {
    pub(crate) inputs: [&'a BrepEnvelope; 2],
    pub(crate) face_ids: [u32; 2],
    pub(crate) regions: [LoopRegion; 2],
    pub(crate) reverse_selected: [bool; 2],
    pub(crate) surfaces: [S; 2],
}

pub(crate) struct ClosedBranchEdge {
    pub(crate) edge: u32,
    pub(crate) vertex: u32,
    pub(crate) pcurves: [PcurveGeometry; 2],
}

pub(crate) struct PatchLoop {
    pub(crate) face: u32,
    loop_id: u32,
    uv_bounds: [Interval; 2],
}

pub(crate) fn loop_containment(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    graph: &IntersectionGraph,
    branch: &IntersectionBranch,
    charts: &[Option<SphereLoopChart>; 2],
    unclassified: &str,
) -> Result<[bool; 2], GeometryError> {
    let inputs = [a, b];
    let patch_inside_other = |side: usize| {
        let sample = match &charts[side] {
            Some(chart) => chart.patch_sample()?,
            None => graph.geometry.surfaces[side]
                .point_at(intersection_loop_sample(graph, branch, side)?)?,
        };
        classify_point(inputs[1 - side], sample)
    };
    let loop_inside_other = [patch_inside_other(0)?, patch_inside_other(1)?];
    if loop_inside_other.iter().any(|classification| {
        matches!(
            classification,
            PointClassification::Unknown | PointClassification::Boundary
        )
    }) {
        return Err(GeometryError::UnresolvedIntersection(unclassified.into()));
    }
    Ok(loop_inside_other.map(|value| value == PointClassification::Inside))
}

pub(crate) fn loop_regions(
    operation: BooleanOp,
    first_is_a: bool,
    loop_inside_other: [bool; 2],
) -> ([LoopRegion; 2], [bool; 2]) {
    let (want_inside, reverse_selected) = loop_selection(operation, first_is_a);
    let regions: [LoopRegion; 2] = std::array::from_fn(|side| {
        if loop_inside_other[side] == want_inside[side] {
            LoopRegion::Patch
        } else {
            LoopRegion::Complement
        }
    });
    (regions, reverse_selected)
}

pub(crate) fn loop_selection(operation: BooleanOp, first_is_a: bool) -> ([bool; 2], [bool; 2]) {
    match operation {
        BooleanOp::Union => ([false, false], [false, false]),
        BooleanOp::Intersection => ([true, true], [false, false]),
        BooleanOp::Subtraction if first_is_a => ([false, true], [false, true]),
        BooleanOp::Subtraction => ([true, false], [true, false]),
    }
}

pub(crate) fn add_complement_input(
    out: &mut BrepEnvelope,
    input: &BrepEnvelope,
    face_id: u32,
    selected_surface: &mut Option<u32>,
    selected_shells: &mut Vec<u32>,
    record_face: impl FnOnce(u32),
) -> Result<(), GeometryError> {
    let face_offset = out.topology.faces.len() as u32;
    let shell_offset = out.topology.shells.len() as u32;
    append_analytic_input(out, input)?;
    let selected_face = face_offset + face_id;
    record_face(selected_face);
    *selected_surface = Some(out.topology.faces[selected_face as usize].surface);
    selected_shells.push(shell_offset);
    for face in face_offset..out.topology.faces.len() as u32 {
        out.topology.faces[face as usize].provenance.role = if face == selected_face {
            FaceRole::Split
        } else {
            FaceRole::Preserved
        };
    }
    Ok(())
}

pub(crate) fn merge_selected_shells(out: &mut BrepEnvelope, selected_shells: &[u32]) {
    if selected_shells.is_empty() {
        out.topology.shells.push(Shell {
            id: 0,
            faces: Vec::new(),
            is_closed: true,
        });
        out.solids.push(SolidRegion {
            outer_shell: 0,
            cavity_shells: Vec::new(),
        });
    } else if selected_shells.len() == 2 {
        let faces = out.topology.shells[1].faces.clone();
        for face in &faces {
            out.topology.faces[*face as usize].shell_ref = Some(0);
        }
        out.topology.shells[0].faces.extend(faces);
        out.topology.shells.pop();
        out.solids.clear();
        out.solids.push(SolidRegion {
            outer_shell: 0,
            cavity_shells: Vec::new(),
        });
    }
}

pub(crate) fn append_closed_branch_geometry(
    out: &mut BrepEnvelope,
    graph: &IntersectionGraph,
    branch: &IntersectionBranch,
    tolerance: f64,
    retarget: impl FnOnce(&mut GeometryStore, u32) -> Result<IntersectionDefinition, GeometryError>,
    surface: impl Fn(usize) -> Result<u32, GeometryError>,
) -> Result<ClosedBranchEdge, GeometryError> {
    let source_curve = graph
        .geometry
        .curves
        .get(branch.curve as usize)
        .ok_or_else(|| GeometryError::MissingReference {
            kind: "curve".into(),
            index: branch.curve,
        })?
        .clone();
    let curve = out.geometry.curves.len() as u32;
    let pcurves = match source_curve {
        CurveGeometry::Intersection { definition } => {
            let definition_geometry = retarget(&mut out.geometry, definition)?;
            add_branch_intersection_curve(out, graph, branch, definition_geometry)?
        }
        curve_geometry => {
            add_branch_analytic_curve(out, graph, branch, curve, &curve_geometry, surface)?
        }
    };
    let vertex = out.topology.vertices.len() as u32;
    out.topology.vertices.push(Vertex {
        id: vertex,
        position: branch.endpoints[0],
        outgoing_halfedge: None,
        tolerance,
    });
    let edge = out.topology.edges.len() as u32;
    out.topology.edges.push(Edge {
        id: edge,
        geometry: EdgeGeometry::Curve {
            curve,
            range: branch.range,
        },
        halfedge: u32::MAX,
        twin_halfedge: None,
        tolerance,
        chart_seam: false,
    });
    Ok(ClosedBranchEdge {
        edge,
        vertex,
        pcurves,
    })
}

fn add_branch_intersection_curve(
    out: &mut BrepEnvelope,
    graph: &IntersectionGraph,
    branch: &IntersectionBranch,
    definition_geometry: IntersectionDefinition,
) -> Result<[PcurveGeometry; 2], GeometryError> {
    let definition = out.geometry.intersections.len() as u32;
    out.geometry.intersections.push(definition_geometry);
    out.geometry
        .curves
        .push(CurveGeometry::Intersection { definition });
    [0, 1]
        .map(
            |side| match graph.geometry.pcurves.get(branch.pcurves[side] as usize) {
                Some(PcurveGeometry::IntersectionSide {
                    side: intersection_side,
                    ..
                }) => Ok(PcurveGeometry::IntersectionSide {
                    definition,
                    side: *intersection_side,
                }),
                Some(_) => Err(GeometryError::InvalidGeometry(
                    "intersection curve requires intersection-side pcurves".into(),
                )),
                None => Err(GeometryError::MissingReference {
                    kind: "pcurve".into(),
                    index: branch.pcurves[side],
                }),
            },
        )
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| GeometryError::InvalidGeometry("closed SSI loop requires two pcurves".into()))
}

fn add_branch_analytic_curve(
    out: &mut BrepEnvelope,
    graph: &IntersectionGraph,
    branch: &IntersectionBranch,
    curve: u32,
    curve_geometry: &CurveGeometry,
    surface: impl Fn(usize) -> Result<u32, GeometryError>,
) -> Result<[PcurveGeometry; 2], GeometryError> {
    out.geometry.curves.push(curve_geometry.clone());
    [0, 1]
        .map(|side| {
            if let Some(exact) = exact_closed_circle_pcurve(
                &graph.geometry,
                &out.geometry,
                branch.pcurves[side],
                curve_geometry,
                surface(side)?,
                out.accuracy.geometric,
            )? {
                return Ok(exact);
            }
            remap_closed_loop_pcurve(
                &graph.geometry.pcurves[branch.pcurves[side] as usize],
                curve,
                surface(side)?,
            )
        })
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| GeometryError::InvalidGeometry("closed SSI loop requires two pcurves".into()))
}

pub(crate) fn retained_definition(
    graph: &IntersectionGraph,
    definition: u32,
    selected_surfaces: [u32; 2],
) -> Result<IntersectionDefinition, GeometryError> {
    let mut definition_geometry = intersection_definition(graph, definition)?;
    definition_geometry.surfaces = definition_geometry
        .surfaces
        .map(|surface| selected_surfaces[surface as usize]);
    Ok(definition_geometry)
}

pub(crate) fn intersection_definition(
    graph: &IntersectionGraph,
    definition: u32,
) -> Result<IntersectionDefinition, GeometryError> {
    Ok(graph
        .geometry
        .intersections
        .get(definition as usize)
        .ok_or_else(|| GeometryError::MissingReference {
            kind: "intersection definition".into(),
            index: definition,
        })?
        .clone())
}

pub(crate) fn loop_sense(side: usize, reversed: bool) -> Orientation {
    let nominal_sense = if side == 0 {
        Orientation::Forward
    } else {
        Orientation::Reverse
    };
    if reversed {
        reverse(nominal_sense)
    } else {
        nominal_sense
    }
}

pub(crate) fn add_hole_loop(
    out: &mut BrepEnvelope,
    loop_edge: &ClosedBranchEdge,
    side: usize,
    face: u32,
    sense: Orientation,
    tolerance: f64,
) -> Result<(), GeometryError> {
    let loop_id = add_closed_intersection_loop(
        out,
        loop_edge.edge,
        loop_edge.vertex,
        face,
        loop_edge.pcurves[side].clone(),
        sense,
        true,
    )?;
    add_hole_lifted_into_face(out, face, loop_id, tolerance)
}

pub(crate) fn add_hole_lifted_into_face(
    out: &mut BrepEnvelope,
    face: u32,
    loop_id: u32,
    tolerance: f64,
) -> Result<(), GeometryError> {
    let face_record = &out.topology.faces[face as usize];
    let window = face_record.trim.uv_bounds;
    let periods = out.geometry.surface(face_record.surface)?.charts()
        [face_record.trim.chart as usize]
        .periods;
    let mut loop_bounds = topology_loop_bounds(out, loop_id, tolerance)?;
    let mut lift = [0; 2];
    for axis in 0..2 {
        let Some(period) = periods[axis] else {
            continue;
        };
        let turns = ((window[axis].lo - loop_bounds[axis].lo) / period).ceil();
        if loop_bounds[axis].hi + turns * period > window[axis].hi {
            return Err(GeometryError::CoverageGap {
                families: [
                    "closed intersection loop as a hole".into(),
                    format!("loop crossing the chart seam of face '{}'", face_record.key),
                ],
            });
        }
        loop_bounds[axis] = Interval::new(
            loop_bounds[axis].lo + turns * period,
            loop_bounds[axis].hi + turns * period,
        )?;
        lift[axis] = turns as i32;
    }
    let start = out.topology.loops[loop_id as usize].start_halfedge;
    out.topology.halfedges[start as usize]
        .geometry_use
        .periodic_lift = lift;
    widen_trim_bounds(out, face, loop_bounds)?;
    out.topology.faces[face as usize].trim.holes.push(loop_id);
    Ok(())
}

pub(crate) fn add_closed_intersection_loop(
    brep: &mut BrepEnvelope,
    edge: u32,
    vertex: u32,
    face: u32,
    pcurve_geometry: PcurveGeometry,
    sense: Orientation,
    is_hole: bool,
) -> Result<u32, GeometryError> {
    let loop_id = brep.topology.loops.len() as u32;
    let halfedge = brep.topology.halfedges.len() as u32;
    let pcurve = brep.geometry.pcurves.len() as u32;
    brep.geometry.pcurves.push(pcurve_geometry);
    let edge_record = brep.topology.edges.get_mut(edge as usize).ok_or_else(|| {
        GeometryError::InvalidTopology("intersection loop edge is missing".into())
    })?;
    let twin = if edge_record.halfedge == u32::MAX {
        edge_record.halfedge = halfedge;
        None
    } else {
        if edge_record.twin_halfedge.is_some() {
            return Err(GeometryError::InvalidTopology(
                "intersection loop edge is nonmanifold".into(),
            ));
        }
        edge_record.twin_halfedge = Some(halfedge);
        brep.topology.halfedges[edge_record.halfedge as usize].twin = Some(halfedge);
        Some(edge_record.halfedge)
    };
    brep.topology.halfedges.push(HalfEdge {
        id: halfedge,
        from: vertex,
        to: vertex,
        twin,
        next: Some(halfedge),
        prev: Some(halfedge),
        edge,
        face: Some(face),
        loop_ref: Some(loop_id),
        wire_ref: None,
        geometry_use: HalfEdgeGeometryUse {
            sense,
            pcurve: Some(pcurve),
            periodic_lift: [0; 2],
        },
    });
    if brep.topology.vertices[vertex as usize]
        .outgoing_halfedge
        .is_none()
    {
        brep.topology.vertices[vertex as usize].outgoing_halfedge = Some(halfedge);
    }
    brep.topology.loops.push(Loop {
        id: loop_id,
        start_halfedge: halfedge,
        face_ref: face,
        is_hole,
    });
    Ok(loop_id)
}

pub(crate) fn widen_trim_bounds(
    out: &mut BrepEnvelope,
    face: u32,
    loop_bounds: [Interval; 2],
) -> Result<(), GeometryError> {
    for axis in 0..2 {
        let bounds = out.topology.faces[face as usize].trim.uv_bounds[axis];
        out.topology.faces[face as usize].trim.uv_bounds[axis] = Interval::new(
            bounds.lo.min(loop_bounds[axis].lo),
            bounds.hi.max(loop_bounds[axis].hi),
        )?;
    }
    Ok(())
}

pub(crate) fn add_patch_loop(
    out: &mut BrepEnvelope,
    loop_edge: &ClosedBranchEdge,
    side: usize,
    sense: Orientation,
    tolerance: f64,
) -> Result<PatchLoop, GeometryError> {
    let face = out.topology.faces.len() as u32;
    let loop_id = add_closed_intersection_loop(
        out,
        loop_edge.edge,
        loop_edge.vertex,
        face,
        loop_edge.pcurves[side].clone(),
        sense,
        false,
    )?;
    let uv_bounds = topology_loop_bounds(out, loop_id, tolerance)?;
    Ok(PatchLoop {
        face,
        loop_id,
        uv_bounds,
    })
}

pub(crate) fn add_patch_face<S: Copy>(
    out: &mut BrepEnvelope,
    sides: &LoopSides<'_, S>,
    side: usize,
    patch: &PatchLoop,
    branch_index: Option<usize>,
    surface: impl FnOnce(S) -> Result<u32, GeometryError>,
) -> Result<(), GeometryError> {
    let source_face = &sides.inputs[side].topology.faces[sides.face_ids[side] as usize];
    out.topology.faces.push(Face {
        id: patch.face,
        key: match branch_index {
            Some(branch_index) => {
                format!("{}:intersection-patch-{branch_index}", source_face.key)
            }
            None => format!("{}:intersection-patch", source_face.key),
        },
        surface: surface(sides.surfaces[side])?,
        sense: source_face.sense,
        trim: TrimRegion {
            chart: source_face.trim.chart,
            uv_bounds: patch.uv_bounds,
            outer: patch.loop_id,
            holes: Vec::new(),
        },
        shell_ref: Some(0),
        provenance: FaceProvenance {
            sources: vec![brep_face_source(sides.inputs[side], sides.face_ids[side])],
            role: if sides.reverse_selected[side] {
                FaceRole::Cut
            } else {
                FaceRole::Split
            },
            reversed: sides.reverse_selected[side],
        },
    });
    out.topology.shells[0].faces.push(patch.face);
    Ok(())
}
