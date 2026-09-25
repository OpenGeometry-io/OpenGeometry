//! Vertical subtraction for an arc-edged extrusion and a planar prism.
//! The planar arrangement remains analytic at every height slab; the result is
//! assembled from planar caps and planar/cylindrical side faces in one B-rep.

use std::collections::{BTreeMap, VecDeque};

use super::{
    booleans::{analytic_face_mappings, brep_face_source, BooleanOp, BooleanReport, BooleanResult},
    geometry::{add, cross, dot, norm, scale, sub, unit},
    primitives::{boundary, plane_boundary, uv_line, Builder, Use},
    query::{classify_point_in_shell, face_contains_uv, PointClassification},
    topology::{
        Accuracy, BrepEnvelope, EdgeGeometry, Face, FaceProvenance, FaceRole, GeometryQuality,
        Orientation, Shell, SolidRegion,
    },
    CurveGeometry, Frame3, GeometryError, Point3, Surface, SurfaceGeometry,
};
use crate::{
    geometry::poly2d::Pt2,
    geometry::{
        boolean2d::PlanarBooleanOp,
        curved_boolean2d::{
            boolean_curved_regions, intersections, winding, CurveEdge2, CurveRegion2,
        },
    },
    math::interval::Interval,
};

fn coverage() -> GeometryError {
    GeometryError::CoverageGap {
        families: [
            "vertical arc-edged extrusion".into(),
            "planar cutter".into(),
        ],
    }
}

fn area(ring: &[CurveEdge2]) -> f64 {
    ring.iter().map(CurveEdge2::twice_area).sum::<f64>() / 2.0
}

fn canonical_arc_start(start: f64, sweep: f64) -> f64 {
    let lower = start.min(start + sweep);
    lower.rem_euclid(std::f64::consts::TAU) + start - lower
}

fn reverse(ring: &[CurveEdge2]) -> Vec<CurveEdge2> {
    ring.iter().rev().map(CurveEdge2::reverse).collect()
}

