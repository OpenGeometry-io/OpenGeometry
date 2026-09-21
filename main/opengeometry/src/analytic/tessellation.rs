use super::{
    geometry::{cross, dot, norm, scale, sub, Surface},
    topology::{BrepEnvelope, EdgeGeometry, Face, Orientation},
    Curve, CurveGeometry, GeometryError, Point3, SurfaceGeometry,
};
use crate::math::interval::Interval;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    hash::{DefaultHasher, Hasher},
    io::Write,
    sync::Arc,
};

#[derive(Clone, Debug)]
pub struct Tessellation {
    pub positions: Vec<f64>,
    pub normals: Vec<f32>,
    pub indices: Vec<u32>,
    pub triangle_face_ids: Vec<u32>,
    pub outline_positions: Vec<f64>,
    pub outline_edge_ids: Vec<u32>,
    pub revision: u64,
    pub achieved_deflection: f64,
}
impl Tessellation {
    fn new(revision: u64, deflection: f64) -> Self {
        Self {
            positions: Vec::new(),
            normals: Vec::new(),
            indices: Vec::new(),
            triangle_face_ids: Vec::new(),
            outline_positions: Vec::new(),
            outline_edge_ids: Vec::new(),
            revision,
            achieved_deflection: deflection,
        }
    }
    fn vertex(&mut self, p: Point3, n: Point3) -> Result<u32, GeometryError> {
        if p.into_iter().chain(n).any(|v| !v.is_finite()) {
            return Err(GeometryError::InvalidGeometry(
                "nonfinite tessellation output".into(),
            ));
        }
        let id = u32::try_from(self.positions.len() / 3)
            .map_err(|_| GeometryError::LimitExceeded("tessellation vertices".into()))?;
        self.positions.extend(p);
        self.normals.extend(n.map(|v| v as f32));
        Ok(id)
    }
    fn point(&self, id: u32) -> Point3 {
        let i = id as usize * 3;
        [
            self.positions[i],
            self.positions[i + 1],
            self.positions[i + 2],
        ]
    }
    fn triangle(
        &mut self,
        mut ids: [u32; 3],
        face: u32,
        normal: Point3,
        max_triangles: usize,
    ) -> Result<(), GeometryError> {
        let [a, b, c] = ids.map(|id| self.point(id));
        let direction = cross(sub(b, a), sub(c, a));
        if direction.iter().any(|v| !v.is_finite()) {
            return Err(GeometryError::UnresolvedTessellation(
                "triangle orientation exceeds arithmetic range".into(),
            ));
        }
        if norm(direction) == 0.0 {
            return Ok(());
        }
        if self.triangle_face_ids.len() >= max_triangles {
            return Err(GeometryError::LimitExceeded(
                "tessellation triangles".into(),
            ));
        }
        let orientation = dot(direction, normal);
        if !orientation.is_finite() || orientation == 0.0 {
            return Err(GeometryError::UnresolvedTessellation(
                "triangle orientation could not be resolved".into(),
            ));
        }
        if orientation < 0.0 {
            ids.swap(1, 2);
        }
        self.indices.extend(ids);
        self.triangle_face_ids.push(face);
        Ok(())
    }
    fn bytes(&self) -> usize {
        self.positions.len() * 8
            + self.normals.len() * 4
            + self.indices.len() * 4
            + self.triangle_face_ids.len() * 4
            + self.outline_positions.len() * 8
            + self.outline_edge_ids.len() * 4
    }
}

struct Rect {
    halfedges: [u32; 4],
}
fn rectangle(brep: &BrepEnvelope, face: &Face) -> Result<Rect, GeometryError> {
    let [u, v] = face.trim.uv_bounds;
    let mut sides = [None; 4];
    let start = brep.topology.loops[face.trim.outer as usize].start_halfedge;
    let mut current = start;
    let mut count = 0;
    for _ in 0..128 {
        let h = &brep.topology.halfedges[current as usize];
        let edge = &brep.topology.edges[h.edge as usize];
        let range = edge.geometry.range();
        let pcurve = h
            .geometry_use
            .pcurve
            .ok_or_else(|| GeometryError::InvalidTopology("missing face pcurve".into()))?;
        if !matches!(
            brep.geometry.pcurves[pcurve as usize],
            super::topology::PcurveGeometry::Line2 { .. }
        ) {
            return Err(GeometryError::UnsupportedGeometry(
                "nonrectangular curved face trimming".into(),
            ));
        }
        let periods =
            brep.geometry.surface(face.surface)?.charts()[face.trim.chart as usize].periods;
        let lifted = |uv: [f64; 2]| {
            std::array::from_fn(|j| {
                uv[j]
                    + periods[j].map_or(0.0, |period| {
                        period * h.geometry_use.periodic_lift[j] as f64
                    })
            })
        };
        let a: [f64; 2] = lifted(brep.geometry.pcurve_at(pcurve, range.lo)?);
        let b: [f64; 2] = lifted(brep.geometry.pcurve_at(pcurve, range.hi)?);
        let close = |x: f64, y: f64| (x - y).abs() <= 1e-12;
        let covers_u =
            (close(a[0], u.lo) && close(b[0], u.hi)) || (close(a[0], u.hi) && close(b[0], u.lo));
        let covers_v =
            (close(a[1], v.lo) && close(b[1], v.hi)) || (close(a[1], v.hi) && close(b[1], v.lo));
        let side = if covers_u && close(a[1], v.lo) && close(b[1], v.lo) {
            0
        } else if covers_v && close(a[0], u.hi) && close(b[0], u.hi) {
            1
        } else if covers_u && close(a[1], v.hi) && close(b[1], v.hi) {
            2
        } else if covers_v && close(a[0], u.lo) && close(b[0], u.lo) {
            3
        } else {
            return Err(GeometryError::UnsupportedGeometry(
                "curved trim is not a chart rectangle".into(),
            ));
        };
        if sides[side].replace(current).is_some() {
            return Err(GeometryError::UnsupportedGeometry(
                "repeated rectangle boundary".into(),
            ));
        }
        count += 1;
        current = h
            .next
            .ok_or_else(|| GeometryError::InvalidTopology("open face loop".into()))?;
        if current == start {
            break;
        }
    }
    if count != 4 || sides.iter().any(Option::is_none) {
        return Err(GeometryError::UnsupportedGeometry(
            "curved trim is not a four-sided rectangle".into(),
        ));
    }
    Ok(Rect {
        halfedges: [
            sides[0].ok_or_else(|| GeometryError::InvalidTopology("bottom edge missing".into()))?,
            sides[1].ok_or_else(|| GeometryError::InvalidTopology("right edge missing".into()))?,
            sides[2].ok_or_else(|| GeometryError::InvalidTopology("top edge missing".into()))?,
            sides[3].ok_or_else(|| GeometryError::InvalidTopology("left edge missing".into()))?,
        ],
    })
}

fn count(value: f64, minimum: usize, max_triangles: usize) -> Result<usize, GeometryError> {
    let value = value.ceil().max(minimum as f64);
    if !value.is_finite() || value > max_triangles as f64 {
        return Err(GeometryError::LimitExceeded(
            "tessellation subdivisions".into(),
        ));
    }
    Ok(value as usize)
}
fn grid_size(
    surface: &SurfaceGeometry,
    bounds: [Interval; 2],
    error: f64,
    max_triangles: usize,
) -> Result<[usize; 2], GeometryError> {
    let u = bounds[0].width();
    let v = bounds[1].width();
    let (nu, nv) = match surface {
        SurfaceGeometry::Cylinder { radius, .. } => (
            count(u * (radius / (8.0 * error)).sqrt(), 3, max_triangles)?,
            1,
        ),
        SurfaceGeometry::Cone { semi_angle, .. } => (
            count(
                u * (bounds[1].hi * semi_angle.tan() / (8.0 * error)).sqrt(),
                3,
                max_triangles,
            )?,
            1,
        ),
        SurfaceGeometry::Sphere { radius, .. } => (
            count(u * (2.0 * radius / error).sqrt(), 3, max_triangles)?,
            count(v * (2.0 * radius / error).sqrt(), 2, max_triangles)?,
        ),
        SurfaceGeometry::Torus {
            major_radius: r,
            minor_radius: a,
            ..
        } => (
            count(u * ((r + 2.0 * a) / error).sqrt(), 3, max_triangles)?,
            count(v * (2.0 * a / error).sqrt(), 3, max_triangles)?,
        ),
        SurfaceGeometry::Plane { .. } => {
            return Err(GeometryError::InvalidGeometry(
                "plane does not use a curved grid".into(),
            ))
        }
    };
    if nu
        .checked_mul(nv)
        .and_then(|n| n.checked_mul(2))
        .is_none_or(|n| n > max_triangles)
    {
        return Err(GeometryError::LimitExceeded(
            "tessellation grid triangles".into(),
        ));
    }
    Ok([nu, nv])
}

