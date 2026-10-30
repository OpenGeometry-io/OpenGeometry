use super::band_rim::{rim_edge, touching_faces};
use crate::brep::{
    Accuracy, BrepEnvelope, Curve, CurveGeometry, Edge, EdgeGeometry, GeometryError, Surface,
    SurfaceGeometry,
};
use crate::intersection::{IntersectionBranch, IntersectionGraph};
use crate::math::{add, cross, dot, norm, scale, sub, Interval, Point3};
use crate::query::{classify_point, PointClassification};
use std::collections::{BTreeMap, BTreeSet};

pub(super) type Pair<'a> = ([u32; 2], &'a IntersectionGraph, Vec<Piece>);

pub(super) struct Piece {
    pub(super) branch: IntersectionBranch,
    pub(super) reversed: bool,
}

impl Piece {
    pub(super) fn ends(&self) -> [Point3; 2] {
        let [start, end] = self.branch.endpoints;
        if self.reversed {
            [end, start]
        } else {
            [start, end]
        }
    }
}

pub(super) struct ChainSide {
    pub(super) face: u32,
    pub(super) pairs: [usize; 2],
    pub(super) loop_halfedges: Vec<u32>,
}

pub(super) struct Corner {
    pub(super) point: Point3,
    pub(super) parameter: f64,
}

pub(super) struct Notch {
    pub(super) cap: u32,
    pub(super) rims: [u32; 2],
    pub(super) bridges: Vec<(u32, usize)>,
}

pub(super) struct Chain {
    pub(super) band: [u32; 2],
    pub(super) sides: Vec<ChainSide>,
    pub(super) corners: BTreeMap<u32, [Corner; 2]>,
    pub(super) notch: Option<Notch>,
}

impl Chain {
    pub(super) fn bridge_side(&self, edge: u32) -> Option<usize> {
        let notch = self.notch.as_ref()?;
        notch
            .bridges
            .iter()
            .find_map(|(bridge, side)| (*bridge == edge).then_some(*side))
    }
}

