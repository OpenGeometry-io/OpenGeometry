/**
 * Copyright (c) 2025, OpenGeometry. All rights reserved.
 * Polyline Primitive for OpenGeometry.
 *
 * A Polyline is a connected sequence of line segments.
 * It can be open or closed, and is defined by a series of points.
 */
use crate::brep::{Brep, Edge, Face, Vertex};
use crate::export::projection::{project_brep_to_scene, CameraParameters, HlrOptions, Scene2D};
use openmaths::Vector3;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct OGPolyline {
    id: String,
    // TODO: Figure out if we can solely rely on Brep for points
    points: Vec<Vector3>,
    is_closed: bool,
    brep: Brep,
}

impl OGPolyline {
    pub fn brep(&self) -> &Brep {
        &self.brep
    }

    pub fn to_projected_scene2d(&self, camera: &CameraParameters, hlr: &HlrOptions) -> Scene2D {
        project_brep_to_scene(&self.brep, camera, hlr)
    }
}

// TODO: Implement Drop for all Primitives
impl Drop for OGPolyline {
    fn drop(&mut self) {
        self.points.clear();
        self.is_closed = false;
        self.brep.clear();
        self.id.clear();
    }
}

#[wasm_bindgen]
impl OGPolyline {
    #[wasm_bindgen(setter)]
    pub fn set_id(&mut self, id: String) {
        self.id = id;
    }

    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.id.clone()
    }

    #[wasm_bindgen(constructor)]
    pub fn new(id: String) -> OGPolyline {
        OGPolyline {
            id,
            points: Vec::new(),
            is_closed: false,
            brep: Brep::new(Uuid::new_v4()),
        }
    }

    #[wasm_bindgen]
    pub fn clone(&self) -> OGPolyline {
        OGPolyline {
            id: self.id.clone(),
            points: self.points.clone(),
            is_closed: self.is_closed,
            brep: self.brep.clone(),
        }
    }

    // #[wasm_bindgen]
    // pub fn translate(&mut self, translation: Vector3) {

    //   self.points.clear();

    //   for i in 0..self.backup_points.len() {
    //     let point = &mut self.backup_points[i].clone();
    //     point.x += translation.x;
    //     point.y += translation.y;
    //     point.z += translation.z;

    //     self.points.push(point.clone());
    //     self.brep.vertices.push(point.clone());
    //   }

    //   self.check_closed_test();
    //   self.generate_brep();
    // }

    // #[wasm_bindgen]
    // pub fn set_position(&mut self, position: Vector3) {
    //   self.position = position;
    // }

    pub fn set_config(&mut self, points: Vec<Vector3>) {
        self.points.clear();

        for point in points {
            self.points.push(point);
        }

        self.check_closed_test();
        self.generate_geometry();
    }

    #[wasm_bindgen]
    pub fn generate_geometry(&mut self) {
        self.brep.clear();
        if self.points.is_empty() {
            return;
        }

        let mut effective_len = self.points.len();
        if self.is_closed && self.points.len() > 2 {
            let first = self.points[0];
            let last = self.points[self.points.len() - 1];
            let dx = first.x - last.x;
            let dy = first.y - last.y;
            let dz = first.z - last.z;
            let duplicate_end = dx * dx + dy * dy + dz * dz <= 1.0e-12;
            if duplicate_end {
                effective_len -= 1;
            }
        }

        for i in 0..effective_len {
            self.brep.vertices.push(Vertex::new(i as u32, self.points[i]));
        }

        if effective_len < 2 {
            return;
        }

        for i in 0..(effective_len - 1) {
            self.brep
                .edges
                .push(Edge::new(self.brep.get_edge_count(), i as u32, (i + 1) as u32));
        }

        if self.is_closed && effective_len > 2 {
            self.brep.edges.push(Edge::new(
                self.brep.get_edge_count(),
                (effective_len - 1) as u32,
                0,
            ));

            let face_indices: Vec<u32> = (0..effective_len as u32).collect();
            self.brep.faces.push(Face::new(0, face_indices));
        }
    }

    #[wasm_bindgen]
    pub fn add_multiple_points(&mut self, points: Vec<Vector3>) {
        self.points.clear();

        for point in points {
            self.points.push(point);
        }

        self.check_closed_test();
        self.generate_geometry();
    }

    #[wasm_bindgen]
    pub fn add_point(&mut self, point: Vector3) {
        self.points.push(point);
        self.check_closed_test();
        self.generate_geometry();
    }

    // Get Points for the Circle
    #[wasm_bindgen]
    pub fn get_points(&self) -> String {
        serde_json::to_string(&self.points).unwrap()
    }

    pub fn get_raw_points(&self) -> Vec<Vector3> {
        self.points.clone()
    }

    #[wasm_bindgen]
    pub fn is_closed(&self) -> bool {
        self.is_closed
    }

    // Simple Check to see if the Polyline is closed
    // This can be made better
    pub fn check_closed_test(&mut self) {
        self.is_closed = false;
        if self.points.len() > 2 {
            if self.points[0].x == self.points[self.points.len() - 1].x
                && self.points[0].y == self.points[self.points.len() - 1].y
                && self.points[0].z == self.points[self.points.len() - 1].z
            {
                self.is_closed = true;
            }
        }
    }

    #[wasm_bindgen]
    pub fn get_brep_serialized(&self) -> String {
        let serialized = serde_json::to_string(&self.brep).unwrap();
        serialized
    }

    #[wasm_bindgen]
    pub fn get_geometry_serialized(&self) -> String {
        let mut vertex_buffer: Vec<f64> = Vec::new();

        let vertices = self.brep.vertices.clone();
        for vertex in vertices {
            vertex_buffer.push(vertex.position.x);
            vertex_buffer.push(vertex.position.y);
            vertex_buffer.push(vertex.position.z);
        }

        let vertex_serialized = serde_json::to_string(&vertex_buffer).unwrap();
        vertex_serialized
    }

    // // Paper - https://seant23.wordpress.com/wp-content/uploads/2010/11/anoffsetalgorithm.pdf
    // // Paper has coverage for curves as well, but we will only implement for polylines
    // #[wasm_bindgen]
    // pub fn get_offset(&self, distance: f64) -> String {
    //   let n = self.points.len();
    //   if n < 2 {
    //       return serde_json::to_string(&Vec::<Vector3>::new()).unwrap();
    //   }

    //   let mut offset_points = Vec::new();

    //   for i in 0..n {
    //     let mut prev = if i == 0 {
    //       self.points[i]
    //     } else {
    //       self.points[i - 1]
    //     };
    //     let mut curr = self.points[i];
    //     let mut next = if i == n - 1 {
    //       self.points[i]
    //     } else {
    //       self.points[i + 1]
    //     };

    //     let v1 = curr.subtract(&prev).normalize();
    //     let v2 = next.subtract(&curr).normalize();

    //     let mut perp1 = Vector3 { x: -v1.z, y: 0.0, z: v1.x };
    //     let mut perp2 = Vector3 { x: -v2.z, y: 0.0, z: v2.x };

    //     let offset_point = if i == 0 {
    //       // Start point: move perpendicular to first segment
    //       curr.add(&perp2.multiply_scalar(distance))
    //     } else if i == n - 1 {
    //       // End point: move perpendicular to last segment
    //       curr.add(&perp1.multiply_scalar(distance))
    //     } else {
    //       // Middle: compute bisector intersection
    //       let a1 = prev.add(&perp1.multiply_scalar(distance));
    //       let a2 = curr.add(&perp1.multiply_scalar(distance));
    //       let b1 = curr.add(&perp2.multiply_scalar(distance));
    //       let b2 = next.add(&perp2.multiply_scalar(distance));

    //       Self::calculate_2D_interesection(&a1, &a2, &b1, &b2)
    //           .unwrap_or(curr.add(&perp1.multiply_scalar(distance)))
    //     };

    //     offset_points.push(offset_point);
    //   }

    //   let ccw_test = windingsort::is_ccw_need(offset_points.clone());

    //   let data = Data {
    //     untreated: offset_points.clone(),
    //     treated: ccw_test.ccw.clone(),
    //     flag: ccw_test.flag,
    //   };

    //   serde_json::to_string(&data).unwrap()
    // }

    // pub fn calculate_2D_interesection(
    //   point_a: &Vector3,
    //   point_b: &Vector3,
    //   point_c: &Vector3,
    //   point_d: &Vector3,
    // ) -> Option<Vector3> {
    //   let dx1 = point_b.x - point_a.x;
    //   let dz1 = point_b.z - point_a.z;
    //   let dx2 = point_d.x - point_c.x;
    //   let dz2 = point_d.z - point_c.z;

    //   let det = dx1 * dz2 - dz1 * dx2;
    //   if det.abs() < 1e-8 {
    //     return None; // Parallel lines
    //   }

    //   let dx = point_c.x - point_a.x;
    //   let dz = point_c.z - point_a.z;

    //   let t = (dx * dz2 - dz * dx2) / det;

    //   Some(Vector3 {
    //     x: point_a.x + t * dx1,
    //     y: 0.0,
    //     z: point_a.z + t * dz1,
    //   })
    // }
}
