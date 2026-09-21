use std::collections::{BTreeMap, VecDeque};

use super::{
    booleans::{BooleanOp, BooleanReport, BooleanResult, FaceMapping},
    geometry::{dot, norm, sub, unit},
    primitives::{boundary, cuboid, uv_line, Builder},
    topology::*,
    CurveGeometry, Frame3, GeometryError, Point3, SurfaceGeometry,
};
use crate::math::interval::Interval;

type GridPoint = [usize; 3];
pub(super) struct BoxInput<'a> {
    pub(super) brep: &'a BrepEnvelope,
    pub(super) frame: Frame3,
    pub(super) size: Point3,
}
fn coverage() -> GeometryError {
    GeometryError::CoverageGap {
        families: [
            "canonical aligned cuboid".into(),
            "canonical aligned cuboid".into(),
        ],
    }
}
fn source(input: &BoxInput<'_>, face: usize) -> FaceSource {
    FaceSource {
        entity: input.brep.id.clone(),
        body: input.brep.id.clone(),
        key: input.brep.topology.faces[face].key.clone(),
        face: face as u32,
    }
}
pub(super) fn full_box(brep: &BrepEnvelope) -> Result<BoxInput<'_>, GeometryError> {
    brep.validate()?;
    let t = &brep.topology;
    if !matches!(brep.quality, GeometryQuality::Analytic)
        || [
            t.vertices.len(),
            t.edges.len(),
            t.halfedges.len(),
            t.loops.len(),
            t.faces.len(),
            t.shells.len(),
        ] != [8, 12, 24, 6, 6, 1]
        || !t.wires.is_empty()
        || brep.solids.len() != 1
        || brep.geometry.surfaces.len() != 6
        || brep.geometry.curves.len() != 12
        || brep.geometry.pcurves.len() != 24
        || !brep.geometry.intersections.is_empty()
    {
        return Err(coverage());
    }
    let mut axes = [[0.0; 3]; 3];
    let mut size = [0.0; 3];
    for i in 0..3 {
        let EdgeGeometry::Curve { curve, range } = t.edges[i].geometry else {
            return Err(coverage());
        };
        let CurveGeometry::Line { direction, .. } = brep.geometry.curves[curve as usize] else {
            return Err(coverage());
        };
        if range.lo != 0.0 {
            return Err(coverage());
        }
        axes[i] = direction;
        size[i] = range.hi;
    }
    let frame = Frame3 {
        origin: t.vertices[0].position,
        x: axes[0],
        y: axes[1],
        z: axes[2],
    };
    frame.validate().map_err(|_| coverage())?;
    let canonical = cuboid(brep.id.clone(), frame, size, brep.accuracy)?;
    let magnitude = size.into_iter().fold(0.0_f64, f64::max).max(
        t.vertices
            .iter()
            .map(|v| norm(v.position))
            .fold(0.0_f64, f64::max),
    );
    let noise = (256.0 * f64::EPSILON * magnitude).min(brep.accuracy.intersection / 8.0);
    let close = |a: Point3, b: Point3| norm(sub(a, b)) <= noise;
    let direction_close =
        |a: Point3, b: Point3| norm(sub(a, b)) * size.into_iter().fold(0.0_f64, f64::max) <= noise;
    for (a, b) in brep
        .geometry
        .surfaces
        .iter()
        .zip(&canonical.geometry.surfaces)
    {
        let (SurfaceGeometry::Plane { frame: a }, SurfaceGeometry::Plane { frame: b }) = (a, b)
        else {
            return Err(coverage());
        };
        if !close(a.origin, b.origin)
            || !direction_close(a.x, b.x)
            || !direction_close(a.y, b.y)
            || !direction_close(a.z, b.z)
        {
            return Err(coverage());
        }
    }
    for (a, b) in brep.geometry.curves.iter().zip(&canonical.geometry.curves) {
        let (
            CurveGeometry::Line {
                origin: ao,
                direction: ad,
            },
            CurveGeometry::Line {
                origin: bo,
                direction: bd,
            },
        ) = (a, b)
        else {
            return Err(coverage());
        };
        if !close(*ao, *bo) || !direction_close(*ad, *bd) {
            return Err(coverage());
        }
    }
    for (a, b) in brep
        .geometry
        .pcurves
        .iter()
        .zip(&canonical.geometry.pcurves)
    {
        let (
            PcurveGeometry::Line2 {
                origin: ao,
                direction: ad,
            },
            PcurveGeometry::Line2 {
                origin: bo,
                direction: bd,
            },
        ) = (a, b)
        else {
            return Err(coverage());
        };
        for i in 0..2 {
            if (ao[i] - bo[i]).abs() > noise
                || (ad[i] - bd[i]).abs() * size.into_iter().fold(0.0_f64, f64::max) > noise
            {
                return Err(coverage());
            }
        }
    }
    let mut topology = t.clone();
    for (v, expected) in topology
        .vertices
        .iter_mut()
        .zip(&canonical.topology.vertices)
    {
        if !close(v.position, expected.position) {
            return Err(coverage());
        }
        v.position = expected.position;
        v.tolerance = expected.tolerance;
    }
    for e in &mut topology.edges {
        e.tolerance = brep.accuracy.geometric;
    }
    for (f, expected) in topology.faces.iter_mut().zip(&canonical.topology.faces) {
        for i in 0..2 {
            if (f.trim.uv_bounds[i].lo - expected.trim.uv_bounds[i].lo).abs() > noise
                || (f.trim.uv_bounds[i].hi - expected.trim.uv_bounds[i].hi).abs() > noise
            {
                return Err(coverage());
            }
        }
        f.trim.uv_bounds = expected.trim.uv_bounds;
        f.key = expected.key.clone();
        f.provenance = expected.provenance.clone();
    }
    let value = |input| {
        serde_json::to_value(input).map_err(|e| GeometryError::InvalidGeometry(e.to_string()))
    };
    if value(&topology)? != value(&canonical.topology)?
        || serde_json::to_value(&brep.solids)
            .map_err(|e| GeometryError::InvalidGeometry(e.to_string()))?
            != serde_json::to_value(&canonical.solids)
                .map_err(|e| GeometryError::InvalidGeometry(e.to_string()))?
    {
        return Err(coverage());
    }
    Ok(BoxInput { brep, frame, size })
}
fn face_index(axis: usize, upper: bool) -> usize {
    match axis {
        0 => 2 + usize::from(upper),
        1 => 4 + usize::from(upper),
        _ => usize::from(upper),
    }
}
fn inside(point: Point3, lo: Point3, hi: Point3) -> bool {
    (0..3).all(|i| point[i] > lo[i] && point[i] < hi[i])
}
fn flat_index(p: GridPoint, n: GridPoint) -> usize {
    (p[0] * n[1] + p[1]) * n[2] + p[2]
}
fn neighbor(mut p: GridPoint, axis: usize, upper: bool, n: GridPoint) -> Option<GridPoint> {
    if upper {
        p[axis] += 1;
        if p[axis] >= n[axis] {
            return None;
        }
    } else {
        p[axis] = p[axis].checked_sub(1)?;
    }
    Some(p)
}

