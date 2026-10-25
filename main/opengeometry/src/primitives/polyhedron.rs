use super::profile::validate_profile_loop;
use crate::brep::{
    padded_uv_bounds, plane_boundary, unit, Accuracy, BrepEnvelope, Builder, CurveGeometry, Frame3,
    GeometryError, Orientation, SurfaceGeometry,
};
use crate::math::{add, cross, dot, norm, scale, sub, Interval, Point3};
use std::collections::BTreeMap;

struct FacePlane {
    frame: Frame3,
    area: Point3,
    centroid: Point3,
    uv: Vec<[f64; 2]>,
}

struct EdgeRecord {
    from: u32,
    faces: Vec<usize>,
}

type EdgeMap = BTreeMap<(u32, u32), EdgeRecord>;

pub fn polyhedron(
    id: String,
    points: &[Point3],
    faces: &[Vec<u32>],
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    accuracy.validate()?;
    let g = accuracy.geometric;
    check_points(points, faces)?;
    check_coincident(points, g)?;
    let planes = faces
        .iter()
        .enumerate()
        .map(|(index, ring)| face_plane(points, ring, index, g))
        .collect::<Result<Vec<_>, _>>()?;
    let edges = edge_map(faces)?;
    check_vertex_fans(points.len(), faces)?;
    check_one_shell(faces, &edges)?;
    check_outward(points, &planes, g)?;
    let mut builder = Builder::new(id, accuracy)?;
    for point in points {
        builder.vertex(*point);
    }
    let uses = add_edges(&mut builder, points, faces)?;
    add_faces(&mut builder, faces, &planes, &uses, g)?;
    builder.finish_solid()
}

fn check_points(points: &[Point3], faces: &[Vec<u32>]) -> Result<(), GeometryError> {
    if points.iter().flatten().any(|value| !value.is_finite()) {
        return Err(invalid("polyhedron points must be finite".into()));
    }
    if points.len() < 4 || faces.len() < 4 {
        return Err(invalid(
            "polyhedron needs at least four points and four faces".into(),
        ));
    }
    let mut used = vec![false; points.len()];
    for (index, ring) in faces.iter().enumerate() {
        if ring.len() < 3 {
            return Err(invalid(format!("face {index} has fewer than three points")));
        }
        for (position, &point) in ring.iter().enumerate() {
            let Some(slot) = used.get_mut(point as usize) else {
                return Err(invalid(format!(
                    "face {index} uses point {point}, which does not exist"
                )));
            };
            if ring[..position].contains(&point) {
                return Err(invalid(format!("face {index} uses point {point} twice")));
            }
            *slot = true;
        }
    }
    if let Some(unused) = used.iter().position(|used| !used) {
        return Err(invalid(format!("point {unused} is not used by any face")));
    }
    Ok(())
}

fn check_coincident(points: &[Point3], g: f64) -> Result<(), GeometryError> {
    let resolution = 4.0 * g;
    let mut order = (0..points.len()).collect::<Vec<_>>();
    order.sort_by(|a, b| points[*a][0].total_cmp(&points[*b][0]));
    for (rank, &a) in order.iter().enumerate() {
        for &b in order[rank + 1..]
            .iter()
            .take_while(|&&b| points[b][0] - points[a][0] <= resolution)
        {
            if norm(sub(points[a], points[b])) <= resolution {
                return Err(invalid(format!(
                    "points {} and {} coincide",
                    a.min(b),
                    a.max(b)
                )));
            }
        }
    }
    Ok(())
}

fn mean(points: &[Point3]) -> Point3 {
    let first = points[0];
    let offsets = points
        .iter()
        .fold([0.0; 3], |sum, point| add(sum, sub(*point, first)));
    add(first, scale(offsets, 1.0 / points.len() as f64))
}

fn face_plane(
    points: &[Point3],
    ring: &[u32],
    index: usize,
    g: f64,
) -> Result<FacePlane, GeometryError> {
    let corners = ring
        .iter()
        .map(|&point| points[point as usize])
        .collect::<Vec<_>>();
    let centroid = mean(&corners);
    let area = corners
        .iter()
        .zip(corners.iter().cycle().skip(1))
        .fold([0.0; 3], |sum, (a, b)| {
            add(sum, cross(sub(*a, centroid), sub(*b, centroid)))
        });
    if norm(area) <= 32.0 * g * g {
        return Err(GeometryError::UnresolvedIntersection(format!(
            "face {index} area is below geometric resolution"
        )));
    }
    let frame = Frame3::from_axis(centroid, area, sub(corners[1], corners[0]))?;
    let local = corners
        .iter()
        .map(|corner| frame.local(*corner))
        .collect::<Vec<_>>();
    if let Some(position) = local.iter().position(|point| point[2].abs() > g) {
        return Err(invalid(format!(
            "point {} is off the plane of face {index}",
            ring[position]
        )));
    }
    let uv = local
        .iter()
        .map(|point| [point[0], point[1]])
        .collect::<Vec<_>>();
    validate_profile_loop(&uv, g, &format!("face {index}"))?;
    Ok(FacePlane {
        frame,
        area,
        centroid,
        uv,
    })
}

