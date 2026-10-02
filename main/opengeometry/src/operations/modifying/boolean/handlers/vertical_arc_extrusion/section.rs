use crate::brep::{
    coverage_gap, BrepEnvelope, CurveGeometry, EdgeGeometry, Face, Frame3, GeometryError, HalfEdge,
    Orientation, SurfaceGeometry,
};
use crate::geom2d::{reversed_ring, winding, CurveEdge2, CurveRegion2, Pt2};
use crate::math::{dot, Point3};

struct FaceBoundary {
    boundary: Vec<(u32, Point3, Point3)>,
    lowest: f64,
    highest: f64,
}

pub(super) fn canonical_arc_start(start: f64, sweep: f64) -> f64 {
    let lower = start.min(start + sweep);
    lower.rem_euclid(std::f64::consts::TAU) + start - lower
}

pub(super) fn face_profile(
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

pub(super) fn coverage() -> GeometryError {
    coverage_gap("vertical arc-edged extrusion", "planar cutter")
}

pub(super) fn face_region(
    host: &BrepEnvelope,
    face: &Face,
    base: Frame3,
) -> Result<CurveRegion2, GeometryError> {
    let mut edges = loop_edges(host, face.trim.outer, base)?;
    if area(&edges) > 0.0 {
        edges = reversed_ring(&edges);
    }
    let holes = face
        .trim
        .holes
        .iter()
        .map(|loop_id| {
            let mut ring = loop_edges(host, *loop_id, base)?;
            if area(&ring) < 0.0 {
                ring = reversed_ring(&ring);
            }
            Ok(ring)
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    Ok(CurveRegion2 {
        outer: edges,
        holes,
    })
}

fn loop_edges(
    host: &BrepEnvelope,
    loop_id: u32,
    base: Frame3,
) -> Result<Vec<CurveEdge2>, GeometryError> {
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
        edges.push(halfedge_edge(host, halfedge, base)?);
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
}

fn halfedge_edge(
    host: &BrepEnvelope,
    halfedge: &HalfEdge,
    base: Frame3,
) -> Result<CurveEdge2, GeometryError> {
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
    Ok(match curve {
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
    })
}

pub(super) fn area(ring: &[CurveEdge2]) -> f64 {
    ring.iter().map(CurveEdge2::twice_area).sum::<f64>() / 2.0
}

pub(super) fn sectional_regions(
    host: &BrepEnvelope,
    base: Frame3,
    level: f64,
    tolerance: f64,
) -> Result<Vec<CurveRegion2>, GeometryError> {
    let edges = section_edges(host, base, level, tolerance)?;
    let rings = section_rings(edges, tolerance)?;
    let samples = ring_samples(&rings, tolerance)?;
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
    nested_regions(&rings, &samples, &depths, tolerance)
}

fn section_edges(
    host: &BrepEnvelope,
    base: Frame3,
    level: f64,
    tolerance: f64,
) -> Result<Vec<CurveEdge2>, GeometryError> {
    let mut edges = Vec::new();
    for face in &host.topology.faces {
        let FaceBoundary {
            boundary,
            lowest,
            highest,
        } = face_boundary(host, face, base)?;
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
        add_bottom_edges(&mut edges, host, base, halfedge_id, from, to)?;
    }
    Ok(edges)
}

fn face_boundary(
    host: &BrepEnvelope,
    face: &Face,
    base: Frame3,
) -> Result<FaceBoundary, GeometryError> {
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
    Ok(FaceBoundary {
        boundary,
        lowest,
        highest,
    })
}

fn add_bottom_edges(
    edges: &mut Vec<CurveEdge2>,
    host: &BrepEnvelope,
    base: Frame3,
    halfedge_id: u32,
    from: Point3,
    to: Point3,
) -> Result<(), GeometryError> {
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
    Ok(())
}

fn section_rings(
    mut edges: Vec<CurveEdge2>,
    tolerance: f64,
) -> Result<Vec<Vec<CurveEdge2>>, GeometryError> {
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
                next = next.reversed();
            }
            end = next.point(1.0);
            ring.push(next);
        }
        if area(&ring).abs() <= tolerance * tolerance {
            return Err(coverage());
        }
        rings.push(ring);
    }
    Ok(rings)
}

fn ring_samples(rings: &[Vec<CurveEdge2>], tolerance: f64) -> Result<Vec<Pt2>, GeometryError> {
    let mut samples = Vec::new();
    for ring in rings {
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
    Ok(samples)
}

fn nested_regions(
    rings: &[Vec<CurveEdge2>],
    samples: &[Pt2],
    depths: &[usize],
    tolerance: f64,
) -> Result<Vec<CurveRegion2>, GeometryError> {
    let mut regions = Vec::<(usize, CurveRegion2)>::new();
    for (index, ring) in rings.iter().enumerate() {
        if depths[index] % 2 == 0 {
            regions.push((
                index,
                CurveRegion2 {
                    outer: if area(ring) < 0.0 {
                        ring.clone()
                    } else {
                        reversed_ring(ring)
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
            reversed_ring(ring)
        });
    }
    Ok(regions.into_iter().map(|(_, region)| region).collect())
}

pub(super) fn append_points(regions: &[CurveRegion2], points: &mut Vec<[f64; 2]>) {
    for region in regions {
        for ring in std::iter::once(&region.outer).chain(&region.holes) {
            points.extend(ring.iter().map(|edge| {
                let p = edge.point(0.0);
                [p.x, p.z]
            }));
        }
    }
}

pub(super) fn split_regions(regions: &mut [CurveRegion2], points: &[[f64; 2]], tolerance: f64) {
    for region in regions {
        region.outer = split_ring(&region.outer, points, tolerance);
        for hole in &mut region.holes {
            *hole = split_ring(hole, points, tolerance);
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
