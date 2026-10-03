mod face_intersection;
mod ssi;
mod ssi_result;
mod universal_ssi;

pub(super) use face_intersection::{
    intersect_breps, intersect_faces, FaceView, IntersectionBranch, IntersectionGraph,
};
pub(super) use ssi::intersect_surfaces;

#[cfg(test)]
pub(super) use universal_ssi::intersect_patches;
