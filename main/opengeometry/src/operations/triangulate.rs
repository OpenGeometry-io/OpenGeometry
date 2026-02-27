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

    // --- 4. Reshape the Result ---
    // The result is a flat list of indices. Group them into triangles.
    let triangle_indices: Vec<[usize; 3]> = triangle_indices
        .chunks_exact(3)
        .map(|chunk| [chunk[0], chunk[1], chunk[2]])
        .collect();

    // if first index of first triangle is odd, reverse all triangles
    // explanation here: If the first triangle is defined in a clockwise manner, we reverse it to ensure a consistent winding order.
    // TODO: This is a temporary fix. A more robust solution would involve checking the winding order of the entire polygon and its holes.
    if triangle_indices.len() > 0 && triangle_indices[0][0] % 2 == 1 {
        let triangle_indices: Vec<[usize; 3]> = triangle_indices
            .into_iter()
            .map(|mut tri| {
                tri.reverse();
                tri
            })
            .collect();
        return triangle_indices;
    } else {
        return triangle_indices;
    }
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
