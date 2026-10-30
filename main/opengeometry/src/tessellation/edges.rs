use super::grid::Rect;
use crate::brep::{
    BrepEnvelope, Curve, CurveGeometry, Edge, EdgeGeometry, GeometryError, Orientation,
};
use crate::math::{find, union, Point3};
use std::collections::HashMap;

pub(super) fn synchronize_edge_samples(
    brep: &BrepEnvelope,
    grids: &[Option<(Option<Rect>, [usize; 2])>],
    counts: &mut [usize],
) {
    let mut parent: Vec<_> = (0..counts.len()).collect();
    for (rect, _) in grids.iter().flatten() {
        let Some(rect) = rect else { continue };
        for [a, b] in [[0, 2], [1, 3]] {
            let a = brep.topology.halfedges[rect.halfedges[a] as usize].edge as usize;
            let b = brep.topology.halfedges[rect.halfedges[b] as usize].edge as usize;
            union(&mut parent, a, b);
        }
    }
    let key = |edge: &Edge| -> Option<[u64; 14]> {
        let EdgeGeometry::Curve { curve, range } = edge.geometry else {
            return None;
        };
        let frame = match &brep.geometry.curves[curve as usize] {
            CurveGeometry::Circle { frame, .. } | CurveGeometry::Ellipse { frame, .. } => frame,
            _ => return None,
        };
        let values = [frame.origin, frame.x, frame.y, frame.z];
        Some(std::array::from_fn(|i| {
            if i < 12 {
                values[i / 3][i % 3].to_bits()
            } else if i == 12 {
                range.lo.to_bits()
            } else {
                range.hi.to_bits()
            }
        }))
    };
    let mut concentric = HashMap::<[u64; 14], usize>::new();
    for edge in &brep.topology.edges {
        if let Some(key) = key(edge) {
            if let Some(&other) = concentric.get(&key) {
                union(&mut parent, edge.id as usize, other);
            } else {
                concentric.insert(key, edge.id as usize);
            }
        }
    }
    let mut maxima = vec![0; counts.len()];
    for (edge, &count) in counts.iter().enumerate() {
        let root = find(&mut parent, edge);
        maxima[root] = maxima[root].max(count);
    }
    for (edge, count) in counts.iter_mut().enumerate() {
        *count = maxima[find(&mut parent, edge)];
    }
}

pub(super) fn sample_edge(
    brep: &BrepEnvelope,
    id: u32,
    n: usize,
) -> Result<Vec<Point3>, GeometryError> {
    let edge = &brep.topology.edges[id as usize];
    let h = &brep.topology.halfedges[edge.halfedge as usize];
    match edge.geometry {
        EdgeGeometry::Collapsed { vertex } => {
            Ok(vec![brep.topology.vertices[vertex as usize].position])
        }
        EdgeGeometry::Curve { curve, range } => {
            let evaluator = brep.geometry.curve(curve)?;
            let mut points = Vec::with_capacity(n + 1);
            for i in 0..=n {
                points.push(
                    evaluator.point_at(range.lo + (range.hi - range.lo) * i as f64 / n as f64)?,
                );
            }
            let (first, last) = if h.geometry_use.sense == Orientation::Forward {
                (h.from, h.to)
            } else {
                (h.to, h.from)
            };
            points[0] = brep.topology.vertices[first as usize].position;
            points[n] = brep.topology.vertices[last as usize].position;
            Ok(points)
        }
    }
}

pub(super) fn boundary_point(
    brep: &BrepEnvelope,
    samples: &[Vec<Point3>],
    halfedge: u32,
    index: usize,
    n: usize,
    axis: usize,
) -> Result<Point3, GeometryError> {
    let h = &brep.topology.halfedges[halfedge as usize];
    let points = &samples[h.edge as usize];
    if points.len() == 1 {
        return Ok(points[0]);
    }
    if points.len() != n + 1 {
        return Err(GeometryError::UnsupportedGeometry(
            "incompatible opposite edge subdivision counts".into(),
        ));
    }
    let range = brep.topology.edges[h.edge as usize].geometry.range();
    let pcurve = h
        .geometry_use
        .pcurve
        .ok_or_else(|| GeometryError::InvalidTopology("missing pcurve".into()))?;
    let a = brep.geometry.pcurve_at(pcurve, range.lo)?;
    let b = brep.geometry.pcurve_at(pcurve, range.hi)?;
    Ok(points[if a[axis] <= b[axis] { index } else { n - index }])
}
