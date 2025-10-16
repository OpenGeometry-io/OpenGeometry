Implementation of a sweep algorithm for OpenGeometry library, designed to be robust, extensible, and well-integrated with in existing codebase.
This implementation includes:
A generic Path trait to allow sweeping along different types of curves.
Handling of profile orientation along the path to prevent twisting.
Creation of a fully enclosed, "solid" B-rep with start and end caps.
A new OGSweptShape primitive for easy use.
Below are the new files and the modifications required to integrate them into your project.

Step 1: Create the Path Trait

First, let's define a generic Path trait. This will allow your sweep operation to work with any curve that can provide points and tangents, such as polylines and arcs.
Create a new file main/opengeometry/src/geometry/path.rs:

Rust


// main/opengeometry/src/geometry/path.rs

use openmaths::{Vector3, Matrix4};

/// Represents a 3D path that can be sampled for points and frames.
pub trait Path {
    /// Returns a list of points that define the path.
    fn get_points(&self) -> Vec<Vector3>;

    /// Returns a list of transformation matrices (frames) along the path.
    /// Each frame defines the position and orientation of the profile at that point.
    fn get_frames(&self) -> Vec<Matrix4>;
}



Now, modify main/opengeometry/src/primitives/polyline.rs and main/opengeometry/src/primitives/arc.rs to implement this Path trait.
In main/opengeometry/src/primitives/polyline.rs:
Add the following use statements at the top:

Rust


use crate::geometry::path::Path;
use openmaths::{Vector3, Matrix4};


And add the Path implementation:

Rust


impl Path for OGPolyline {
    fn get_points(&self) -> Vec<Vector3> {
        self.points.clone()
    }

    fn get_frames(&self) -> Vec<Matrix4> {
        let mut frames = Vec::new();
        if self.points.len() < 2 {
            return frames;
        }

        for i in 0..self.points.len() {
            let position = self.points[i];
            let tangent = if i < self.points.len() - 1 {
                self.points[i+1].subtract(&position).normalize()
            } else {
                position.subtract(&self.points[i-1]).normalize()
            };

            // A simple approach for up vector. This can be improved to handle vertical tangents.
            let mut up = Vector3::new(0.0, 1.0, 0.0);
            if tangent.y.abs() > 0.999 {
                up = Vector3::new(0.0, 0.0, 1.0);
            }

            let right = tangent.cross(&up).normalize();
            let new_up = right.cross(&tangent).normalize();

            frames.push(Matrix4::from_basis(&right, &new_up, &tangent, &position));
        }
        frames
    }
}


In main/opengeometry/src/primitives/arc.rs:
Add use statements:

Rust


use crate::geometry::path::Path;
use openmaths::Matrix4;


And the Path implementation:

Rust


impl Path for OGArc {
    fn get_points(&self) -> Vec<Vector3> {
        // Sample points along the arc. Here we use a fixed number of segments.
        // For production, you might want a more adaptive sampling.
        let mut points = Vec::new();
        let segments = 32; // Or calculate based on arc length/radius for better quality
        for i in 0..=segments {
            let angle = self.start_angle + (self.end_angle - self.start_angle) * (i as f64 / segments as f64);
            points.push(self.center.add(&Vector3::new(self.radius * angle.cos(), self.radius * angle.sin(), 0.0)));
        }
        points
    }

    fn get_frames(&self) -> Vec<Matrix4> {
        let mut frames = Vec::new();
        let points = self.get_points();
        
        for point in points {
            let to_center = self.center.subtract(&point);
            let tangent = Vector3::new(to_center.y, -to_center.x, 0.0).normalize();
            let up = Vector3::new(0.0, 0.0, 1.0); // Assuming arc is on XY plane
            let right = tangent.cross(&up).normalize();

            frames.push(Matrix4::from_basis(&right, &up, &tangent, &point));
        }
        frames
    }
}



Step 2: Implement the Sweep Operation

Now, create the core sweep logic in a new file main/opengeometry/src/operations/sweep.rs:

Rust


// main/opengeometry/src/operations/sweep.rs

use crate::brep::{Brep, Vertex, Face};
use crate::geometry::path::Path;
use openmaths::{Vector3, Matrix4};
use uuid::Uuid;
use crate::operations::triangulate::triangulate;


