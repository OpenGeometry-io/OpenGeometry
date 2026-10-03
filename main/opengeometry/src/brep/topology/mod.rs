mod entities;
mod envelope;
#[cfg(test)]
mod fixtures;
mod orientation;
mod provenance;
#[cfg(test)]
mod tests;

pub use entities::{Edge, Face, HalfEdge, Topology, Vertex};
pub(crate) use entities::{
    EdgeGeometry, HalfEdgeGeometryUse, Loop, Shell, SolidRegion, TrimRegion, Wire,
};
pub use envelope::BrepEnvelope;
pub(crate) use envelope::GeometryQuality;
pub(super) use envelope::SCHEMA_VERSION;
#[cfg(test)]
pub(crate) use fixtures::{circular_face, coarse_accuracy, fine_accuracy};
pub(crate) use orientation::Orientation;
pub use provenance::{FaceProvenance, FaceRole, FaceSource};
