use super::band_chain::Notch;
use super::band_chain_seam::{clear_band_seam, relift, Directed, Step};
use super::band_rim::rim_parameter;
use crate::brep::{
    boundary, BrepEnvelope, Builder, EdgeGeometry, FaceRole, GeometryError, Loop, Orientation,
    SurfaceGeometry, Topology, Use,
};
use crate::math::Interval;
use crate::operations::modifying::boolean::assembly::reverse;
use std::collections::BTreeSet;

pub(super) fn cut_notch(
    builder: &mut Builder,
    band: [u32; 2],
    notch: &Notch,
    directions: &[[Option<Directed>; 2]],
    bridges: Vec<Use>,
) -> Result<bool, GeometryError> {
    let mut rims = Vec::with_capacity(2);
    for (index, face) in band.into_iter().enumerate() {
        let Some(paths) = directions
            .iter()
            .map(|steps| steps[index].as_ref())
            .collect::<Option<Vec<_>>>()
        else {
            return Ok(false);
        };
        let Some(steps) = open_chain(&paths) else {
            return Ok(false);
        };
        let Some(rim) = notch_band(builder, face, notch.rims[index], &steps)? else {
            return Ok(false);
        };
        rims.push(rim);
    }
    notch_cap(builder, notch.cap, rims, bridges)
}

fn open_chain<'a>(paths: &[&'a Directed]) -> Option<Vec<&'a Step>> {
    let mut current = *paths
        .iter()
        .find(|path| paths.iter().all(|other| other.to != path.from))?;
    let mut steps = current.steps.iter().collect::<Vec<_>>();
    for _ in 1..paths.len() {
        current = *paths.iter().find(|path| path.from == current.to)?;
        steps.extend(current.steps.iter());
    }
    Some(steps)
}

fn notch_band(
    builder: &mut Builder,
    face: u32,
    rim: u32,
    steps: &[&Step],
) -> Result<Option<(u32, Use)>, GeometryError> {
    let (Some(first), Some(last)) = (steps.first(), steps.last()) else {
        return Ok(None);
    };
    let ends = [first.from, last.to];
    let start = builder.brep.topology.halfedges.len() as u32;
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
        .collect();
    let path = builder.face_loop(face, false, uses)?;
    if !clear_band_seam(&mut builder.brep, face, path)? {
        return Ok(None);
    }
    let outer = builder.brep.topology.faces[face as usize].trim.outer;
    if rim_order(&builder.brep, rim, face, ends) == Some(false) {
        flip_loops(&mut builder.brep.topology, outer);
    }
    if rim_order(&builder.brep, rim, face, ends) != Some(true) {
        return Ok(None);
    }
    let Some((kept, rest, cap_use)) = split_rim(builder, rim, face, ends)? else {
        return Ok(None);
    };
    let topology = &mut builder.brep.topology;
    let members = (start..start + steps.len() as u32).collect::<Vec<_>>();
    let Some(next) = topology.halfedges[kept as usize].next else {
        return Ok(None);
    };
    for (from, to) in [
        (kept, start),
        (start + steps.len() as u32 - 1, rest),
        (rest, next),
    ] {
        link(topology, from, to);
    }
    for member in members.iter().chain([&rest]) {
        topology.halfedges[*member as usize].loop_ref = Some(outer);
    }
    if topology.loops.len() as u32 != path + 1 {
        return Ok(None);
    }
    topology.loops.pop();
    let surface = builder.brep.topology.faces[face as usize].surface;
    if matches!(
        builder.brep.geometry.surfaces[surface as usize],
        SurfaceGeometry::Cylinder { .. }
    ) {
        relift(&mut builder.brep, face, &members)?;
    }
    builder.brep.topology.faces[face as usize].provenance.role = FaceRole::Split;
    Ok(Some(cap_use))
}

fn rim_use(brep: &BrepEnvelope, rim: u32, face: u32) -> Option<u32> {
    let edge = &brep.topology.edges[rim as usize];
    [Some(edge.halfedge), edge.twin_halfedge]
        .into_iter()
        .flatten()
        .find(|halfedge| brep.topology.halfedges[*halfedge as usize].face == Some(face))
}

