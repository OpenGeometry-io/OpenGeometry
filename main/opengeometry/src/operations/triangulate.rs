use openmaths::Vector3;

pub fn triangulate_polygon_with_holes(
    face_vertices: &Vec<Vector3>,
    holes: &Vec<Vec<Vector3>>,
) -> Vec<[usize; 3]> {
    if face_vertices.len() < 3 {
        return Vec::new();
    }

    // --- 1. Projection to 2D ---
    // First, determine the best 2D plane to project onto by finding the
    // dominant axis of the face's normal.
    let normal = calculate_normal(face_vertices);

    let (axis_u, axis_v) = if normal.z.abs() > normal.x.abs() && normal.z.abs() > normal.y.abs() {
        // Project to XY plane
        (0, 1) // Corresponds to (x, y)
    } else if normal.x.abs() > normal.y.abs() {
        // Project to YZ plane
        (1, 2) // Corresponds to (y, z)
    } else {
        // Project to XZ plane
        (0, 2) // Corresponds to (x, z)
    };

    // --- 2. Flatten Data for earcutr ---
    // earcutr needs a flat list of 2D coordinates and a list of indices
    // where the holes begin.

    let mut vertices_2d = Vec::with_capacity(
        (face_vertices.len() + holes.iter().map(|h| h.len()).sum::<usize>()) * 2,
    );
    let mut hole_indices = Vec::with_capacity(holes.len());

    // Add outer loop vertices
    for v in face_vertices {
        let coord_u = match axis_u {
            0 => v.x,
            1 => v.y,
            2 => v.z,
            _ => unreachable!("Invalid axis_u"),
        };
        let coord_v = match axis_v {
            0 => v.x,
            1 => v.y,
            2 => v.z,
            _ => unreachable!("Invalid axis_v"),
        };
        vertices_2d.push(coord_u);
        vertices_2d.push(coord_v);
    }

    // Add hole vertices
    if !holes.is_empty() {
        let mut current_index = face_vertices.len();
        for hole in holes {
            hole_indices.push(current_index);
            for v in hole {
                let coord_u = match axis_u {
                    0 => v.x,
                    1 => v.y,
                    2 => v.z,
                    _ => unreachable!("Invalid axis_u"),
                };
                let coord_v = match axis_v {
                    0 => v.x,
                    1 => v.y,
                    2 => v.z,
                    _ => unreachable!("Invalid axis_v"),
                };
                vertices_2d.push(coord_u);
                vertices_2d.push(coord_v);
                current_index += 1;
            }
        }
    }

    // --- 3. Run Earcut Algorithm ---
    let triangle_indices = earcutr::earcut(&vertices_2d, &hole_indices, 2);

    // --- 4. Reshape and enforce triangle winding in 3D ---
    // Earcut runs in projected 2D space; when mapped back to 3D we must ensure
    // every output triangle follows the face normal direction.
    let mut all_vertices = Vec::with_capacity(
        face_vertices.len() + holes.iter().map(|hole| hole.len()).sum::<usize>(),
    );
    all_vertices.extend(face_vertices.iter().copied());
    for hole in holes {
        all_vertices.extend(hole.iter().copied());
    }

    let face_normal = calculate_normal(face_vertices);

    triangle_indices
        .chunks_exact(3)
        .map(|chunk| {
            let mut tri = [chunk[0], chunk[1], chunk[2]];

            let p0 = all_vertices[tri[0]];
            let p1 = all_vertices[tri[1]];
            let p2 = all_vertices[tri[2]];

            let edge_a = [p1.x - p0.x, p1.y - p0.y, p1.z - p0.z];
            let edge_b = [p2.x - p0.x, p2.y - p0.y, p2.z - p0.z];
            let tri_normal = cross(edge_a, edge_b);
            let face_normal_arr = [face_normal.x, face_normal.y, face_normal.z];

            if dot(tri_normal, face_normal_arr) < 0.0 {
                tri.swap(1, 2);
            }

            tri
        })
        .collect()
}

/**
 * Calculates the average normal of a polygon.
 * This is used to determine the best projection plane.
 */
fn calculate_normal(vertices: &[Vector3]) -> Vector3 {
    if vertices.len() < 3 {
        return Vector3::new(0.0, 0.0, 1.0); // Default to Z-axis
    }
    let mut normal = Vector3::new(0.0, 0.0, 0.0);
    for i in 0..vertices.len() {
        let p1 = &vertices[i];
        let p2 = &vertices[(i + 1) % vertices.len()];
        normal.x += (p1.y - p2.y) * (p1.z + p2.z);
        normal.y += (p1.z - p2.z) * (p1.x + p2.x);
        normal.z += (p1.x - p2.x) * (p1.y + p2.y);
    }
    normal.normalize();
    normal
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangulation_preserves_winding_for_slanted_quad() {
        let vertices = vec![
            Vector3::new(1.0, -1.0, -1.0),
            Vector3::new(-1.0, 1.0, -1.0),
            Vector3::new(-1.0, 1.0, 1.0),
            Vector3::new(1.0, -1.0, 1.0),
        ];

        let triangles = triangulate_polygon_with_holes(&vertices, &Vec::new());
        assert_eq!(triangles.len(), 2);

        let normal = calculate_normal(&vertices);
        let normal_arr = [normal.x, normal.y, normal.z];

        for tri in triangles {
            let p0 = vertices[tri[0]];
            let p1 = vertices[tri[1]];
            let p2 = vertices[tri[2]];

            let edge_a = [p1.x - p0.x, p1.y - p0.y, p1.z - p0.z];
            let edge_b = [p2.x - p0.x, p2.y - p0.y, p2.z - p0.z];
            let tri_normal = cross(edge_a, edge_b);

            assert!(dot(tri_normal, normal_arr) >= 0.0);
        }
    }
}
