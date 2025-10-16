/**
 * Copyright (c) 2025, OpenGeometry. All rights reserved.
 * Sweep operation for OpenGeometry.
 * 
 * This module provides functionality to sweep a 2D profile along a 3D path
 * to create a solid B-Rep representation.
 */

use crate::brep::{Brep, Face};
use crate::geometry::path::Path;
use crate::operations::triangulate::triangulate_polygon_with_holes;
use openmaths::{Vector3, Matrix4};
use uuid::Uuid;

/**
 * Transform a point by a Matrix4 transformation matrix
 */
fn transform_point(point: &Vector3, matrix: &Matrix4) -> Vector3 {
    // Extract matrix elements using the elements() method
    let elements = matrix.elements();
    
    // Extract translation from the matrix (last column)
    let translation_x = elements[12];
    let translation_y = elements[13]; 
    let translation_z = elements[14];
    
    // Extract rotation part (first 3x3 submatrix) and apply to point
    let x = elements[0] * point.x + elements[4] * point.y + elements[8] * point.z + translation_x;
    let y = elements[1] * point.x + elements[5] * point.y + elements[9] * point.z + translation_y;
    let z = elements[2] * point.x + elements[6] * point.y + elements[10] * point.z + translation_z;
    
    Vector3::new(x, y, z)
}

/**
 * Sweeps a 2D profile along a 3D path to create a solid B-Rep
 * 
 * @param profile_brep - The 2D profile to sweep (should be a single face)
 * @param path - The path along which to sweep the profile
 * @returns A new B-Rep representing the swept solid
 */
pub fn sweep_profile_along_path(profile_brep: &Brep, path: &dyn Path) -> Brep {
    let mut swept_brep = Brep::new(Uuid::new_v4());
    let frames = path.get_frames();

    if frames.is_empty() {
        return swept_brep;
    }

    let profile_vertices = profile_brep.get_flattened_vertices();
    let profile_vertex_count = profile_vertices.len();

    if profile_vertex_count == 0 {
        return swept_brep;
    }

    // Generate vertices for each frame along the path
    for frame in &frames {
        for profile_vertex in &profile_vertices {
            let transformed_vertex = transform_point(profile_vertex, frame);
            swept_brep.add_vertex(transformed_vertex);
        }
    }

    // Create side faces connecting consecutive cross-sections
    for i in 0..(frames.len() - 1) {
        let current_step_base_index = (i * profile_vertex_count) as u32;
        let next_step_base_index = ((i + 1) * profile_vertex_count) as u32;

        for j in 0..profile_vertex_count {
            let next_j = (j + 1) % profile_vertex_count;

            // Create a quad face using four vertices
            let v1 = current_step_base_index + j as u32;
            let v2 = next_step_base_index + j as u32;
            let v3 = next_step_base_index + next_j as u32;
            let v4 = current_step_base_index + next_j as u32;

            // Add the face (ensure proper winding order)
            swept_brep.add_face(vec![v1, v2, v3, v4]);
        }
    }

    // Create start cap (first cross-section)
    let start_cap_indices: Vec<u32> = (0..profile_vertex_count as u32).collect();
    let start_cap_vertices: Vec<Vector3> = start_cap_indices.iter()
        .map(|&i| swept_brep.vertices[i as usize].position)
        .collect();

    // Triangulate the start cap
    let start_triangles = triangulate_polygon_with_holes(&start_cap_vertices, &Vec::new());
    for triangle in start_triangles {
        // Reverse winding order for start cap to face outward
        swept_brep.add_face(vec![
            start_cap_indices[triangle[2]], 
            start_cap_indices[triangle[1]], 
            start_cap_indices[triangle[0]]
        ]);
    }

    // Create end cap (last cross-section)
    let last_section_start = ((frames.len() - 1) * profile_vertex_count) as u32;
    let end_cap_indices: Vec<u32> = (last_section_start..(last_section_start + profile_vertex_count as u32)).collect();
    let end_cap_vertices: Vec<Vector3> = end_cap_indices.iter()
        .map(|&i| swept_brep.vertices[i as usize].position)
        .collect();

    // Triangulate the end cap
    let end_triangles = triangulate_polygon_with_holes(&end_cap_vertices, &Vec::new());
    for triangle in end_triangles {
        // Normal winding order for end cap to face outward
        swept_brep.add_face(vec![
            end_cap_indices[triangle[0]],
            end_cap_indices[triangle[1]], 
            end_cap_indices[triangle[2]]
        ]);
    }

    swept_brep
}