fn rim_parameters(
    brep: &BrepEnvelope,
    rim: u32,
    ends: [u32; 2],
) -> Option<(u32, Interval, [f64; 2])> {
    let EdgeGeometry::Curve { curve, range } = brep.topology.edges[rim as usize].geometry else {
        return None;
    };
    let geometry = &brep.geometry.curves[curve as usize];
    let [x, y] = ends.map(|vertex| {
        rim_parameter(
            geometry,
            brep.topology.vertices[vertex as usize].position,
            range,
        )
    });
    Some((curve, range, [x?, y?]))
}

fn rim_order(brep: &BrepEnvelope, rim: u32, face: u32, ends: [u32; 2]) -> Option<bool> {
    let halfedge = rim_use(brep, rim, face)?;
    let (_, _, [x, y]) = rim_parameters(brep, rim, ends)?;
    let forward = brep.topology.halfedges[halfedge as usize]
        .geometry_use
        .sense
        == Orientation::Forward;
    Some(if forward { x < y } else { x > y })
}

fn split_rim(
    builder: &mut Builder,
    rim: u32,
    face: u32,
    [x, y]: [u32; 2],
) -> Result<Option<(u32, u32, (u32, Use))>, GeometryError> {
    let (Some(kept), Some((curve, range, [tx, ty]))) = (
        rim_use(&builder.brep, rim, face),
        rim_parameters(&builder.brep, rim, [x, y]),
    ) else {
        return Ok(None);
    };
    let record = &builder.brep.topology.halfedges[kept as usize];
    let (to, sense, lift) = (
        record.to,
        record.geometry_use.sense,
        record.geometry_use.periodic_lift,
    );
    let (Some(pcurve), Some(twin)) = (record.geometry_use.pcurve, record.twin) else {
        return Ok(None);
    };
    let Some(cap_pcurve) = builder.brep.topology.halfedges[twin as usize]
        .geometry_use
        .pcurve
    else {
        return Ok(None);
    };
    let (own, rest) = if sense == Orientation::Forward {
        (Interval::new(range.lo, tx)?, Interval::new(ty, range.hi)?)
    } else {
        (Interval::new(tx, range.hi)?, Interval::new(range.lo, ty)?)
    };
    builder.brep.topology.edges[rim as usize].geometry = EdgeGeometry::Curve { curve, range: own };
    let geometry = builder.brep.geometry.curves[curve as usize].clone();
    let edge = builder.edge(geometry, rest, false);
    let pcurves = [pcurve, cap_pcurve].map(|id| builder.brep.geometry.pcurves[id as usize].clone());
    let [band_pcurve, cap_pcurve] = pcurves;
    let added = builder.brep.topology.halfedges.len() as u32;
    builder.face_loop(face, false, vec![boundary(edge, y, to, sense, band_pcurve)])?;
    builder.brep.topology.loops.pop();
    let topology = &mut builder.brep.topology;
    topology.halfedges[added as usize]
        .geometry_use
        .periodic_lift = lift;
    topology.halfedges[kept as usize].to = x;
    topology.halfedges[twin as usize].from = x;
    let cap_use = boundary(edge, to, y, reverse(sense), cap_pcurve);
    Ok(Some((kept, added, (twin, cap_use))))
}

fn notch_cap(
    builder: &mut Builder,
    cap: u32,
    rims: Vec<(u32, Use)>,
    bridges: Vec<Use>,
) -> Result<bool, GeometryError> {
    let start = builder.brep.topology.halfedges.len() as u32;
    let (old, mut uses): (Vec<u32>, Vec<Use>) = rims.into_iter().unzip();
    uses.extend(bridges);
    builder.face_loop(cap, false, uses)?;
    builder.brep.topology.loops.pop();
    let topology = &mut builder.brep.topology;
    let loops = old
        .iter()
        .filter_map(|halfedge| topology.halfedges[*halfedge as usize].loop_ref)
        .collect::<BTreeSet<_>>();
    let members = topology
        .halfedges
        .iter()
        .filter(|halfedge| {
            halfedge.id >= start || halfedge.loop_ref.is_some_and(|id| loops.contains(&id))
        })
        .map(|halfedge| halfedge.id)
        .collect::<Vec<_>>();
    for &member in &members {
        let to = topology.halfedges[member as usize].to;
        let mut next = members
            .iter()
            .filter(|other| topology.halfedges[**other as usize].from == to);
        let (Some(&next), None) = (next.next(), next.next()) else {
            return Ok(false);
        };
        link(topology, member, next);
    }
    let cycles = cycles(topology, &members);
    let outer = topology.faces[cap as usize].trim.outer;
    let single = topology.faces[cap as usize].trim.holes.is_empty();
    let (kept, split) = match (cycles.as_slice(), loops.len()) {
        ([kept], _) if loops.contains(&outer) => (kept, None),
        ([kept, split], 1) if single => (kept, Some(split)),
        _ => return Ok(false),
    };
    for member in kept {
        topology.halfedges[*member as usize].loop_ref = Some(outer);
    }
    topology.loops[outer as usize].start_halfedge = kept[0];
    if let Some(split) = split {
        split_cap(topology, cap, split);
    }
    for removed in loops.iter().rev().filter(|id| **id != outer) {
        remove_loop(topology, *removed);
    }
    topology.faces[cap as usize].provenance.role = FaceRole::Split;
    Ok(true)
}