fn face_region(
    host: &BrepEnvelope,
    face: &Face,
    base: Frame3,
) -> Result<CurveRegion2, GeometryError> {
    let read_loop = |loop_id: u32| -> Result<Vec<CurveEdge2>, GeometryError> {
        let start = host
            .topology
            .loops
            .get(loop_id as usize)
            .ok_or_else(coverage)?
            .start_halfedge;
        let mut current = start;
        let mut edges = Vec::new();
        loop {
            let halfedge = host
                .topology
                .halfedges
                .get(current as usize)
                .ok_or_else(coverage)?;
            let from = base.local(
                host.topology
                    .vertices
                    .get(halfedge.from as usize)
                    .ok_or_else(coverage)?
                    .position,
            );
            let to = base.local(
                host.topology
                    .vertices
                    .get(halfedge.to as usize)
                    .ok_or_else(coverage)?
                    .position,
            );
            let edge = host
                .topology
                .edges
                .get(halfedge.edge as usize)
                .ok_or_else(coverage)?;
            let EdgeGeometry::Curve { curve, range } = edge.geometry else {
                return Err(coverage());
            };
            let curve = host
                .geometry
                .curves
                .get(curve as usize)
                .ok_or_else(coverage)?;
            edges.push(match curve {
                CurveGeometry::Line { .. } => CurveEdge2::Line {
                    from: [from[0], from[1]],
                    to: [to[0], to[1]],
                },
                CurveGeometry::Circle { frame, radius } => {
                    if dot(frame.z, base.z).abs() < 1.0 - 1e-10 {
                        return Err(coverage());
                    }
                    let centre = base.local(frame.origin);
                    let start_angle = (from[1] - centre[1]).atan2(from[0] - centre[0]);
                    let sense = if halfedge.geometry_use.sense == Orientation::Forward {
                        1.0
                    } else {
                        -1.0
                    };
                    CurveEdge2::Arc {
                        center: [centre[0], centre[1]],
                        radius: *radius,
                        start_angle,
                        sweep_angle: sense * dot(frame.z, base.z).signum() * (range.hi - range.lo),
                    }
                }
                _ => return Err(coverage()),
            });
            current = halfedge.next.ok_or_else(coverage)?;
            if current == start {
                break;
            }
            if edges.len() > host.topology.halfedges.len() {
                return Err(coverage());
            }
        }
        if edges.len() == 1 {
            if let CurveEdge2::Arc {
                center,
                radius,
                start_angle,
                sweep_angle,
            } = edges[0].clone()
            {
                if sweep_angle.abs() >= std::f64::consts::TAU - 1e-9 {
                    edges = vec![
                        CurveEdge2::Arc {
                            center,
                            radius,
                            start_angle,
                            sweep_angle: sweep_angle / 2.0,
                        },
                        CurveEdge2::Arc {
                            center,
                            radius,
                            start_angle: start_angle + sweep_angle / 2.0,
                            sweep_angle: sweep_angle / 2.0,
                        },
                    ];
                }
            }
        }
        Ok(edges)
    };
    let mut edges = read_loop(face.trim.outer)?;
    if area(&edges) > 0.0 {
        edges = reverse(&edges);
    }
    let holes = face
        .trim
        .holes
        .iter()
        .map(|loop_id| {
            let mut ring = read_loop(*loop_id)?;
            if area(&ring) < 0.0 {
                ring = reverse(&ring);
            }
            Ok(ring)
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    Ok(CurveRegion2 {
        outer: edges,
        holes,
    })
}

fn face_profile(
    host: &BrepEnvelope,
    base: Frame3,
    level: f64,
    tolerance: f64,
) -> Result<CurveRegion2, GeometryError> {
    let face = host
        .topology
        .faces
        .iter()
        .find(|face| {
            matches!(host.geometry.surfaces.get(face.surface as usize),
            Some(SurfaceGeometry::Plane { frame }) if dot(frame.z, base.z) > 1.0 - 1e-10
            && (base.local(frame.origin)[2] - level).abs() <= tolerance)
        })
        .ok_or_else(coverage)?;
    face_region(host, face, base)
}

fn sectional_regions(
    host: &BrepEnvelope,
    base: Frame3,
    level: f64,
    tolerance: f64,
) -> Result<Vec<CurveRegion2>, GeometryError> {
    let mut edges = Vec::new();
    for face in &host.topology.faces {
        let start = host.topology.loops[face.trim.outer as usize].start_halfedge;
        let mut current = start;
        let mut lowest = f64::INFINITY;
        let mut highest = f64::NEG_INFINITY;
        let mut boundary = Vec::new();
        loop {
            let halfedge = &host.topology.halfedges[current as usize];
            let from = host.topology.vertices[halfedge.from as usize].position;
            let to = host.topology.vertices[halfedge.to as usize].position;
            for point in [from, to] {
                let height = base.local(point)[2];
                lowest = lowest.min(height);
                highest = highest.max(height);
            }
            boundary.push((current, from, to));
            current = halfedge.next.ok_or_else(coverage)?;
            if current == start {
                break;
            }
            if boundary.len() > host.topology.halfedges.len() {
                return Err(coverage());
            }
        }
        if level <= lowest + tolerance || level >= highest - tolerance {
            continue;
        }
        if !face.trim.holes.is_empty() {
            return Err(coverage());
        }
        match &host.geometry.surfaces[face.surface as usize] {
            SurfaceGeometry::Plane { frame } if dot(frame.z, base.z).abs() > 1.0e-10 => {
                return Err(coverage());
            }
            SurfaceGeometry::Cylinder { frame, .. }
                if dot(frame.z, base.z).abs() < 1.0 - 1.0e-10 =>
            {
                return Err(coverage());
            }
            SurfaceGeometry::Plane { .. } | SurfaceGeometry::Cylinder { .. } => {}
            _ => return Err(coverage()),
        }
        let bottom = boundary
            .iter()
            .filter(|(_, from, to)| {
                (base.local(*from)[2] - lowest).abs() <= tolerance
                    && (base.local(*to)[2] - lowest).abs() <= tolerance
            })
            .collect::<Vec<_>>();
        if bottom.len() != 1 {
            return Err(coverage());
        }
        let &(halfedge_id, from, to) = bottom[0];
        let halfedge = &host.topology.halfedges[halfedge_id as usize];
        let edge = &host.topology.edges[halfedge.edge as usize];
        let EdgeGeometry::Curve { curve, range } = edge.geometry else {
            return Err(coverage());
        };
        let from = base.local(from);
        let to = base.local(to);
        match &host.geometry.curves[curve as usize] {
            CurveGeometry::Line { .. } => edges.push(CurveEdge2::Line {
                from: [from[0], from[1]],
                to: [to[0], to[1]],
            }),
            CurveGeometry::Circle { frame, radius } => {
                if dot(frame.z, base.z).abs() < 1.0 - 1.0e-10 {
                    return Err(coverage());
                }
                let centre = base.local(frame.origin);
                let start_angle = (from[1] - centre[1]).atan2(from[0] - centre[0]);
                let sense = if halfedge.geometry_use.sense == Orientation::Forward {
                    1.0
                } else {
                    -1.0
                };
                let sweep = sense * dot(frame.z, base.z).signum() * (range.hi - range.lo);
                if sweep.abs() >= std::f64::consts::TAU - 1.0e-9 {
                    for half in 0..2 {
                        edges.push(CurveEdge2::Arc {
                            center: [centre[0], centre[1]],
                            radius: *radius,
                            start_angle: start_angle + sweep * half as f64 / 2.0,
                            sweep_angle: sweep / 2.0,
                        });
                    }
                } else {
                    edges.push(CurveEdge2::Arc {
                        center: [centre[0], centre[1]],
                        radius: *radius,
                        start_angle,
                        sweep_angle: sweep,
                    });
                }
            }
            _ => return Err(coverage()),
        }
    }
    let mut rings = Vec::<Vec<CurveEdge2>>::new();
    while let Some(first) = edges.pop() {
        let start = first.point(0.0);
        let mut end = first.point(1.0);
        let mut ring = vec![first];
        while (end.x - start.x).hypot(end.z - start.z) > tolerance {
            let matches = edges
                .iter()
                .enumerate()
                .filter_map(|(index, edge)| {
                    let from = edge.point(0.0);
                    let to = edge.point(1.0);
                    let forward = (from.x - end.x).hypot(from.z - end.z) <= tolerance;
                    let reverse = (to.x - end.x).hypot(to.z - end.z) <= tolerance;
                    (forward || reverse).then_some((index, reverse))
                })
                .collect::<Vec<_>>();
            if matches.len() != 1 {
                return Err(coverage());
            }
            let (index, reverse) = matches[0];
            let mut next = edges.swap_remove(index);
            if reverse {
                next = next.reverse();
            }
            end = next.point(1.0);
            ring.push(next);
        }
        if area(&ring).abs() <= tolerance * tolerance {
            return Err(coverage());
        }
        rings.push(ring);
    }
    let mut samples = Vec::new();
    for ring in &rings {
        let edge = &ring[0];
        let middle = edge.point(0.5);
        let before = edge.point(0.49);
        let after = edge.point(0.51);
        let tangent = Pt2::new(after.x - before.x, after.z - before.z);
        let length = tangent.x.hypot(tangent.z);
        if length <= tolerance {
            return Err(coverage());
        }
        let offset = (length * 0.1).min(1.0e-4).max(tolerance * 16.0);
        let normal = Pt2::new(-tangent.z * offset / length, tangent.x * offset / length);
        let left = Pt2::new(middle.x + normal.x, middle.z + normal.z);
        let right = Pt2::new(middle.x - normal.x, middle.z - normal.z);
        let left_inside = winding(left, ring, tolerance) != 0;
        let right_inside = winding(right, ring, tolerance) != 0;
        let sample = match (left_inside, right_inside) {
            (true, false) => left,
            (false, true) => right,
            _ => return Err(coverage()),
        };
        samples.push(sample);
    }
    let depths = samples
        .iter()
        .enumerate()
        .map(|(index, sample)| {
            rings
                .iter()
                .enumerate()
                .filter(|(other, ring)| *other != index && winding(*sample, ring, tolerance) != 0)
                .count()
        })
        .collect::<Vec<_>>();
    let mut regions = Vec::<(usize, CurveRegion2)>::new();
    for (index, ring) in rings.iter().enumerate() {
        if depths[index] % 2 == 0 {
            regions.push((
                index,
                CurveRegion2 {
                    outer: if area(ring) < 0.0 {
                        ring.clone()
                    } else {
                        reverse(ring)
                    },
                    holes: Vec::new(),
                },
            ));
        }
    }
    for (index, ring) in rings.iter().enumerate() {
        if depths[index] % 2 == 0 {
            continue;
        }
        let parent = regions
            .iter()
            .enumerate()
            .filter(|(_, (outer, region))| {
                depths[*outer] + 1 == depths[index]
                    && winding(samples[index], &region.outer, tolerance) != 0
            })
            .map(|(parent, (_, region))| (parent, area(&region.outer).abs()))
            .min_by(|left, right| left.1.total_cmp(&right.1))
            .map(|(parent, _)| parent)
            .ok_or_else(coverage)?;
        regions[parent].1.holes.push(if area(ring) > 0.0 {
            ring.clone()
        } else {
            reverse(ring)
        });
    }
    Ok(regions.into_iter().map(|(_, region)| region).collect())
}

fn append_points(regions: &[CurveRegion2], points: &mut Vec<[f64; 2]>) {
    for region in regions {
        for ring in std::iter::once(&region.outer).chain(&region.holes) {
            points.extend(ring.iter().map(|edge| {
                let p = edge.point(0.0);
                [p.x, p.z]
            }));
        }
    }
}

fn split_ring(ring: &[CurveEdge2], points: &[[f64; 2]], tolerance: f64) -> Vec<CurveEdge2> {
    ring.iter()
        .flat_map(|edge| {
            let mut stations = vec![0.0, 1.0];
            for point in points {
                if let Some(station) = edge.parameter(Pt2::new(point[0], point[1]), tolerance) {
                    if station > 1e-10 && station < 1.0 - 1e-10 {
                        stations.push(station);
                    }
                }
            }
            stations.sort_by(f64::total_cmp);
            stations.dedup_by(|a, b| (*a - *b).abs() < 1e-10);
            stations
                .windows(2)
                .map(|span| edge.slice(span[0], span[1]))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn split_regions(regions: &mut [CurveRegion2], points: &[[f64; 2]], tolerance: f64) {
    for region in regions {
        region.outer = split_ring(&region.outer, points, tolerance);
        for hole in &mut region.holes {
            *hole = split_ring(hole, points, tolerance);
        }
    }
}

struct Facets {
    builder: Builder,
    vertices: Vec<(Point3, u32)>,
    edges: BTreeMap<(u32, u32, i64, i64, i64), (u32, u32, u32, bool)>,
    accuracy: Accuracy,
    base: Frame3,
}

struct SidePatch {
    edge: CurveEdge2,
    levels: Vec<f64>,
    provenance: FaceProvenance,
}

fn same_side_patch(a: &SidePatch, b: &SidePatch, tolerance: f64) -> bool {
    if std::mem::discriminant(&a.edge) != std::mem::discriminant(&b.edge)
        || a.provenance.role != b.provenance.role
        || a.provenance.reversed != b.provenance.reversed
        || a.provenance.sources.len() != b.provenance.sources.len()
        || !a
            .provenance
            .sources
            .iter()
            .zip(&b.provenance.sources)
            .all(|(a, b)| {
                a.entity == b.entity && a.body == b.body && a.key == b.key && a.face == b.face
            })
        || (a.edge.length() - b.edge.length()).abs() > tolerance
    {
        return false;
    }
    [0.0, 0.5, 1.0].into_iter().all(|station| {
        let a = a.edge.point(station);
        let b = b.edge.point(station);
        (a.x - b.x).hypot(a.z - b.z) <= tolerance
    })
}

impl Facets {
    fn new(id: String, accuracy: Accuracy, base: Frame3) -> Result<Self, GeometryError> {
        Ok(Self {
            builder: Builder::new(id, accuracy)?,
            vertices: Vec::new(),
            edges: BTreeMap::new(),
            accuracy,
            base,
        })
    }

    fn vertex(&mut self, point: Point3) -> u32 {
        if let Some((_, id)) = self
            .vertices
            .iter()
            .find(|(other, _)| norm(sub(*other, point)) <= self.accuracy.geometric / 4.0)
        {
            return *id;
        }
        let id = self.builder.vertex(point);
        self.vertices.push((point, id));
        id
    }

    fn edge(
        &mut self,
        from: Point3,
        to: Point3,
        middle: Point3,
        circle: Option<([f64; 2], f64, f64, f64)>,
    ) -> Result<(u32, u32, u32, Orientation), GeometryError> {
        let from_id = self.vertex(from);
        let to_id = self.vertex(to);
        if from_id == to_id {
            return Err(coverage());
        }
        let step = self.accuracy.geometric / 4.0;
        let key = (
            from_id.min(to_id),
            from_id.max(to_id),
            (middle[0] / step).round() as i64,
            (middle[1] / step).round() as i64,
            (middle[2] / step).round() as i64,
        );
        if let Some(&(id, stored_from, stored_to, forward)) = self.edges.get(&key) {
            let same = from_id == stored_from && to_id == stored_to;
            let orientation = if same == forward {
                Orientation::Forward
            } else {
                Orientation::Reverse
            };
            return Ok((id, from_id, to_id, orientation));
        }
        let (curve, range, forward) = if let Some((centre, radius, start, sweep)) = circle {
            let origin = self
                .base
                .point([centre[0], centre[1], self.base.local(from)[2]]);
            let frame = Frame3 {
                origin,
                ..self.base
            };
            let start = canonical_arc_start(start, sweep);
            let end = start + sweep;
            (
                CurveGeometry::Circle { frame, radius },
                Interval::new(start.min(end), start.max(end))?,
                sweep > 0.0,
            )
        } else {
            let delta = sub(to, from);
            (
                CurveGeometry::Line {
                    origin: from,
                    direction: unit(delta)?,
                },
                Interval::new(0.0, norm(delta))?,
                true,
            )
        };
        let id = self.builder.edge(curve, range, false);
        self.edges.insert(key, (id, from_id, to_id, forward));
        Ok((
            id,
            from_id,
            to_id,
            if forward {
                Orientation::Forward
            } else {
                Orientation::Reverse
            },
        ))
    }

    fn edge_at(
        &mut self,
        edge: &CurveEdge2,
        level: f64,
    ) -> Result<(u32, u32, u32, Orientation), GeometryError> {
        let point = |t| {
            let p = edge.point(t);
            self.base.point([p.x, p.z, level])
        };
        let curve = match edge {
            CurveEdge2::Arc {
                center,
                radius,
                start_angle,
                sweep_angle,
            } => Some((*center, *radius, *start_angle, *sweep_angle)),
            CurveEdge2::Line { .. } => None,
        };
        self.edge(point(0.0), point(1.0), point(0.5), curve)
    }

    fn cap_uses(
        &mut self,
        ring: &[CurveEdge2],
        level: f64,
        frame: Frame3,
    ) -> Result<Vec<Use>, GeometryError> {
        let mut uses = Vec::new();
        for edge in ring {
            let (id, from, to, sense) = self.edge_at(edge, level)?;
            uses.push(plane_boundary(&self.builder, frame, id, from, to, sense)?);
        }
        Ok(uses)
    }

    fn cap(
        &mut self,
        regions: &[CurveRegion2],
        level: f64,
        up: bool,
        provenance: FaceProvenance,
    ) -> Result<(), GeometryError> {
        let origin = self.base.point([0.0, 0.0, level]);
        let frame = if up {
            Frame3 {
                origin,
                ..self.base
            }
        } else {
            Frame3 {
                origin,
                x: self.base.x,
                y: scale(self.base.y, -1.0),
                z: scale(self.base.z, -1.0),
            }
        };
        for region in regions {
            let outer = if up {
                region.outer.clone()
            } else {
                reverse(&region.outer)
            };
            let holes = if up {
                region.holes.clone()
            } else {
                region.holes.iter().map(|ring| reverse(ring)).collect()
            };
            let mut bounds = [[f64::INFINITY, f64::NEG_INFINITY]; 2];
            for edge in outer.iter().chain(holes.iter().flatten()) {
                let mut stations = vec![0.0, 0.5, 1.0];
                if let CurveEdge2::Arc { center, radius, .. } = edge {
                    for angle in [
                        0.0,
                        std::f64::consts::FRAC_PI_2,
                        std::f64::consts::PI,
                        3.0 * std::f64::consts::FRAC_PI_2,
                    ] {
                        let point = Pt2::new(
                            center[0] + radius * angle.cos(),
                            center[1] + radius * angle.sin(),
                        );
                        if let Some(station) = edge.parameter(point, self.accuracy.intersection) {
                            stations.push(station);
                        }
                    }
                }
                for t in stations {
                    let point = edge.point(t);
                    let local = frame.local(self.base.point([point.x, point.z, level]));
                    for axis in 0..2 {
                        bounds[axis][0] = bounds[axis][0].min(local[axis]);
                        bounds[axis][1] = bounds[axis][1].max(local[axis]);
                    }
                }
            }
            for bound in &mut bounds {
                bound[0] -= self.accuracy.geometric;
                bound[1] += self.accuracy.geometric;
            }
            let outer_uses = self.cap_uses(&outer, level, frame)?;
            let hole_uses = holes
                .iter()
                .map(|ring| self.cap_uses(ring, level, frame))
                .collect::<Result<Vec<_>, _>>()?;
            let id = self.builder.brep.topology.faces.len();
            self.builder.face_with_holes(
                &format!("{}-cap-{id}", if up { "upper" } else { "lower" }),
                SurfaceGeometry::Plane { frame },
                bounds,
                outer_uses,
                hole_uses,
            )?;
            self.builder.brep.topology.faces[id].provenance = provenance.clone();
        }
        Ok(())
    }

    fn side(
        &mut self,
        edge: &CurveEdge2,
        levels: &[f64],
        provenance: FaceProvenance,
    ) -> Result<(), GeometryError> {
        if levels.len() < 2 || levels.windows(2).any(|span| span[1] <= span[0]) {
            return Err(coverage());
        }
        let lo = levels[0];
        let hi = *levels.last().ok_or_else(coverage)?;
        let base = self.base;
        let point = |t, level| {
            let p = edge.point(t);
            base.point([p.x, p.z, level])
        };
        let lower_from = point(0.0, lo);
        let lower_to = point(1.0, lo);
        let (bottom, bf, bt, bottom_sense) = self.edge_at(edge, lo)?;
        let (top, tf, tt, top_sense) = self.edge_at(edge, hi)?;
        let mut end_segments = Vec::with_capacity(levels.len() - 1);
        let mut start_segments = Vec::with_capacity(levels.len() - 1);
        for span in levels.windows(2) {
            let from = point(1.0, span[0]);
            let to = point(1.0, span[1]);
            let (edge, from_id, to_id, sense) =
                self.edge(from, to, scale(add(from, to), 0.5), None)?;
            end_segments.push((edge, from_id, to_id, sense, span[0], span[1]));
        }
        for span in levels.windows(2).rev() {
            let from = point(0.0, span[1]);
            let to = point(0.0, span[0]);
            let (edge, from_id, to_id, sense) =
                self.edge(from, to, scale(add(from, to), 0.5), None)?;
            start_segments.push((edge, from_id, to_id, sense, span[0], span[1]));
        }
        let id = self.builder.brep.topology.faces.len();
        match edge {
            CurveEdge2::Line { .. } => {
                let edge_direction = unit(sub(lower_to, lower_from))?;
                let normal = unit(cross(edge_direction, self.base.z))?;
                let frame = Frame3::from_axis(lower_from, normal, edge_direction)?;
                let mut uses = Vec::with_capacity(2 + 2 * end_segments.len());
                uses.push(plane_boundary(
                    &self.builder,
                    frame,
                    bottom,
                    bf,
                    bt,
                    bottom_sense,
                )?);
                for &(edge, from, to, sense, _, _) in &end_segments {
                    uses.push(plane_boundary(&self.builder, frame, edge, from, to, sense)?);
                }
                uses.push(plane_boundary(
                    &self.builder,
                    frame,
                    top,
                    tt,
                    tf,
                    if top_sense == Orientation::Forward {
                        Orientation::Reverse
                    } else {
                        Orientation::Forward
                    },
                )?);
                for &(edge, from, to, sense, _, _) in &start_segments {
                    uses.push(plane_boundary(&self.builder, frame, edge, from, to, sense)?);
                }
                self.builder.face(
                    &format!("vertical-plane-{id}"),
                    SurfaceGeometry::Plane { frame },
                    [
                        [
                            -self.accuracy.geometric,
                            norm(sub(lower_to, lower_from)) + self.accuracy.geometric,
                        ],
                        [-self.accuracy.geometric, hi - lo + self.accuracy.geometric],
                    ],
                    uses,
                )?;
                // Layer boundaries are walked clockwise with material to
                // their right; the side frame's natural normal points left.
                self.builder.brep.topology.faces[id].sense = Orientation::Reverse;
            }
            CurveEdge2::Arc {
                center,
                radius,
                start_angle,
                sweep_angle,
            } => {
                let origin = self.base.point([center[0], center[1], 0.0]);
                let frame = Frame3 {
                    origin,
                    ..self.base
                };
                let start_angle = canonical_arc_start(*start_angle, *sweep_angle);
                let end_angle = start_angle + sweep_angle;
                let mut uses = Vec::with_capacity(2 + 2 * end_segments.len());
                uses.push(boundary(
                    bottom,
                    bf,
                    bt,
                    bottom_sense,
                    uv_line([0.0, lo], [1.0, 0.0]),
                ));
                for &(edge, from, to, sense, start, end) in &end_segments {
                    uses.push(boundary(
                        edge,
                        from,
                        to,
                        sense,
                        if sense == Orientation::Forward {
                            uv_line([end_angle, start], [0.0, 1.0])
                        } else {
                            uv_line([end_angle, end], [0.0, -1.0])
                        },
                    ));
                }
                uses.push(boundary(
                    top,
                    tt,
                    tf,
                    if top_sense == Orientation::Forward {
                        Orientation::Reverse
                    } else {
                        Orientation::Forward
                    },
                    uv_line([0.0, hi], [1.0, 0.0]),
                ));
                for &(edge, from, to, sense, start, end) in &start_segments {
                    uses.push(boundary(
                        edge,
                        from,
                        to,
                        sense,
                        if sense == Orientation::Forward {
                            uv_line([start_angle, end], [0.0, -1.0])
                        } else {
                            uv_line([start_angle, start], [0.0, 1.0])
                        },
                    ));
                }
                self.builder.face(
                    &format!("vertical-cylinder-{id}"),
                    SurfaceGeometry::Cylinder {
                        frame,
                        radius: *radius,
                    },
                    [
                        [
                            start_angle.min(end_angle) - self.accuracy.geometric / radius,
                            start_angle.max(end_angle) + self.accuracy.geometric / radius,
                        ],
                        [lo - self.accuracy.geometric, hi + self.accuracy.geometric],
                    ],
                    uses,
                )?;
                // Layer outer rings are clockwise and holes counterclockwise.
                // A counterclockwise circular edge therefore bounds an inner
                // face; its natural radial normal points into material.
                if *sweep_angle > 0.0 {
                    self.builder.brep.topology.faces[id].sense = Orientation::Reverse;
                }
            }
        }
        self.builder.brep.topology.faces[id].provenance = provenance;
        Ok(())
    }
}

fn source_for_side(
    brep: &BrepEnvelope,
    point: Point3,
    axis: Point3,
    tolerance: f64,
) -> Result<Vec<super::topology::FaceSource>, GeometryError> {
    let mut sources = Vec::new();
    for face in &brep.topology.faces {
        let surface = brep.geometry.surface(face.surface)?;
        let on = match surface {
            SurfaceGeometry::Plane { frame } => {
                let local = frame.local(point);
                local[2].abs() <= tolerance && dot(frame.z, axis).abs() < 1.0 - 1e-10
            }
            SurfaceGeometry::Cylinder { frame, radius } => {
                let local = frame.local(point);
                (local[0].hypot(local[1]) - radius).abs() <= tolerance
            }
            _ => false,
        };
        if !on {
            continue;
        }
        let hint = [
            face.trim.uv_bounds[0].midpoint(),
            face.trim.uv_bounds[1].midpoint(),
        ];
        let uv = surface.project(point, Some(hint))?;
        match face_contains_uv(brep, face, uv)? {
            Some(true) => sources.push(brep_face_source(brep, face.id)),
            Some(false) => {}
            None => return Err(coverage()),
        }
    }
    Ok(sources)
}

fn contains_region_point(region: &CurveRegion2, point: Pt2, tolerance: f64) -> bool {
    winding(point, &region.outer, tolerance) != 0
        && region
            .holes
            .iter()
            .all(|hole| winding(point, hole, tolerance) == 0)
}

// Cap ancestry only asks whether two trimmed regions share positive area. Split
// at the same analytic edge intersections as the Boolean arrangement, then
// probe each open boundary interval on its material side. This also handles
// coincident circular edges without assembling a second, potentially
// degenerate Boolean boundary solely for provenance.
fn cap_regions_overlap(a: &CurveRegion2, b: &CurveRegion2, tolerance: f64) -> bool {
    for (region, other) in [(a, b), (b, a)] {
        for ring in std::iter::once(&region.outer).chain(&region.holes) {
            for edge in ring {
                let mut parameters = vec![0.0, 1.0];
                for other_ring in std::iter::once(&other.outer).chain(&other.holes) {
                    for other_edge in other_ring {
                        parameters.extend(
                            intersections(edge, other_edge, tolerance)
                                .into_iter()
                                .filter_map(|point| edge.parameter(point, tolerance)),
                        );
                    }
                }
                parameters.sort_by(f64::total_cmp);
                parameters
                    .dedup_by(|left, right| (*left - *right).abs() * edge.length() <= tolerance);
                for span in parameters.windows(2) {
                    let interval_length = (span[1] - span[0]) * edge.length();
                    if interval_length <= tolerance * 4.0 {
                        continue;
                    }
                    let midpoint = (span[0] + span[1]) / 2.0;
                    let point = edge.point(midpoint);
                    let tangent = edge.tangent(midpoint);
                    let tangent_length = tangent.x.hypot(tangent.z);
                    if tangent_length <= tolerance {
                        continue;
                    }
                    let mut offset = (interval_length * 0.1).min(1.0e-4);
                    for _ in 0..8 {
                        if offset <= tolerance * 2.0 {
                            break;
                        }
                        let probe = Pt2::new(
                            point.x + tangent.z * offset / tangent_length,
                            point.z - tangent.x * offset / tangent_length,
                        );
                        if contains_region_point(region, probe, tolerance)
                            && contains_region_point(other, probe, tolerance)
                        {
                            return true;
                        }
                        offset *= 0.5;
                    }
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod cap_overlap_tests {
    use super::*;

    fn rectangle(x0: f64, z0: f64, x1: f64, z1: f64) -> CurveRegion2 {
        let points = [[x0, z0], [x0, z1], [x1, z1], [x1, z0]];
        CurveRegion2 {
            outer: (0..4)
                .map(|index| CurveEdge2::Line {
                    from: points[index],
                    to: points[(index + 1) % 4],
                })
                .collect(),
            holes: Vec::new(),
        }
    }

    #[test]
    fn cap_overlap_requires_positive_shared_area() {
        let host = rectangle(0.0, 0.0, 2.0, 2.0);
        assert!(cap_regions_overlap(
            &host,
            &rectangle(1.0, 1.0, 3.0, 3.0),
            1e-7
        ));
        assert!(cap_regions_overlap(
            &host,
            &rectangle(0.5, 0.5, 1.5, 1.5),
            1e-7
        ));
        assert!(!cap_regions_overlap(
            &host,
            &rectangle(2.0, 0.0, 3.0, 1.0),
            1e-7
        ));
        assert!(!cap_regions_overlap(
            &host,
            &rectangle(3.0, 0.0, 4.0, 1.0),
            1e-7
        ));
    }

    #[test]
    fn cap_overlap_excludes_a_cutter_inside_a_hole() {
        let mut host = rectangle(0.0, 0.0, 3.0, 3.0);
        host.holes
            .push(reverse(&rectangle(1.0, 1.0, 2.0, 2.0).outer));
        assert!(!cap_regions_overlap(
            &host,
            &rectangle(1.2, 1.2, 1.8, 1.8),
            1e-7
        ));
        assert!(cap_regions_overlap(
            &host,
            &rectangle(0.8, 1.2, 1.2, 1.8),
            1e-7
        ));
    }
}

fn source_for_cap_region(
    brep: &BrepEnvelope,
    base: Frame3,
    level: f64,
    region: &CurveRegion2,
    tolerance: f64,
) -> Result<Vec<super::topology::FaceSource>, GeometryError> {
    let mut candidates = Vec::new();
    for face in &brep.topology.faces {
        let SurfaceGeometry::Plane { frame } = brep.geometry.surface(face.surface)? else {
            continue;
        };
        if dot(frame.z, base.z).abs() > 1.0 - 1.0e-10
            && (base.local(frame.origin)[2] - level).abs() <= tolerance
        {
            candidates.push(face);
        }
    }
    let mut sources = Vec::new();
    for face in candidates {
        let source_region = face_region(brep, face, base)?;
        if cap_regions_overlap(region, &source_region, tolerance) {
            sources.push(brep_face_source(brep, face.id));
        }
    }
    Ok(sources)
}

fn cap_provenance(
    host: &BrepEnvelope,
    cutters: &[&BrepEnvelope],
    base: Frame3,
    level: f64,
    region: &CurveRegion2,
    tolerance: f64,
) -> Result<FaceProvenance, GeometryError> {
    let mut sources = source_for_cap_region(host, base, level, region, tolerance)?;
    let mut cut = false;
    for cutter in cutters {
        let cutter_sources = source_for_cap_region(cutter, base, level, region, tolerance)?;
        cut |= !cutter_sources.is_empty();
        sources.extend(cutter_sources);
    }
    if sources.is_empty() {
        return Err(coverage());
    }
    Ok(FaceProvenance {
        sources,
        role: if cut { FaceRole::Cut } else { FaceRole::Split },
        reversed: cut,
    })
}

fn finish(
    mut facets: Facets,
    a: &BrepEnvelope,
    cutters: &[&BrepEnvelope],
) -> Result<BooleanResult, GeometryError> {
    let mut assigned = vec![false; facets.builder.brep.topology.faces.len()];
    let mut outer_shells = Vec::new();
    let mut cavity_shells = Vec::new();
    for start in 0..assigned.len() {
        if assigned[start] {
            continue;
        }
        let shell = facets.builder.brep.topology.shells.len() as u32;
        let mut faces = Vec::new();
        let mut queue = VecDeque::from([start]);
        assigned[start] = true;
        while let Some(face) = queue.pop_front() {
            facets.builder.brep.topology.faces[face].shell_ref = Some(shell);
            faces.push(face as u32);
            for halfedge in facets
                .builder
                .brep
                .topology
                .halfedges
                .iter()
                .filter(|edge| edge.face == Some(face as u32))
            {
                let twin = halfedge.twin.ok_or_else(coverage)?;
                let adjacent = facets.builder.brep.topology.halfedges[twin as usize]
                    .face
                    .ok_or_else(coverage)? as usize;
                if !assigned[adjacent] {
                    assigned[adjacent] = true;
                    queue.push_back(adjacent);
                }
            }
        }
        let mut signed_volume = 0.0;
        for &face_id in &faces {
            let face = &facets.builder.brep.topology.faces[face_id as usize];
            let SurfaceGeometry::Plane { frame } = facets
                .builder
                .brep
                .geometry
                .surfaces
                .get(face.surface as usize)
                .ok_or_else(coverage)?
            else {
                continue;
            };
            let outward = dot(frame.z, facets.base.z) * face.sense.multiplier();
            if outward.abs() <= 1.0 - 1e-10 {
                continue;
            }
            let region = face_region(&facets.builder.brep, face, facets.base)?;
            let cap_area = area(&region.outer).abs()
                - region
                    .holes
                    .iter()
                    .map(|hole| area(hole).abs())
                    .sum::<f64>();
            if !cap_area.is_finite() || cap_area <= facets.accuracy.geometric.powi(2) {
                return Err(coverage());
            }
            signed_volume += facets.base.local(frame.origin)[2] * outward * cap_area;
        }
        if !signed_volume.is_finite() || signed_volume.abs() <= facets.accuracy.geometric.powi(3) {
            return Err(coverage());
        }
        facets.builder.brep.topology.shells.push(Shell {
            id: shell,
            faces,
            is_closed: true,
        });
        if signed_volume > 0.0 {
            outer_shells.push((shell, signed_volume));
        } else {
            cavity_shells.push(shell);
        }
    }
    for &(outer_shell, _) in &outer_shells {
        facets.builder.brep.solids.push(SolidRegion {
            outer_shell,
            cavity_shells: Vec::new(),
        });
    }
    for cavity_shell in cavity_shells {
        let shell = &facets.builder.brep.topology.shells[cavity_shell as usize];
        let face = &facets.builder.brep.topology.faces[shell.faces[0] as usize];
        let start = facets.builder.brep.topology.loops[face.trim.outer as usize].start_halfedge;
        let vertex = facets.builder.brep.topology.halfedges[start as usize].from;
        let point = facets.builder.brep.topology.vertices[vertex as usize].position;
        let mut enclosing = Vec::new();
        for (index, &(outer_shell, volume)) in outer_shells.iter().enumerate() {
            let outer = &facets.builder.brep.topology.shells[outer_shell as usize];
            match classify_point_in_shell(&facets.builder.brep, &outer.faces, point)? {
                PointClassification::Inside => enclosing.push((index, volume)),
                PointClassification::Outside => {}
                PointClassification::Boundary | PointClassification::Unknown => {
                    return Err(coverage())
                }
            }
        }
        let Some(&(index, _)) = enclosing
            .iter()
            .min_by(|left, right| left.1.total_cmp(&right.1))
        else {
            return Err(coverage());
        };
        facets.builder.brep.solids[index]
            .cavity_shells
            .push(cavity_shell);
    }
    let mut out = facets.builder.brep;
    out.revision = cutters
        .iter()
        .fold(a.revision, |revision, cutter| revision.max(cutter.revision))
        .checked_add(1)
        .ok_or_else(coverage)?;
    out.validate()?;
    let face_mappings =
        analytic_face_mappings(&out, std::iter::once(a).chain(cutters.iter().copied()));
    Ok(BooleanResult {
        report: BooleanReport {
            operation: BooleanOp::Subtraction,
            quality: GeometryQuality::Analytic,
            contacts: Vec::new(),
            coincident: true,
            face_mappings,
        },
        brep: out,
    })
}

pub(super) fn subtract_vertical_arc_extrusion(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    subtract_vertical_arc_extrusion_batch(a, &[b], id)
}

pub(super) fn subtract_vertical_arc_extrusion_batch(
    a: &BrepEnvelope,
    cutters: &[&BrepEnvelope],
    id: String,
) -> Result<BooleanResult, GeometryError> {
    if cutters.is_empty() {
        return Err(coverage());
    }
    let original_host = a
        .topology
        .faces
        .iter()
        .any(|face| face.key == "top" || face.key == "upper_cap");
    if !a
        .geometry
        .surfaces
        .iter()
        .any(|surface| matches!(surface, SurfaceGeometry::Cylinder { .. }))
    {
        return Err(coverage());
    }
    if cutters.iter().any(|cutter| {
        cutter.solids.len() != 1
            || cutter
                .geometry
                .surfaces
                .iter()
                .any(|surface| !matches!(surface, SurfaceGeometry::Plane { .. }))
    }) {
        return Err(coverage());
    }
    let accuracy = cutters
        .iter()
        .fold(a.accuracy, |accuracy, cutter| Accuracy {
            geometric: accuracy.geometric.max(cutter.accuracy.geometric),
            intersection: accuracy.intersection.max(cutter.accuracy.intersection),
            tessellation: accuracy.tessellation.max(cutter.accuracy.tessellation),
            exchange: accuracy.exchange.max(cutter.accuracy.exchange),
        });
    // A transverse round cut also adds a cylinder to an extruded profile.
    // Its axis differs from the host extrusion axis, so it cannot be treated as
    // the vertical arc support used by this layered reconstruction.
    let cap_axes = a
        .topology
        .faces
        .iter()
        .filter(|face| {
            face.key == "top"
                || face.key == "upper_cap"
                || face.key.ends_with(":top")
                || face.key.starts_with("upper-cap-")
        })
        .filter_map(
            |face| match a.geometry.surfaces.get(face.surface as usize) {
                Some(SurfaceGeometry::Plane { frame }) => Some(frame.z),
                _ => None,
            },
        )
        .collect::<Vec<_>>();
    let axis = a
        .topology
        .faces
        .iter()
        .find_map(
            |face| match a.geometry.surfaces.get(face.surface as usize) {
                Some(SurfaceGeometry::Cylinder { frame, .. })
                    if cap_axes
                        .iter()
                        .any(|cap| dot(*cap, frame.z).abs() > 1.0 - 1e-10) =>
                {
                    Some(frame.z)
                }
                _ => None,
            },
        )
        .ok_or_else(coverage)?;
    let host_top = a
        .topology
        .faces
        .iter()
        .filter_map(
            |face| match a.geometry.surfaces.get(face.surface as usize) {
                Some(SurfaceGeometry::Plane { frame }) if dot(frame.z, axis) > 1.0 - 1e-10 => {
                    Some(*frame)
                }
                _ => None,
            },
        )
        .max_by(|left, right| dot(left.origin, axis).total_cmp(&dot(right.origin, axis)))
        .ok_or_else(coverage)?;
    let positions = a
        .topology
        .vertices
        .iter()
        .map(|vertex| host_top.local(vertex.position)[2])
        .collect::<Vec<_>>();
    let low = positions.iter().copied().fold(f64::INFINITY, f64::min);
    let high = positions.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if !low.is_finite() || high - low <= accuracy.geometric * 4.0 {
        return Err(coverage());
    }
    let base = Frame3 {
        origin: host_top.point([0.0, 0.0, low]),
        ..host_top
    };
    let original_profile = if original_host {
        Some(face_profile(a, base, high - low, accuracy.intersection)?)
    } else {
        None
    };
    let cutter_profiles = cutters
        .iter()
        .map(|cutter| {
            let cutter_levels = cutter
                .topology
                .vertices
                .iter()
                .map(|vertex| base.local(vertex.position)[2])
                .collect::<Vec<_>>();
            let lo = cutter_levels.iter().copied().fold(f64::INFINITY, f64::min);
            let hi = cutter_levels
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max);
            if !lo.is_finite()
                || hi - lo <= accuracy.geometric * 4.0
                || cutter_levels.iter().any(|level| {
                    (level - lo).abs() > accuracy.geometric
                        && (level - hi).abs() > accuracy.geometric
                })
            {
                return Err(coverage());
            }
            Ok((
                *cutter,
                lo,
                hi,
                face_profile(cutter, base, hi, accuracy.intersection)?,
            ))
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    let mut levels = vec![0.0, high - low];
    if !original_host {
        for vertex in &a.topology.vertices {
            let level = base.local(vertex.position)[2];
            if level > accuracy.geometric && level < high - low - accuracy.geometric {
                levels.push(level);
            }
        }
    }
    for (_, lo, hi, _) in &cutter_profiles {
        for level in [*lo, *hi] {
            if level > accuracy.geometric && level < high - low - accuracy.geometric {
                levels.push(level);
            }
        }
    }
    levels.sort_by(f64::total_cmp);
    levels.dedup_by(|left, right| (*left - *right).abs() <= accuracy.geometric);
    let mut layers = levels
        .windows(2)
        .map(|span| {
            let middle = (span[0] + span[1]) / 2.0;
            let host_regions = if let Some(profile) = &original_profile {
                vec![profile.clone()]
            } else {
                sectional_regions(a, base, middle, accuracy.intersection)?
            };
            let active = cutter_profiles
                .iter()
                .filter(|(_, lo, hi, _)| middle > *lo && middle < *hi)
                .map(|(_, _, _, profile)| profile.clone())
                .collect::<Vec<_>>();
            boolean_curved_regions(
                &host_regions,
                &active,
                PlanarBooleanOp::Subtraction,
                accuracy.intersection,
            )
            .map_err(GeometryError::UnresolvedIntersection)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut exposed_up = (1..layers.len())
        .map(|index| {
            boolean_curved_regions(
                &layers[index - 1],
                &layers[index],
                PlanarBooleanOp::Subtraction,
                accuracy.intersection,
            )
            .map_err(GeometryError::UnresolvedIntersection)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut exposed_down = (1..layers.len())
        .map(|index| {
            boolean_curved_regions(
                &layers[index],
                &layers[index - 1],
                PlanarBooleanOp::Subtraction,
                accuracy.intersection,
            )
            .map_err(GeometryError::UnresolvedIntersection)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut all_points = Vec::new();
    for region in &layers {
        append_points(region, &mut all_points);
    }
    for region in exposed_up.iter().chain(&exposed_down) {
        append_points(region, &mut all_points);
    }
    for region in &mut layers {
        split_regions(region, &all_points, accuracy.intersection);
    }
    for region in exposed_up.iter_mut().chain(exposed_down.iter_mut()) {
        split_regions(region, &all_points, accuracy.intersection);
    }
    let mut facets = Facets::new(id, accuracy, base)?;
    let mut side_patches = Vec::<SidePatch>::new();
    for region in &layers[0] {
        let provenance = cap_provenance(a, &[], base, 0.0, region, accuracy.intersection)?;
        facets.cap(std::slice::from_ref(region), 0.0, false, provenance)?;
    }
    for (index, regions) in layers.iter().enumerate() {
        let lo = levels[index];
        let hi = levels[index + 1];
        for region in regions {
            for ring in std::iter::once(&region.outer).chain(&region.holes) {
                for edge in ring {
                    let p = edge.point(0.5);
                    let middle = base.point([p.x, p.z, (lo + hi) / 2.0]);
                    let from_a = source_for_side(a, middle, base.z, accuracy.intersection)?;
                    let mut from_b = Vec::new();
                    for cutter in cutters {
                        from_b.extend(source_for_side(
                            cutter,
                            middle,
                            base.z,
                            accuracy.intersection,
                        )?);
                    }
                    let cut = from_a.is_empty() && !from_b.is_empty();
                    let sources = if cut { from_b } else { from_a };
                    if sources.is_empty() {
                        return Err(coverage());
                    }
                    side_patches.push(SidePatch {
                        edge: edge.clone(),
                        levels: vec![lo, hi],
                        provenance: FaceProvenance {
                            sources,
                            role: if cut { FaceRole::Cut } else { FaceRole::Split },
                            reversed: cut,
                        },
                    });
                }
            }
        }
        if index + 1 < layers.len() {
            let cap_level = levels[index + 1];
            for region in &exposed_up[index] {
                let provenance =
                    cap_provenance(a, cutters, base, cap_level, region, accuracy.intersection)?;
                facets.cap(std::slice::from_ref(region), cap_level, true, provenance)?;
            }
            for region in &exposed_down[index] {
                let provenance =
                    cap_provenance(a, cutters, base, cap_level, region, accuracy.intersection)?;
                facets.cap(std::slice::from_ref(region), cap_level, false, provenance)?;
            }
        }
    }
    for region in layers.last().ok_or_else(coverage)? {
        let provenance = cap_provenance(a, &[], base, high - low, region, accuracy.intersection)?;
        facets.cap(std::slice::from_ref(region), high - low, true, provenance)?;
    }
    let mut consumed = vec![false; side_patches.len()];
    for first in 0..side_patches.len() {
        if consumed[first] {
            continue;
        }
        consumed[first] = true;
        let mut levels = side_patches[first].levels.clone();
        loop {
            let candidates = (first + 1..side_patches.len())
                .filter(|&index| {
                    !consumed[index]
                        && (side_patches[index].levels[0] - levels[levels.len() - 1]).abs()
                            <= accuracy.geometric
                        && same_side_patch(
                            &side_patches[first],
                            &side_patches[index],
                            accuracy.geometric / 4.0,
                        )
                })
                .collect::<Vec<_>>();
            if candidates.len() != 1 {
                break;
            }
            let next = candidates[0];
            consumed[next] = true;
            levels.push(side_patches[next].levels[1]);
        }
        facets.side(
            &side_patches[first].edge,
            &levels,
            side_patches[first].provenance.clone(),
        )?;
    }
    finish(facets, a, cutters)
}
