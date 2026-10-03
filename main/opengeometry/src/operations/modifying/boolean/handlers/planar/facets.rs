use crate::brep::{
    coverage_gap, padded_uv_bounds, plane_boundary, unit, Accuracy, BrepEnvelope, Builder,
    CurveGeometry, EdgeGeometry, FaceProvenance, Frame3, GeometryError, Orientation, Shell,
    SolidRegion, SurfaceGeometry, Use,
};
use crate::math::{cross, dot, norm, sub, Interval, Point3};
use crate::operations::modifying::boolean::assembly::{
    add_welded_vertex, analytic_face_mappings, assign_shell_faces, enclosing_solid,
};
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanReport, BooleanResult};
use crate::query::face_contains_uv;
use std::collections::BTreeMap;

pub(super) struct PlanarFacets {
    builder: Builder,
    vertices: Vec<(Point3, u32)>,
    edges: BTreeMap<(u32, u32), u32>,
    pub(super) accuracy: Accuracy,
}

pub(super) fn loop_positions(
    brep: &BrepEnvelope,
    loop_id: u32,
) -> Result<Vec<Point3>, GeometryError> {
    let start = brep
        .topology
        .loops
        .get(loop_id as usize)
        .ok_or_else(coverage)?
        .start_halfedge;
    let mut current = start;
    let mut points = Vec::new();
    loop {
        let halfedge = brep
            .topology
            .halfedges
            .get(current as usize)
            .ok_or_else(coverage)?;
        points.push(
            brep.topology
                .vertices
                .get(halfedge.from as usize)
                .ok_or_else(coverage)?
                .position,
        );
        current = halfedge.next.ok_or_else(coverage)?;
        if current == start {
            break;
        }
        if points.len() > brep.topology.halfedges.len() {
            return Err(coverage());
        }
    }
    Ok(points)
}

pub(super) fn coverage() -> GeometryError {
    coverage_gap(
        "layered planar profile extrusion",
        "layered planar profile extrusion",
    )
}

impl PlanarFacets {
    pub(super) fn new(id: String, accuracy: Accuracy) -> Result<Self, GeometryError> {
        Ok(Self {
            builder: Builder::new(id, accuracy)?,
            vertices: Vec::new(),
            edges: BTreeMap::new(),
            accuracy,
        })
    }

    fn vertex(&mut self, point: Point3) -> u32 {
        add_welded_vertex(&mut self.builder, &mut self.vertices, point, self.accuracy)
    }

    fn uses(&mut self, ring: &[Point3], frame: Frame3) -> Result<Vec<Use>, GeometryError> {
        let ids = ring
            .iter()
            .map(|point| self.vertex(*point))
            .collect::<Vec<_>>();
        let mut uses = Vec::with_capacity(ids.len());
        for index in 0..ids.len() {
            let next = (index + 1) % ids.len();
            let from = ids[index];
            let to = ids[next];
            if from == to {
                return Err(GeometryError::UnresolvedIntersection(
                    "layered planar face has a sub-tolerance edge".into(),
                ));
            }
            let key = (from.min(to), from.max(to));
            let edge = if let Some(&edge) = self.edges.get(&key) {
                edge
            } else {
                let delta = sub(ring[next], ring[index]);
                let length = norm(delta);
                let edge = self.builder.edge(
                    CurveGeometry::Line {
                        origin: ring[index],
                        direction: unit(delta)?,
                    },
                    Interval::new(0.0, length)?,
                    false,
                );
                self.edges.insert(key, edge);
                edge
            };
            let EdgeGeometry::Curve { curve, .. } =
                self.builder.brep.topology.edges[edge as usize].geometry
            else {
                return Err(coverage());
            };
            let CurveGeometry::Line { origin, direction } =
                self.builder.brep.geometry.curves[curve as usize]
            else {
                return Err(coverage());
            };
            let sense = if norm(sub(origin, ring[index])) <= self.accuracy.geometric / 4.0
                && dot(direction, sub(ring[next], ring[index])) > 0.0
            {
                Orientation::Forward
            } else {
                Orientation::Reverse
            };
            uses.push(plane_boundary(&self.builder, frame, edge, from, to, sense)?);
        }
        Ok(uses)
    }

    pub(super) fn face(
        &mut self,
        key: &str,
        frame: Frame3,
        outer: Vec<Point3>,
        holes: Vec<Vec<Point3>>,
        provenance: FaceProvenance,
    ) -> Result<(), GeometryError> {
        for point in outer.iter().chain(holes.iter().flatten()) {
            let local = frame.local(*point);
            if local[2].abs() > 4.0 * self.accuracy.geometric {
                return Err(GeometryError::UnresolvedIntersection(
                    "layered planar face is not coplanar".into(),
                ));
            }
        }
        let bounds = padded_uv_bounds(
            outer
                .iter()
                .chain(holes.iter().flatten())
                .map(|point| frame.local(*point)),
            self.accuracy.geometric,
        );
        let outer_uses = self.uses(&outer, frame)?;
        let hole_uses = holes
            .iter()
            .map(|hole| self.uses(hole, frame))
            .collect::<Result<Vec<_>, _>>()?;
        let face = self.builder.brep.topology.faces.len();
        self.builder.face_with_holes(
            &format!("{key}-{face}"),
            SurfaceGeometry::Plane { frame },
            bounds,
            outer_uses,
            hole_uses,
        )?;
        self.builder.brep.topology.faces[face].provenance = provenance;
        Ok(())
    }
}

