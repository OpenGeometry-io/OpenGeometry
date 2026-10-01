use crate::vector::{add, dot, norm, scale, sub, unit};
use opengeometry::brep::{BrepEnvelope, Similarity3, SurfaceGeometry};
use opengeometry::math::Point3;
use opengeometry::world_graph::WorldGraph;

struct Axis {
    points: Vec<Point3>,
    radius: f64,
}

struct Ring {
    centre: Point3,
    normal: Point3,
    direction: Point3,
}

struct WorldPlane {
    origin: Point3,
    normal: Point3,
}

pub fn rail_minus_wall_clearance(graph: &WorldGraph, rail: &str, wall: &str) -> f64 {
    let axis = rail_axis(graph, rail);
    let brep = graph.brep(wall).unwrap();
    let placement = graph.world_placement(wall).unwrap();
    let planes = wall_planes(&brep, placement);
    let ring_gap = rings(&axis)
        .iter()
        .flat_map(|ring| {
            planes
                .iter()
                .map(|plane| ring_plane_gap(ring, plane, axis.radius))
        })
        .fold(f64::INFINITY, f64::min);
    let edge_gap = wall_segments(&brep, placement)
        .iter()
        .map(|segment| edge_gap(*segment, &axis))
        .fold(f64::INFINITY, f64::min);
    ring_gap.min(edge_gap)
}

fn rail_axis(graph: &WorldGraph, rail: &str) -> Axis {
    let brep = graph.brep(rail).unwrap();
    let placement = graph.world_placement(rail).unwrap();
    let mut cylinders = Vec::new();
    for face in &brep.topology.faces {
        if let SurfaceGeometry::Cylinder { frame, radius } =
            brep.geometry.surfaces[face.surface as usize]
        {
            if cylinders.last().map(|(origin, _, _)| *origin) != Some(frame.origin) {
                cylinders.push((frame.origin, frame.z, radius));
            }
        }
    }
    let (_, last_direction, radius) = *cylinders.last().unwrap();
    let end = brep
        .geometry
        .surfaces
        .iter()
        .find_map(|surface| match surface {
            SurfaceGeometry::Plane { frame } if dot(frame.z, last_direction) > 1.0 - 1e-12 => {
                Some(frame.origin)
            }
            _ => None,
        })
        .unwrap();
    let mut points: Vec<Point3> = cylinders.iter().map(|(origin, _, _)| *origin).collect();
    points.push(end);
    Axis {
        points: points
            .into_iter()
            .map(|point| placement.apply_point(point))
            .collect(),
        radius,
    }
}

fn rings(axis: &Axis) -> Vec<Ring> {
    let points = &axis.points;
    let directions: Vec<Point3> = points
        .windows(2)
        .map(|pair| unit(sub(pair[1], pair[0])))
        .collect();
    let last = directions.len();
    let mut rings = vec![Ring {
        centre: points[0],
        normal: directions[0],
        direction: directions[0],
    }];
    for joint in 1..last {
        rings.push(Ring {
            centre: points[joint],
            normal: unit(add(directions[joint - 1], directions[joint])),
            direction: directions[joint - 1],
        });
    }
    rings.push(Ring {
        centre: points[last],
        normal: directions[last - 1],
        direction: directions[last - 1],
    });
    rings
}

fn wall_planes(brep: &BrepEnvelope, placement: Similarity3) -> Vec<WorldPlane> {
    brep.topology
        .faces
        .iter()
        .map(|face| match brep.geometry.surfaces[face.surface as usize] {
            SurfaceGeometry::Plane { frame } => {
                let origin = placement.apply_point(frame.origin);
                let tip = placement.apply_point(add(frame.origin, frame.z));
                WorldPlane {
                    origin,
                    normal: unit(sub(tip, origin)),
                }
            }
            _ => panic!("the wall has a face that is not planar"),
        })
        .collect()
}

fn wall_segments(brep: &BrepEnvelope, placement: Similarity3) -> Vec<[Point3; 2]> {
    let position =
        |vertex: u32| placement.apply_point(brep.topology.vertices[vertex as usize].position);
    brep.topology
        .halfedges
        .iter()
        .map(|halfedge| [position(halfedge.from), position(halfedge.to)])
        .collect()
}

fn ring_plane_gap(ring: &Ring, plane: &WorldPlane, radius: f64) -> f64 {
    let n = plane.normal;
    let d = ring.direction;
    let b = ring.normal;
    let offset = dot(n, sub(ring.centre, plane.origin));
    let g = sub(n, scale(b, dot(n, d) / dot(b, d)));
    let reach = norm(sub(g, scale(d, dot(g, d))));
    (offset.abs() - radius * reach).max(0.0)
}

fn edge_gap(segment: [Point3; 2], axis: &Axis) -> f64 {
    let distance = axis
        .points
        .windows(2)
        .map(|pair| segment_distance(segment, [pair[0], pair[1]]))
        .fold(f64::INFINITY, f64::min);
    (distance - axis.radius).max(0.0)
}

fn segment_distance(p: [Point3; 2], q: [Point3; 2]) -> f64 {
    let u = sub(p[1], p[0]);
    let v = sub(q[1], q[0]);
    let w = sub(p[0], q[0]);
    let (a, b, c, e, f) = (dot(u, u), dot(u, v), dot(u, w), dot(v, v), dot(v, w));
    let denominator = a * e - b * b;
    let mut s = if denominator > 1e-12 * a * e {
        ((b * f - c * e) / denominator).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mut t = (b * s + f) / e;
    if t < 0.0 {
        t = 0.0;
        s = (-c / a).clamp(0.0, 1.0);
    } else if t > 1.0 {
        t = 1.0;
        s = ((b - c) / a).clamp(0.0, 1.0);
    }
    norm(sub(add(p[0], scale(u, s)), add(q[0], scale(v, t))))
}
