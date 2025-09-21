use super::{BooleanEngine, BooleanOperation, validation::validate_brep_for_boolean};
use crate::brep::Brep;
use crate::geometry::triangle::Triangle;
use openmaths::Vector3;
use uuid::Uuid;

pub struct SimpleBooleanEngine {
    tolerance: f64,
}

impl SimpleBooleanEngine {
    pub fn new(tolerance: f64) -> Self {
        Self { tolerance }
    }

    pub fn tolerance(&self) -> f64 {
        self.tolerance
    }

    fn get_triangles_from_brep(&self, brep: &Brep) -> Vec<Triangle> {
        if let Some(ref triangles) = brep.triangulation {
            triangles.clone()
        } else {
            Vec::new()
        }
    }

    fn remove_overlapping_triangles(&self, triangles_a: &[Triangle], triangles_b: &[Triangle]) -> Vec<Triangle> {
        let mut result = Vec::new();
        
        for tri_a in triangles_a {
            let mut overlaps = false;
            for tri_b in triangles_b {
                if self.triangles_overlap(tri_a, tri_b) {
                    overlaps = true;
                    break;
                }
            }
            if !overlaps {
                result.push(tri_a.clone());
            }
        }
        
        result
    }

    fn triangles_overlap(&self, tri_a: &Triangle, tri_b: &Triangle) -> bool {
        let center_a = self.triangle_center(tri_a);
        let center_b = self.triangle_center(tri_b);
        
        let mut diff = center_a.clone();
        diff.subtract(&center_b);
        let distance = diff.magnitude();
        distance < self.tolerance * 2.0
    }

    fn triangle_center(&self, triangle: &Triangle) -> Vector3 {
        let mut sum = triangle.a.clone();
        sum.add(&triangle.b);
        sum.add(&triangle.c);
        sum.divide_scalar(3.0);
        sum
    }

    fn find_intersecting_triangles(&self, triangles_a: &[Triangle], triangles_b: &[Triangle]) -> Vec<Triangle> {
        let mut result = Vec::new();
        
        for tri_a in triangles_a {
            for tri_b in triangles_b {
                if self.triangles_intersect(tri_a, tri_b) {
                    // Create intersection triangle (simplified approximation)
                    let center_a = self.triangle_center(tri_a);
                    let center_b = self.triangle_center(tri_b);
                    let mut center = center_a.clone();
                    center.add(&center_b);
                    center.divide_scalar(2.0);
                    
                    // Create a smaller triangle at the intersection point
                    let offset = Vector3::new(self.tolerance, 0.0, 0.0);
                    let mut p1 = center.clone();
                    p1.subtract(&offset);
                    let mut p2 = center.clone();
                    p2.add(&offset);
                    let mut p3 = center.clone();
                    p3.add(&Vector3::new(0.0, self.tolerance, 0.0));
                    
                    let intersection = Triangle {
                        a: p1,
                        b: p2,
                        c: p3,
                    };
                    result.push(intersection);
                }
            }
        }
        
        result
    }

    fn triangles_intersect(&self, tri_a: &Triangle, tri_b: &Triangle) -> bool {
        // Simple bounding box intersection test
        let min_a = Vector3::new(
            tri_a.a.x.min(tri_a.b.x).min(tri_a.c.x),
            tri_a.a.y.min(tri_a.b.y).min(tri_a.c.y),
            tri_a.a.z.min(tri_a.b.z).min(tri_a.c.z),
        );
        let max_a = Vector3::new(
            tri_a.a.x.max(tri_a.b.x).max(tri_a.c.x),
            tri_a.a.y.max(tri_a.b.y).max(tri_a.c.y),
            tri_a.a.z.max(tri_a.b.z).max(tri_a.c.z),
        );
        
        let min_b = Vector3::new(
            tri_b.a.x.min(tri_b.b.x).min(tri_b.c.x),
            tri_b.a.y.min(tri_b.b.y).min(tri_b.c.y),
            tri_b.a.z.min(tri_b.b.z).min(tri_b.c.z),
        );
        let max_b = Vector3::new(
            tri_b.a.x.max(tri_b.b.x).max(tri_b.c.x),
            tri_b.a.y.max(tri_b.b.y).max(tri_b.c.y),
            tri_b.a.z.max(tri_b.b.z).max(tri_b.c.z),
        );
        
        // Check if bounding boxes overlap
        max_a.x >= min_b.x && min_a.x <= max_b.x &&
        max_a.y >= min_b.y && min_a.y <= max_b.y &&
        max_a.z >= min_b.z && min_a.z <= max_b.z
    }

