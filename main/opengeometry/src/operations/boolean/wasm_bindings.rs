use wasm_bindgen::prelude::*;
use crate::operations::boolean::{simple_engine::SimpleBooleanEngine, BooleanEngine, BooleanOperation};
use crate::operations::boolean::validation::ensure_triangulated;
use crate::brep::Brep;

#[wasm_bindgen]
pub struct OGBooleanEngine {
    engine: SimpleBooleanEngine,
}

#[wasm_bindgen]
impl OGBooleanEngine {
    #[wasm_bindgen(constructor)]
    pub fn new(tolerance: f64) -> OGBooleanEngine {
        OGBooleanEngine {
            engine: SimpleBooleanEngine::new(tolerance),
        }
    }

    #[wasm_bindgen(getter)]
    pub fn tolerance(&self) -> f64 {
        self.engine.tolerance()
    }

    #[wasm_bindgen]
    pub fn union(&self, a: &OGBrep, b: &OGBrep) -> Result<OGBrep, String> {
        let mut brep_a = a.brep.clone();
        let mut brep_b = b.brep.clone();
        
        // Ensure both Breps are triangulated
        ensure_triangulated(&mut brep_a)
            .map_err(|e| format!("Failed to prepare Brep A for boolean operation: {}", e))?;
        ensure_triangulated(&mut brep_b)
            .map_err(|e| format!("Failed to prepare Brep B for boolean operation: {}", e))?;
        
        let result = self.engine.execute(BooleanOperation::Union, &brep_a, &brep_b)?;
        Ok(OGBrep::from_brep(result))
    }

    #[wasm_bindgen]
    pub fn intersection(&self, a: &OGBrep, b: &OGBrep) -> Result<OGBrep, String> {
        let mut brep_a = a.brep.clone();
        let mut brep_b = b.brep.clone();
        
        ensure_triangulated(&mut brep_a)
            .map_err(|e| format!("Failed to prepare Brep A for boolean operation: {}", e))?;
        ensure_triangulated(&mut brep_b)
            .map_err(|e| format!("Failed to prepare Brep B for boolean operation: {}", e))?;
        
        let result = self.engine.execute(BooleanOperation::Intersection, &brep_a, &brep_b)?;
        Ok(OGBrep::from_brep(result))
    }

    #[wasm_bindgen]
    pub fn difference(&self, a: &OGBrep, b: &OGBrep) -> Result<OGBrep, String> {
        let mut brep_a = a.brep.clone();
        let mut brep_b = b.brep.clone();
        
        ensure_triangulated(&mut brep_a)
            .map_err(|e| format!("Failed to prepare Brep A for boolean operation: {}", e))?;
        ensure_triangulated(&mut brep_b)
            .map_err(|e| format!("Failed to prepare Brep B for boolean operation: {}", e))?;
        
        let result = self.engine.execute(BooleanOperation::Difference, &brep_a, &brep_b)?;
        Ok(OGBrep::from_brep(result))
    }

    #[wasm_bindgen]
    pub fn symmetric_difference(&self, a: &OGBrep, b: &OGBrep) -> Result<OGBrep, String> {
        let mut brep_a = a.brep.clone();
        let mut brep_b = b.brep.clone();
        
        ensure_triangulated(&mut brep_a)
            .map_err(|e| format!("Failed to prepare Brep A for boolean operation: {}", e))?;
        ensure_triangulated(&mut brep_b)
            .map_err(|e| format!("Failed to prepare Brep B for boolean operation: {}", e))?;
        
        let result = self.engine.execute(BooleanOperation::SymmetricDifference, &brep_a, &brep_b)?;
        Ok(OGBrep::from_brep(result))
    }
}

#[wasm_bindgen]
pub struct OGBrep {
    pub(crate) brep: Brep,
}

impl OGBrep {
    /// Create from Brep (internal use only, not WASM-exposed)
    pub fn from_brep(brep: Brep) -> OGBrep {
        OGBrep { brep }
    }
}

#[wasm_bindgen]
impl OGBrep {
    #[wasm_bindgen(constructor)]
    pub fn new(geometry_type: String) -> OGBrep {
        OGBrep {
            brep: Brep::new_with_type(uuid::Uuid::new_v4(), geometry_type),
        }
    }

    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.brep.id.to_string()
    }

    #[wasm_bindgen(getter)]
    pub fn geometry_type(&self) -> String {
        self.brep.geometry_type.clone()
    }

    #[wasm_bindgen]
    pub fn is_triangulated(&self) -> bool {
        self.brep.triangulation.is_some()
    }

    #[wasm_bindgen]
    pub fn triangle_count(&self) -> u32 {
        self.brep.triangulation.as_ref().map_or(0, |t| t.len() as u32)
    }

    #[wasm_bindgen]
    pub fn get_geometry_serialized(&self) -> Result<String, String> {
        if let Some(ref triangles) = self.brep.triangulation {
            let mut vertices: Vec<f32> = Vec::new();
            let mut indices: Vec<u32> = Vec::new();
            
            for (i, triangle) in triangles.iter().enumerate() {
                let base_index = i * 3;
                
                // Add vertices
                vertices.extend(&[
                    triangle.a.x as f32, triangle.a.y as f32, triangle.a.z as f32,
                    triangle.b.x as f32, triangle.b.y as f32, triangle.b.z as f32,
                    triangle.c.x as f32, triangle.c.y as f32, triangle.c.z as f32,
                ]);
                
                // Add indices
                indices.extend(&[
                    base_index as u32,
                    (base_index + 1) as u32,
                    (base_index + 2) as u32,
                ]);
            }
            
            let geometry_data = serde_json::json!({
                "vertices": vertices,
                "indices": indices,
                "triangleCount": triangles.len(),
                "vertexCount": vertices.len() / 3,
                "geometryType": self.brep.geometry_type
            });

            // print to js console for debugging
            web_sys::console::log_1(&serde_json::to_string(&geometry_data).unwrap().into());
            
            serde_json::to_string(&geometry_data)
                .map_err(|e| format!("Failed to serialize geometry: {}", e))
        } else {
            Err("No triangulation available".to_string())
        }
    }

    #[wasm_bindgen]
    pub fn validate(&self) -> Result<(), String> {
        crate::operations::boolean::validation::validate_brep_for_boolean(&self.brep)
    }
}