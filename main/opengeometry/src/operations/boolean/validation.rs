use crate::brep::Brep;
use crate::geometry::triangle::Triangle;

/// Validates that a Brep is suitable for boolean operations
pub fn validate_brep_for_boolean(brep: &Brep) -> Result<(), String> {
    // Check if Brep has valid basic structure
    if brep.id.is_nil() {
        return Err("Brep has invalid ID".to_string());
    }
    
    // Check if triangulation exists and is valid
    if let Some(ref triangles) = brep.triangulation {
        if triangles.is_empty() {
            return Err("Brep triangulation is empty".to_string());
        }
        
        // Validate each triangle
        for (i, triangle) in triangles.iter().enumerate() {
            if !is_valid_triangle(triangle) {
                return Err(format!("Invalid triangle at index {}", i));
            }
        }
        
        // Check for manifold properties
        if !is_manifold_mesh(triangles) {
            return Err("Mesh is not manifold - cannot perform boolean operations".to_string());
        }
    } else {
        return Err("Brep must be triangulated before boolean operations".to_string());
    }
    
    Ok(())
}

/// Checks if a triangle is valid (non-degenerate)
fn is_valid_triangle(triangle: &Triangle) -> bool {
    let epsilon = 1e-10;
    
    // Check for duplicate vertices
    if triangle.a.distance(&triangle.b) < epsilon ||
       triangle.b.distance(&triangle.c) < epsilon ||
       triangle.c.distance(&triangle.a) < epsilon {
        return false;
    }
    
    // Check for collinear vertices (zero area)
    let edge1 = triangle.b.clone().subtract(&triangle.a);
    let edge2 = triangle.c.clone().subtract(&triangle.a);
    let cross = edge1.cross(&edge2);
    
    cross.magnitude() > epsilon
}

/// Basic check for manifold properties
fn is_manifold_mesh(triangles: &[Triangle]) -> bool {
    // For a basic check, ensure we have at least some triangles
    // More sophisticated manifold checking would require edge adjacency analysis
    !triangles.is_empty()
}

/// Ensures a Brep is triangulated and ready for boolean operations
pub fn ensure_triangulated(brep: &mut Brep) -> Result<(), String> {
    if brep.triangulation.is_none() {
        return Err("Brep triangulation is required but not present. Call generate_geometry() first.".to_string());
    }
    
    validate_brep_for_boolean(brep)?;
    Ok(())
}

/// Repairs common mesh issues
pub fn repair_mesh(brep: &mut Brep) -> Result<(), String> {
    if let Some(ref mut triangles) = brep.triangulation {
        // Remove degenerate triangles
        triangles.retain(|triangle| is_valid_triangle(triangle));
        
        if triangles.is_empty() {
            return Err("All triangles were degenerate after repair".to_string());
        }
    }
    
    Ok(())
}