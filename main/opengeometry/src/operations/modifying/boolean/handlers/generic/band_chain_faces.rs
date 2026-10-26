use super::band_chain::{edge_line, path_ends, shared_edge, Chain, Pair, Piece};
use super::band_chain_seam::{clear_band_seam, set_periodic_lifts, Step};
use crate::brep::{
    boundary, Accuracy, BrepEnvelope, Builder, Curve, CurveGeometry, EdgeGeometry, Face,
    FaceProvenance, FaceRole, GeometryError, GeometryStore, IntersectionDefinition, Orientation,
    PcurveGeometry, TraceAnchor, TrimRegion, Use,
};
use crate::intersection::IntersectionGraph;
use crate::math::{add, dot, norm, scale, sub, Interval, Point3};
use crate::operations::modifying::boolean::assembly::{
    append_analytic_input, append_closed_branch_geometry, finish_analytic_result,
    retained_definition, reverse_face,
};
use crate::operations::modifying::boolean::operands::brep_face_source;
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};
use std::collections::BTreeMap;

struct PieceEdge {
    edge: u32,
    ends: [u32; 2],
    pcurves: [PcurveGeometry; 2],
}

struct BranchPath {
    pieces: Vec<PieceEdge>,
    vertices: Vec<u32>,
}

struct CornerEdge {
    edge: u32,
    ends: [u32; 2],
}

struct ChainBuild<'a> {
    cutter: &'a BrepEnvelope,
    chain: &'a Chain,
    vertices: BTreeMap<(u32, usize), u32>,
    paths: Vec<[BranchPath; 2]>,
    corner_edges: BTreeMap<u32, CornerEdge>,
}

pub(super) fn build_chain_cut(
    host: &BrepEnvelope,
    cutter: &BrepEnvelope,
    pairs: &[Pair<'_>],
    chain: &Chain,
    operation: BooleanOp,
    id: String,
) -> Result<Option<BooleanResult>, GeometryError> {
    let mut out = BrepEnvelope::new(id, chain_accuracy(host, cutter))?;
    append_analytic_input(&mut out, host)?;
    let side_surfaces = chain
        .sides
        .iter()
        .map(|side| {
            let surface = out.geometry.surfaces.len() as u32;
            out.geometry.surfaces.push(
                cutter.geometry.surfaces
                    [cutter.topology.faces[side.face as usize].surface as usize]
                    .clone(),
            );
            surface
        })
        .collect::<Vec<_>>();
    let mut builder = Builder { brep: out };
    let mut vertices = BTreeMap::new();
    for (edge, corners) in &chain.corners {
        for (index, corner) in corners.iter().enumerate() {
            vertices.insert((*edge, index), builder.vertex(corner.point));
        }
    }
    let mut build = ChainBuild {
        cutter,
        chain,
        vertices,
        paths: Vec::new(),
        corner_edges: BTreeMap::new(),
    };
    for (side_index, surface) in side_surfaces.iter().enumerate() {
        let mut side_paths = Vec::with_capacity(2);
        for band in 0..2 {
            let place = PathPlace {
                pairs,
                side: side_index,
                band,
                side_surface: *surface,
            };
            let Some(path) = add_branch_path(&mut builder, &build, &place)? else {
                return Ok(None);
            };
            side_paths.push(path);
        }
        let Ok(side_paths) = <[BranchPath; 2]>::try_from(side_paths) else {
            return Ok(None);
        };
        build.paths.push(side_paths);
    }
    if !add_corner_edges(&mut builder, &mut build) {
        return Ok(None);
    }
    let mut cut_faces = Vec::with_capacity(chain.sides.len());
    let mut directions = Vec::with_capacity(chain.sides.len());
    for (index, surface) in side_surfaces.iter().enumerate() {
        let mut steps = [None, None];
        let Some(face) = add_cut_face(&mut builder, &build, index, *surface, &mut steps)? else {
            return Ok(None);
        };
        cut_faces.push(face);
        directions.push(steps);
    }
    for band in 0..2 {
        if !add_band_cut(&mut builder, &build, band, &directions)? {
            return Ok(None);
        }
    }
    let mut out = builder.brep;
    for face in cut_faces {
        reverse_face(&mut out, face);
    }
    finish_analytic_result(out, host, cutter, true, operation, false).map(Some)
}

fn chain_accuracy(host: &BrepEnvelope, cutter: &BrepEnvelope) -> Accuracy {
    Accuracy::combined(host.accuracy, cutter.accuracy)
}

fn side_corner_edges(build: &ChainBuild<'_>, side: usize) -> Vec<u32> {
    let cutter = build.cutter;
    build.chain.sides[side]
        .loop_halfedges
        .iter()
        .filter(|halfedge| shared_edge(cutter, &build.chain.sides, **halfedge))
        .map(|halfedge| cutter.topology.halfedges[*halfedge as usize].edge)
        .collect()
}

struct PathPlace<'a, 'b> {
    pairs: &'a [Pair<'b>],
    side: usize,
    band: usize,
    side_surface: u32,
}

