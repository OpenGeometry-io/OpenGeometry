use crate::brep::{
    unit, BrepEnvelope, CurveGeometry, FaceProvenance, Frame3, GeometryError, Orientation,
    SurfaceGeometry,
};
use crate::geom2d::Pt2;
use crate::math::{cross, dot, norm, scale, sub, Point3};
use crate::operations::modifying::boolean::handlers::planar::facets::{coverage, loop_positions};
use crate::query::face_contains_uv;

pub(super) struct PlanarFace {
    pub(super) id: u32,
    pub(super) frame: Frame3,
    pub(super) polygon: Vec<Point3>,
    pub(super) holes: Vec<Vec<Point3>>,
}

pub(super) struct PlanarFacet {
    pub(super) key: String,
    pub(super) frame: Frame3,
    pub(super) outer: Vec<Point3>,
    pub(super) holes: Vec<Vec<Point3>>,
    pub(super) provenance: FaceProvenance,
}

pub(super) fn convex_faces(
    brep: &BrepEnvelope,
    tolerance: f64,
) -> Result<Option<Vec<PlanarFace>>, GeometryError> {
    let Some(faces) = planar_faces(brep, tolerance)? else {
        return Ok(None);
    };
    if brep.solids.len() != 1
        || brep.topology.shells.len() != 1
        || faces.iter().any(|face| !face.holes.is_empty())
    {
        return Ok(None);
    }
    for face in &faces {
        if brep
            .topology
            .vertices
            .iter()
            .any(|vertex| dot(face.frame.z, sub(vertex.position, face.frame.origin)) > tolerance)
        {
            return Ok(None);
        }
    }
    Ok(Some(faces))
}

pub(super) fn planar_faces(
    brep: &BrepEnvelope,
    tolerance: f64,
) -> Result<Option<Vec<PlanarFace>>, GeometryError> {
    if brep.solids.is_empty()
        || brep
            .geometry
            .curves
            .iter()
            .any(|curve| !matches!(curve, CurveGeometry::Line { .. }))
    {
        return Ok(None);
    }
    let mut faces = Vec::with_capacity(brep.topology.faces.len());
    for face in &brep.topology.faces {
        let Some(SurfaceGeometry::Plane { frame: surface }) =
            brep.geometry.surfaces.get(face.surface as usize)
        else {
            return Ok(None);
        };
        let frame = if face.sense == Orientation::Forward {
            *surface
        } else {
            Frame3 {
                origin: surface.origin,
                x: surface.x,
                y: scale(surface.y, -1.0),
                z: scale(surface.z, -1.0),
            }
        };
        let polygon = loop_positions(brep, face.trim.outer)?;
        if polygon.len() < 3 {
            return Ok(None);
        }
        let mut holes = face
            .trim
            .holes
            .iter()
            .map(|&loop_id| loop_positions(brep, loop_id))
            .collect::<Result<Vec<_>, _>>()?;
        let orient = |mut ring: Vec<Point3>, outer: bool| -> Option<Vec<Point3>> {
            let area = ring
                .iter()
                .enumerate()
                .map(|(index, point)| {
                    let a = frame.local(*point);
                    let b = frame.local(ring[(index + 1) % ring.len()]);
                    a[0] * b[1] - b[0] * a[1]
                })
                .sum::<f64>();
            if ring.len() < 3 || area.abs() <= tolerance * tolerance {
                return None;
            }
            if (area > 0.0) != outer {
                ring.reverse();
            }
            Some(ring)
        };
        let Some(polygon) = orient(polygon, true) else {
            return Ok(None);
        };
        for hole in &mut holes {
            let Some(oriented) = orient(std::mem::take(hole), false) else {
                return Ok(None);
            };
            *hole = oriented;
        }
        faces.push(PlanarFace {
            id: face.id,
            frame,
            polygon,
            holes,
        });
    }
    Ok(Some(faces))
}