    fn remove_internal_triangles(&self, triangles_a: &[Triangle], triangles_b: &[Triangle]) -> Vec<Triangle> {
        let mut result = Vec::new();
        
        for tri_a in triangles_a {
            let center = self.triangle_center(tri_a);
            if !self.point_inside_mesh(center, triangles_b) {
                result.push(tri_a.clone());
            }
        }
        
        result
    }

    fn point_inside_mesh(&self, point: Vector3, triangles: &[Triangle]) -> bool {
        // Simple ray casting test - count intersections with triangles
        let ray_direction = Vector3::new(1.0, 0.0, 0.0);
        let mut intersections = 0;
        
        for triangle in triangles {
            if self.ray_triangle_intersection(point, ray_direction, triangle) {
                intersections += 1;
            }
        }
        
        // Point is inside if odd number of intersections
        intersections % 2 == 1
    }

    fn ray_triangle_intersection(&self, origin: Vector3, direction: Vector3, triangle: &Triangle) -> bool {
        // Simplified ray-triangle intersection
        let center = self.triangle_center(triangle);
        let mut to_center = center.clone();
        to_center.subtract(&origin);
        
        // Check if ray passes close to triangle center
        let projection = to_center.dot(&direction);
        if projection < 0.0 {
            return false;
        }
        
        let mut direction_scaled = direction.clone();
        direction_scaled.multiply_scalar(projection);
        let mut closest_point = origin.clone();
        closest_point.add(&direction_scaled);
        
        let mut distance_vec = closest_point.clone();
        distance_vec.subtract(&center);
        let distance = distance_vec.magnitude();
        
        distance < self.tolerance * 5.0
    }

    fn create_brep_from_triangles(&self, triangles: Vec<Triangle>, operation_name: &str) -> Result<Brep, String> {
        if triangles.is_empty() {
            return Err(format!("No triangles generated for {} operation", operation_name));
        }

        let id = Uuid::new_v4();
        let mut brep = Brep::new(id);
        
        // Set triangulation data directly
        brep.triangulation = Some(triangles);
        
        Ok(brep)
    }
}

impl BooleanEngine for SimpleBooleanEngine {
    fn execute(&self, op: BooleanOperation, a: &Brep, b: &Brep) -> Result<Brep, String> {
        // Validate inputs
        validate_brep_for_boolean(a)?;
        validate_brep_for_boolean(b)?;

        let triangles_a = self.get_triangles_from_brep(a);
        // log to js console for debugging
        web_sys::console::log_1(&format!("Brep A has {} triangles", triangles_a.len()).into());

        let triangles_b = self.get_triangles_from_brep(b);

        if triangles_a.is_empty() {
            return Err("Brep A has no triangulation data".to_string());
        }
        if triangles_b.is_empty() {
            return Err("Brep B has no triangulation data".to_string());
        }

        let result_triangles = match op {
            BooleanOperation::Union => {
                // Union: A + B - overlaps
                let mut result = triangles_a.clone();
                let non_overlapping_b = self.remove_overlapping_triangles(&triangles_b, &triangles_a);
                result.extend(non_overlapping_b);
                result
            },
            BooleanOperation::Intersection => {
                // Intersection: find overlapping/intersecting regions
                self.find_intersecting_triangles(&triangles_a, &triangles_b)
            },
            BooleanOperation::Difference => {
                // Difference: A - (parts of A inside B)
                self.remove_internal_triangles(&triangles_a, &triangles_b)
            },
            BooleanOperation::SymmetricDifference => {
                // Symmetric difference: (A - B) + (B - A)
                let a_minus_b = self.remove_internal_triangles(&triangles_a, &triangles_b);
                let b_minus_a = self.remove_internal_triangles(&triangles_b, &triangles_a);
                let mut result = a_minus_b;
                result.extend(b_minus_a);
                result
            },
        };

        self.create_brep_from_triangles(result_triangles, &format!("{:?}", op))
    }
}