pub(super) fn recognized_chain<'a>(
    host: &BrepEnvelope,
    cutter: &BrepEnvelope,
    graphs: &[([u32; 2], &'a IntersectionGraph)],
) -> Result<Option<(Vec<Pair<'a>>, Chain)>, GeometryError> {
    let accuracy = Accuracy::combined(host.accuracy, cutter.accuracy);
    let touching = touching_faces(host, cutter, 4.0 * accuracy.geometric);
    let graphs = graphs
        .iter()
        .filter(|(faces, _)| !touching.contains(&faces[1]))
        .collect::<Vec<_>>();
    let cap = cap_face(&graphs);
    let mut pairs = Vec::with_capacity(graphs.len());
    for (faces, graph) in graphs {
        if Some(faces[0]) == cap {
            continue;
        }
        let Some(pieces) = piece_path(graph, 16.0 * accuracy.geometric) else {
            return Ok(None);
        };
        pairs.push((*faces, *graph, pieces));
    }
    let Some(chain) = chain(host, cutter, &pairs, accuracy, (&touching, cap))? else {
        return Ok(None);
    };
    Ok(Some((pairs, chain)))
}

fn piece_path(graph: &IntersectionGraph, reach: f64) -> Option<Vec<Piece>> {
    if graph.coincident || !graph.contacts.is_empty() || graph.branches.is_empty() {
        return None;
    }
    let near = |a: Point3, b: Point3| norm(sub(a, b)) <= reach;
    let piece = |index: usize, reversed: bool| {
        let branch = &graph.branches[index];
        Piece {
            branch: IntersectionBranch {
                curve: branch.curve,
                pcurves: branch.pcurves,
                range: branch.range,
                endpoints: branch.endpoints,
            },
            reversed,
        }
    };
    let alone = |point: Point3, skip: usize| {
        graph.branches.iter().enumerate().all(|(index, branch)| {
            index == skip || branch.endpoints.iter().all(|end| !near(point, *end))
        })
    };
    let (first, reversed) = (0..graph.branches.len())
        .flat_map(|index| [(index, false), (index, true)])
        .find(|&(index, reversed)| alone(piece(index, reversed).ends()[0], index))?;
    let mut path = vec![piece(first, reversed)];
    let mut remaining = (0..graph.branches.len())
        .filter(|index| *index != first)
        .collect::<Vec<_>>();
    while !remaining.is_empty() {
        let tail = path[path.len() - 1].ends()[1];
        let (slot, reversed) = remaining.iter().enumerate().find_map(|(slot, index)| {
            let ends = graph.branches[*index].endpoints;
            (near(tail, ends[0]) || near(tail, ends[1])).then_some((slot, !near(tail, ends[0])))
        })?;
        let next = piece(remaining.remove(slot), reversed);
        let last = path.len() - 1;
        match merged(graph, &path[last], &next) {
            Some(joined) => path[last] = joined,
            None => path.push(next),
        }
    }
    Some(path)
}

fn merged(graph: &IntersectionGraph, first: &Piece, second: &Piece) -> Option<Piece> {
    let curve = graph.geometry.curves.get(first.branch.curve as usize)?;
    if first.reversed != second.reversed
        || graph.geometry.curves.get(second.branch.curve as usize)? != curve
    {
        return None;
    }
    let (low, high) = if first.reversed {
        (&second.branch, &first.branch)
    } else {
        (&first.branch, &second.branch)
    };
    let shift = if matches!(curve, CurveGeometry::Circle { .. }) {
        let tau = std::f64::consts::TAU;
        ((low.range.hi - high.range.lo) / tau).round() * tau
    } else {
        0.0
    };
    if (high.range.lo + shift - low.range.hi).abs() > 1e-6 {
        return None;
    }
    Some(Piece {
        branch: IntersectionBranch {
            curve: low.curve,
            pcurves: low.pcurves,
            range: Interval::new(low.range.lo, high.range.hi + shift).ok()?,
            endpoints: [low.endpoints[0], high.endpoints[1]],
        },
        reversed: first.reversed,
    })
}

pub(super) fn path_ends(pieces: &[Piece]) -> Option<[Point3; 2]> {
    Some([pieces.first()?.ends()[0], pieces.last()?.ends()[1]])
}

fn cap_face(graphs: &[&([u32; 2], &IntersectionGraph)]) -> Option<u32> {
    let mut faces = BTreeMap::<u32, BTreeSet<u32>>::new();
    for (pair, _) in graphs {
        faces.entry(pair[0]).or_default().insert(pair[1]);
    }
    if faces.len() != 3 {
        return None;
    }
    faces
        .iter()
        .find(|(face, cutters)| {
            faces.iter().all(|(other, set)| {
                other == *face || (cutters.is_subset(set) && cutters.len() < set.len())
            })
        })
        .map(|(face, _)| *face)
}

fn chain(
    host: &BrepEnvelope,
    cutter: &BrepEnvelope,
    pairs: &[Pair<'_>],
    accuracy: Accuracy,
    (touching, cap): (&[u32], Option<u32>),
) -> Result<Option<Chain>, GeometryError> {
    let Some(band) = band_faces(pairs) else {
        return Ok(None);
    };
    let Some(sides) = chain_sides(cutter, pairs, band) else {
        return Ok(None);
    };
    let curved = sides.iter().any(|side| {
        !matches!(
            cutter.geometry.surfaces[cutter.topology.faces[side.face as usize].surface as usize],
            SurfaceGeometry::Plane { .. }
        )
    });
    let Some(bridges) = prism_bridges(cutter, &sides) else {
        return Ok(None);
    };
    if sides.len() < 2
        || !curved
        || !caps_outside(host, cutter, &sides, touching)?
        || touching.iter().any(|face| {
            bridges
                .iter()
                .any(|bridge| bridge_face(cutter, &sides, *bridge) != Some(*face))
        })
    {
        return Ok(None);
    }
    let Some(mut corners) = corners(host, cutter, pairs, band, &sides, accuracy)? else {
        return Ok(None);
    };
    let notch = match bridges.len() {
        0 if cap.is_none() && touching.is_empty() => None,
        2 => {
            let place = (pairs, band, sides.as_slice());
            let Some(notch) = rim_corners(host, cutter, place, bridges, accuracy, &mut corners)?
            else {
                return Ok(None);
            };
            if cap.is_some_and(|face| face != notch.cap) {
                return Ok(None);
            }
            Some(notch)
        }
        _ => return Ok(None),
    };
    if band_edges_inside(host, cutter, band, notch.as_ref().map(|notch| notch.cap))? {
        return Ok(None);
    }
    Ok(Some(Chain {
        band,
        sides,
        corners,
        notch,
    }))
}

fn bridge_face(
    cutter: &BrepEnvelope,
    sides: &[ChainSide],
    (edge, side): (u32, usize),
) -> Option<u32> {
    let halfedge = sides[side]
        .loop_halfedges
        .iter()
        .find(|halfedge| cutter.topology.halfedges[**halfedge as usize].edge == edge)?;
    twin_face(cutter, *halfedge)
}

fn band_faces(pairs: &[Pair<'_>]) -> Option<[u32; 2]> {
    let mut band = Vec::new();
    for (faces, _, _) in pairs {
        if !band.contains(&faces[0]) {
            band.push(faces[0]);
        }
    }
    band.try_into().ok()
}

fn chain_sides(
    cutter: &BrepEnvelope,
    pairs: &[Pair<'_>],
    band: [u32; 2],
) -> Option<Vec<ChainSide>> {
    let mut sides = BTreeMap::<u32, [Option<usize>; 2]>::new();
    for (index, (faces, _, _)) in pairs.iter().enumerate() {
        let slot = &mut sides.entry(faces[1]).or_default()[usize::from(faces[0] == band[1])];
        if slot.replace(index).is_some() {
            return None;
        }
    }
    sides
        .into_iter()
        .map(|(face, slots)| {
            let pairs = [slots[0]?, slots[1]?];
            let face_record = cutter.topology.faces.get(face as usize)?;
            if !face_record.trim.holes.is_empty() {
                return None;
            }
            let start = cutter.topology.loops[face_record.trim.outer as usize].start_halfedge;
            let mut loop_halfedges = vec![start];
            let mut cursor = cutter.topology.halfedges[start as usize].next?;
            while cursor != start && loop_halfedges.len() <= cutter.topology.halfedges.len() {
                loop_halfedges.push(cursor);
                cursor = cutter.topology.halfedges[cursor as usize].next?;
            }
            Some(ChainSide {
                face,
                pairs,
                loop_halfedges,
            })
        })
        .collect()
}

pub(super) fn twin_face(cutter: &BrepEnvelope, halfedge: u32) -> Option<u32> {
    let twin = cutter.topology.halfedges[halfedge as usize].twin?;
    cutter.topology.halfedges[twin as usize].face
}

pub(super) fn shared_edge(cutter: &BrepEnvelope, sides: &[ChainSide], halfedge: u32) -> bool {
    twin_face(cutter, halfedge).is_some_and(|face| sides.iter().any(|side| side.face == face))
}

pub(super) fn edge_line(cutter: &BrepEnvelope, edge: u32) -> Option<(Point3, Point3)> {
    let EdgeGeometry::Curve { curve, .. } = cutter.topology.edges[edge as usize].geometry else {
        return None;
    };
    match cutter.geometry.curves[curve as usize] {
        CurveGeometry::Line { origin, direction } => Some((origin, direction)),
        _ => None,
    }
}

fn prism_bridges(cutter: &BrepEnvelope, sides: &[ChainSide]) -> Option<Vec<(u32, usize)>> {
    let line = |halfedge: u32| edge_line(cutter, cutter.topology.halfedges[halfedge as usize].edge);
    let reference = sides
        .iter()
        .flat_map(|side| side.loop_halfedges.iter())
        .find(|halfedge| shared_edge(cutter, sides, **halfedge))
        .and_then(|halfedge| line(*halfedge))?
        .1;
    let mut bridges = Vec::new();
    for (index, side) in sides.iter().enumerate() {
        let shared = side
            .loop_halfedges
            .iter()
            .filter(|halfedge| shared_edge(cutter, sides, **halfedge))
            .count();
        let laterals = side
            .loop_halfedges
            .iter()
            .filter(|halfedge| {
                line(**halfedge).is_some_and(|(_, along)| norm(cross(reference, along)) <= 1e-10)
            })
            .collect::<Vec<_>>();
        let open = laterals
            .iter()
            .filter(|halfedge| !shared_edge(cutter, sides, ***halfedge))
            .map(|halfedge| cutter.topology.halfedges[**halfedge as usize].edge)
            .collect::<Vec<_>>();
        if side.loop_halfedges.len() != 4 || laterals.len() != 2 || shared + open.len() != 2 {
            return None;
        }
        bridges.extend(open.into_iter().map(|edge| (edge, index)));
        let surface =
            &cutter.geometry.surfaces[cutter.topology.faces[side.face as usize].surface as usize];
        let parallel = match surface {
            SurfaceGeometry::Plane { frame } => dot(frame.z, reference).abs() <= 1e-10,
            SurfaceGeometry::Cylinder { frame, .. } => norm(cross(frame.z, reference)) <= 1e-10,
            _ => false,
        };
        if !parallel {
            return None;
        }
    }
    Some(bridges)
}

fn caps_outside(
    host: &BrepEnvelope,
    cutter: &BrepEnvelope,
    sides: &[ChainSide],
    touching: &[u32],
) -> Result<bool, GeometryError> {
    for face in &cutter.topology.faces {
        if sides.iter().any(|side| side.face == face.id) || touching.contains(&face.id) {
            continue;
        }
        let start = cutter.topology.loops[face.trim.outer as usize].start_halfedge;
        let points = cutter
            .topology
            .halfedges
            .iter()
            .filter(|halfedge| {
                halfedge.loop_ref == cutter.topology.halfedges[start as usize].loop_ref
            })
            .map(|halfedge| cutter.topology.vertices[halfedge.from as usize].position)
            .collect::<Vec<_>>();
        let centre = scale(
            points.iter().fold([0.0; 3], |sum, point| add(sum, *point)),
            1.0 / points.len().max(1) as f64,
        );
        if classify_point(host, centre)? != PointClassification::Outside {
            return Ok(false);
        }
    }
    Ok(true)
}

fn corners(
    host: &BrepEnvelope,
    cutter: &BrepEnvelope,
    pairs: &[Pair<'_>],
    band: [u32; 2],
    sides: &[ChainSide],
    accuracy: Accuracy,
) -> Result<Option<BTreeMap<u32, [Corner; 2]>>, GeometryError> {
    let mut corners = BTreeMap::new();
    for side in sides {
        for &halfedge in &side.loop_halfedges {
            let edge = cutter.topology.halfedges[halfedge as usize].edge;
            if !shared_edge(cutter, sides, halfedge) || corners.contains_key(&edge) {
                continue;
            }
            let Some((origin, direction)) = edge_line(cutter, edge) else {
                return Ok(None);
            };
            let mut found = Vec::with_capacity(2);
            for (index, host_face) in band.into_iter().enumerate() {
                let Some([first, last]) = path_ends(&pairs[side.pairs[index]].2) else {
                    return Ok(None);
                };
                let nearest = if line_distance(first, origin, direction)
                    <= line_distance(last, origin, direction)
                {
                    first
                } else {
                    last
                };
                let surface = host
                    .geometry
                    .surface(host.topology.faces[host_face as usize].surface)?;
                let Some(parameter) = surface_corner(
                    surface,
                    &CurveGeometry::Line { origin, direction },
                    dot(sub(nearest, origin), direction),
                )?
                else {
                    return Ok(None);
                };
                let point = add(origin, scale(direction, parameter));
                if norm(sub(point, nearest)) > 8.0 * accuracy.geometric {
                    return Ok(None);
                }
                found.push(Corner { point, parameter });
            }
            let Ok(pair) = <[Corner; 2]>::try_from(found) else {
                return Ok(None);
            };
            corners.insert(edge, pair);
        }
    }
    Ok(Some(corners))
}

fn rim_corners(
    host: &BrepEnvelope,
    cutter: &BrepEnvelope,
    (pairs, band, sides): (&[Pair<'_>], [u32; 2], &[ChainSide]),
    bridges: Vec<(u32, usize)>,
    accuracy: Accuracy,
    corners: &mut BTreeMap<u32, [Corner; 2]>,
) -> Result<Option<Notch>, GeometryError> {
    let (mut rims, mut cap) = ([None; 2], None);
    for &(edge, side) in &bridges {
        let Some((origin, direction)) = edge_line(cutter, edge) else {
            return Ok(None);
        };
        let face = &cutter.topology.faces[sides[side].face as usize];
        let surface = cutter.geometry.surface(face.surface)?;
        let mut found = Vec::with_capacity(2);
        for (index, host_face) in band.into_iter().enumerate() {
            let Some([first, last]) = path_ends(&pairs[sides[side].pairs[index]].2) else {
                return Ok(None);
            };
            let end = if line_distance(first, origin, direction)
                <= line_distance(last, origin, direction)
            {
                first
            } else {
                last
            };
            let Some((rim, curve, start, twin)) =
                rim_edge(host, host_face, end, 16.0 * accuracy.geometric)?
            else {
                return Ok(None);
            };
            let Some(parameter) = surface_corner(surface, curve, start)? else {
                return Ok(None);
            };
            let point = curve.elementary_point(parameter)?;
            if norm(sub(point, end)) > 8.0 * accuracy.geometric
                || *rims[index].get_or_insert(rim) != rim
                || *cap.get_or_insert(twin) != twin
            {
                return Ok(None);
            }
            found.push(Corner { point, parameter });
        }
        let Ok(pair) = <[Corner; 2]>::try_from(found) else {
            return Ok(None);
        };
        corners.insert(edge, pair);
    }
    let (Some(cap), [Some(first), Some(second)]) = (cap, rims) else {
        return Ok(None);
    };
    if !matches!(
        host.geometry.surfaces[host.topology.faces[cap as usize].surface as usize],
        SurfaceGeometry::Plane { .. }
    ) {
        return Ok(None);
    }
    Ok(Some(Notch {
        cap,
        rims: [first, second],
        bridges,
    }))
}

pub(super) fn line_distance(point: Point3, origin: Point3, direction: Point3) -> f64 {
    let offset = sub(point, origin);
    norm(sub(offset, scale(direction, dot(offset, direction))))
}

pub(super) fn surface_corner(
    surface: &SurfaceGeometry,
    curve: &CurveGeometry,
    start: f64,
) -> Result<Option<f64>, GeometryError> {
    let mut parameter = start;
    let mut hint = None;
    for _ in 0..8 {
        let point = curve.elementary_point(parameter)?;
        let uv = surface.project(point, hint)?;
        hint = Some(uv);
        let normal = surface.normal_at(uv)?;
        let along = dot(curve.elementary_tangent(parameter)?, normal);
        if along.abs() <= 1e-6 {
            return Ok(None);
        }
        let distance = dot(sub(point, surface.point_at(uv)?), normal);
        parameter -= distance / along;
    }
    Ok(Some(parameter))
}

pub(super) fn band_edges_inside(
    host: &BrepEnvelope,
    cutter: &BrepEnvelope,
    band: [u32; 2],
    cap: Option<u32>,
) -> Result<bool, GeometryError> {
    for halfedge in &host.topology.halfedges {
        let Some(face) = halfedge.face.filter(|face| band.contains(face)) else {
            continue;
        };
        let edge = &host.topology.edges[halfedge.edge as usize];
        let twin = halfedge
            .twin
            .and_then(|twin| host.topology.halfedges[twin as usize].face);
        if (edge.chart_seam && seam_twin_face(host, edge) == Some(face))
            || (cap.is_some() && twin == cap)
        {
            continue;
        }
        let EdgeGeometry::Curve { curve, range } = edge.geometry else {
            continue;
        };
        let view = host.geometry.curve(curve)?;
        for step in 1..8 {
            let point = view.point_at(range.lo + range.width() * f64::from(step) / 8.0)?;
            if classify_point(cutter, point)? == PointClassification::Inside {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

pub(super) fn seam_twin_face(host: &BrepEnvelope, edge: &Edge) -> Option<u32> {
    let first = host.topology.halfedges.get(edge.halfedge as usize)?;
    let second = host.topology.halfedges.get(edge.twin_halfedge? as usize)?;
    (first.face == second.face).then_some(first.face?)
}