pub(super) fn planar_section_segments(
    brep: &BrepEnvelope,
    faces: &[PlanarFace],
    section: Frame3,
    tolerance: f64,
) -> Result<Vec<(Pt2, Pt2)>, GeometryError> {
    let mut segments = Vec::<(Pt2, Pt2)>::new();
    for source in faces {
        let direction = cross(source.frame.z, section.z);
        if norm(direction) <= 1.0e-10 {
            continue;
        }
        let direction = unit(direction)?;
        let crossings = section_crossings(source, section, direction, tolerance);
        let face = &brep.topology.faces[source.id as usize];
        let SurfaceGeometry::Plane { frame: surface } =
            &brep.geometry.surfaces[face.surface as usize]
        else {
            return Err(coverage());
        };
        for pair in crossings.windows(2) {
            if norm(sub(pair[0], pair[1])) <= tolerance {
                continue;
            }
            let midpoint = scale(
                [
                    pair[0][0] + pair[1][0],
                    pair[0][1] + pair[1][1],
                    pair[0][2] + pair[1][2],
                ],
                0.5,
            );
            let local = surface.local(midpoint);
            let on_trim_edge = on_source_trim_edge(source, midpoint, tolerance);
            if face_contains_uv(brep, face, [local[0], local[1]])? != Some(true) && !on_trim_edge {
                continue;
            }
            let a = section.local(pair[0]);
            let b = section.local(pair[1]);
            let mut from = Pt2::new(a[0], a[1]);
            let mut to = Pt2::new(b[0], b[1]);
            let edge = sub(pair[1], pair[0]);
            if dot(cross(edge, section.z), source.frame.z) < 0.0 {
                std::mem::swap(&mut from, &mut to);
            }
            segments.push((from, to));
        }
    }
    Ok(segments)
}

fn section_crossings(
    source: &PlanarFace,
    section: Frame3,
    direction: Point3,
    tolerance: f64,
) -> Vec<Point3> {
    let mut crossings = Vec::new();
    for ring in std::iter::once(&source.polygon).chain(&source.holes) {
        for index in 0..ring.len() {
            let from = ring[index];
            let to = ring[(index + 1) % ring.len()];
            let from_distance = dot(section.z, sub(from, section.origin));
            let to_distance = dot(section.z, sub(to, section.origin));
            if from_distance.abs() <= tolerance {
                crossings.push(from);
            }
            if (from_distance < -tolerance && to_distance > tolerance)
                || (from_distance > tolerance && to_distance < -tolerance)
            {
                let fraction = from_distance / (from_distance - to_distance);
                crossings.push([
                    from[0] + fraction * (to[0] - from[0]),
                    from[1] + fraction * (to[1] - from[1]),
                    from[2] + fraction * (to[2] - from[2]),
                ]);
            }
        }
    }
    crossings.sort_by(|left, right| dot(*left, direction).total_cmp(&dot(*right, direction)));
    crossings.dedup_by(|left, right| norm(sub(*left, *right)) <= tolerance);
    crossings
}

fn on_source_trim_edge(source: &PlanarFace, midpoint: Point3, tolerance: f64) -> bool {
    std::iter::once(&source.polygon)
        .chain(&source.holes)
        .any(|ring| {
            ring.iter().enumerate().any(|(index, &from)| {
                let to = ring[(index + 1) % ring.len()];
                let edge = sub(to, from);
                let length_squared = dot(edge, edge);
                if length_squared <= tolerance * tolerance {
                    return false;
                }
                let fraction = dot(sub(midpoint, from), edge) / length_squared;
                fraction >= -tolerance
                    && fraction <= 1.0 + tolerance
                    && norm(sub(sub(midpoint, from), scale(edge, fraction))) <= tolerance
            })
        })
}