struct Directed {
    from: u32,
    to: u32,
    steps: Vec<Step>,
}

fn add_branch_path(
    builder: &mut Builder,
    build: &ChainBuild<'_>,
    place: &PathPlace<'_, '_>,
) -> Result<Option<BranchPath>, GeometryError> {
    let (faces, graph, pieces) = &place.pairs[build.chain.sides[place.side].pairs[place.band]];
    let corner_edges = side_corner_edges(build, place.side);
    let (Some([start, _]), &[first, second]) = (path_ends(pieces), corner_edges.as_slice()) else {
        return Ok(None);
    };
    let distance = |edge: u32| norm(sub(build.chain.corners[&edge][place.band].point, start));
    let (first, last) = if distance(first) <= distance(second) {
        (first, second)
    } else {
        (second, first)
    };
    let mut vertices = vec![build.vertices[&(first, place.band)]];
    for pair in pieces.windows(2) {
        let joint = scale(add(pair[0].ends()[1], pair[1].ends()[0]), 0.5);
        vertices.push(builder.vertex(joint));
    }
    vertices.push(build.vertices[&(last, place.band)]);
    let host_surface = builder.brep.topology.faces[faces[0] as usize].surface;
    let selected = [host_surface, place.side_surface];
    let mut edges = Vec::with_capacity(pieces.len());
    for (index, piece) in pieces.iter().enumerate() {
        let ends = [vertices[index], vertices[index + 1]];
        let Some(edge) = add_piece_edge(builder, graph, piece, selected, ends)? else {
            return Ok(None);
        };
        edges.push(edge);
    }
    Ok(Some(BranchPath {
        pieces: edges,
        vertices,
    }))
}

fn add_piece_edge(
    builder: &mut Builder,
    graph: &IntersectionGraph,
    piece: &Piece,
    selected: [u32; 2],
    ends: [u32; 2],
) -> Result<Option<PieceEdge>, GeometryError> {
    let geometric = builder.brep.accuracy.geometric;
    let closed = append_closed_branch_geometry(
        &mut builder.brep,
        graph,
        &piece.branch,
        geometric,
        |_, definition| retained_definition(graph, definition, selected),
        |part| Ok(selected[part]),
    )?;
    builder.brep.topology.vertices.pop();
    let EdgeGeometry::Curve { curve, range } =
        builder.brep.topology.edges[closed.edge as usize].geometry
    else {
        return Ok(None);
    };
    let starts = if piece.reversed {
        [range.hi, range.lo]
    } else {
        [range.lo, range.hi]
    };
    let mut welded = Vec::with_capacity(2);
    for (vertex, start) in ends.into_iter().zip(starts) {
        let target = builder.brep.topology.vertices[vertex as usize].position;
        let Some(parameter) = curve_parameter(&builder.brep, curve, target, start)? else {
            return Ok(None);
        };
        welded.push((parameter, vertex));
    }
    welded.sort_by(|a, b| a.0.total_cmp(&b.0));
    let [(lo, low), (hi, high)] = welded[..] else {
        return Ok(None);
    };
    builder.brep.topology.edges[closed.edge as usize].geometry = EdgeGeometry::Curve {
        curve,
        range: Interval::new(lo, hi)?,
    };
    if let CurveGeometry::Intersection { definition } = builder.brep.geometry.curves[curve as usize]
    {
        let Some(trimmed) = trimmed_definition(&builder.brep.geometry, definition, lo, hi)? else {
            return Ok(None);
        };
        builder.brep.geometry.intersections[definition as usize] = trimmed;
    }
    Ok(Some(PieceEdge {
        edge: closed.edge,
        ends: [low, high],
        pcurves: closed.pcurves,
    }))
}