fn point_segment_distance(point: Point3, a: Point3, b: Point3) -> f64 {
    let chord = sub(b, a);
    let length_squared = dot(chord, chord);
    if length_squared == 0.0 {
        return norm(sub(point, a));
    }
    let parameter = (dot(sub(point, a), chord) / length_squared).clamp(0.0, 1.0);
    norm(sub(
        point,
        [
            a[0] + parameter * chord[0],
            a[1] + parameter * chord[1],
            a[2] + parameter * chord[2],
        ],
    ))
}

fn bounds_chord_deviation(bounds: super::geometry::PatchBounds, a: Point3, b: Point3) -> f64 {
    let mut maximum: f64 = 0.0;
    for mask in 0..8 {
        let corner = std::array::from_fn(|axis| {
            if mask & (1 << axis) == 0 {
                bounds.axes[axis].lo
            } else {
                bounds.axes[axis].hi
            }
        });
        maximum = maximum.max(point_segment_distance(corner, a, b));
    }
    maximum
}

#[cfg(test)]
fn bounded_curve_count(
    curve: &impl Curve,
    range: Interval,
    error: f64,
    max_segments: usize,
) -> Result<usize, GeometryError> {
    let mut stack = vec![(range, 0usize)];
    let mut maximum_depth = 0usize;
    let mut visits = 0usize;
    while let Some((interval, depth)) = stack.pop() {
        visits = visits
            .checked_add(1)
            .ok_or_else(|| GeometryError::LimitExceeded("curve tessellation visits".into()))?;
        if visits > max_segments.saturating_mul(2) {
            return Err(GeometryError::LimitExceeded(
                "curve tessellation visits".into(),
            ));
        }
        let a = curve.point_at(interval.lo)?;
        let b = curve.point_at(interval.hi)?;
        if bounds_chord_deviation(curve.enclose(interval)?, a, b) <= error {
            maximum_depth = maximum_depth.max(depth);
            continue;
        }
        if depth >= usize::BITS as usize - 1 || (1usize << (depth + 1)) > max_segments {
            return Err(GeometryError::LimitExceeded(
                "curve tessellation segments".into(),
            ));
        }
        let midpoint = interval.midpoint();
        let left = Interval::new(interval.lo, midpoint)?;
        let right = Interval::new(midpoint, interval.hi)?;
        stack.push((right, depth + 1));
        stack.push((left, depth + 1));
    }
    Ok(1usize << maximum_depth)
}

fn intersection_chord_deviation(
    brep: &BrepEnvelope,
    definition: u32,
    range: Interval,
    chord_start: Point3,
    chord_end: Point3,
) -> Result<f64, GeometryError> {
    let definition = brep
        .geometry
        .intersections
        .get(definition as usize)
        .ok_or_else(|| GeometryError::InvalidGeometry("missing intersection definition".into()))?;
    let support_a = brep.geometry.surface(definition.surfaces[0])?;
    let support_b = brep.geometry.surface(definition.surfaces[1])?;
    let mut maximum = 0.0_f64;
    for (index, tube) in definition.uv_tubes.iter().enumerate() {
        let anchor_start = definition.anchors[index].parameter;
        let anchor_end = definition.anchors[index + 1].parameter;
        let lo = range.lo.max(anchor_start);
        let hi = range.hi.min(anchor_end);
        if lo > hi {
            continue;
        }
        let bounds_a = support_a.enclose([tube[0], tube[1]])?;
        let bounds_b = support_b.enclose([tube[2], tube[3]])?;
        let mut axes = [Interval::point(0.0)?; 3];
        for axis in 0..3 {
            axes[axis] = Interval::new(
                bounds_a.axes[axis].lo.max(bounds_b.axes[axis].lo),
                bounds_a.axes[axis].hi.min(bounds_b.axes[axis].hi),
            )
            .map_err(|_| GeometryError::InvalidGeometry("disjoint trace support boxes".into()))?;
        }
        let local_start = definition.evaluate(lo, &brep.geometry)?.point;
        let local_end = definition.evaluate(hi, &brep.geometry)?.point;
        let local_deviation = bounds_chord_deviation(
            super::geometry::PatchBounds { axes },
            local_start,
            local_end,
        );
        let guide_deviation = point_segment_distance(local_start, chord_start, chord_end)
            .max(point_segment_distance(local_end, chord_start, chord_end));
        maximum = maximum.max(local_deviation + guide_deviation);
    }
    Ok(maximum)
}

fn enclosed_intersection_count(
    brep: &BrepEnvelope,
    definition: u32,
    curve: u32,
    range: Interval,
    error: f64,
    max_segments: usize,
) -> Result<usize, GeometryError> {
    let evaluator = brep.geometry.curve(curve)?;
    let mut stack = vec![(range, 0usize)];
    let mut maximum_depth = 0usize;
    let mut visits = 0usize;
    while let Some((interval, depth)) = stack.pop() {
        visits = visits
            .checked_add(1)
            .ok_or_else(|| GeometryError::LimitExceeded("curve tessellation visits".into()))?;
        if visits > max_segments.saturating_mul(2) {
            return Err(GeometryError::LimitExceeded(
                "curve tessellation visits".into(),
            ));
        }
        let start = evaluator.point_at(interval.lo)?;
        let end = evaluator.point_at(interval.hi)?;
        if intersection_chord_deviation(brep, definition, interval, start, end)? <= error {
            maximum_depth = maximum_depth.max(depth);
            continue;
        }
        if depth >= usize::BITS as usize - 1 || (1usize << (depth + 1)) > max_segments {
            return Err(GeometryError::LimitExceeded(
                "curve tessellation segments".into(),
            ));
        }
        let midpoint = interval.midpoint();
        stack.push((Interval::new(midpoint, interval.hi)?, depth + 1));
        stack.push((Interval::new(interval.lo, midpoint)?, depth + 1));
    }
    Ok(1usize << maximum_depth)
}

fn adaptive_intersection_count(
    brep: &BrepEnvelope,
    definition: u32,
    curve: u32,
    range: Interval,
    error: f64,
    max_segments: usize,
) -> Result<usize, GeometryError> {
    let evaluator = brep.geometry.curve(curve)?;
    brep.geometry
        .intersections
        .get(definition as usize)
        .ok_or_else(|| GeometryError::InvalidGeometry("missing intersection definition".into()))?;
    let closed = norm(sub(
        evaluator.point_at(range.lo)?,
        evaluator.point_at(range.hi)?,
    )) <= brep.accuracy.intersection;
    let mut count = if closed { 8 } else { 1 };
    loop {
        let mut achieved = 0.0_f64;
        for segment in 0..count {
            let lo = range.lo + range.width() * segment as f64 / count as f64;
            let hi = range.lo + range.width() * (segment + 1) as f64 / count as f64;
            let start = evaluator.point_at(lo)?;
            let end = evaluator.point_at(hi)?;
            let chord = sub(end, start);
            let length = norm(chord);
            if length == 0.0 {
                achieved = f64::INFINITY;
                break;
            }
            let direction = scale(chord, 1.0 / length);
            let mut tangent_turn = 0.0_f64;
            for fraction in [0.0, 0.25, 0.5, 0.75, 1.0] {
                let parameter = lo + (hi - lo) * fraction;
                let point = evaluator.point_at(parameter)?;
                achieved = achieved.max(point_segment_distance(point, start, end));
                let tangent = evaluator.tangent_at(parameter)?;
                let tangent = scale(tangent, 1.0 / norm(tangent));
                tangent_turn = tangent_turn.max(norm(cross(tangent, direction)));
            }
            achieved = achieved.max(0.25 * length * tangent_turn);
        }
        if achieved <= error {
            return Ok(count);
        }
        count = count
            .checked_mul(2)
            .filter(|next| *next <= max_segments)
            .ok_or_else(|| GeometryError::LimitExceeded("curve tessellation segments".into()))?;
    }
}

fn bounded_intersection_count(
    brep: &BrepEnvelope,
    definition: u32,
    curve: u32,
    range: Interval,
    error: f64,
    max_segments: usize,
) -> Result<usize, GeometryError> {
    match enclosed_intersection_count(brep, definition, curve, range, error, max_segments) {
        Ok(count) => Ok(count),
        Err(GeometryError::LimitExceeded(_)) => {
            let count =
                adaptive_intersection_count(brep, definition, curve, range, error, max_segments)?;
            Ok(count)
        }
        Err(error) => Err(error),
    }
}

