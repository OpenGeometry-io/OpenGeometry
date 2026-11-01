mod accuracy;
mod body_type;
mod bounds;
mod builder;
#[cfg(test)]
mod diagnostics;
mod error;
mod frame;
mod geometry;
mod pcurve;
mod placement;
mod similarity;
#[cfg(test)]
mod tests;
mod topology;
mod validate;

pub use accuracy::Accuracy;
pub use body_type::BodyType;
pub use bounds::PatchBounds;
pub use builder::{boundary, plane_boundary, uv_line, Builder, Use};
pub(super) use builder::{dimensions, padded_uv_bounds, WireUse};
pub(super) use error::coverage_gap;
pub use error::GeometryError;
pub use frame::Frame3;
pub(super) use frame::{checked, unit, UVBox, GROUND, UV};
pub(super) use geometry::{
    bounds_chord_deviation, point_segment_distance, Curve, IntersectionDefinition, SsiBudget,
    TraceAnchor,
};
pub use geometry::{
    CurveGeometry, GeometryStore, IntersectionPoint, Surface, SurfaceChart, SurfaceGeometry,
    SurfaceJet2,
};
pub(super) use pcurve::period_lifted_uv;
pub use pcurve::{IntersectionSide, PcurveGeometry};
pub use placement::{mirror::mirrored, placed};
pub use similarity::Similarity3;
#[cfg(test)]
pub(super) use topology::{circular_face, coarse_accuracy, fine_accuracy};
pub use topology::{
    BrepEnvelope, Edge, Face, FaceProvenance, FaceRole, FaceSource, HalfEdge, Orientation,
    Topology, Vertex,
};
pub(super) use topology::{
    EdgeGeometry, GeometryQuality, HalfEdgeGeometryUse, Loop, Shell, SolidRegion, TrimRegion, Wire,
};
