use super::append::append_analytic_input;
use super::loop_pcurves::{
    exact_closed_circle_pcurve, intersection_loop_sample, remap_closed_loop_pcurve,
    topology_loop_bounds,
};
use super::provenance::reverse;
use crate::brep::{
    unit, BrepEnvelope, CurveGeometry, Edge, EdgeGeometry, Face, FaceProvenance, FaceRole, Frame3,
    GeometryError, GeometryStore, HalfEdge, HalfEdgeGeometryUse, IntersectionDefinition, Loop,
    Orientation, PcurveGeometry, Shell, SolidRegion, Surface, SurfaceGeometry, TrimRegion, Vertex,
};
use crate::intersection::{IntersectionBranch, IntersectionGraph};
use crate::math::{dot, Interval, Point3};
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

struct SphereCandidate {
    score: f64,
    surface: SurfaceGeometry,
    coordinates: Vec<[f64; 2]>,
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
    unclassified: &str,
) -> Result<[bool; 2], GeometryError> {
    let loop_inside_other = [
        classify_point(
            b,
            graph.geometry.surfaces[0].point_at(intersection_loop_sample(graph, branch, 0)?)?,
        )?,
        classify_point(
            a,
            graph.geometry.surfaces[1].point_at(intersection_loop_sample(graph, branch, 1)?)?,
        )?,
    ];
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

pub(crate) fn reframe_sphere_intersection(
    geometry: &mut GeometryStore,
    definition: &mut IntersectionDefinition,
    side: usize,
) -> Result<(), GeometryError> {
    let surface_id = definition.surfaces[side];
    let (frame, radius) = match geometry.surface(surface_id)? {
        SurfaceGeometry::Sphere { frame, radius } => (*frame, *radius),
        _ => {
            return Err(GeometryError::CoverageGap {
                families: ["periodic winding".into(), "non-spherical support".into()],
            })
        }
    };
    let references = [frame.x, frame.y, frame.z];
    let mut best: Option<SphereCandidate> = None;
    for i in -1..=1 {
        for j in -1..=1 {
            for k in -1..=1 {
                if [i, j, k] == [0, 0, 0] {
                    continue;
                }
                let axis = unit(frame.vector([i as f64, j as f64, k as f64]))?;
                let Some(candidate) =
                    scored_sphere_candidate(frame, radius, references, axis, definition)?
                else {
                    continue;
                };
                if best
                    .as_ref()
                    .is_none_or(|best| candidate.score < best.score)
                {
                    best = Some(candidate);
                }
            }
        }
    }
    let SphereCandidate {
        surface: candidate,
        coordinates,
        ..
    } = best.ok_or_else(|| GeometryError::CoverageGap {
        families: ["sphere".into(), "unresolved periodic winding".into()],
    })?;
    geometry.surfaces[surface_id as usize] = candidate;
    for (anchor, uv) in definition.anchors.iter_mut().zip(&coordinates) {
        if side == 0 {
            anchor.uv_a = *uv;
        } else {
            anchor.uv_b = *uv;
        }
    }
    let offset = side * 2;
    for (index, tube) in definition.uv_tubes.iter_mut().enumerate() {
        for axis in 0..2 {
            let a = coordinates[index][axis];
            let b = coordinates[index + 1][axis];
            let padding = (a - b).abs().max(definition.residual_tolerance / radius) * 2.0;
            tube[offset + axis] = Interval::new(a.min(b) - padding, a.max(b) + padding)?;
        }
    }
    definition.validate(geometry)
}

fn scored_sphere_candidate(
    frame: Frame3,
    radius: f64,
    references: [Point3; 3],
    axis: Point3,
    definition: &IntersectionDefinition,
) -> Result<Option<SphereCandidate>, GeometryError> {
    let reference = references
        .into_iter()
        .min_by(|left, right| dot(*left, axis).abs().total_cmp(&dot(*right, axis).abs()))
        .ok_or_else(|| GeometryError::InvalidGeometry("no reference axes".into()))?;
    let candidate = SurfaceGeometry::Sphere {
        frame: Frame3::from_axis(frame.origin, axis, reference)?,
        radius,
    };
    let mut coordinates = Vec::with_capacity(definition.anchors.len());
    let mut hint = None;
    let mut failed = false;
    for anchor in &definition.anchors {
        match candidate.project(anchor.point, hint) {
            Ok(uv) => {
                coordinates.push(uv);
                hint = Some(uv);
            }
            Err(_) => {
                failed = true;
                break;
            }
        }
    }
    if failed {
        return Ok(None);
    }
    let Some(last) = coordinates.last() else {
        return Ok(None);
    };
    let longitude_span = (last[0] - coordinates[0][0]).abs();
    let pole_proximity = coordinates
        .iter()
        .map(|uv| uv[1].abs())
        .fold(0.0_f64, f64::max);
    if longitude_span >= std::f64::consts::PI
        || pole_proximity >= std::f64::consts::FRAC_PI_2 - 1.0e-6
    {
        return Ok(None);
    }
    let score = pole_proximity + longitude_span;
    Ok(Some(SphereCandidate {
        score,
        surface: candidate,
        coordinates,
    }))
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
    let loop_bounds = topology_loop_bounds(out, loop_id, tolerance)?;
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
