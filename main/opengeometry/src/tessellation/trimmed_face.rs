use super::grid::Rect;
use super::mesh::Tessellation;
use super::trim::{
    merged_grid_coordinates, periodic_band_loop, trim_loop_samples, trimmed_surface_edge_deviation,
};
use crate::brep::{BrepEnvelope, Face, GeometryError, Surface, SurfaceGeometry};
use crate::geom2d::{point_in_ring2, Pt2};
use crate::math::{scale, Interval, Point3};
use std::collections::{BTreeSet, HashMap};

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
    if rect.is_some() && !face.trim.holes.is_empty() {
        shift_periodic_holes(face, surface, periods, &mut loops)?;
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
    add_grid_vertices(&grid, &all_boundary, coordinate_tolerance, v, mesh)?;
    add_grid_triangles(&grid, &loops, start, mesh, max_triangles)?;
    Ok(())
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
    let mut holes = Vec::new();
    let mut boundary_edges = BTreeSet::new();
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
    let boundary_count = uv.len();
    let mut triangles = earcut_triangles(&uv, &holes)?;
    let surface = brep.geometry.surface(face.surface)?;
    for _ in 0..128 {
        let split_edges = edges_to_split(&triangles, &boundary_edges, surface, &uv, error);
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
        let midpoints =
            add_edge_midpoints(&split_edges, surface, &mut uv, &mut points, boundary_count)?;
        triangles = refined_triangles(triangles, &midpoints);
    }
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

fn edges_to_split(
    triangles: &[[usize; 3]],
    boundary_edges: &BTreeSet<(usize, usize)>,
    surface: &SurfaceGeometry,
    uv: &[[f64; 2]],
    error: f64,
) -> BTreeSet<(usize, usize)> {
    let mut split_edges = BTreeSet::new();
    for triangle in triangles {
        for [a, b] in [
            [triangle[0], triangle[1]],
            [triangle[1], triangle[2]],
            [triangle[2], triangle[0]],
        ] {
            let key = if a < b { (a, b) } else { (b, a) };
            if boundary_edges.contains(&key) {
                continue;
            }
            let needs_refinement = trimmed_surface_edge_deviation(surface, uv, a, b) > error * 2.0;
            if needs_refinement {
                split_edges.insert(key);
            }
        }
    }
    split_edges
}

fn add_edge_midpoints(
    split_edges: &BTreeSet<(usize, usize)>,
    surface: &SurfaceGeometry,
    uv: &mut Vec<[f64; 2]>,
    points: &mut Vec<Point3>,
    boundary_count: usize,
) -> Result<HashMap<(usize, usize), usize>, GeometryError> {
    let mut midpoints = HashMap::with_capacity(split_edges.len());
    for &(a, b) in split_edges {
        let coordinate = [(uv[a][0] + uv[b][0]) * 0.5, (uv[a][1] + uv[b][1]) * 0.5];
        let id = uv.len();
        uv.push(coordinate);
        let match_boundary = (0..boundary_count).find(|&index| {
            (0..2).all(|axis| {
                (uv[index][axis] - coordinate[axis]).abs()
                    <= 4.0 * f64::EPSILON * coordinate[axis].abs().max(1.0)
            })
        });
        points.push(if let Some(index) = match_boundary {
            points[index]
        } else {
            surface.point_at(coordinate)?
        });
        midpoints.insert((a, b), id);
    }
    Ok(midpoints)
}

fn refined_triangles(
    triangles: Vec<[usize; 3]>,
    midpoints: &HashMap<(usize, usize), usize>,
) -> Vec<[usize; 3]> {
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
            (Some(ab), Some(bc), None) => refined.extend([[b, bc, ab], [ab, bc, c], [ab, c, a]]),
            (None, Some(bc), Some(ca)) => refined.extend([[c, ca, bc], [bc, ca, a], [bc, a, b]]),
            (Some(ab), None, Some(ca)) => refined.extend([[a, ab, ca], [ca, ab, b], [ca, b, c]]),
            (Some(ab), Some(bc), Some(ca)) => {
                refined.extend([[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]])
            }
        }
    }
    refined
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
