mod classification;
mod classify;
mod face_trim;
mod measure;
mod orientation;
mod ray_roots;
#[cfg(test)]
mod tests;

pub use classification::PointClassification;
pub use classify::classify_point;
pub(super) use classify::classify_point_in_shell;
pub(crate) use classify::Classifier;
pub(super) use face_trim::{face_contains_uv, FaceTrim};
pub use measure::{validity, Validity};
pub use orientation::validate_orientation;
