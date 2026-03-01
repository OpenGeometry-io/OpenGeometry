use serde::{Deserialize, Serialize};
/**
 * Copyright (c) 2025, OpenGeometry. All rights reserved.
 * Arc Primitive for OpenGeometry.
 *
 * An Arc is a segment of a circle defined by a center, radius, start angle, end angle, and number of segments.
 * It can be used to create circular arcs in 3D space.
 * Created with a center, radius, start angle, end angle, and number of segments.
 **/
// TODO: What if we create the Circle using the Formula for Angles.
use wasm_bindgen::prelude::*;

use crate::brep::{Brep, Edge, Face, Vertex};
use crate::export::projection::{project_brep_to_scene, CameraParameters, HlrOptions, Scene2D};
use openmaths::Vector3;
use uuid::Uuid;

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct OGArc {
    id: String,
    center: Vector3,
    radius: f64,
    start_angle: f64,
    end_angle: f64,
    segments: u32,
    brep: Brep,
}

#[wasm_bindgen]
impl OGArc {
    #[wasm_bindgen(setter)]
    pub fn set_id(&mut self, id: String) {
        self.id = id;
    }

    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.id.clone()
    }

    #[wasm_bindgen(constructor)]
    pub fn new(id: String) -> OGArc {
        let internal_id = Uuid::new_v4();

        OGArc {
            id,
            center: Vector3::new(0.0, 0.0, 0.0),
            radius: 1.0,
            start_angle: 0.0,
            end_angle: 2.0 * std::f64::consts::PI,
            segments: 32,
            brep: Brep::new(internal_id),
        }
    }

    #[wasm_bindgen]
    pub fn set_config(
        &mut self,
        center: Vector3,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
        segments: u32,
    ) {
        self.center = center;
        self.radius = radius;
        self.start_angle = start_angle;
        self.end_angle = end_angle;
        self.segments = segments;
    }

    #[wasm_bindgen]
    pub fn generate_geometry(&mut self) {
        self.dispose_points();

        let segment_count = self.segments.max(1);
        let mut angle = self.start_angle;
        let angle_diff = (self.end_angle - self.start_angle) / segment_count as f64;

        for _ in 0..segment_count + 1 {
            let x = self.center.x + self.radius * angle.cos();
            let y = self.center.y;
            let z = self.center.z + self.radius * angle.sin();
            self.brep.vertices.push(Vertex::new(
                self.brep.get_vertex_count() as u32,
                Vector3::new(x, y, z),
            ));
            angle += angle_diff;
        }

        let is_closed =
            (self.end_angle - self.start_angle).abs() >= 2.0 * std::f64::consts::PI - 1.0e-9;
        let mut edge_vertex_count = self.brep.vertices.len();

        if is_closed && edge_vertex_count > 2 {
            let first = self.brep.vertices[0].position;
            let last = self.brep.vertices[edge_vertex_count - 1].position;
            let dx = first.x - last.x;
            let dy = first.y - last.y;
            let dz = first.z - last.z;
            let duplicate_end = dx * dx + dy * dy + dz * dz <= 1.0e-12;
            if duplicate_end {
                edge_vertex_count -= 1;
            }
        }

        if edge_vertex_count < 2 {
            return;
        }

        for i in 0..(edge_vertex_count - 1) {
            self.brep.edges.push(Edge::new(
                self.brep.get_edge_count(),
                i as u32,
                (i + 1) as u32,
            ));
        }

        if is_closed && edge_vertex_count > 2 {
            self.brep.edges.push(Edge::new(
                self.brep.get_edge_count(),
                (edge_vertex_count - 1) as u32,
                0,
            ));

            let face_indices: Vec<u32> = (0..edge_vertex_count as u32).collect();
            self.brep.faces.push(Face::new(0, face_indices));
        }
    }

    // Dispose
    #[wasm_bindgen]
    pub fn dispose_points(&mut self) {
        self.brep.clear();
    }

    // Destroy and Free memory
    #[wasm_bindgen]
    pub fn destroy(&mut self) {
        self.brep.clear();
        self.id.clear();
    }

    #[wasm_bindgen]
    pub fn get_brep_serialized(&self) -> String {
        let serialized = serde_json::to_string(&self.brep).unwrap();
        serialized
    }

    // TODO: For Line based primitives we are iterating just vertices
    // Figure out if it's benefical to create a edges and faces for Arc as well - Technically it's not needed
    #[wasm_bindgen]
    pub fn get_geometry_serialized(&mut self) -> String {
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
}

impl OGArc {
    pub fn brep(&self) -> &Brep {
        &self.brep
    }

    pub fn to_projected_scene2d(&self, camera: &CameraParameters, hlr: &HlrOptions) -> Scene2D {
        project_brep_to_scene(&self.brep, camera, hlr)
    }
}