/// Regularized material booleans for canonical cuboids sharing the same axes.
/// The bounded plane arrangement is independent of rendering tessellation.
pub fn boolean_boxes(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let a = full_box(a)?;
    let b = full_box(b)?;
    if [a.frame.x, a.frame.y, a.frame.z] != [b.frame.x, b.frame.y, b.frame.z] {
        return Err(coverage());
    }
    let accuracy = Accuracy {
        geometric: a.brep.accuracy.geometric.max(b.brep.accuracy.geometric),
        intersection: a
            .brep
            .accuracy
            .intersection
            .max(b.brep.accuracy.intersection),
        tessellation: a
            .brep
            .accuracy
            .tessellation
            .max(b.brep.accuracy.tessellation),
        exchange: a.brep.accuracy.exchange.max(b.brep.accuracy.exchange),
    };
    let alo = [0.0; 3];
    let ahi = a.size;
    let blo = a.frame.local(b.frame.origin);
    let bhi: Point3 = std::array::from_fn(|i| blo[i] + b.size[i]);
    if blo.into_iter().chain(bhi).any(|value| !value.is_finite()) {
        return Err(GeometryError::UnresolvedIntersection(
            "cuboid relative coordinates exceed numerical range".into(),
        ));
    }
    let mut grid: [Vec<f64>; 3] = std::array::from_fn(|i| vec![alo[i], ahi[i], blo[i], bhi[i]]);
    // Only arithmetic roundoff is merged; actual unresolved feature widths fail.
    let magnitude = [a.frame.origin, b.frame.origin]
        .into_iter()
        .map(norm)
        .fold(0.0_f64, f64::max)
        .max(a.size.into_iter().fold(0.0_f64, f64::max))
        .max(b.size.into_iter().fold(0.0_f64, f64::max));
    let noise = (256.0 * f64::EPSILON * magnitude).min(accuracy.intersection / 8.0);
    for values in &mut grid {
        values.sort_by(f64::total_cmp);
        let mut unique: Vec<f64> = Vec::new();
        for &value in values.iter() {
            if let Some(last) = unique.last() {
                let gap = value - last;
                if gap <= noise {
                    continue;
                }
                if gap <= 4.0 * accuracy.geometric {
                    return Err(GeometryError::UnresolvedIntersection(
                        "cuboid arrangement contains sub-tolerance features".into(),
                    ));
                }
            }
            unique.push(value);
        }
        *values = unique;
    }
    let snap = |p: Point3| {
        std::array::from_fn(|i| {
            grid[i]
                .iter()
                .copied()
                .min_by(|x, y| (x - p[i]).abs().total_cmp(&(y - p[i]).abs()))
                .unwrap_or(p[i])
        })
    };
    let alo = snap(alo);
    let ahi = snap(ahi);
    let blo = snap(blo);
    let bhi = snap(bhi);
    let coincident = alo == blo && ahi == bhi;
    let overlap_lo: Point3 = std::array::from_fn(|i| alo[i].max(blo[i]));
    let overlap_hi: Point3 = std::array::from_fn(|i| ahi[i].min(bhi[i]));
    let mut contacts = Vec::new();
    if (0..3).all(|i| overlap_lo[i] <= overlap_hi[i])
        && (0..3).any(|i| overlap_lo[i] == overlap_hi[i])
    {
        for mask in 0..8 {
            let point = a.frame.point(std::array::from_fn(|i| {
                if mask & (1 << i) == 0 {
                    overlap_lo[i]
                } else {
                    overlap_hi[i]
                }
            }));
            if !contacts.contains(&point) {
                contacts.push(point);
            }
        }
    }
    let n = grid.each_ref().map(|g| g.len() - 1);
    let count = n[0] * n[1] * n[2];
    let mut material = vec![false; count];
    for x in 0..n[0] {
        for y in 0..n[1] {
            for z in 0..n[2] {
                let p = [x, y, z];
                let center = std::array::from_fn(|i| {
                    grid[i][p[i]] + (grid[i][p[i] + 1] - grid[i][p[i]]) / 2.0
                });
                let ia = inside(center, alo, ahi);
                let ib = inside(center, blo, bhi);
                material[flat_index(p, n)] = match operation {
                    BooleanOp::Union => ia || ib,
                    BooleanOp::Intersection => ia && ib,
                    BooleanOp::Subtraction => ia && !ib,
                };
            }
        }
    }
    let mut components = vec![usize::MAX; count];
    let mut component_count = 0;
    for x in 0..n[0] {
        for y in 0..n[1] {
            for z in 0..n[2] {
                let p = [x, y, z];
                let index = flat_index(p, n);
                if !material[index] || components[index] != usize::MAX {
                    continue;
                }
                components[index] = component_count;
                let mut queue = VecDeque::from([p]);
                while let Some(cell) = queue.pop_front() {
                    for axis in 0..3 {
                        for upper in [false, true] {
                            if let Some(adjacent) = neighbor(cell, axis, upper, n) {
                                let j = flat_index(adjacent, n);
                                if material[j] && components[j] == usize::MAX {
                                    components[j] = component_count;
                                    queue.push_back(adjacent);
                                }
                            }
                        }
                    }
                }
                component_count += 1;
            }
        }
    }
    let mut builder = Builder::new(id, accuracy)?;
    let mut vertex_ids = BTreeMap::new();
    let mut edge_ids = BTreeMap::new();
    let mut face_components = Vec::new();
    let mut volumes = Vec::new();
    for x in 0..n[0] {
        for y in 0..n[1] {
            for z in 0..n[2] {
                let p = [x, y, z];
                let cell = flat_index(p, n);
                if !material[cell] {
                    continue;
                }
                let component = components[cell];
                for axis in 0..3 {
                    for upper in [false, true] {
                        if neighbor(p, axis, upper, n).is_some_and(|q| material[flat_index(q, n)]) {
                            continue;
                        }
                        let coordinate = grid[axis][p[axis] + usize::from(upper)];
                        let mut center: Point3 = std::array::from_fn(|i| {
                            grid[i][p[i]] + (grid[i][p[i] + 1] - grid[i][p[i]]) / 2.0
                        });
                        center[axis] = coordinate;
                        let mut sources = Vec::new();
                        let normal_sign = if upper { 1.0 } else { -1.0 };
                        let mut owner = None;
                        for (input, lo, hi, cutter) in [(&a, alo, ahi, false), (&b, blo, bhi, true)]
                        {
                            for side in [false, true] {
                                let plane = if side { hi[axis] } else { lo[axis] };
                                if coordinate == plane
                                    && (0..3)
                                        .filter(|i| *i != axis)
                                        .all(|i| center[i] > lo[i] && center[i] < hi[i])
                                {
                                    let face = face_index(axis, side);
                                    let reversed = side != upper;
                                    if !reversed || (operation == BooleanOp::Subtraction && cutter)
                                    {
                                        sources.push(source(input, face));
                                        if owner.is_none() {
                                            owner = Some((input, face, reversed, cutter, lo, hi));
                                        }
                                    }
                                }
                            }
                        }
                        let (input, source_face, reversed, cutter, lo, hi) =
                            owner.ok_or_else(|| {
                                GeometryError::InvalidTopology(
                                    "cuboid boundary has no input-face ancestry".into(),
                                )
                            })?;
                        let surface = input.brep.geometry.surfaces
                            [input.brep.topology.faces[source_face].surface as usize]
                            .clone();
                        let face_frame = *surface.frame();
                        let u = (axis + 1) % 3;
                        let v = (axis + 2) % 3;
                        let mut corners = [[0; 3]; 4];
                        for (i, (du, dv)) in
                            [(0, 0), (1, 0), (1, 1), (0, 1)].into_iter().enumerate()
                        {
                            corners[i] = p;
                            corners[i][axis] += usize::from(upper);
                            corners[i][u] += du;
                            corners[i][v] += dv;
                        }
                        if !upper {
                            corners.reverse();
                        }
                        let local = corners.map(|q| std::array::from_fn(|i| grid[i][q[i]]));
                        let points = local.map(|q| a.frame.point(q));
                        let mut vids = [0; 4];
                        for i in 0..4 {
                            vids[i] = *vertex_ids
                                .entry((component, corners[i]))
                                .or_insert_with(|| builder.vertex(points[i]));
                        }
                        let mut uses = Vec::new();
                        let mut bounds = [[f64::INFINITY, f64::NEG_INFINITY]; 2];
                        for i in 0..4 {
                            let j = (i + 1) % 4;
                            let (from, to) = if corners[i] < corners[j] {
                                (i, j)
                            } else {
                                (j, i)
                            };
                            let key = (component, corners[from], corners[to]);
                            let edge = if let Some(&edge) = edge_ids.get(&key) {
                                edge
                            } else {
                                let delta = sub(points[to], points[from]);
                                let length = norm(delta);
                                let edge = builder.edge(
                                    CurveGeometry::Line {
                                        origin: points[from],
                                        direction: unit(delta)?,
                                    },
                                    Interval::new(0.0, length)?,
                                    false,
                                );
                                edge_ids.insert(key, edge);
                                edge
                            };
                            let EdgeGeometry::Curve { curve, .. } =
                                builder.brep.topology.edges[edge as usize].geometry
                            else {
                                return Err(GeometryError::InvalidTopology(
                                    "cuboid edge is not a line".into(),
                                ));
                            };
                            let CurveGeometry::Line { origin, direction } =
                                builder.brep.geometry.curves[curve as usize]
                            else {
                                return Err(GeometryError::InvalidTopology(
                                    "cuboid curve is not a line".into(),
                                ));
                            };
                            let uv = face_frame.local(origin);
                            uses.push(boundary(
                                edge,
                                vids[i],
                                vids[j],
                                if i == from {
                                    Orientation::Forward
                                } else {
                                    Orientation::Reverse
                                },
                                uv_line(
                                    [uv[0], uv[1]],
                                    [dot(direction, face_frame.x), dot(direction, face_frame.y)],
                                ),
                            ));
                            let uv = face_frame.local(points[i]);
                            for k in 0..2 {
                                bounds[k][0] = bounds[k][0].min(uv[k]);
                                bounds[k][1] = bounds[k][1].max(uv[k]);
                            }
                        }
                        for bound in &mut bounds {
                            bound[0] -= accuracy.geometric;
                            bound[1] += accuracy.geometric;
                        }
                        let face = builder.brep.topology.faces.len();
                        builder
                            .face(
                                &format!("{}-{face}", input.brep.topology.faces[source_face].key),
                                surface,
                                bounds,
                                uses,
                            )
                            .map_err(|error| match error {
                                GeometryError::InvalidTopology(message)
                                    if message == "nonmanifold primitive edge" =>
                                {
                                    GeometryError::UnresolvedIntersection(
                                        "cuboid boundary contains an unresolved contact event"
                                            .into(),
                                    )
                                }
                                other => other,
                            })?;
                        let whole = [u, v]
                            .into_iter()
                            .all(|i| grid[i][p[i]] == lo[i] && grid[i][p[i] + 1] == hi[i]);
                        builder.brep.topology.faces[face].sense = if reversed {
                            Orientation::Reverse
                        } else {
                            Orientation::Forward
                        };
                        builder.brep.topology.faces[face].provenance = FaceProvenance {
                            role: if sources.len() > 1 {
                                FaceRole::Coincident
                            } else if cutter && operation == BooleanOp::Subtraction {
                                FaceRole::Cut
                            } else if whole {
                                FaceRole::Preserved
                            } else {
                                FaceRole::Split
                            },
                            sources,
                            reversed,
                        };
                        face_components.push(component);
                        // Local divergence-theorem volume avoids cancellation at building coordinates.
                        volumes.push(
                            normal_sign
                                * coordinate
                                * (grid[u][p[u] + 1] - grid[u][p[u]])
                                * (grid[v][p[v] + 1] - grid[v][p[v]])
                                / 3.0,
                        );
                    }
                }
            }
        }
    }
    let mut out = builder.brep;
    let mut assigned = vec![false; out.topology.faces.len()];
    let mut outer = vec![None; component_count];
    let mut cavities = vec![Vec::new(); component_count];
    for start in 0..out.topology.faces.len() {
        if assigned[start] {
            continue;
        }
        let shell = out.topology.shells.len() as u32;
        let component = face_components[start];
        let mut faces = Vec::new();
        let mut volume = 0.0;
        assigned[start] = true;
        let mut queue = VecDeque::from([start]);
        while let Some(face) = queue.pop_front() {
            volume += volumes[face];
            faces.push(face as u32);
            out.topology.faces[face].shell_ref = Some(shell);
            for h in out
                .topology
                .halfedges
                .iter()
                .filter(|h| h.face == Some(face as u32))
            {
                let twin = h.twin.ok_or_else(|| {
                    GeometryError::InvalidTopology("open cuboid boolean boundary".into())
                })?;
                let adjacent = out.topology.halfedges[twin as usize].face.ok_or_else(|| {
                    GeometryError::InvalidTopology("cuboid twin has no face".into())
                })? as usize;
                if face_components[adjacent] != component {
                    return Err(GeometryError::InvalidTopology(
                        "cuboid contact incorrectly sewed separate material components".into(),
                    ));
                }
                if !assigned[adjacent] {
                    assigned[adjacent] = true;
                    queue.push_back(adjacent);
                }
            }
        }
        out.topology.shells.push(Shell {
            id: shell,
            faces,
            is_closed: true,
        });
        if volume > 0.0 {
            if outer[component].replace(shell).is_some() {
                return Err(GeometryError::UnresolvedIntersection(
                    "cuboid material component has multiple outer shells".into(),
                ));
            }
        } else if volume < 0.0 {
            cavities[component].push(shell);
        } else {
            return Err(GeometryError::UnresolvedIntersection(
                "cuboid shell volume is unresolved".into(),
            ));
        }
    }
    for (component, shell) in outer.into_iter().enumerate() {
        out.solids.push(SolidRegion {
            outer_shell: shell.ok_or_else(|| {
                GeometryError::InvalidTopology("cuboid material has no outer shell".into())
            })?,
            cavity_shells: cavities[component].clone(),
        });
    }
    out.revision = a
        .brep
        .revision
        .max(b.brep.revision)
        .checked_add(1)
        .ok_or_else(|| GeometryError::LimitExceeded("boolean revision overflow".into()))?;
    out.validate()?;
    let face_mappings = [&a, &b]
        .into_iter()
        .flat_map(|input| (0..6).map(move |face| source(input, face)))
        .map(|source| FaceMapping {
            result_faces: out
                .topology
                .faces
                .iter()
                .filter(|face| {
                    face.provenance.sources.iter().any(|s| {
                        s.entity == source.entity
                            && s.body == source.body
                            && s.key == source.key
                            && s.face == source.face
                    })
                })
                .map(|face| face.id)
                .collect(),
            source,
        })
        .collect();
    Ok(BooleanResult {
        report: BooleanReport {
            operation,
            quality: out.quality.clone(),
            contacts,
            coincident,
            face_mappings,
        },
        brep: out,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytic::{
        exchange::export_step, ifc_exchange::prepare_ifc_body, placement::placed,
        tessellation::tessellate,
    };
    fn box_at(id: &str, origin: Point3, size: Point3) -> BrepEnvelope {
        cuboid(
            id.into(),
            Frame3 {
                origin,
                x: [1.0, 0.0, 0.0],
                y: [0.0, 1.0, 0.0],
                z: [0.0, 0.0, 1.0],
            },
            size,
            Accuracy {
                geometric: 1e-8,
                intersection: 1e-9,
                tessellation: 1e-3,
                exchange: 1e-6,
            },
        )
        .unwrap()
    }
    fn volume(brep: &BrepEnvelope) -> f64 {
        let mesh = tessellate(brep, 0.01, 100_000).unwrap();
        mesh.indices
            .chunks_exact(3)
            .map(|ids| {
                let p: [Point3; 3] = std::array::from_fn(|j| {
                    std::array::from_fn(|i| mesh.positions[3 * ids[j] as usize + i])
                });
                dot(p[0], crate::analytic::geometry::cross(p[1], p[2])) / 6.0
            })
            .sum()
    }
    #[test]
    fn aligned_box_boolean_volume_topology_provenance_and_exchange() {
        let a = box_at("host", [0.0; 3], [2.0; 3]);
        let b = box_at("cutter", [1.0, 0.5, 0.5], [2.0; 3]);
        for (op, expected) in [
            (BooleanOp::Union, 13.75),
            (BooleanOp::Intersection, 2.25),
            (BooleanOp::Subtraction, 5.75),
        ] {
            let result = boolean_boxes(&a, &b, op, "result".into()).unwrap();
            assert!((volume(&result.brep) - expected).abs() < 1e-10);
            assert_eq!(result.brep.solids.len(), 1);
            assert_eq!(result.report.face_mappings.len(), 12);
            assert!(result
                .brep
                .topology
                .faces
                .iter()
                .all(|f| !f.provenance.sources.is_empty()));
            assert!(result
                .brep
                .geometry
                .surfaces
                .iter()
                .all(|s| matches!(s, SurfaceGeometry::Plane { .. })));
            let fine = tessellate(&result.brep, 0.001, 100_000).unwrap();
            let coarse = tessellate(&result.brep, 0.1, 100_000).unwrap();
            assert_eq!(fine.indices.len(), coarse.indices.len());
            if op == BooleanOp::Union {
                assert!(fine.outline_edge_ids.len() < result.brep.topology.edges.len());
            }
            export_step(&result.brep, "metre").unwrap();
            prepare_ifc_body(&result.brep).unwrap();
            if op == BooleanOp::Subtraction {
                assert!(result
                    .brep
                    .topology
                    .faces
                    .iter()
                    .any(|f| f.provenance.role == FaceRole::Cut
                        && f.provenance.reversed
                        && f.sense == Orientation::Reverse));
            }
        }
    }
    #[test]
    fn aligned_box_cavities_split_solids_and_contacts() {
        let a = box_at("host", [0.0; 3], [3.0; 3]);
        let b = box_at("cavity", [1.0; 3], [1.0; 3]);
        let result = boolean_boxes(&a, &b, BooleanOp::Subtraction, "cavity-result".into()).unwrap();
        assert_eq!(result.brep.solids[0].cavity_shells.len(), 1);
        assert!((volume(&result.brep) - 26.0).abs() < 1e-10);
        let cutter = box_at("through", [1.0, -1.0, -1.0], [1.0, 5.0, 5.0]);
        let result = boolean_boxes(&a, &cutter, BooleanOp::Subtraction, "split".into()).unwrap();
        assert_eq!(result.brep.solids.len(), 2);
        assert!((volume(&result.brep) - 18.0).abs() < 1e-10);
        for (origin, solids, contacts) in [
            ([3.0, 0.0, 0.0], 1, 4),
            ([3.0, 3.0, 0.0], 2, 2),
            ([3.0; 3], 2, 1),
        ] {
            let b = box_at("contact", origin, [3.0; 3]);
            let result = boolean_boxes(&a, &b, BooleanOp::Union, "contact-result".into()).unwrap();
            assert_eq!(result.brep.solids.len(), solids);
            assert_eq!(result.report.contacts.len(), contacts);
            let intersection =
                boolean_boxes(&a, &b, BooleanOp::Intersection, "empty".into()).unwrap();
            assert!(intersection.brep.solids.is_empty());
        }
    }
    #[test]
    fn aligned_box_boolean_identities_determinism_and_scale() {
        let a = box_at("a", [0.0; 3], [2.0; 3]);
        for op in [
            BooleanOp::Union,
            BooleanOp::Intersection,
            BooleanOp::Subtraction,
        ] {
            let result =
                super::super::booleans::boolean_brep(&a, &a, op, "identity".into()).unwrap();
            let expected = if op == BooleanOp::Subtraction {
                0.0
            } else {
                8.0
            };
            assert!((volume(&result.brep) - expected).abs() < 1e-10);
            let repeat =
                super::super::booleans::boolean_brep(&a, &a, op, "identity".into()).unwrap();
            assert_eq!(
                result.brep.to_json().unwrap(),
                repeat.brep.to_json().unwrap()
            );
            assert_eq!(result.brep.revision, a.revision + 1);
        }
        for factor in [1e-8, 1.0, 1e5] {
            let b = box_at("b", [1.0, 0.5, 0.5], [2.0; 3]);
            let frame = Frame3::from_axis([0.0; 3], [0.0, 1.0, 0.0], [0.8, 0.0, 0.6]).unwrap();
            let a = placed(&a, frame, factor).unwrap();
            let b = placed(&b, frame, factor).unwrap();
            let result = boolean_boxes(&a, &b, BooleanOp::Subtraction, "scaled".into()).unwrap();
            let mesh = tessellate(&result.brep, 0.01 * factor, 100_000).unwrap();
            let measured: f64 = mesh
                .indices
                .chunks_exact(3)
                .map(|ids| {
                    let p: [Point3; 3] = std::array::from_fn(|j| {
                        std::array::from_fn(|i| mesh.positions[3 * ids[j] as usize + i] / factor)
                    });
                    dot(p[0], crate::analytic::geometry::cross(p[1], p[2])) / 6.0
                })
                .sum();
            assert!((measured - 5.75).abs() < 1e-9);
        }
    }
    #[test]
    fn aligned_box_similarity_and_resolution_errors() {
        let a = box_at("a", [0.0; 3], [2.0; 3]);
        let b = box_at("b", [1.0, 0.5, 0.5], [2.0; 3]);
        let frame = Frame3::from_axis([5.0, 6.0, 7.0], [0.0, 1.0, 0.0], [0.8, 0.0, 0.6]).unwrap();
        let a = placed(&a, frame, 1.25).unwrap();
        let b = placed(&b, frame, 1.25).unwrap();
        let result = boolean_boxes(&a, &b, BooleanOp::Subtraction, "placed".into()).unwrap();
        assert!((volume(&result.brep) - 5.75 * 1.25_f64.powi(3)).abs() < 1e-9);
        let other = box_at("different-axes", [0.0; 3], [2.0; 3]);
        assert!(matches!(
            boolean_boxes(&a, &other, BooleanOp::Union, "gap".into()),
            Err(GeometryError::CoverageGap { .. })
        ));
        let a = box_at("a", [0.0; 3], [2.0; 3]);
        let b = box_at("b", [2.0 + 2.0 * a.accuracy.geometric, 0.0, 0.0], [2.0; 3]);
        assert!(matches!(
            boolean_boxes(&a, &b, BooleanOp::Union, "unresolved".into()),
            Err(GeometryError::UnresolvedIntersection(_))
        ));
    }
}
