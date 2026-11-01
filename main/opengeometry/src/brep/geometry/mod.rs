mod chord;
mod conoid;
mod curve;
mod intersection_definition;
mod store;
mod surface;
#[cfg(test)]
mod tests;
mod trace;
mod turned;
mod wrap;

pub(crate) use chord::{bounds_chord_deviation, point_segment_distance};
pub use conoid::conoid_ellipse;
pub(crate) use curve::Curve;
pub use curve::CurveGeometry;
pub(crate) use intersection_definition::{IntersectionDefinition, TraceAnchor};
pub use store::GeometryStore;
pub use surface::{Surface, SurfaceChart, SurfaceGeometry, SurfaceJet2};
pub use trace::IntersectionPoint;
pub(crate) use trace::SsiBudget;
pub use wrap::Wrap;