pub(super) fn finish(
    mut facets: PlanarFacets,
    a: &BrepEnvelope,
    b: &BrepEnvelope,
) -> Result<BooleanResult, GeometryError> {
    let mut assigned = vec![false; facets.builder.brep.topology.faces.len()];
    let mut outer_shells = Vec::new();
    let mut cavity_shells = Vec::new();
    for start in 0..assigned.len() {
        if assigned[start] {
            continue;
        }
        let shell = facets.builder.brep.topology.shells.len() as u32;
        let faces = assign_shell_faces(
            &mut facets.builder.brep,
            &mut assigned,
            start,
            shell,
            || GeometryError::InvalidTopology("layered planar Boolean has an open boundary".into()),
            || GeometryError::InvalidTopology("layered planar twin has no face".into()),
        )?;
        let volume = shell_volume(&facets.builder.brep, &faces)?;
        if volume.abs() <= facets.accuracy.geometric.powi(3) {
            return Err(coverage());
        }
        facets.builder.brep.topology.shells.push(Shell {
            id: shell,
            faces,
            is_closed: true,
        });
        if volume > 0.0 {
            outer_shells.push((shell, volume));
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
        let index = enclosing_solid(
            &facets.builder.brep,
            &outer_shells,
            cavity_shell,
            |brep, faces, point| {
                shell_contains_point(brep, faces, point, facets.accuracy.intersection)
            },
            coverage,
        )?;
        facets.builder.brep.solids[index]
            .cavity_shells
            .push(cavity_shell);
    }
    finish_planar_result(facets.builder.brep, a, b)
}

fn shell_volume(brep: &BrepEnvelope, faces: &[u32]) -> Result<f64, GeometryError> {
    let mut volume = 0.0;
    for &face_id in faces {
        let face = &brep.topology.faces[face_id as usize];
        for &loop_id in std::iter::once(&face.trim.outer).chain(&face.trim.holes) {
            let start = brep.topology.loops[loop_id as usize].start_halfedge;
            let mut current = start;
            let mut points = Vec::new();
            loop {
                let halfedge = &brep.topology.halfedges[current as usize];
                points.push(brep.topology.vertices[halfedge.from as usize].position);
                current = halfedge.next.ok_or_else(|| {
                    GeometryError::InvalidTopology("layered planar face has open loop".into())
                })?;
                if current == start {
                    break;
                }
                if points.len() > brep.topology.halfedges.len() {
                    return Err(coverage());
                }
            }
            for index in 1..points.len() - 1 {
                volume += dot(points[0], cross(points[index], points[index + 1])) / 6.0;
            }
        }
    }
    Ok(volume)
}

fn shell_contains_point(
    brep: &BrepEnvelope,
    faces: &[u32],
    point: Point3,
    tolerance: f64,
) -> Result<bool, GeometryError> {
    for direction in [
        unit([1.0, 0.371, 0.127])?,
        unit([0.193, 1.0, 0.419])?,
        unit([0.311, 0.233, 1.0])?,
    ] {
        let mut hits: Vec<f64> = Vec::new();
        let mut uncertain = false;
        for &face_id in faces {
            let face = &brep.topology.faces[face_id as usize];
            let SurfaceGeometry::Plane { frame } = brep.geometry.surface(face.surface)? else {
                return Err(coverage());
            };
            let denominator = dot(frame.z, direction);
            let numerator = dot(frame.z, sub(frame.origin, point));
            if denominator.abs() <= 64.0 * f64::EPSILON {
                if numerator.abs() <= tolerance {
                    uncertain = true;
                    break;
                }
                continue;
            }
            let distance = numerator / denominator;
            if distance <= tolerance {
                continue;
            }
            let position = [
                point[0] + direction[0] * distance,
                point[1] + direction[1] * distance,
                point[2] + direction[2] * distance,
            ];
            let local = frame.local(position);
            match face_contains_uv(brep, face, [local[0], local[1]])? {
                Some(true) if hits.iter().any(|hit| (hit - distance).abs() <= tolerance) => {
                    uncertain = true;
                    break;
                }
                Some(true) => hits.push(distance),
                Some(false) => {}
                None => {
                    uncertain = true;
                    break;
                }
            }
        }
        if !uncertain {
            return Ok(hits.len() % 2 == 1);
        }
    }
    Err(coverage())
}

fn finish_planar_result(
    mut out: BrepEnvelope,
    a: &BrepEnvelope,
    b: &BrepEnvelope,
) -> Result<BooleanResult, GeometryError> {
    out.revision =
        a.revision.max(b.revision).checked_add(1).ok_or_else(|| {
            GeometryError::LimitExceeded("planar Boolean revision overflow".into())
        })?;
    out.validate()?;
    let face_mappings = analytic_face_mappings(&out, [a, b]);
    Ok(BooleanResult {
        report: BooleanReport {
            operation: BooleanOp::Subtraction,
            quality: out.quality.clone(),
            contacts: Vec::new(),
            coincident: true,
            face_mappings,
        },
        brep: out,
    })
}
