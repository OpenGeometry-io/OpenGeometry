use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::brep::{Brep, BrepBuilder};
use crate::export::projection::{project_brep_to_scene, CameraParameters, HlrOptions, Scene2D};
use crate::geometry::geometrybuffer::GeometryBuffer;
use crate::spatial::placement::Placement3D;

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
        self.build_local_brep()?;
        self.build_geometry();
        Ok(())
    }

    #[wasm_bindgen]
    pub fn set_center(&mut self, center: Vector3) {
        self.center = center;
        self.placement.set_anchor(self.center);
    }

    #[wasm_bindgen]
    pub fn set_transform(
        &mut self,
        position: Vector3,
        rotation: Vector3,
        scale: Vector3,
    ) -> Result<(), JsValue> {
        self.placement
            .set_transform(position, rotation, scale)
            .map_err(|err| JsValue::from_str(&err))
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
    pub fn set_scale(&mut self, scale: Vector3) -> Result<(), JsValue> {
        self.placement
            .set_scale(scale)
            .map_err(|err| JsValue::from_str(&err))
    }

    #[wasm_bindgen]
    pub fn generate_geometry(&mut self) -> Result<(), JsValue> {
        self.build_local_brep()?;
        self.build_geometry();
        Ok(())
    }

    #[wasm_bindgen]
    pub fn get_brep_serialized(&self) -> String {
        serde_json::to_string(&self.world_brep()).unwrap()
    }

    #[wasm_bindgen]
    pub fn get_local_brep_serialized(&self) -> String {
        serde_json::to_string(&self.brep).unwrap()
    }

    #[wasm_bindgen]
    pub fn get_geometry_serialized(&self) -> String {
        serde_json::to_string(&self.world_geometry_buffer().vertices).unwrap()
    }

    #[wasm_bindgen]
    pub fn get_local_geometry_serialized(&self) -> String {
        serde_json::to_string(&self.geometry_buffer.vertices).unwrap()
    }

    #[wasm_bindgen]
    pub fn get_geometry_buffer(&self) -> Vec<f64> {
        self.world_geometry_buffer().vertices
    }

    #[wasm_bindgen]
    pub fn get_local_geometry_buffer(&self) -> Vec<f64> {
        self.geometry_buffer.vertices.clone()
    }

    #[wasm_bindgen]
    pub fn get_outline_geometry_serialized(&self) -> String {
        serde_json::to_string(&self.world_geometry_buffer().outline_vertices).unwrap()
    }

    #[wasm_bindgen]
    pub fn get_local_outline_geometry_serialized(&self) -> String {
        serde_json::to_string(&self.geometry_buffer.outline_vertices).unwrap()
    }

    #[wasm_bindgen]
    pub fn get_outline_geometry_buffer(&self) -> Vec<f64> {
        self.world_geometry_buffer().outline_vertices
    }

    #[wasm_bindgen]
    pub fn get_local_outline_geometry_buffer(&self) -> Vec<f64> {
        self.geometry_buffer.outline_vertices.clone()
    }

    #[wasm_bindgen]
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

    pub fn world_brep(&self) -> Brep {
        self.brep.transformed(&self.placement)
    }

    pub fn world_geometry_buffer(&self) -> GeometryBuffer {
        self.geometry_buffer.transformed(&self.placement)
    }

    pub fn to_projected_scene2d(&self, camera: &CameraParameters, hlr: &HlrOptions) -> Scene2D {
        project_brep_to_scene(&self.world_brep(), camera, hlr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f64, expected: f64) {
        let delta = (actual - expected).abs();
        assert!(
            delta <= 1.0e-9,
            "expected {expected}, got {actual}, delta {delta}"
        );
    }

    #[test]
    fn placement_world_getters_transform_geometry_and_leave_local_buffers_unchanged() {
        let mut cuboid = OGCuboid::new("placement-cuboid".to_string());
        cuboid
            .set_config(Vector3::new(10.0, 20.0, 30.0), 2.0, 4.0, 6.0)
            .expect("cuboid config");

        let local_geometry = cuboid.get_local_geometry_serialized();
        let local_outline = cuboid.get_local_outline_geometry_serialized();
        let local_brep = cuboid.get_local_brep_serialized();

        cuboid
            .set_transform(
                Vector3::new(1.0, -2.0, 3.0),
                Vector3::new(0.25, 0.5, -0.35),
                Vector3::new(1.5, 1.5, 1.5),
            )
            .expect("uniform positive scale should be accepted");

        assert_eq!(local_brep, cuboid.get_local_brep_serialized());
        assert_eq!(local_geometry, cuboid.get_local_geometry_serialized());
        assert_eq!(
            local_outline,
            cuboid.get_local_outline_geometry_serialized()
        );

        let expected_world_geometry = cuboid.geometry_buffer.transformed(&cuboid.placement);

        let world_geometry: Vec<f64> =
            serde_json::from_str(&cuboid.get_geometry_serialized()).expect("world geometry");
        let world_outline: Vec<f64> =
            serde_json::from_str(&cuboid.get_outline_geometry_serialized()).expect("world outline");

        assert_eq!(world_geometry.len(), expected_world_geometry.vertices.len());
        for (actual, expected) in world_geometry
            .iter()
            .zip(expected_world_geometry.vertices.iter())
        {
            assert_close(*actual, *expected);
        }

        assert_eq!(
            world_outline.len(),
            expected_world_geometry.outline_vertices.len()
        );
        for (actual, expected) in world_outline
            .iter()
            .zip(expected_world_geometry.outline_vertices.iter())
        {
            assert_close(*actual, *expected);
        }

        let world_brep: Brep =
            serde_json::from_str(&cuboid.get_brep_serialized()).expect("world brep");
        let world_center = world_brep.bounds_center().expect("world bounds");
        assert_close(world_center.x, 11.0);
        assert_close(world_center.y, 18.0);
        assert_close(world_center.z, 33.0);
    }

    #[test]
    fn placement_rejects_mirrored_and_non_uniform_scale() {
        let mut cuboid = OGCuboid::new("placement-rejection-cuboid".to_string());
        cuboid
            .set_config(Vector3::new(0.0, 0.0, 0.0), 1.0, 1.0, 1.0)
            .expect("cuboid config");

        let mirrored = cuboid.placement.set_transform(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(-1.0, -1.0, -1.0),
        );
        assert!(mirrored.is_err());

        let non_uniform = cuboid.placement.set_transform(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 1.2, 1.0),
        );
        assert!(non_uniform.is_err());
    }
}
