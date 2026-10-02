use crate::brep::{BrepEnvelope, SurfaceGeometry};

pub(crate) fn all_planar(body: &BrepEnvelope) -> bool {
    body.geometry
        .surfaces
        .iter()
        .all(|surface| matches!(surface, SurfaceGeometry::Plane { .. }))
}