fn trimmed_definition(
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

fn curve_parameter(
    brep: &BrepEnvelope,
    curve: u32,
    target: Point3,
    start: f64,
) -> Result<Option<f64>, GeometryError> {
    let view = brep.geometry.curve(curve)?;
    let mut parameter = start;
    for _ in 0..8 {
        let tangent = view.tangent_at(parameter)?;
        let speed = dot(tangent, tangent);
        if speed <= 0.0 {
            return Ok(None);
        }
        parameter += dot(sub(target, view.point_at(parameter)?), tangent) / speed;
    }
    let reached = norm(sub(view.point_at(parameter)?, target)) <= brep.accuracy.geometric;
    Ok(reached.then_some(parameter))
}

fn add_corner_edges(builder: &mut Builder, build: &mut ChainBuild<'_>) -> bool {
    for (edge, corners) in &build.chain.corners {
        let Some((origin, direction)) = edge_line(build.cutter, *edge) else {
            return false;
        };
        let (lo, hi) = if corners[0].parameter <= corners[1].parameter {
            (0, 1)
        } else {
            (1, 0)
        };
        let Ok(range) = Interval::new(corners[lo].parameter, corners[hi].parameter) else {
            return false;
        };
        let id = builder.edge(CurveGeometry::Line { origin, direction }, range, false);
        build.corner_edges.insert(
            *edge,
            CornerEdge {
                edge: id,
                ends: [build.vertices[&(*edge, lo)], build.vertices[&(*edge, hi)]],
            },
        );
    }
    true
}

fn nearest_band(build: &ChainBuild<'_>, edge: u32, point: Point3) -> usize {
    let corners = &build.chain.corners[&edge];
    usize::from(norm(sub(corners[1].point, point)) < norm(sub(corners[0].point, point)))
}

fn add_cut_face(
    builder: &mut Builder,
    build: &ChainBuild<'_>,
    side_index: usize,
    surface: u32,
    directions: &mut [Option<Directed>; 2],
) -> Result<Option<u32>, GeometryError> {
    let cutter = build.cutter;
    let side = &build.chain.sides[side_index];
    let Some(uses) = cut_face_uses(build, side_index, directions) else {
        return Ok(None);
    };
    let source = &cutter.topology.faces[side.face as usize];
    let face = builder.brep.topology.faces.len() as u32;
    let outer = builder.face_loop(face, false, uses)?;
    builder.brep.topology.faces.push(Face {
        id: face,
        key: format!("{}:through-cut", source.key),
        surface,
        sense: source.sense,
        trim: TrimRegion {
            chart: source.trim.chart,
            uv_bounds: source.trim.uv_bounds,
            outer,
            holes: Vec::new(),
        },
        shell_ref: Some(0),
        provenance: FaceProvenance {
            sources: vec![brep_face_source(cutter, source.id)],
            role: FaceRole::Cut,
            reversed: true,
        },
    });
    builder.brep.topology.shells[0].faces.push(face);
    set_periodic_lifts(&mut builder.brep, outer, false)?;
    Ok(Some(face))
}

fn cut_face_uses(
    build: &ChainBuild<'_>,
    side_index: usize,
    directions: &mut [Option<Directed>; 2],
) -> Option<Vec<Use>> {
    let cutter = build.cutter;
    let halfedges = &build.chain.sides[side_index].loop_halfedges;
    let mut uses = Vec::with_capacity(4);
    for (position, &halfedge) in halfedges.iter().enumerate() {
        let record = &cutter.topology.halfedges[halfedge as usize];
        let start = cutter.topology.vertices[record.from as usize].position;
        if let Some(corner) = build.corner_edges.get(&record.edge) {
            let band = nearest_band(build, record.edge, start);
            let from = build.vertices[&(record.edge, band)];
            let sense = if from == corner.ends[0] {
                Orientation::Forward
            } else {
                Orientation::Reverse
            };
            let pcurve = cutter.geometry.pcurves[record.geometry_use.pcurve? as usize].clone();
            let to = build.vertices[&(record.edge, 1 - band)];
            uses.push(boundary(corner.edge, from, to, sense, pcurve));
            continue;
        }
        let neighbour = |offset: usize| {
            cutter.topology.halfedges[halfedges[(position + offset) % halfedges.len()] as usize]
                .edge
        };
        let (previous, next) = (neighbour(halfedges.len() - 1), neighbour(1));
        if !build.chain.corners.contains_key(&previous) || !build.chain.corners.contains_key(&next)
        {
            return None;
        }
        let band = nearest_band(build, previous, start);
        let from = build.vertices[&(previous, band)];
        let to = build.vertices[&(next, band)];
        let (path_uses, steps) = path_steps(&build.paths[side_index][band], from, to)?;
        uses.extend(path_uses);
        directions[band] = Some(Directed { from, to, steps });
    }
    Some(uses)
}

fn path_steps(path: &BranchPath, from: u32, to: u32) -> Option<(Vec<Use>, Vec<Step>)> {
    let forward = path.vertices.first() == Some(&from) && path.vertices.last() == Some(&to);
    if !forward && (path.vertices.first() != Some(&to) || path.vertices.last() != Some(&from)) {
        return None;
    }
    let mut order = (0..path.pieces.len()).collect::<Vec<_>>();
    if !forward {
        order.reverse();
    }
    let mut uses = Vec::with_capacity(order.len());
    let mut steps = Vec::with_capacity(order.len());
    for index in order {
        let piece = &path.pieces[index];
        let (a, b) = (path.vertices[index], path.vertices[index + 1]);
        let (a, b) = if forward { (a, b) } else { (b, a) };
        let sense = match piece.ends {
            ends if ends == [a, b] => Orientation::Forward,
            ends if ends == [b, a] => Orientation::Reverse,
            _ => return None,
        };
        uses.push(boundary(piece.edge, a, b, sense, piece.pcurves[1].clone()));
        steps.push(Step {
            edge: piece.edge,
            from: a,
            to: b,
            sense,
            pcurve: piece.pcurves[0].clone(),
        });
    }
    Some((uses, steps))
}

fn add_band_cut(
    builder: &mut Builder,
    build: &ChainBuild<'_>,
    band: usize,
    directions: &[[Option<Directed>; 2]],
) -> Result<bool, GeometryError> {
    let Some(paths) = directions
        .iter()
        .map(|steps| steps[band].as_ref())
        .collect::<Option<Vec<_>>>()
    else {
        return Ok(false);
    };
    let mut order = vec![0];
    while order.len() < paths.len() {
        let to = paths[order[order.len() - 1]].to;
        let Some(next) =
            (0..paths.len()).find(|side| paths[*side].from == to && !order.contains(side))
        else {
            return Ok(false);
        };
        order.push(next);
    }
    if paths[order[order.len() - 1]].to != paths[order[0]].from {
        return Ok(false);
    }
    let steps = order
        .iter()
        .flat_map(|side| paths[*side].steps.iter())
        .collect::<Vec<_>>();
    let host_face = build.chain.band[band];
    let uses = steps
        .iter()
        .map(|step| {
            boundary(
                step.edge,
                step.from,
                step.to,
                step.sense,
                step.pcurve.clone(),
            )
        })
        .collect::<Vec<Use>>();
    let hole = builder.face_loop(host_face, true, uses)?;
    if !clear_band_seam(&mut builder.brep, host_face, hole)? {
        return Ok(false);
    }
    set_periodic_lifts(&mut builder.brep, hole, true)?;
    let face = &mut builder.brep.topology.faces[host_face as usize];
    face.trim.holes.push(hole);
    face.provenance.role = FaceRole::Split;
    Ok(true)
}
