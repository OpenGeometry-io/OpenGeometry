use super::delaunay::{straight_run_corners, Triangulation};
use super::grid::Rect;
use super::mesh::Tessellation;
use super::trim::{
    merged_grid_coordinates, periodic_band_loop, trim_loop_samples, trimmed_surface_edge_deviation,
};
use crate::brep::{BrepEnvelope, Face, GeometryError, Surface, SurfaceGeometry};
use crate::geom2d::{point_in_ring2, Pt2};
use crate::math::{norm, scale, Interval, Point3};
use std::collections::BTreeSet;

const COUNT_ROUNDING: f64 = 1e-9;

pub(super) struct FaceGrid<'a> {
    pub(super) rect: Option<&'a Rect>,
    pub(super) desired: [usize; 2],
}

struct TrimGrid<'a> {
    us: &'a [f64],
    vs: &'a [f64],
    surface: &'a SurfaceGeometry,
    face: &'a Face,
}

pub(super) fn curved_trimmed_face(
    brep: &BrepEnvelope,
    face: &Face,
    face_grid: &FaceGrid<'_>,
    samples: &[Vec<Point3>],
    error: f64,
    mesh: &mut Tessellation,
    max_triangles: usize,
) -> Result<(), GeometryError> {
    let FaceGrid { rect, desired } = *face_grid;
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
    let axis_aligned = all_axis_aligned(&loops);
    let extent = loop_extent(&loops[0])?;
    if rect.is_some() && !face.trim.holes.is_empty() {
        shift_periodic_holes(face, surface, periods, &mut loops)?;
    }
    if !axis_aligned {
        return general_curved_trimmed_face(brep, face, size, &loops, error, mesh, max_triangles);
    }
    let all_boundary = loops.iter().flatten().collect::<Vec<_>>();
    let us = merged_grid_coordinates(
        uniform_coordinates(extent[0], face.trim.uv_bounds[0], size[0]),
        all_boundary.iter().map(|(uv, _)| uv[0]),
    );
    let vs = merged_grid_coordinates(
        uniform_coordinates(extent[1], face.trim.uv_bounds[1], size[1]),
        all_boundary.iter().map(|(uv, _)| uv[1]),
    );
    check_grid_budget(&us, &vs, max_triangles)?;
    let coordinate_tolerance = 256.0
        * f64::EPSILON
        * us.iter()
            .chain(&vs)
            .map(|value| value.abs())
            .fold(1.0_f64, f64::max);
    let surface = brep.geometry.surface(face.surface)?;
    let start = mesh.positions.len() / 3;
    let grid = TrimGrid {
        us: &us,
        vs: &vs,
        surface,
        face,
    };
    add_grid_vertices(
        &grid,
        &all_boundary,
        coordinate_tolerance,
        face.trim.uv_bounds[1],
        mesh,
    )?;
    add_grid_triangles(&grid, &loops, start, mesh, max_triangles)?;
    Ok(())
}

fn loop_extent(points: &[([f64; 2], Point3)]) -> Result<[Interval; 2], GeometryError> {
    let mut low = [f64::INFINITY; 2];
    let mut high = [f64::NEG_INFINITY; 2];
    for (uv, _) in points {
        for axis in 0..2 {
            low[axis] = low[axis].min(uv[axis]);
            high[axis] = high[axis].max(uv[axis]);
        }
    }
    Ok([
        Interval::new(low[0], high[0])?,
        Interval::new(low[1], high[1])?,
    ])
}

fn uniform_coordinates(
    extent: Interval,
    bounds: Interval,
    count: usize,
) -> impl Iterator<Item = f64> {
    let scaled = count as f64 * extent.width() / bounds.width() * (1.0 - COUNT_ROUNDING);
    let count = (scaled.ceil() as usize).max(1);
    (0..=count).map(move |index| extent.lo + extent.width() * index as f64 / count as f64)
}