fn edge_count(
    brep: &BrepEnvelope,
    id: u32,
    error: f64,
    max_triangles: usize,
) -> Result<usize, GeometryError> {
    match brep.topology.edges[id as usize].geometry {
        EdgeGeometry::Collapsed { .. } => Ok(1),
        EdgeGeometry::Curve { curve, range } => match &brep.geometry.curves[curve as usize] {
            CurveGeometry::Line { .. } => Ok(1),
            CurveGeometry::Circle { radius, .. } => count(
                range.width() * (radius / (8.0 * error)).sqrt(),
                if range.width() >= std::f64::consts::TAU - 1e-12 {
                    3
                } else {
                    1
                },
                max_triangles,
            ),
            CurveGeometry::Ellipse { major_radius, .. } => count(
                range.width() * (major_radius / (8.0 * error)).sqrt(),
                if range.width() >= std::f64::consts::TAU - 1e-12 {
                    3
                } else {
                    1
                },
                max_triangles,
            ),
            CurveGeometry::Intersection { definition } => {
                bounded_intersection_count(brep, *definition, curve, range, error, max_triangles)
            }
        },
    }
}

fn synchronize_edge_samples(
    brep: &BrepEnvelope,
    grids: &[Option<(Option<Rect>, [usize; 2])>],
    counts: &mut [usize],
) {
    fn find(parent: &mut [usize], value: usize) -> usize {
        let mut root = value;
        while parent[root] != root {
            root = parent[root];
        }
        let mut current = value;
        while parent[current] != current {
            let next = parent[current];
            parent[current] = root;
            current = next;
        }
        root
    }
    fn union(parent: &mut [usize], a: usize, b: usize) {
        let a = find(parent, a);
        let b = find(parent, b);
        if a != b {
            parent[b] = a;
        }
    }

    let mut parent: Vec<_> = (0..counts.len()).collect();
    for (rect, _) in grids.iter().flatten() {
        let Some(rect) = rect else { continue };
        for [a, b] in [[0, 2], [1, 3]] {
            let a = brep.topology.halfedges[rect.halfedges[a] as usize].edge as usize;
            let b = brep.topology.halfedges[rect.halfedges[b] as usize].edge as usize;
            union(&mut parent, a, b);
        }
    }
    let key = |edge: &super::topology::Edge| -> Option<[u64; 14]> {
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

fn sample_edge(brep: &BrepEnvelope, id: u32, n: usize) -> Result<Vec<Point3>, GeometryError> {
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

fn boundary_point(
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

fn curved_face(
    brep: &BrepEnvelope,
    face: &Face,
    rect: &Rect,
    desired: [usize; 2],
    samples: &[Vec<Point3>],
    mesh: &mut Tessellation,
    max_triangles: usize,
) -> Result<(), GeometryError> {
    let mut size = desired;
    for (i, &h) in rect.halfedges.iter().enumerate() {
        let n = samples[brep.topology.halfedges[h as usize].edge as usize]
            .len()
            .saturating_sub(1);
        size[i % 2] = size[i % 2].max(n);
    }
    let [nu, nv] = size;
    let [u, v] = face.trim.uv_bounds;
    let surface = brep.geometry.surface(face.surface)?;
    let start = mesh.positions.len() / 3;
    if nu
        .checked_add(1)
        .and_then(|n| nv.checked_add(1).and_then(|m| n.checked_mul(m)))
        .is_none_or(|n| n > max_triangles * 3)
    {
        return Err(GeometryError::LimitExceeded(
            "tessellation grid vertices".into(),
        ));
    }
    for j in 0..=nv {
        for i in 0..=nu {
            let uv = [
                (u.lo + u.width() * i as f64 / nu as f64).clamp(u.lo, u.hi),
                (v.lo + v.width() * j as f64 / nv as f64).clamp(v.lo, v.hi),
            ];
            let point = if j == 0 {
                boundary_point(brep, samples, rect.halfedges[0], i, nu, 0)?
            } else if j == nv {
                boundary_point(brep, samples, rect.halfedges[2], i, nu, 0)?
            } else if i == 0 {
                boundary_point(brep, samples, rect.halfedges[3], j, nv, 1)?
            } else if i == nu {
                boundary_point(brep, samples, rect.halfedges[1], j, nv, 1)?
            } else {
                surface.point_at(uv)?
            };
            let normal_uv = if matches!(surface, SurfaceGeometry::Cone { .. }) && uv[1] == 0.0 {
                [uv[0], v.hi]
            } else {
                uv
            };
            let normal = scale(surface.normal_at(normal_uv)?, face.sense.multiplier());
            mesh.vertex(point, normal)?;
        }
    }
    for j in 0..nv {
        for i in 0..nu {
            let a = (start + j * (nu + 1) + i) as u32;
            let b = a + 1;
            let c = a + (nu + 1) as u32;
            let d = c + 1;
            let uv = [
                u.lo + u.width() * (i as f64 + 0.5) / nu as f64,
                v.lo + v.width() * (j as f64 + 0.5) / nv as f64,
            ];
            let normal = scale(surface.normal_at(uv)?, face.sense.multiplier());
            mesh.triangle([a, b, d], face.id, normal, max_triangles)?;
            mesh.triangle([a, d, c], face.id, normal, max_triangles)?;
        }
    }
    Ok(())
}

fn trim_loop_samples(
    brep: &BrepEnvelope,
    face: &Face,
    loop_id: u32,
    samples: &[Vec<Point3>],
) -> Result<Vec<([f64; 2], Point3)>, GeometryError> {
    let periods = brep.geometry.surface(face.surface)?.charts()[face.trim.chart as usize].periods;
    let start = brep.topology.loops[loop_id as usize].start_halfedge;
    let mut current = start;
    let mut result = Vec::new();
    loop {
        let halfedge = &brep.topology.halfedges[current as usize];
        let edge = &brep.topology.edges[halfedge.edge as usize];
        let source = &samples[halfedge.edge as usize];
        let pcurve = halfedge
            .geometry_use
            .pcurve
            .ok_or_else(|| GeometryError::InvalidTopology("missing face pcurve".into()))?;
        let range = match edge.geometry {
            EdgeGeometry::Curve { range, .. } => range,
            EdgeGeometry::Collapsed { vertex } => {
                let parameter = if halfedge.geometry_use.sense == Orientation::Forward {
                    0.0
                } else {
                    1.0
                };
                let mut uv = brep.geometry.pcurve_at(pcurve, parameter)?;
                for axis in 0..2 {
                    uv[axis] += periods[axis].map_or(0.0, |period| {
                        period * halfedge.geometry_use.periodic_lift[axis] as f64
                    });
                }
                result.push((uv, brep.topology.vertices[vertex as usize].position));
                current = halfedge
                    .next
                    .ok_or_else(|| GeometryError::InvalidTopology("open face loop".into()))?;
                if current == start {
                    break;
                }
                continue;
            }
        };
        let segment_count = source.len().saturating_sub(1);
        if segment_count == 0 {
            return Err(GeometryError::InvalidTopology(
                "empty curved trim edge sample".into(),
            ));
        }
        for offset in 0..segment_count {
            let sample_index = if halfedge.geometry_use.sense == Orientation::Forward {
                offset
            } else {
                segment_count - offset
            };
            let parameter = range.lo + range.width() * sample_index as f64 / segment_count as f64;
            let mut uv = brep.geometry.pcurve_at(pcurve, parameter)?;
            for axis in 0..2 {
                uv[axis] += periods[axis].map_or(0.0, |period| {
                    period * halfedge.geometry_use.periodic_lift[axis] as f64
                });
            }
            result.push((uv, source[sample_index]));
        }
        current = halfedge
            .next
            .ok_or_else(|| GeometryError::InvalidTopology("open face loop".into()))?;
        if current == start {
            break;
        }
        if result.len()
            > brep
                .topology
                .halfedges
                .len()
                .saturating_mul(maximum_edge_samples(samples))
        {
            return Err(GeometryError::LimitExceeded(
                "curved trim loop samples".into(),
            ));
        }
    }
    if result.len() < 4 {
        return Err(GeometryError::InvalidTopology(
            "curved trim loop requires at least four samples".into(),
        ));
    }
    Ok(result)
}

fn maximum_edge_samples(samples: &[Vec<Point3>]) -> usize {
    samples.iter().map(Vec::len).max().unwrap_or(1)
}

fn periodic_ring_winding(ring: &[([f64; 2], Point3)], axis: usize, period: f64) -> i32 {
    let delta = ring
        .iter()
        .zip(ring.iter().cycle().skip(1))
        .take(ring.len())
        .map(|(a, b)| {
            let raw = b.0[axis] - a.0[axis];
            raw - (raw / period).round() * period
        })
        .sum::<f64>();
    (delta / period).round() as i32
}

fn open_periodic_ring(
    ring: &[([f64; 2], Point3)],
    axis: usize,
    period: f64,
    seam: f64,
    other_period: Option<f64>,
) -> Option<Vec<([f64; 2], Point3)>> {
    let other = 1 - axis;
    let mut sorted = ring
        .iter()
        .map(|&(mut uv, point)| {
            uv[axis] = seam + (uv[axis] - seam).rem_euclid(period);
            (uv, point)
        })
        .collect::<Vec<_>>();
    sorted.sort_by(|a, b| a.0[axis].total_cmp(&b.0[axis]));
    let tolerance = 256.0 * f64::EPSILON * period.max(1.0);
    sorted.dedup_by(|a, b| (a.0[axis] - b.0[axis]).abs() <= tolerance);
    if sorted.len() < 3 {
        return None;
    }

    let first = sorted[0];
    let last = *sorted.last()?;
    let before = last.0[axis] - period;
    let span = first.0[axis] - before;
    if span <= tolerance {
        return None;
    }
    let t = ((seam - before) / span).clamp(0.0, 1.0);
    let mut other_delta = first.0[other] - last.0[other];
    if let Some(other_period) = other_period {
        other_delta -= (other_delta / other_period).round() * other_period;
    }
    let other_value = last.0[other] + other_delta * t;
    let seam_point = std::array::from_fn(|component| {
        last.1[component] + (first.1[component] - last.1[component]) * t
    });
    let mut seam_uv = first.0;
    seam_uv[axis] = seam;
    seam_uv[other] = other_value;

    let mut opened = Vec::with_capacity(sorted.len() + 2);
    opened.push((seam_uv, seam_point));
    opened.extend(
        sorted
            .into_iter()
            .filter(|(uv, _)| (uv[axis] - seam).abs() > tolerance),
    );
    let mut end_uv = seam_uv;
    end_uv[axis] += period;
    opened.push((end_uv, seam_point));
    Some(opened)
}

fn periodic_band_loop(
    face: &Face,
    periods: [Option<f64>; 2],
    loops: &[Vec<([f64; 2], Point3)>],
) -> Option<Vec<([f64; 2], Point3)>> {
    if loops.len() != 2 {
        return None;
    }
    for axis in 0..2 {
        let Some(period) = periods[axis] else {
            continue;
        };
        let windings = [
            periodic_ring_winding(&loops[0], axis, period),
            periodic_ring_winding(&loops[1], axis, period),
        ];
        if windings.iter().any(|winding| winding.abs() != 1) {
            continue;
        }
        let other = 1 - axis;
        if let Some(other_period) = periods[other] {
            if loops
                .iter()
                .any(|ring| periodic_ring_winding(ring, other, other_period) != 0)
            {
                continue;
            }
        }
        let seam = face.trim.uv_bounds[axis].lo;
        let mut first = open_periodic_ring(&loops[0], axis, period, seam, periods[other])?;
        let mut second = open_periodic_ring(&loops[1], axis, period, seam, periods[other])?;
        let first_mean = first.iter().map(|(uv, _)| uv[other]).sum::<f64>() / first.len() as f64;
        let second_mean = second.iter().map(|(uv, _)| uv[other]).sum::<f64>() / second.len() as f64;
        if first_mean > second_mean {
            std::mem::swap(&mut first, &mut second);
        }
        second.reverse();
        first.extend(second);
        return Some(first);
    }
    None
}

fn merged_grid_coordinates(
    uniform: impl Iterator<Item = f64>,
    boundary: impl Iterator<Item = f64>,
) -> Vec<f64> {
    let mut values = uniform
        .map(|value| (value, false))
        .chain(boundary.map(|value| (value, true)))
        .collect::<Vec<_>>();
    values.sort_by(|a, b| a.0.total_cmp(&b.0));
    let magnitude = values
        .iter()
        .map(|(value, _)| value.abs())
        .fold(1.0_f64, f64::max);
    let tolerance = 128.0 * f64::EPSILON * magnitude;
    let mut merged: Vec<(f64, bool)> = Vec::with_capacity(values.len());
    for value in values {
        if let Some(last) = merged.last_mut() {
            if (value.0 - last.0).abs() <= tolerance {
                if value.1 {
                    *last = value;
                }
                continue;
            }
        }
        merged.push(value);
    }
    merged.into_iter().map(|(value, _)| value).collect()
}

fn trimmed_surface_edge_deviation(
    surface: &SurfaceGeometry,
    uv: &[[f64; 2]],
    a: usize,
    b: usize,
) -> f64 {
    let du = (uv[b][0] - uv[a][0]).abs();
    let dv = (uv[b][1] - uv[a][1]).abs();
    let v = [uv[a][1].min(uv[b][1]), uv[a][1].max(uv[b][1])];
    let maximum_trig = |offset: f64, values: fn(f64) -> f64| {
        let period = std::f64::consts::PI;
        let contains_extremum =
            ((v[0] - offset) / period).ceil() <= ((v[1] - offset) / period).floor();
        if contains_extremum {
            1.0
        } else {
            values(v[0]).abs().max(values(v[1]).abs())
        }
    };
    let acceleration = match surface {
        SurfaceGeometry::Plane { .. } => 0.0,
        SurfaceGeometry::Sphere { radius, .. } => {
            let cos_v = maximum_trig(0.0, f64::cos);
            let sin_v = maximum_trig(std::f64::consts::FRAC_PI_2, f64::sin);
            radius * (cos_v * du * du + 2.0 * sin_v * du * dv + dv * dv)
        }
        SurfaceGeometry::Cylinder { radius, .. } => radius * du * du,
        SurfaceGeometry::Cone { semi_angle, .. } => {
            let slope = semi_angle.tan();
            let radius = uv[a][1].abs().max(uv[b][1].abs()) * slope;
            radius * du * du + 2.0 * slope * du * dv
        }
        SurfaceGeometry::Torus {
            major_radius,
            minor_radius,
            ..
        } => {
            (major_radius + minor_radius) * du * du
                + 2.0 * minor_radius * du * dv
                + minor_radius * dv * dv
        }
    };
    acceleration / 8.0
}

fn general_curved_trimmed_face(
    brep: &BrepEnvelope,
    face: &Face,
    _desired: [usize; 2],
    loops: &[Vec<([f64; 2], Point3)>],
    error: f64,
    mesh: &mut Tessellation,
    max_triangles: usize,
) -> Result<(), GeometryError> {
    let mut uv = Vec::new();
    let mut points = Vec::new();
    let mut holes = Vec::new();
    let mut boundary_edges = HashSet::new();
    for (loop_index, ring) in loops.iter().enumerate() {
        if loop_index > 0 {
            holes.push(uv.len());
        }
        let start = uv.len();
        for &(coordinate, point) in ring {
            uv.push(coordinate);
            points.push(point);
        }
        for offset in 0..ring.len() {
            let a = start + offset;
            let b = start + (offset + 1) % ring.len();
            boundary_edges.insert(if a < b { (a, b) } else { (b, a) });
        }
    }
    let flattened = uv
        .iter()
        .flat_map(|coordinate| *coordinate)
        .collect::<Vec<_>>();
    let indices = earcutr::earcut(&flattened, &holes, 2);
    if indices.is_empty() {
        return Err(GeometryError::InvalidTopology(
            "curved trim triangulation failed".into(),
        ));
    }
    let mut triangles = indices
        .chunks_exact(3)
        .map(|triangle| [triangle[0], triangle[1], triangle[2]])
        .collect::<Vec<_>>();
    let surface = brep.geometry.surface(face.surface)?;
    for _ in 0..128 {
        let mut split_edges = HashSet::new();
        for triangle in &triangles {
            for [a, b] in [
                [triangle[0], triangle[1]],
                [triangle[1], triangle[2]],
                [triangle[2], triangle[0]],
            ] {
                let key = if a < b { (a, b) } else { (b, a) };
                if boundary_edges.contains(&key) {
                    continue;
                }
                let needs_refinement =
                    trimmed_surface_edge_deviation(surface, &uv, a, b) > error * 2.0;
                if needs_refinement {
                    split_edges.insert(key);
                }
            }
        }
        if split_edges.is_empty() {
            break;
        }
        if triangles
            .len()
            .checked_mul(4)
            .is_none_or(|count| count > max_triangles)
        {
            return Err(GeometryError::LimitExceeded(
                "curved trim refinement triangles".into(),
            ));
        }
        let mut midpoints = HashMap::with_capacity(split_edges.len());
        for &(a, b) in &split_edges {
            let coordinate = [(uv[a][0] + uv[b][0]) * 0.5, (uv[a][1] + uv[b][1]) * 0.5];
            let id = uv.len();
            uv.push(coordinate);
            points.push(surface.point_at(coordinate)?);
            midpoints.insert((a, b), id);
        }
        let midpoint = |a: usize, b: usize| {
            let key = if a < b { (a, b) } else { (b, a) };
            midpoints.get(&key).copied()
        };
        let mut refined = Vec::with_capacity(triangles.len() * 4);
        for [a, b, c] in triangles {
            let ab = midpoint(a, b);
            let bc = midpoint(b, c);
            let ca = midpoint(c, a);
            match (ab, bc, ca) {
                (None, None, None) => refined.push([a, b, c]),
                (Some(ab), None, None) => refined.extend([[a, ab, c], [ab, b, c]]),
                (None, Some(bc), None) => refined.extend([[b, bc, a], [bc, c, a]]),
                (None, None, Some(ca)) => refined.extend([[c, ca, b], [ca, a, b]]),
                (Some(ab), Some(bc), None) => {
                    refined.extend([[b, bc, ab], [ab, bc, c], [ab, c, a]])
                }
                (None, Some(bc), Some(ca)) => {
                    refined.extend([[c, ca, bc], [bc, ca, a], [bc, a, b]])
                }
                (Some(ab), None, Some(ca)) => {
                    refined.extend([[a, ab, ca], [ca, ab, b], [ca, b, c]])
                }
                (Some(ab), Some(bc), Some(ca)) => {
                    refined.extend([[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]])
                }
            }
        }
        triangles = refined;
    }
    for triangle in &triangles {
        for [a, b] in [
            [triangle[0], triangle[1]],
            [triangle[1], triangle[2]],
            [triangle[2], triangle[0]],
        ] {
            let key = if a < b { (a, b) } else { (b, a) };
            let needs_refinement = trimmed_surface_edge_deviation(surface, &uv, a, b) > error * 2.0;
            if !boundary_edges.contains(&key) && needs_refinement {
                return Err(GeometryError::LimitExceeded(format!(
                    "curved trim refinement depth on face {}: bound {} for edge {:?} to {:?}",
                    face.id,
                    trimmed_surface_edge_deviation(surface, &uv, a, b),
                    uv[a],
                    uv[b]
                )));
            }
        }
    }
    if triangles.len() > max_triangles {
        return Err(GeometryError::LimitExceeded("curved trim triangles".into()));
    }
    let start = mesh.positions.len() / 3;
    for (coordinate, point) in uv.iter().zip(points) {
        let normal_coordinate = if matches!(surface, SurfaceGeometry::Cone { .. })
            && coordinate[1].abs() <= brep.accuracy.geometric
        {
            let bounds = face.trim.uv_bounds[1];
            [
                coordinate[0],
                if bounds.lo.abs() > bounds.hi.abs() {
                    bounds.lo
                } else {
                    bounds.hi
                },
            ]
        } else {
            *coordinate
        };
        let normal = scale(
            surface.normal_at(normal_coordinate)?,
            face.sense.multiplier(),
        );
        mesh.vertex(point, normal)?;
    }
    for triangle in triangles {
        let center = std::array::from_fn(|axis| {
            (uv[triangle[0]][axis] + uv[triangle[1]][axis] + uv[triangle[2]][axis]) / 3.0
        });
        let normal_coordinate = if matches!(surface, SurfaceGeometry::Cone { .. })
            && center[1].abs() <= brep.accuracy.geometric
        {
            let bounds = face.trim.uv_bounds[1];
            [
                center[0],
                if bounds.lo.abs() > bounds.hi.abs() {
                    bounds.lo
                } else {
                    bounds.hi
                },
            ]
        } else {
            center
        };
        let normal = scale(
            surface.normal_at(normal_coordinate)?,
            face.sense.multiplier(),
        );
        mesh.triangle(
            triangle.map(|index| (start + index) as u32),
            face.id,
            normal,
            max_triangles,
        )?;
    }
    Ok(())
}

fn curved_trimmed_face(
    brep: &BrepEnvelope,
    face: &Face,
    rect: Option<&Rect>,
    desired: [usize; 2],
    samples: &[Vec<Point3>],
    error: f64,
    mesh: &mut Tessellation,
    max_triangles: usize,
) -> Result<(), GeometryError> {
    use crate::geometry::poly2d::{point_in_ring2, Pt2};

    let mut size = desired;
    if let Some(rect) = rect {
        for (axis, &halfedge) in rect.halfedges.iter().enumerate() {
            let edge = brep.topology.halfedges[halfedge as usize].edge as usize;
            size[axis % 2] = size[axis % 2].max(samples[edge].len().saturating_sub(1));
        }
    }
    let mut loops = Vec::with_capacity(face.trim.holes.len() + 1);
    for &loop_id in std::iter::once(&face.trim.outer).chain(&face.trim.holes) {
        let points = trim_loop_samples(brep, face, loop_id, samples)?;
        loops.push(points);
    }
    let surface = brep.geometry.surface(face.surface)?;
    let periods = surface.charts()[face.trim.chart as usize].periods;
    if let Some(band) = periodic_band_loop(face, periods, &loops) {
        loops = vec![band];
    }
    let mut axis_aligned = true;
    for points in &loops {
        let magnitude = points
            .iter()
            .flat_map(|(uv, _)| uv)
            .map(|value| value.abs())
            .fold(1.0_f64, f64::max);
        let tolerance = 128.0 * f64::EPSILON * magnitude;
        for pair in points
            .iter()
            .zip(points.iter().cycle().skip(1))
            .take(points.len())
        {
            let du = (pair.0 .0[0] - pair.1 .0[0]).abs();
            let dv = (pair.0 .0[1] - pair.1 .0[1]).abs();
            if du > tolerance && dv > tolerance {
                axis_aligned = false;
            }
        }
    }
    if rect.is_some() && !face.trim.holes.is_empty() {
        let mut bounds = face.trim.uv_bounds;
        let mut shifted = false;
        for axis in 0..2 {
            let Some(period) = periods[axis] else {
                continue;
            };
            let mut lo = f64::INFINITY;
            let mut hi = f64::NEG_INFINITY;
            for (uv, _) in loops[1..].iter().flatten() {
                lo = lo.min(uv[axis]);
                hi = hi.max(uv[axis]);
            }
            if !lo.is_finite() || hi - lo >= period {
                continue;
            }
            let original = face.trim.uv_bounds[axis];
            let tolerance =
                256.0 * f64::EPSILON * original.lo.abs().max(original.hi.abs()).max(1.0);
            if lo >= original.lo - tolerance && hi <= original.hi + tolerance {
                continue;
            }
            let center = 0.5 * (lo + hi);
            bounds[axis] = Interval::new(center - 0.5 * period, center + 0.5 * period)?;
            for ring in &mut loops[1..] {
                let ring_center =
                    ring.iter().map(|(uv, _)| uv[axis]).sum::<f64>() / ring.len().max(1) as f64;
                let offset = ((center - ring_center) / period).round() * period;
                for (uv, _) in ring {
                    uv[axis] += offset;
                }
            }
            shifted = true;
        }
        if shifted {
            let coordinates = [
                [bounds[0].lo, bounds[1].lo],
                [bounds[0].hi, bounds[1].lo],
                [bounds[0].hi, bounds[1].hi],
                [bounds[0].lo, bounds[1].hi],
            ];
            loops[0] = coordinates
                .into_iter()
                .map(|uv| Ok((uv, surface.point_at(uv)?)))
                .collect::<Result<Vec<_>, GeometryError>>()?;
        }
    }
    if !axis_aligned {
        return general_curved_trimmed_face(brep, face, size, &loops, error, mesh, max_triangles);
    }
    let [u, v] = face.trim.uv_bounds;
    let all_boundary = loops.iter().flatten().collect::<Vec<_>>();
    let us = merged_grid_coordinates(
        (0..=size[0]).map(|index| u.lo + u.width() * index as f64 / size[0] as f64),
        all_boundary.iter().map(|(uv, _)| uv[0]),
    );
    let vs = merged_grid_coordinates(
        (0..=size[1]).map(|index| v.lo + v.width() * index as f64 / size[1] as f64),
        all_boundary.iter().map(|(uv, _)| uv[1]),
    );
    let vertex_count = us
        .len()
        .checked_mul(vs.len())
        .ok_or_else(|| GeometryError::LimitExceeded("trimmed surface grid vertices".into()))?;
    let cell_count = us
        .len()
        .saturating_sub(1)
        .checked_mul(vs.len().saturating_sub(1))
        .and_then(|count| count.checked_mul(2))
        .ok_or_else(|| GeometryError::LimitExceeded("trimmed surface grid triangles".into()))?;
    if vertex_count > max_triangles.saturating_mul(3) || cell_count > max_triangles {
        return Err(GeometryError::LimitExceeded(
            "trimmed surface grid budget".into(),
        ));
    }
    let coordinate_tolerance = 256.0
        * f64::EPSILON
        * us.iter()
            .chain(&vs)
            .map(|value| value.abs())
            .fold(1.0_f64, f64::max);
    let surface = brep.geometry.surface(face.surface)?;
    let start = mesh.positions.len() / 3;
    for &chart_v in &vs {
        for &chart_u in &us {
            let uv = [chart_u, chart_v];
            let point = all_boundary
                .iter()
                .find(|(boundary_uv, _)| {
                    (boundary_uv[0] - chart_u).abs() <= coordinate_tolerance
                        && (boundary_uv[1] - chart_v).abs() <= coordinate_tolerance
                })
                .map_or_else(|| surface.point_at(uv), |(_, point)| Ok(*point))?;
            let normal_uv = if matches!(surface, SurfaceGeometry::Cone { .. }) && chart_v == 0.0 {
                [chart_u, v.hi]
            } else {
                uv
            };
            let normal = scale(surface.normal_at(normal_uv)?, face.sense.multiplier());
            mesh.vertex(point, normal)?;
        }
    }
    let rings = loops
        .iter()
        .map(|points| {
            points
                .iter()
                .map(|(uv, _)| Pt2::new(uv[0], uv[1]))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    for row in 0..vs.len() - 1 {
        for column in 0..us.len() - 1 {
            let center = Pt2::new(
                (us[column] + us[column + 1]) * 0.5,
                (vs[row] + vs[row + 1]) * 0.5,
            );
            if !point_in_ring2(center, &rings[0])
                || rings[1..].iter().any(|hole| point_in_ring2(center, hole))
            {
                continue;
            }
            let a = (start + row * us.len() + column) as u32;
            let b = a + 1;
            let c = a + us.len() as u32;
            let d = c + 1;
            let normal = scale(
                surface.normal_at([center.x, center.z])?,
                face.sense.multiplier(),
            );
            mesh.triangle([a, b, d], face.id, normal, max_triangles)?;
            mesh.triangle([a, d, c], face.id, normal, max_triangles)?;
        }
    }
    Ok(())
}

fn planar_face(
    brep: &BrepEnvelope,
    face: &Face,
    samples: &[Vec<Point3>],
    mesh: &mut Tessellation,
    max_triangles: usize,
) -> Result<(), GeometryError> {
    let surface = brep.geometry.surface(face.surface)?;
    let normal = scale(surface.normal_at([0.0; 2])?, face.sense.multiplier());
    let mut points = Vec::new();
    let mut uv_flat = Vec::new();
    let mut holes = Vec::new();
    for (i, &loop_id) in std::iter::once(&face.trim.outer)
        .chain(&face.trim.holes)
        .enumerate()
    {
        if i > 0 {
            holes.push(points.len());
        }
        let start = brep.topology.loops[loop_id as usize].start_halfedge;
        let mut current = start;
        loop {
            let h = &brep.topology.halfedges[current as usize];
            let source = &samples[h.edge as usize];
            if source.len() > 1 {
                let n = source.len() - 1;
                for j in 0..n {
                    let p = source[if h.geometry_use.sense == Orientation::Forward {
                        j
                    } else {
                        n - j
                    }];
                    let uv = surface.project(p, None)?;
                    points.push(p);
                    uv_flat.extend(uv);
                }
            }
            current = h
                .next
                .ok_or_else(|| GeometryError::InvalidTopology("open planar face loop".into()))?;
            if current == start {
                break;
            }
        }
        let loop_start = if i == 0 { 0 } else { holes[i - 1] };
        if points.len() - loop_start < 3 {
            return Err(GeometryError::InvalidTopology(
                "insufficient tessellated planar boundary".into(),
            ));
        }
    }
    let triangles = earcutr::earcut(&uv_flat, &holes, 2);
    if triangles.is_empty() && points.len() >= 3 {
        return Err(GeometryError::InvalidTopology(
            "planar triangulation failed".into(),
        ));
    }
    let mut ids = Vec::with_capacity(points.len());
    for p in points {
        ids.push(mesh.vertex(p, normal)?);
    }
    for tri in triangles.chunks_exact(3) {
        let a = *ids
            .get(tri[0])
            .ok_or_else(|| GeometryError::InvalidTopology("triangulation index".into()))?;
        let b = *ids
            .get(tri[1])
            .ok_or_else(|| GeometryError::InvalidTopology("triangulation index".into()))?;
        let c = *ids
            .get(tri[2])
            .ok_or_else(|| GeometryError::InvalidTopology("triangulation index".into()))?;
        mesh.triangle([a, b, c], face.id, normal, max_triangles)?;
    }
    Ok(())
}

pub fn tessellate(
    brep: &BrepEnvelope,
    deflection: f64,
    max_triangles: usize,
) -> Result<Tessellation, GeometryError> {
    brep.validate()?;
    if !deflection.is_finite()
        || deflection <= 2.0 * brep.accuracy.geometric
        || max_triangles == 0
        || max_triangles > 10_000_000
    {
        return Err(GeometryError::InvalidGeometry(
            "deflection must exceed twice geometric tolerance; triangle limit must be 1..=10000000"
                .into(),
        ));
    }
    let error = deflection * 0.5;
    let mut magnitude: f64 = 0.0;
    for face in &brep.topology.faces {
        let surface = brep.geometry.surface(face.surface)?;
        for value in surface.frame().origin {
            magnitude = magnitude.max(value.abs());
        }
        let bounds = surface.enclose(face.trim.uv_bounds)?;
        for axis in bounds.axes {
            magnitude = magnitude.max(axis.lo.abs()).max(axis.hi.abs());
        }
    }
    for vertex in &brep.topology.vertices {
        for value in vertex.position {
            magnitude = magnitude.max(value.abs());
        }
    }
    for edge in &brep.topology.edges {
        if let EdgeGeometry::Curve { curve, range } = edge.geometry {
            let curve = brep.geometry.curve(curve)?;
            for axis in curve.enclose(range)?.axes {
                magnitude = magnitude.max(axis.lo.abs()).max(axis.hi.abs());
            }
            match curve.geometry {
                CurveGeometry::Circle { frame, radius } => {
                    magnitude = magnitude.max(*radius);
                    for value in frame.origin {
                        magnitude = magnitude.max(value.abs());
                    }
                }
                CurveGeometry::Ellipse {
                    frame,
                    major_radius,
                    ..
                } => {
                    magnitude = magnitude.max(*major_radius);
                    for value in frame.origin {
                        magnitude = magnitude.max(value.abs());
                    }
                }
                _ => {}
            }
        }
    }
    let coordinate_floor = 64.0 * f64::EPSILON * magnitude;
    if deflection <= 2.0 * (brep.accuracy.geometric + coordinate_floor) {
        return Err(GeometryError::LimitExceeded(
            "coordinate precision cannot meet requested deflection".into(),
        ));
    }
    let mut counts = Vec::with_capacity(brep.topology.edges.len());
    for edge in &brep.topology.edges {
        counts.push(edge_count(brep, edge.id, error, max_triangles)?);
    }
    let mut grids = Vec::with_capacity(brep.topology.faces.len());
    for face in &brep.topology.faces {
        let surface = brep.geometry.surface(face.surface)?;
        if matches!(surface, SurfaceGeometry::Plane { .. }) {
            grids.push(None);
            continue;
        }
        let rect = match rectangle(brep, face) {
            Ok(rect) => Some(rect),
            Err(GeometryError::UnsupportedGeometry(_)) => None,
            Err(error) => return Err(error),
        };
        let desired = grid_size(surface, face.trim.uv_bounds, error, max_triangles)?;
        if let Some(rect) = &rect {
            for (i, &h) in rect.halfedges.iter().enumerate() {
                let id = brep.topology.halfedges[h as usize].edge;
                counts[id as usize] = counts[id as usize].max(desired[i % 2]);
            }
        }
        grids.push(Some((rect, desired)));
    }
    // Opposite chart edges and concentric trim curves must share subdivision counts.
    synchronize_edge_samples(brep, &grids, &mut counts);
    let sample_count = counts
        .iter()
        .try_fold(0usize, |total, &n| total.checked_add(n + 1))
        .ok_or_else(|| GeometryError::LimitExceeded("edge samples".into()))?;
    if sample_count > max_triangles * 3 {
        return Err(GeometryError::LimitExceeded("edge sample budget".into()));
    }
    let mut samples = Vec::with_capacity(counts.len());
    for (id, n) in counts.into_iter().enumerate() {
        samples.push(sample_edge(brep, id as u32, n)?);
    }
    let mut mesh = Tessellation::new(brep.revision, deflection);
    for (face, grid) in brep.topology.faces.iter().zip(&grids) {
        let before = mesh.triangle_face_ids.len();
        match grid {
            Some((Some(rect), desired)) if face.trim.holes.is_empty() => curved_face(
                brep,
                face,
                rect,
                *desired,
                &samples,
                &mut mesh,
                max_triangles,
            )?,
            Some((rect, desired)) => curved_trimmed_face(
                brep,
                face,
                rect.as_ref(),
                *desired,
                &samples,
                error,
                &mut mesh,
                max_triangles,
            )?,
            None => planar_face(brep, face, &samples, &mut mesh, max_triangles)?,
        }
        if mesh.triangle_face_ids.len() == before {
            return Err(GeometryError::UnresolvedTessellation(format!(
                "face {} produced no nondegenerate tessellation triangles",
                face.id
            )));
        }
    }
    for (edge, points) in brep.topology.edges.iter().zip(samples) {
        let planar_subdivision = edge
            .twin_halfedge
            .and_then(|twin| {
                let first = &brep.topology.halfedges[edge.halfedge as usize];
                let second = &brep.topology.halfedges[twin as usize];
                let a = &brep.topology.faces[first.face? as usize];
                let b = &brep.topology.faces[second.face? as usize];
                match (
                    &brep.geometry.surfaces[a.surface as usize],
                    &brep.geometry.surfaces[b.surface as usize],
                ) {
                    (
                        SurfaceGeometry::Plane { frame: a_frame },
                        SurfaceGeometry::Plane { frame: b_frame },
                    ) => Some(
                        scale(a_frame.z, a.sense.multiplier())
                            == scale(b_frame.z, b.sense.multiplier()),
                    ),
                    _ => Some(false),
                }
            })
            .unwrap_or(false);
        if edge.chart_seam || planar_subdivision {
            continue;
        }
        for pair in points.windows(2) {
            mesh.outline_positions.extend(pair[0]);
            mesh.outline_positions.extend(pair[1]);
            mesh.outline_edge_ids.push(edge.id);
        }
    }
    Ok(mesh)
}

struct HashWriter(DefaultHasher);
impl Write for HashWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.write(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
struct CacheEntry {
    body: u64,
    signature: u64,
    deflection: u64,
    max_triangles: usize,
    mesh: Arc<Tessellation>,
}
pub struct TessellationCache {
    entries: VecDeque<CacheEntry>,
    max_bytes: usize,
}
impl TessellationCache {
    pub fn new(max_bytes: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            max_bytes,
        }
    }
    pub fn tessellate(
        &mut self,
        brep: &BrepEnvelope,
        deflection: f64,
        max_triangles: usize,
    ) -> Result<Arc<Tessellation>, GeometryError> {
        brep.validate()?;
        let mut writer = HashWriter(DefaultHasher::new());
        serde_json::to_writer(&mut writer, brep)
            .map_err(|e| GeometryError::InvalidGeometry(e.to_string()))?;
        let signature = writer.0.finish();
        if let Some(i) = self.entries.iter().position(|e| {
            e.signature == signature
                && e.deflection == deflection.to_bits()
                && e.max_triangles == max_triangles
        }) {
            let entry = self
                .entries
                .remove(i)
                .ok_or_else(|| GeometryError::InvalidGeometry("cache entry missing".into()))?;
            let mesh = Arc::clone(&entry.mesh);
            self.entries.push_back(entry);
            return Ok(mesh);
        }
        let mesh = Arc::new(tessellate(brep, deflection, max_triangles)?);
        if mesh.bytes() > self.max_bytes {
            return Ok(mesh);
        }
        let mut hasher = DefaultHasher::new();
        hasher.write(brep.id.as_bytes());
        let body = hasher.finish();
        while self.entries.iter().filter(|e| e.body == body).count() >= 3 {
            if let Some(i) = self.entries.iter().position(|e| e.body == body) {
                self.entries.remove(i);
            } else {
                break;
            }
        }
        while self.entries.iter().map(|e| e.mesh.bytes()).sum::<usize>() + mesh.bytes()
            > self.max_bytes
        {
            self.entries.pop_front();
        }
        self.entries.push_back(CacheEntry {
            body,
            signature,
            deflection: deflection.to_bits(),
            max_triangles,
            mesh: Arc::clone(&mesh),
        });
        Ok(mesh)
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytic::{primitives, topology::Accuracy, Frame3};
    use std::collections::HashMap;
    fn accuracy() -> Accuracy {
        Accuracy {
            geometric: 1e-9,
            intersection: 1e-10,
            tessellation: 1e-3,
            exchange: 1e-5,
        }
    }
    fn shapes() -> Vec<BrepEnvelope> {
        vec![
            primitives::cylinder("c".into(), Frame3::IDENTITY, 1.0, 2.0, accuracy()).unwrap(),
            primitives::cone("k".into(), Frame3::IDENTITY, 1.0, 2.0, accuracy()).unwrap(),
            primitives::frustum("f".into(), Frame3::IDENTITY, 2.0, 1.0, 3.0, accuracy()).unwrap(),
            primitives::sphere("s".into(), Frame3::IDENTITY, 1.0, accuracy()).unwrap(),
            primitives::torus("t".into(), Frame3::IDENTITY, 3.0, 1.0, accuracy()).unwrap(),
            primitives::circular_wall(
                "wall".into(),
                Frame3::IDENTITY,
                3.0,
                0.4,
                2.5,
                0.7,
                -1.8,
                accuracy(),
            )
            .unwrap(),
        ]
    }
    #[test]
    fn conservative_curve_subdivision_respects_requested_error() {
        let mut store = crate::analytic::topology::GeometryStore::new();
        store.curves.push(CurveGeometry::Circle {
            frame: Frame3::from_axis([1.0, 2.0, 3.0], [1.0, 1.0, 1.0], [1.0, -1.0, 0.0]).unwrap(),
            radius: 2.0,
        });
        let curve = store.curve(0).unwrap();
        let range = Interval::new(0.0, std::f64::consts::TAU).unwrap();
        let loose = bounded_curve_count(&curve, range, 0.2, 1_000_000).unwrap();
        let fine = bounded_curve_count(&curve, range, 0.02, 1_000_000).unwrap();
        assert!(fine > loose);
        for index in 0..fine {
            let interval = Interval::new(
                range.lo + range.width() * index as f64 / fine as f64,
                range.lo + range.width() * (index + 1) as f64 / fine as f64,
            )
            .unwrap();
            assert!(
                bounds_chord_deviation(
                    curve.enclose(interval).unwrap(),
                    curve.point_at(interval.lo).unwrap(),
                    curve.point_at(interval.hi).unwrap(),
                ) <= 0.02
            );
        }
    }
    #[test]
    fn lod_changes_mesh_not_topology_and_retains_face_ids() {
        for b in shapes() {
            let original = b.to_json().unwrap();
            let coarse = tessellate(&b, 0.1, 1_000_000).unwrap();
            let fine = tessellate(&b, 0.02, 1_000_000).unwrap();
            assert!(fine.indices.len() > coarse.indices.len());
            assert_eq!(fine.indices.len() / 3, fine.triangle_face_ids.len());
            assert!(fine
                .triangle_face_ids
                .iter()
                .all(|&f| (f as usize) < b.topology.faces.len()));
            assert_eq!(b.to_json().unwrap(), original);
            for n in fine.normals.chunks_exact(3) {
                assert!((norm([n[0] as f64, n[1] as f64, n[2] as f64]) - 1.0).abs() < 1e-6);
            }
        }
    }
    #[test]
    fn shared_boundaries_make_a_closed_geometric_mesh() {
        for b in shapes() {
            let mesh = tessellate(&b, 0.05, 1_000_000).unwrap();
            let key = |p: Point3| p.map(f64::to_bits);
            let mut edges = HashMap::<([u64; 3], [u64; 3]), usize>::new();
            for t in mesh.indices.chunks_exact(3) {
                for (a, c) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                    let mut pair = [key(mesh.point(a)), key(mesh.point(c))];
                    pair.sort();
                    *edges.entry((pair[0], pair[1])).or_default() += 1;
                }
            }
            assert!(
                edges.values().all(|&count| count == 2),
                "{} has unmatched geometric mesh edges",
                b.id
            );
        }
    }
    #[test]
    fn spherical_triangle_samples_obey_deflection() {
        let b = primitives::sphere("s".into(), Frame3::IDENTITY, 1.0, accuracy()).unwrap();
        let deflection = 0.05;
        let mesh = tessellate(&b, deflection, 1_000_000).unwrap();
        for ids in mesh.indices.chunks_exact(3) {
            let p = ids
                .iter()
                .map(|&id| mesh.point(id))
                .fold([0.0; 3], super::super::geometry::add);
            assert!(1.0 - norm(scale(p, 1.0 / 3.0)) <= deflection);
        }
    }
    #[test]
    fn periodic_chart_lifts_keep_shared_positions_and_hide_seams() {
        let b =
            primitives::cylinder("lift".into(), Frame3::IDENTITY, 1.0, 2.0, accuracy()).unwrap();
        let original = tessellate(&b, 0.05, 1_000_000).unwrap();
        let mut lifted = b;
        for h in &mut lifted.topology.halfedges {
            if h.face == Some(0) {
                h.geometry_use.periodic_lift = [1, 0];
            }
        }
        let tau = std::f64::consts::TAU;
        lifted.topology.faces[0].trim.uv_bounds[0] = Interval::new(tau, 2.0 * tau).unwrap();
        let mesh = tessellate(&lifted, 0.05, 1_000_000).unwrap();
        assert_eq!(original.positions, mesh.positions);
        assert_eq!(original.outline_positions, mesh.outline_positions);
        assert_eq!(original.outline_edge_ids, mesh.outline_edge_ids);
    }
    #[test]
    fn thin_circular_wall_keeps_its_footprint_at_coarse_deflection() {
        let sweep = 7.0 * (0.2_f64 / 3.0).sqrt();
        let thickness = 0.0001;
        let b = primitives::circular_wall(
            "thin".into(),
            Frame3::IDENTITY,
            3.0,
            thickness,
            2.0,
            0.7,
            sweep,
            accuracy(),
        )
        .unwrap();
        let mesh = tessellate(&b, 0.05, 1_000_000).unwrap();
        let mut area = 0.0;
        for (ids, &face) in mesh.indices.chunks_exact(3).zip(&mesh.triangle_face_ids) {
            if face != 3 {
                continue;
            }
            let [a, c, d] = [mesh.point(ids[0]), mesh.point(ids[1]), mesh.point(ids[2])];
            area += norm(cross(sub(c, a), sub(d, a))) * 0.5;
        }
        assert!((area / (3.0 * thickness * sweep) - 1.0).abs() < 0.1);
    }
    #[test]
    fn planar_tessellation_preserves_a_circular_hole() {
        let mut b = crate::analytic::topology::tests::circular_face();
        b.geometry.curves[0] = CurveGeometry::Circle {
            frame: Frame3::IDENTITY,
            radius: 2.0,
        };
        b.geometry.pcurves[0] = crate::analytic::topology::PcurveGeometry::Conic2 {
            origin: [0.0; 2],
            axis_a: [2.0, 0.0],
            axis_b: [0.0, 2.0],
        };
        b.topology.vertices[0].position = [2.0, 0.0, 0.0];
        b.topology.faces[0].trim.uv_bounds = [Interval::new(-2.0, 2.0).unwrap(); 2];
        b.topology.faces[0].trim.holes.push(1);
        b.geometry.curves.push(CurveGeometry::Circle {
            frame: Frame3::IDENTITY,
            radius: 1.0,
        });
        b.geometry
            .pcurves
            .push(crate::analytic::topology::PcurveGeometry::Conic2 {
                origin: [0.0; 2],
                axis_a: [1.0, 0.0],
                axis_b: [0.0, 1.0],
            });
        let mut vertex = b.topology.vertices[0].clone();
        vertex.id = 1;
        vertex.position = [1.0, 0.0, 0.0];
        vertex.outgoing_halfedge = Some(1);
        b.topology.vertices.push(vertex);
        let mut edge = b.topology.edges[0].clone();
        edge.id = 1;
        edge.halfedge = 1;
        edge.geometry = EdgeGeometry::Curve {
            curve: 1,
            range: Interval::new(0.0, std::f64::consts::TAU).unwrap(),
        };
        b.topology.edges.push(edge);
        let mut h = b.topology.halfedges[0].clone();
        h.id = 1;
        h.from = 1;
        h.to = 1;
        h.next = Some(1);
        h.prev = Some(1);
        h.edge = 1;
        h.loop_ref = Some(1);
        h.geometry_use.pcurve = Some(1);
        h.geometry_use.sense = Orientation::Reverse;
        b.topology.halfedges.push(h);
        b.topology.loops.push(crate::analytic::topology::Loop {
            id: 1,
            start_halfedge: 1,
            face_ref: 0,
            is_hole: true,
        });
        let deflection = 0.02;
        let mesh = tessellate(&b, deflection, 1_000_000).unwrap();
        let mut area = 0.0;
        for ids in mesh.indices.chunks_exact(3) {
            let [a, c, d] = [mesh.point(ids[0]), mesh.point(ids[1]), mesh.point(ids[2])];
            let center = scale(
                crate::analytic::geometry::add(crate::analytic::geometry::add(a, c), d),
                1.0 / 3.0,
            );
            assert!(center[0].hypot(center[1]) >= 1.0 - deflection);
            area += norm(cross(sub(c, a), sub(d, a))) * 0.5;
        }
        assert!(
            (area - 3.0 * std::f64::consts::PI).abs() < 6.0 * std::f64::consts::PI * deflection
        );
    }
    #[test]
    fn precision_limits_do_not_claim_unachievable_deflection() {
        let near_cylinder = primitives::frustum(
            "near".into(),
            Frame3::IDENTITY,
            1.0,
            1.0 + 2.0_f64.powi(-48),
            1.0,
            accuracy(),
        )
        .unwrap();
        assert!(
            matches!(tessellate(&near_cylinder, 0.01, 1_000_000), Err(GeometryError::LimitExceeded(message)) if message.contains("coordinate precision"))
        );
        let frame = Frame3 {
            origin: [1e12, 0.0, 0.0],
            ..Frame3::IDENTITY
        };
        let b = primitives::sphere(
            "far".into(),
            frame,
            1.0,
            Accuracy {
                geometric: 0.001,
                intersection: 0.0001,
                ..accuracy()
            },
        )
        .unwrap();
        assert!(
            matches!(tessellate(&b, 0.01, 1_000_000), Err(GeometryError::LimitExceeded(message)) if message.contains("coordinate precision"))
        );
        let mut cache = TessellationCache::new(16_000_000);
        let coarse = cache.tessellate(&b, 0.1, 1_000_000).unwrap();
        assert!(cache.tessellate(&b, 0.01, 1_000_000).is_err());
        assert!(Arc::ptr_eq(
            &coarse,
            &cache.tessellate(&b, 0.1, 1_000_000).unwrap()
        ));
    }
    #[test]
    fn extreme_scales_return_errors_instead_of_invalid_or_empty_meshes() {
        for (radius, geometric, deflection) in [(1e300, 1e290, 1e299), (1e-200, 1e-210, 1e-201)] {
            let b = primitives::sphere(
                "extreme".into(),
                Frame3::IDENTITY,
                radius,
                Accuracy {
                    geometric,
                    intersection: geometric / 4.0,
                    ..accuracy()
                },
            )
            .unwrap();
            assert!(matches!(
                tessellate(&b, deflection, 1_000_000),
                Err(GeometryError::UnresolvedTessellation(_))
            ));
        }
    }
    #[test]
    fn cache_tracks_geometry_even_when_revision_is_not_incremented() {
        let mut b =
            primitives::cylinder("c".into(), Frame3::IDENTITY, 1.0, 2.0, accuracy()).unwrap();
        let mut cache = TessellationCache::new(16_000_000);
        let a = cache.tessellate(&b, 0.1, 1_000_000).unwrap();
        let again = cache.tessellate(&b, 0.1, 1_000_000).unwrap();
        assert!(Arc::ptr_eq(&a, &again));
        b.topology.faces[0].key = "changed".into();
        let changed = cache.tessellate(&b, 0.1, 1_000_000).unwrap();
        assert!(!Arc::ptr_eq(&a, &changed));
        for d in [0.08, 0.06, 0.04, 0.02] {
            cache.tessellate(&b, d, 1_000_000).unwrap();
        }
        assert_eq!(cache.len(), 3);
        assert!(tessellate(&b, 0.001, 1).is_err());
    }
}
