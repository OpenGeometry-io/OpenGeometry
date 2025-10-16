/**
 * Copyright (c) 2025, OpenGeometry. All rights reserved.
 * Path trait for sweep operations.
 * 
 * This trait defines a 3D path that can be sampled for points and frames,
 * allowing sweep operations to work with different types of curves like
 * polylines and arcs.
 */

use openmaths::{Vector3, Matrix4};

/// Represents a 3D path that can be sampled for points and frames.
pub trait Path {
    /// Returns a list of points that define the path.
    fn get_points(&self) -> Vec<Vector3>;

    /// Returns a list of transformation matrices (frames) along the path.
    /// Each frame defines the position and orientation of the profile at that point.
    fn get_frames(&self) -> Vec<Matrix4>;
}