fn all_axis_aligned(loops: &[Vec<([f64; 2], Point3)>]) -> bool {
    let mut axis_aligned = true;
    for points in loops {
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
    axis_aligned
}

fn shift_periodic_holes(
    face: &Face,
    surface: &SurfaceGeometry,
    periods: [Option<f64>; 2],
    loops: &mut [Vec<([f64; 2], Point3)>],
) -> Result<(), GeometryError> {
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
        let tolerance = 256.0 * f64::EPSILON * original.lo.abs().max(original.hi.abs()).max(1.0);
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
    Ok(())
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
    let mut boundary_edges = BTreeSet::new();
    for ring in loops {
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
    let surface = brep.geometry.surface(face.surface)?;
    let metric = uv_metric(surface, &uv[..loops[0].len()])?;
    let mut triangulation = triangulate(brep, &mut uv, loops, metric, &boundary_edges)?;
    triangulation.flip(&uv, triangulation.free_edges());
    let mut queue = triangulation.free_edges();
    while let Some((a, b)) = queue.pop() {
        if !triangulation.has_edge(a, b)
            || trimmed_surface_edge_deviation(surface, &uv, a, b) <= error * 2.0
        {
            continue;
        }
        if triangulation.triangles.len() + 2 > max_triangles {
            return Err(GeometryError::LimitExceeded(
                "curved trim refinement triangles".into(),
            ));
        }
        let coordinate = [(uv[a][0] + uv[b][0]) * 0.5, (uv[a][1] + uv[b][1]) * 0.5];
        uv.push(coordinate);
        points.push(surface.point_at(coordinate)?);
        queue.extend(triangulation.split(&uv, a, b, uv.len() - 1));
    }
    let triangles = triangulation.triangles;
    check_trim_refinement(face, surface, &triangles, &boundary_edges, &uv, error)?;
    if triangles.len() > max_triangles {
        return Err(GeometryError::LimitExceeded("curved trim triangles".into()));
    }
    let start = mesh.positions.len() / 3;
    for (coordinate, point) in uv.iter().zip(points) {
        let normal = trim_normal(brep, face, surface, *coordinate)?;
        mesh.vertex(point, normal)?;
    }
    for triangle in triangles {
        let center = std::array::from_fn(|axis| {
            (uv[triangle[0]][axis] + uv[triangle[1]][axis] + uv[triangle[2]][axis]) / 3.0
        });
        let normal = trim_normal(brep, face, surface, center)?;
        mesh.triangle(
            triangle.map(|index| (start + index) as u32),
            face.id,
            normal,
            max_triangles,
        )?;
    }
    Ok(())
}

fn snap_axis_noise(ring: &mut [[f64; 2]], tolerance: [f64; 2]) {
    let count = ring.len();
    for index in 0..count {
        let previous = ring[(index + count - 1) % count];
        for axis in 0..2 {
            if (ring[index][axis] - previous[axis]).abs() <= tolerance[axis] {
                ring[index][axis] = previous[axis];
            }
        }
    }
}

fn triangulate<'a>(
    brep: &BrepEnvelope,
    uv: &mut [[f64; 2]],
    loops: &[Vec<([f64; 2], Point3)>],
    metric: [f64; 2],
    boundary_edges: &'a BTreeSet<(usize, usize)>,
) -> Result<Triangulation<'a>, GeometryError> {
    let tolerance = metric.map(|scale| brep.accuracy.geometric / scale);
    let mut rings = Vec::with_capacity(loops.len());
    for ring in loops {
        let start = rings
            .last()
            .map_or(0, |range: &std::ops::Range<usize>| range.end);
        rings.push(start..start + ring.len());
        snap_axis_noise(&mut uv[start..start + ring.len()], tolerance);
    }
    let corners = rings
        .iter()
        .map(|ring| straight_run_corners(uv, ring.clone(), metric))
        .collect::<Vec<_>>();
    let kept = corners.concat();
    let mut kept_holes = Vec::with_capacity(corners.len());
    for ring in &corners[..corners.len() - 1] {
        kept_holes.push(kept_holes.last().copied().unwrap_or(0) + ring.len());
    }
    let kept_uv = kept.iter().map(|&index| uv[index]).collect::<Vec<_>>();
    let triangles = earcut_triangles(&kept_uv, &kept_holes)?
        .into_iter()
        .map(|triangle| triangle.map(|index| kept[index]))
        .collect();
    let mut triangulation = Triangulation::new(triangles, metric, boundary_edges);
    for (ring, corners) in rings.iter().zip(&corners) {
        let next = |index: usize| ring.start + (index - ring.start + 1) % ring.len();
        for (k, &corner) in corners.iter().enumerate() {
            let end = corners[(k + 1) % corners.len()];
            let (mut current, mut index) = (corner, next(corner));
            while index != end && index != corner {
                triangulation.insert_on_boundary(uv, current, end, index);
                (current, index) = (index, next(index));
            }
        }
    }
    Ok(triangulation)
}

