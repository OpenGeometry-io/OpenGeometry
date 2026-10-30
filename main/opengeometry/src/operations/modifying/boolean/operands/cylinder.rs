use super::compare::{cylinder_geometry_matches, normalize_cylinder_topology};
use crate::brep::{
    coverage_gap, BrepEnvelope, Frame3, GeometryError, GeometryQuality, SurfaceGeometry,
};
use crate::math::{norm, sub};
use crate::primitives;

pub(crate) struct CylinderInput<'a> {
    pub(crate) brep: &'a BrepEnvelope,
    pub(crate) frame: Frame3,
    pub(crate) radius: f64,
    pub(crate) height: f64,
}

fn cylinder_gap() -> GeometryError {
    coverage_gap("canonical full cylinder", "canonical full cylinder")
}

pub(crate) fn full_cylinder(brep: &BrepEnvelope) -> Result<CylinderInput<'_>, GeometryError> {
    brep.validate()?;
    let (frame, radius) = match brep.geometry.surfaces.as_slice() {
        [SurfaceGeometry::Cylinder { frame, radius }, SurfaceGeometry::Plane { .. }, SurfaceGeometry::Plane { .. }] => {
            (*frame, *radius)
        }
        _ => return Err(cylinder_gap()),
    };
    if !matches!(brep.quality, GeometryQuality::Analytic)
        || !brep.geometry.intersections.is_empty()
        || brep.topology.faces.len() != 3
        || brep.topology.vertices.len() != 2
        || brep.topology.edges.len() != 3
        || brep.topology.halfedges.len() != 6
        || brep.topology.loops.len() != 3
        || !brep.topology.wires.is_empty()
        || brep.topology.shells.len() != 1
        || brep.solids.len() != 1
    {
        return Err(cylinder_gap());
    }
    let axial_range = brep.topology.faces[0].trim.uv_bounds[1];
    if axial_range.lo != 0.0 {
        return Err(cylinder_gap());
    }
    let height = axial_range.hi;
    let canonical = primitives::cylinder(brep.id.clone(), frame, radius, height, brep.accuracy)?;
    let mut topology = brep.topology.clone();
    for (face, expected) in topology.faces.iter_mut().zip(&canonical.topology.faces) {
        face.key = expected.key.clone();
        face.provenance = expected.provenance.clone();
    }
    for vertex in &mut topology.vertices {
        vertex.tolerance = brep.accuracy.geometric;
        let expected = canonical.topology.vertices[vertex.id as usize].position;
        if norm(sub(vertex.position, expected)) > brep.accuracy.geometric {
            return Err(cylinder_gap());
        }
        vertex.position = expected;
    }
    for edge in &mut topology.edges {
        edge.tolerance = brep.accuracy.geometric;
    }
    if !normalize_cylinder_topology(&mut topology, &canonical.topology, brep.accuracy.geometric) {
        return Err(cylinder_gap());
    }
    if !cylinder_geometry_matches(&brep.geometry, &canonical.geometry, brep.accuracy.geometric)
        || topology != canonical.topology
        || brep.solids != canonical.solids
    {
        return Err(cylinder_gap());
    }
    Ok(CylinderInput {
        brep,
        frame,
        radius,
        height,
    })
}