fn cycles(topology: &Topology, members: &[u32]) -> Vec<Vec<u32>> {
    let mut seen = BTreeSet::new();
    let mut cycles = Vec::new();
    for &member in members {
        let mut cycle = Vec::new();
        let mut cursor = Some(member);
        while let Some(current) = cursor.filter(|current| seen.insert(*current)) {
            cycle.push(current);
            cursor = topology.halfedges[current as usize].next;
        }
        if !cycle.is_empty() {
            cycles.push(cycle);
        }
    }
    cycles
}

fn split_cap(topology: &mut Topology, cap: u32, cycle: &[u32]) {
    let (face, id) = (topology.faces.len() as u32, topology.loops.len() as u32);
    let mut record = topology.faces[cap as usize].clone();
    record.id = face;
    record.key = format!("{}:notch-{}", record.key, face);
    record.provenance.role = FaceRole::Split;
    record.trim.outer = id;
    topology.loops.push(Loop {
        id,
        start_halfedge: cycle[0],
        face_ref: face,
        is_hole: false,
    });
    for member in cycle {
        let halfedge = &mut topology.halfedges[*member as usize];
        halfedge.face = Some(face);
        halfedge.loop_ref = Some(id);
    }
    if let Some(shell) = record.shell_ref {
        topology.shells[shell as usize].faces.push(face);
    }
    topology.faces.push(record);
}

fn link(topology: &mut Topology, from: u32, to: u32) {
    topology.halfedges[from as usize].next = Some(to);
    topology.halfedges[to as usize].prev = Some(from);
}

fn remove_loop(topology: &mut Topology, id: u32) {
    topology.loops.remove(id as usize);
    let shift = |value: u32| if value > id { value - 1 } else { value };
    for (index, record) in topology.loops.iter_mut().enumerate() {
        record.id = index as u32;
    }
    for halfedge in &mut topology.halfedges {
        halfedge.loop_ref = halfedge.loop_ref.map(shift);
    }
    for face in &mut topology.faces {
        face.trim.outer = shift(face.trim.outer);
        face.trim.holes.retain(|hole| *hole != id);
        for hole in &mut face.trim.holes {
            *hole = shift(*hole);
        }
    }
}

fn flip_loops(topology: &mut Topology, start: u32) {
    let mut component = BTreeSet::from([start]);
    let mut stack = vec![start];
    while let Some(current) = stack.pop() {
        let reached = topology
            .halfedges
            .iter()
            .filter(|halfedge| halfedge.loop_ref == Some(current))
            .filter_map(|halfedge| halfedge.twin)
            .filter_map(|twin| topology.halfedges[twin as usize].loop_ref)
            .collect::<Vec<_>>();
        for next in reached {
            if component.insert(next) {
                stack.push(next);
            }
        }
    }
    for halfedge in &mut topology.halfedges {
        if halfedge.loop_ref.is_some_and(|id| component.contains(&id)) {
            std::mem::swap(&mut halfedge.from, &mut halfedge.to);
            std::mem::swap(&mut halfedge.next, &mut halfedge.prev);
            halfedge.geometry_use.sense = reverse(halfedge.geometry_use.sense);
        }
    }
}