fn edge_map(faces: &[Vec<u32>]) -> Result<EdgeMap, GeometryError> {
    let mut edges = EdgeMap::new();
    for (index, ring) in faces.iter().enumerate() {
        for (position, &from) in ring.iter().enumerate() {
            let to = ring[(position + 1) % ring.len()];
            let record = edges
                .entry((from.min(to), from.max(to)))
                .or_insert(EdgeRecord {
                    from,
                    faces: Vec::new(),
                });
            if record.faces.len() == 2 {
                return Err(topology(format!(
                    "edge {from}-{to} is used by more than two faces"
                )));
            }
            if let Some(other) = record.faces.first().filter(|_| record.from == from) {
                return Err(topology(format!(
                    "faces {other} and {index} both run {from}-{to}"
                )));
            }
            record.faces.push(index);
        }
    }
    if let Some(((lo, hi), _)) = edges.iter().find(|(_, record)| record.faces.len() == 1) {
        return Err(topology(format!(
            "edge {lo}-{hi} has one face: the shell is open"
        )));
    }
    Ok(edges)
}

fn check_vertex_fans(count: usize, faces: &[Vec<u32>]) -> Result<(), GeometryError> {
    let mut corners = vec![Vec::new(); count];
    for ring in faces {
        for (position, &point) in ring.iter().enumerate() {
            let previous = ring[(position + ring.len() - 1) % ring.len()];
            let next = ring[(position + 1) % ring.len()];
            corners[point as usize].push((previous, next));
        }
    }
    for (vertex, fan) in corners.iter().enumerate() {
        let links = fan.iter().copied().collect::<BTreeMap<_, _>>();
        let Some(&(start, _)) = fan.first() else {
            continue;
        };
        let mut incoming = start;
        let mut visited = 0;
        while let Some(&outgoing) = links.get(&incoming) {
            visited += 1;
            incoming = outgoing;
            if incoming == start || visited > fan.len() {
                break;
            }
        }
        if visited != fan.len() || links.len() != fan.len() {
            return Err(topology(format!(
                "vertex {vertex} is pinched: its faces do not form one fan"
            )));
        }
    }
    Ok(())
}

fn check_one_shell(faces: &[Vec<u32>], edges: &EdgeMap) -> Result<(), GeometryError> {
    let mut neighbours = vec![Vec::new(); faces.len()];
    for record in edges.values() {
        if let [a, b] = record.faces[..] {
            neighbours[a].push(b);
            neighbours[b].push(a);
        }
    }
    let mut reached = vec![false; faces.len()];
    let mut stack = vec![0];
    reached[0] = true;
    while let Some(face) = stack.pop() {
        for &next in &neighbours[face] {
            if !reached[next] {
                reached[next] = true;
                stack.push(next);
            }
        }
    }
    if reached.contains(&false) {
        return Err(topology("the faces form more than one shell".into()));
    }
    Ok(())
}

fn check_outward(points: &[Point3], planes: &[FacePlane], g: f64) -> Result<(), GeometryError> {
    let origin = mean(points);
    let volume = planes
        .iter()
        .map(|plane| dot(plane.area, sub(plane.centroid, origin)))
        .sum::<f64>();
    let area = planes.iter().map(|plane| norm(plane.area)).sum::<f64>();
    if volume <= 3.0 * g * area {
        return Err(topology(
            "faces wind inward or enclose no volume; reverse every ring of an inward shell".into(),
        ));
    }
    Ok(())
}

fn add_edges(
    builder: &mut Builder,
    points: &[Point3],
    faces: &[Vec<u32>],
) -> Result<Vec<Vec<(u32, Orientation)>>, GeometryError> {
    let mut created = BTreeMap::new();
    let mut uses = Vec::with_capacity(faces.len());
    for ring in faces {
        let mut ring_uses = Vec::with_capacity(ring.len());
        for (position, &from) in ring.iter().enumerate() {
            let to = ring[(position + 1) % ring.len()];
            let key = (from.min(to), from.max(to));
            if let Some(&(edge, first)) = created.get(&key) {
                let sense = if first == from {
                    Orientation::Forward
                } else {
                    Orientation::Reverse
                };
                ring_uses.push((edge, sense));
                continue;
            }
            let delta = sub(points[to as usize], points[from as usize]);
            let edge = builder.edge(
                CurveGeometry::Line {
                    origin: points[from as usize],
                    direction: unit(delta)?,
                },
                Interval::new(0.0, norm(delta))?,
                false,
            );
            created.insert(key, (edge, from));
            ring_uses.push((edge, Orientation::Forward));
        }
        uses.push(ring_uses);
    }
    Ok(uses)
}

fn add_faces(
    builder: &mut Builder,
    faces: &[Vec<u32>],
    planes: &[FacePlane],
    uses: &[Vec<(u32, Orientation)>],
    g: f64,
) -> Result<(), GeometryError> {
    for (index, ((ring, plane), ring_uses)) in faces.iter().zip(planes).zip(uses).enumerate() {
        let boundary = ring
            .iter()
            .zip(ring_uses)
            .enumerate()
            .map(|(position, (&from, &(edge, sense)))| {
                let to = ring[(position + 1) % ring.len()];
                plane_boundary(builder, plane.frame, edge, from, to, sense)
            })
            .collect::<Result<Vec<_>, _>>()?;
        builder.face(
            &format!("face-{index}"),
            SurfaceGeometry::Plane { frame: plane.frame },
            padded_uv_bounds(&plane.uv, g),
            boundary,
        )?;
    }
    Ok(())
}

fn invalid(message: String) -> GeometryError {
    GeometryError::InvalidGeometry(message)
}

fn topology(message: String) -> GeometryError {
    GeometryError::InvalidTopology(message)
}
