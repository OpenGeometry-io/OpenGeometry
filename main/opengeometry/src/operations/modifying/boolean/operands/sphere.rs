use crate::brep::{
    coverage_gap, BrepEnvelope, Frame3, GeometryError, GeometryQuality, SurfaceGeometry,
};
use crate::math::{norm, sub};
use crate::primitives;

pub(crate) struct SphereInput<'a> {
    pub(crate) brep: &'a BrepEnvelope,
    pub(crate) frame: Frame3,
    pub(crate) radius: f64,
}

pub(crate) fn gap() -> GeometryError {
    coverage_gap("full sphere input", "full sphere input")
}

pub(crate) fn full_sphere(brep: &BrepEnvelope) -> Result<SphereInput<'_>, GeometryError> {
    brep.validate()?;
    let (frame, radius) = match brep.geometry.surfaces.as_slice() {
        [SurfaceGeometry::Sphere { frame, radius }] => (*frame, *radius),
        _ => return Err(gap()),
    };
    if !matches!(brep.quality, GeometryQuality::Analytic)
        || brep.geometry.curves.len() != 1
        || brep.geometry.pcurves.len() != 4
        || !brep.geometry.intersections.is_empty()
        || brep.topology.faces.len() != 1
        || brep.topology.vertices.len() != 2
        || brep.topology.edges.len() != 3
        || brep.topology.halfedges.len() != 4
        || brep.topology.loops.len() != 1
        || !brep.topology.wires.is_empty()
        || brep.topology.shells.len() != 1
        || brep.solids.len() != 1
    {
        return Err(gap());
    }
    let canonical = primitives::sphere(brep.id.clone(), frame, radius, brep.accuracy)?;
    let mut topology = brep.topology.clone();
    topology.faces[0].key = canonical.topology.faces[0].key.clone();
    topology.faces[0].provenance = canonical.topology.faces[0].provenance.clone();
    for vertex in &mut topology.vertices {
        vertex.tolerance = brep.accuracy.geometric;
        let expected = canonical.topology.vertices[vertex.id as usize].position;
        if norm(sub(vertex.position, expected)) > brep.accuracy.geometric {
            return Err(gap());
        }
        vertex.position = expected;
    }
    for edge in &mut topology.edges {
        edge.tolerance = brep.accuracy.geometric;
    }
    if brep.geometry != canonical.geometry
        || topology != canonical.topology
        || brep.solids != canonical.solids
    {
        return Err(gap());
    }
    Ok(SphereInput {
        brep,
        frame,
        radius,
    })
}
