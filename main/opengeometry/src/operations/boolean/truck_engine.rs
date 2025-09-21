use super::{BooleanEngine, BooleanOperation};
use super::validation::{validate_brep_for_boolean, repair_mesh};
use crate::brep::Brep;
use crate::geometry::triangle::Triangle;
use openmaths::Vector3;
use std::collections::HashMap;
use uuid::Uuid;

use truck_modeling::*;
use truck_topology::*;
use truck_meshalgo::*;
use truck_polymesh::*;

pub struct TruckBooleanEngine {
    tolerance: f64,
}

impl TruckBooleanEngine {
    pub fn new(tolerance: f64) -> Self {
        Self { 
            tolerance: tolerance.max(1e-10) 
        }
    }
    
    pub fn tolerance(&self) -> f64 {
        self.tolerance
    }

    /// Convert OpenGeometry Brep to truck Solid
    fn brep_to_solid(&self, brep: &Brep) -> Result<Solid, String> {
        if let Some(ref triangles) = brep.triangulation {
            if triangles.is_empty() {
                return Err("Empty triangulation".to_string());
            }

            // Convert triangles to truck mesh
            let mut vertices = Vec::new();
            let mut indices = Vec::new();

            for (i, triangle) in triangles.iter().enumerate() {
                let base_idx = i * 3;
                
                // Add vertices
                vertices.push(Point3::new(triangle.a.x, triangle.a.y, triangle.a.z));
                vertices.push(Point3::new(triangle.b.x, triangle.b.y, triangle.b.z));
                vertices.push(Point3::new(triangle.c.x, triangle.c.y, triangle.c.z));
                
                // Add triangle indices
                indices.push([base_idx, base_idx + 1, base_idx + 2]);
            }

            // Create polymesh
            let polymesh = PolygonMesh::new(StandardAttributes {
                positions: vertices,
                ..Default::default()
            }, indices);
            
            // Convert polymesh to solid
            self.polymesh_to_solid(polymesh)
        } else {
            Err("No triangulation data available".to_string())
        }
    }

    /// Convert truck polymesh to solid 
    fn polymesh_to_solid(&self, _mesh: PolygonMesh) -> Result<Solid, String> {
        // For demonstration, create a simple box
        // In a real implementation, you would convert the mesh to proper B-Rep topology
        let vertex0 = builder::vertex(Point3::new(0.0, 0.0, 0.0));
        let vertex1 = builder::vertex(Point3::new(1.0, 0.0, 0.0));
        let vertex2 = builder::vertex(Point3::new(1.0, 1.0, 0.0));
        let vertex3 = builder::vertex(Point3::new(0.0, 1.0, 0.0));

        let edge0 = builder::line(&vertex0, &vertex1);
        let edge1 = builder::line(&vertex1, &vertex2);
        let edge2 = builder::line(&vertex2, &vertex3);
        let edge3 = builder::line(&vertex3, &vertex0);

        let wire = builder::wire_from_edges(&[&edge0, &edge1, &edge2, &edge3])
            .map_err(|e| format!("Failed to create wire: {:?}", e))?;
        
        let face = builder::try_attach_plane(&[wire])
            .map_err(|e| format!("Failed to create face: {:?}", e))?;

        let solid = builder::tsweep(&face, truck_modeling::Vector3::new(0.0, 0.0, 1.0));
        
        Ok(solid)
    }

    /// Convert truck Solid back to OpenGeometry Brep
    fn solid_to_brep(&self, solid: Solid, operation_type: &str) -> Result<Brep, String> {
        // Tessellate the solid into triangles
        let mesh = solid.triangulation(self.tolerance)
            .map_err(|e| format!("Failed to triangulate solid: {:?}", e))?;

        let mut triangles = Vec::new();
        
        // Convert mesh to triangles
        let positions = mesh.positions();
        let faces = mesh.tri_faces();

        for face in faces {
            let v0 = positions[face[0]];
            let v1 = positions[face[1]];
            let v2 = positions[face[2]];

            let triangle = Triangle::new_with_vertices(
                Vector3::new(v0.x as f64, v0.y as f64, v0.z as f64),
                Vector3::new(v1.x as f64, v1.y as f64, v1.z as f64),
                Vector3::new(v2.x as f64, v2.y as f64, v2.z as f64),
            );
            triangles.push(triangle);
        }

        let mut result_brep = Brep::new_with_type(Uuid::new_v4(), format!("{}_result", operation_type));
        result_brep.triangulation = Some(triangles);

        Ok(result_brep)
    }
}

impl BooleanEngine for TruckBooleanEngine {
    fn execute(&self, op: BooleanOperation, a: &Brep, b: &Brep) -> Result<Brep, String> {
        // Validate inputs
        validate_brep_for_boolean(a)
            .map_err(|e| format!("Invalid Brep A: {}", e))?;
        validate_brep_for_boolean(b)
            .map_err(|e| format!("Invalid Brep B: {}", e))?;

        // Convert to truck solids
        let solid_a = self.brep_to_solid(a)?;
        let solid_b = self.brep_to_solid(b)?;

        // Perform boolean operation
        let result_solid = match op {
            BooleanOperation::Union => {
                solid_a.union(&solid_b)
                    .map_err(|e| format!("Union operation failed: {:?}", e))?
            },
            BooleanOperation::Intersection => {
                solid_a.intersection(&solid_b)
                    .map_err(|e| format!("Intersection operation failed: {:?}", e))?
            },
            BooleanOperation::Difference => {
                solid_a.difference(&solid_b)
                    .map_err(|e| format!("Difference operation failed: {:?}", e))?
            },
            BooleanOperation::SymmetricDifference => {
                // Symmetric difference = (A - B) ∪ (B - A)
                let diff_ab = solid_a.difference(&solid_b)
                    .map_err(|e| format!("Difference A-B failed: {:?}", e))?;
                let diff_ba = solid_b.difference(&solid_a)
                    .map_err(|e| format!("Difference B-A failed: {:?}", e))?;
                diff_ab.union(&diff_ba)
                    .map_err(|e| format!("Symmetric difference union failed: {:?}", e))?
            },
        };

        // Convert back to Brep
        let operation_name = match op {
            BooleanOperation::Union => "union",
            BooleanOperation::Intersection => "intersection", 
            BooleanOperation::Difference => "difference",
            BooleanOperation::SymmetricDifference => "symmetric_difference",
        };

        self.solid_to_brep(result_solid, operation_name)
    }
}