pub(super) fn convex_section_polygon(
    cutter: &BrepEnvelope,
    faces: &[PlanarFace],
    section: Frame3,
    tolerance: f64,
) -> Vec<Pt2> {
    let mut limits = [[f64::INFINITY, f64::NEG_INFINITY]; 2];
    for vertex in &cutter.topology.vertices {
        let local = section.local(vertex.position);
        for axis in 0..2 {
            limits[axis][0] = limits[axis][0].min(local[axis]);
            limits[axis][1] = limits[axis][1].max(local[axis]);
        }
    }
    if limits.iter().any(|bound| !bound[0].is_finite()) {
        return Vec::new();
    }
    let span = (limits[0][1] - limits[0][0])
        .max(limits[1][1] - limits[1][0])
        .max(1.0);
    let corners = [
        [limits[0][0] - span, limits[1][0] - span],
        [limits[0][1] + span, limits[1][0] - span],
        [limits[0][1] + span, limits[1][1] + span],
        [limits[0][0] - span, limits[1][1] + span],
    ]
    .map(|point| section.point([point[0], point[1], 0.0]));
    let clipped = clip_convex_face(&corners, faces, tolerance);
    if clipped.len() < 3 {
        return Vec::new();
    }
    let result = clipped
        .iter()
        .map(|point| {
            let local = section.local(*point);
            Pt2::new(local[0], local[1])
        })
        .collect::<Vec<_>>();
    let twice_area = result
        .iter()
        .enumerate()
        .map(|(index, point)| {
            let next = result[(index + 1) % result.len()];
            point.x * next.z - next.x * point.z
        })
        .sum::<f64>();
    if twice_area.abs() <= tolerance * tolerance {
        Vec::new()
    } else {
        result
    }
}

fn clip_convex_face(polygon: &[Point3], planes: &[PlanarFace], tolerance: f64) -> Vec<Point3> {
    let mut result = polygon.to_vec();
    for plane in planes {
        if result.len() < 3 {
            return Vec::new();
        }
        let mut next = Vec::new();
        let Some(&mut_previous) = result.last() else {
            return Vec::new();
        };
        let mut previous = mut_previous;
        let mut previous_distance = dot(plane.frame.z, sub(previous, plane.frame.origin));
        for &current in &result {
            let distance = dot(plane.frame.z, sub(current, plane.frame.origin));
            let previous_inside = previous_distance <= tolerance;
            let current_inside = distance <= tolerance;
            if previous_inside != current_inside {
                let fraction = previous_distance / (previous_distance - distance);
                next.push([
                    previous[0] + fraction * (current[0] - previous[0]),
                    previous[1] + fraction * (current[1] - previous[1]),
                    previous[2] + fraction * (current[2] - previous[2]),
                ]);
            }
            if current_inside {
                next.push(current);
            }
            previous = current;
            previous_distance = distance;
        }
        next.dedup_by(|left, right| norm(sub(*left, *right)) <= tolerance / 4.0);
        if next.len() > 1 && norm(sub(next[0], next[next.len() - 1])) <= tolerance / 4.0 {
            next.pop();
        }
        result = next;
    }
    if result.len() < 3 {
        Vec::new()
    } else {
        result
    }
}

pub(super) fn split_planar_ring(
    ring: &[Point3],
    vertices: &[Point3],
    tolerance: f64,
) -> Vec<Point3> {
    let mut split = Vec::new();
    for index in 0..ring.len() {
        let from = ring[index];
        let to = ring[(index + 1) % ring.len()];
        let delta = sub(to, from);
        let length = norm(delta);
        if length <= tolerance {
            continue;
        }
        split.push(from);
        let mut interior = Vec::new();
        for &point in vertices {
            let offset = sub(point, from);
            let fraction = dot(offset, delta) / (length * length);
            if fraction > tolerance / length
                && fraction < 1.0 - tolerance / length
                && norm(sub(offset, scale(delta, fraction))) <= tolerance
            {
                interior.push((fraction, point));
            }
        }
        interior.sort_by(|left, right| left.0.total_cmp(&right.0));
        for (_, point) in interior {
            if split
                .last()
                .is_none_or(|last| norm(sub(*last, point)) > tolerance)
            {
                split.push(point);
            }
        }
    }
    split
}