pub fn sweep_profile_along_path(profile_brep: &Brep, path: &dyn Path) -> Brep {
    let mut swept_brep = Brep::new(Uuid::new_v4());
    let frames = path.get_frames();

    if frames.is_empty() {
        return swept_brep;
    }

    let profile_vertices = profile_brep.get_flattened_vertices();
    let profile_vertex_count = profile_vertices.len();

    // Generate vertices for each frame
    for frame in &frames {
        for profile_vertex in &profile_vertices {
            let transformed_vertex = frame.transform_point(profile_vertex);
            swept_brep.add_vertex(transformed_vertex);
        }
    }

    // Create side faces
    for i in 0..(frames.len() - 1) {
        let current_step_base_index = (i * profile_vertex_count) as u32;
        let next_step_base_index = ((i + 1) * profile_vertex_count) as u32;

        for j in 0..profile_vertex_count {
            let next_j = (j + 1) % profile_vertex_count;

            let v1 = current_step_base_index + j as u32;
            let v2 = next_step_base_index + j as u32;
            let v3 = next_step_base_index + next_j as u32;
            let v4 = current_step_base_index + next_j as u32;

            swept_brep.add_face(vec![v1, v2, v3, v4]);
        }
    }

    // Create start and end caps
    let start_cap_indices: Vec<u32> = (0..profile_vertex_count as u32).collect();
    let end_cap_indices: Vec<u32> = ((swept_brep.get_vertex_count() - profile_vertex_count) as u32..swept_brep.get_vertex_count() as u32).collect();

    let start_cap_vertices: Vec<Vector3> = start_cap_indices.iter().map(|&i| swept_brep.vertices[i as usize].point).collect();
    let end_cap_vertices: Vec<Vector3> = end_cap_indices.iter().map(|&i| swept_brep.vertices[i as usize].point).collect();

    if let Some(triangles) = triangulate(&start_cap_vertices) {
        for triangle in triangles {
            swept_brep.add_face(vec![start_cap_indices[triangle.0], start_cap_indices[triangle.1], start_cap_indices[triangle.2]].iter().rev().cloned().collect());
        }
    }

    if let Some(triangles) = triangulate(&end_cap_vertices) {
        for triangle in triangles {
            swept_brep.add_face(vec![end_cap_indices[triangle.0], end_cap_indices[triangle.1], end_cap_indices[triangle.2]]);
        }
    }

    swept_brep
}



Step 3: Create the OGSweptShape Primitive

Create a high-level primitive for users to easily create swept shapes. Create a new file main/opengeometry/src/primitives/swept_shape.rs:

Rust


// main/opengeometry/src/primitives/swept_shape.rs

use crate::brep::Brep;
use crate::geometry::path::Path;
use crate::operations::sweep::sweep_profile_along_path;

pub struct OGSweptShape;

impl OGSweptShape {
    pub fn new(profile: &Brep, path: &dyn Path) -> Brep {
        sweep_profile_along_path(profile, path)
    }
}




Step 4: Integrate the New Modules

Finally, you need to register your new modules and structs in the project.
In main/opengeometry/src/lib.rs:
Add the following lines to declare the new modules:

Rust


pub mod geometry;
pub mod primitives;


And to make them public:

Rust


pub use primitives::swept_shape::OGSweptShape;


In main/opengeometry/src/operations/mod.rs:
Add:

Rust


pub mod sweep;


In main/opengeometry/src/primitives/mod.rs:
Add:

Rust


pub mod swept_shape;


And in main/opengeometry/src/geometry/mod.rs (if it exists, otherwise create it):

Rust


pub mod path;



How to Use the New Sweep Functionality

Now you can create swept shapes in your application code. Here's an example of how you might use it:

Rust


use opengeometry::brep::Brep;
use opengeometry::primitives::polygon::OGPolygon;
use opengeometry::primitives::polyline::OGPolyline;
use opengeometry::primitives::swept_shape::OGSweptShape;
use openmaths::Vector3;

// 1. Create a profile (e.g., a square)
let square_points = vec![
    Vector3::new(-0.5, -0.5, 0.0),
    Vector3::new(0.5, -0.5, 0.0),
    Vector3::new(0.5, 0.5, 0.0),
    Vector3::new(-0.5, 0.5, 0.0),
];
let square_profile = OGPolygon::new(square_points).to_brep();

// 2. Create a path (e.g., a polyline)
let path_points = vec![
    Vector3::new(0.0, 0.0, 0.0),
    Vector3::new(5.0, 0.0, 0.0),
    Vector3::new(5.0, 5.0, 0.0),
];
let sweep_path = OGPolyline::new(path_points);

// 3. Create the swept shape
let swept_b_rep = OGSweptShape::new(&square_profile, &sweep_path);

// 'swept_b_rep' now contains the B-rep mesh of the swept shape.


This implementation provides a solid, extensible foundation for sweeping operations in your geometry kernel. You can further enhance it by adding support for more complex path types (like NURBS curves) or by implementing more advanced frame calculation techniques to handle complex 3D paths.
