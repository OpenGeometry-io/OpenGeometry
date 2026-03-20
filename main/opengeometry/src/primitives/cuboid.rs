use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::brep::{Brep, BrepBuilder};
use crate::export::projection::{project_brep_to_scene, CameraParameters, HlrOptions, Scene2D};
use crate::geometry::geometrybuffer::GeometryBuffer;
use crate::spatial::placement::{self, Placement3D};

use openmaths::Vector3;
use uuid::Uuid;

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct OGCuboid {
    id: String,
    center: Vector3,
    width: f64,
    height: f64,
    depth: f64,
    placement: Placement3D,
    brep: Brep,
    geometry_buffer: GeometryBuffer,
}

#[wasm_bindgen]
impl OGCuboid {
    #[wasm_bindgen(setter)]
    pub fn set_id(&mut self, id: String) {
        self.id = id;
    }

    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.id.clone()
    }

    #[wasm_bindgen(constructor)]
    pub fn new(id: String) -> OGCuboid {
        let internal_id = Uuid::new_v4();

        OGCuboid {
            id,
            center: Vector3::new(0.0, 0.0, 0.0),
            width: 1.0,
            height: 1.0,
            depth: 1.0,
            placement: Placement3D::new(),
            brep: Brep::new(internal_id),
            geometry_buffer: GeometryBuffer::new(internal_id),
        }
    }

    #[wasm_bindgen]
    pub fn set_config(
        &mut self,
        center: Vector3,
        width: f64,
        height: f64,
        depth: f64,
    ) -> Result<(), JsValue> {
        self.center = center;
        self.width = width;
        self.height = height;
        self.depth = depth;

        self.placement.set_anchor(self.center);
        self.build_local_brep();
        self.build_geometry();
        Ok(())
    }

    #[wasm_bindgen]
    pub fn set_center(&mut self, center: Vector3) {
        self.center = center;
        self.placement.set_anchor(self.center);
    }

    #[wasm_bindgen]
    pub fn set_transform(&mut self, position: Vector3, rotation: Vector3, scale: Vector3) {
        self.placement.set_transform(position, rotation, scale);
    }

    #[wasm_bindgen]
    pub fn set_translation(&mut self, translation: Vector3) {
        self.placement.set_translation(translation);
    }

    #[wasm_bindgen]
    pub fn set_rotation(&mut self, rotation: Vector3) {
        self.placement.set_rotation(rotation);
    }

    #[wasm_bindgen]
    pub fn set_scale(&mut self, scale: Vector3) {
        self.placement.set_scale(scale);
    }

    #[wasm_bindgen]
    pub fn apply_placement_to_brep(&mut self) {
        self.brep.apply_transform(&self.placement);
    }

    pub fn apply_placement_to_geometry_buffer(&mut self) {
        self.geometry_buffer.apply_transform(&self.placement);
    }

    #[wasm_bindgen]
    pub fn generate_geometry(&mut self) -> Result<(), JsValue> {
        self.build_local_brep()?;
        self.build_geometry();
        Ok(())
    }

    #[wasm_bindgen]
    pub fn get_brep_serialized(&self) -> String {
        serde_json::to_string(&self.brep).unwrap()
    }

    #[wasm_bindgen]
    pub fn get_geometry_serialized(&self) -> String {
        serde_json::to_string(&self.geometry_buffer.vertices).unwrap()
    }

    #[wasm_bindgen]
    pub fn get_outline_geometry_serialized(&self) -> String {
        serde_json::to_string(&self.geometry_buffer.outline_vertices).unwrap()
    }

    #[wasm_bindgen(js_name = getAnchor)]
    pub fn get_anchor(&self) -> Vector3 {
        self.placement.anchor
    }

    // TODO: add dispose method to clean up resources
}

impl OGCuboid {
    fn build_local_brep(&mut self) -> Result<(), JsValue> {
        let half_width = self.width * 0.5;
        let half_height = self.height * 0.5;
        let half_depth = self.depth * 0.5;

        let vertices = vec![
            Vector3::new(-half_width, -half_height, -half_depth),
            Vector3::new(half_width, -half_height, -half_depth),
            Vector3::new(half_width, -half_height, half_depth),
            Vector3::new(-half_width, -half_height, half_depth),
            Vector3::new(-half_width, half_height, -half_depth),
            Vector3::new(half_width, half_height, -half_depth),
            Vector3::new(half_width, half_height, half_depth),
            Vector3::new(-half_width, half_height, half_depth),
        ];

        let faces = vec![
            vec![0, 1, 2, 3],
            vec![4, 7, 6, 5],
            vec![0, 4, 5, 1],
            vec![3, 2, 6, 7],
            vec![0, 3, 7, 4],
            vec![1, 5, 6, 2],
        ];

        let mut builder = BrepBuilder::new(self.brep.id);
        builder.add_vertices(&vertices);

        for face in &faces {
            builder.add_face(face, &[]).map_err(|error| {
                JsValue::from_str(&format!("Failed to build cuboid face: {}", error))
            })?;
        }

        builder.add_shell_from_all_faces(true).map_err(|error| {
            JsValue::from_str(&format!("Failed to build cuboid shell: {}", error))
        })?;

        self.brep = builder.build().map_err(|error| {
            JsValue::from_str(&format!("Failed to finalize cuboid BREP: {}", error))
        })?;
        Ok(())
    }

    /**
     * Builds the geometry buffer
     */
    fn build_geometry(&mut self) {
        self.geometry_buffer.vertices = self.brep.get_triangle_vertex_buffer();
        self.geometry_buffer.outline_vertices = self.brep.get_outline_vertex_buffer();
    }

    pub fn brep(&self) -> &Brep {
        &self.brep
    }
}