fn uv_metric(surface: &SurfaceGeometry, outer: &[[f64; 2]]) -> Result<[f64; 2], GeometryError> {
    let count = outer.len().max(1) as f64;
    let center = [0, 1].map(|axis| outer.iter().map(|uv| uv[axis]).sum::<f64>() / count);
    let jet = surface.derivatives(center)?;
    let metric = [norm(jet.du), norm(jet.dv)];
    if metric.iter().all(|value| value.is_finite() && *value > 0.0) {
        Ok(metric)
    } else {
        Ok([1.0, 1.0])
    }
}

fn earcut_triangles(uv: &[[f64; 2]], holes: &[usize]) -> Result<Vec<[usize; 3]>, GeometryError> {
    let flattened = uv
        .iter()
        .flat_map(|coordinate| *coordinate)
        .collect::<Vec<_>>();
    let indices = earcutr::earcut(&flattened, holes, 2);
    if indices.is_empty() {
        return Err(GeometryError::InvalidTopology(
            "curved trim triangulation failed".into(),
        ));
    }
    Ok(indices
        .chunks_exact(3)
        .map(|triangle| [triangle[0], triangle[1], triangle[2]])
        .collect::<Vec<_>>())
}

fn check_trim_refinement(
    face: &Face,
    surface: &SurfaceGeometry,
    triangles: &[[usize; 3]],
    boundary_edges: &BTreeSet<(usize, usize)>,
    uv: &[[f64; 2]],
    error: f64,
) -> Result<(), GeometryError> {
    for triangle in triangles {
        for [a, b] in [
            [triangle[0], triangle[1]],
            [triangle[1], triangle[2]],
            [triangle[2], triangle[0]],
        ] {
            let key = if a < b { (a, b) } else { (b, a) };
            let needs_refinement = trimmed_surface_edge_deviation(surface, uv, a, b) > error * 2.0;
            if !boundary_edges.contains(&key) && needs_refinement {
                return Err(GeometryError::LimitExceeded(format!(
                    "curved trim refinement depth on face {}: bound {} for edge {:?} to {:?}",
                    face.id,
                    trimmed_surface_edge_deviation(surface, uv, a, b),
                    uv[a],
                    uv[b]
                )));
            }
        }
    }
    Ok(())
}

fn trim_normal(
    brep: &BrepEnvelope,
    face: &Face,
    surface: &SurfaceGeometry,
    coordinate: [f64; 2],
) -> Result<Point3, GeometryError> {
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
        coordinate
    };
    Ok(scale(
        surface.normal_at(normal_coordinate)?,
        face.sense.multiplier(),
    ))
}

fn check_grid_budget(us: &[f64], vs: &[f64], max_triangles: usize) -> Result<(), GeometryError> {
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
    Ok(())
}

fn add_grid_vertices(
    grid: &TrimGrid<'_>,
    all_boundary: &[&([f64; 2], Point3)],
    coordinate_tolerance: f64,
    v: Interval,
    mesh: &mut Tessellation,
) -> Result<(), GeometryError> {
    let TrimGrid {
        us,
        vs,
        surface,
        face,
    } = *grid;
    for &chart_v in vs {
        for &chart_u in us {
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
    Ok(())
}

fn add_grid_triangles(
    grid: &TrimGrid<'_>,
    loops: &[Vec<([f64; 2], Point3)>],
    start: usize,
    mesh: &mut Tessellation,
    max_triangles: usize,
) -> Result<(), GeometryError> {
    let TrimGrid {
        us,
        vs,
        surface,
        face,
    } = *grid;
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
