/**
 * Copyright (c) 2025, OpenGeometry. All rights reserved.
 * Polygon Primitive for OpenGeometry.
 * 
 * Base polygon created by default on XY plane with no vertices.
 * Polygon Points should be in CCW order based on viewing Axis
 * Holes should be in CW order
 */
use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};

use crate::brep::{Brep, Edge, Face, Vertex};
use crate::drawing::{Path2D, Vec2};
use crate::operations::triangulate::triangulate_polygon_with_holes;
use crate::utility::bgeometry::BufferGeometry;
use openmaths::{Matrix4, Vector3};
use uuid::Uuid;

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct OGPolygon {
  id: String,
  // geometry: basegeometry::BaseGeometry,
  // pub extruded: bool,
  // pub extruded_height: f64,
  // pub is_polygon: bool,
  // pub position: Vector3,
  // pub rotation: Vector3,
  // pub scale: Vector3,
  points: Vec<Vector3>,
  geometry: BufferGeometry,
  brep: Brep
}

// TODO: Implement Drop for all Primitives
// impl Drop for OGPolygon {
//   fn drop(&mut self) {
//     self.buffer.clear();
//     self.geometry.reset_geometry();
//     self.variable_geometry.reset_geometry();
//     self.brep.clear();
//     web_sys::console::log_1(&format!("Clearing Polygon with ID: {}", self.id).into());
//   }
// }

#[wasm_bindgen]
impl OGPolygon {
  // Why Getter and Setter - https://github.com/rustwasm/wasm-bindgen/issues/1775
  #[wasm_bindgen(setter)]
  pub fn set_id(&mut self, id: String) {
    self.id = id;
  }

  #[wasm_bindgen(getter)]
  pub fn id(&self) -> String {
    self.id.clone()
  }

  // Add the ability to create polygon with list of verticies passed in constructor itself
  // as of now use add_vertices method to push all vertices at once
  #[wasm_bindgen(constructor)]
  pub fn new(id: String) -> OGPolygon {

    let internal_id = Uuid::new_v4();

    OGPolygon {
      id: id.clone(),
      points: Vec::new(),
      geometry: BufferGeometry::new(internal_id),
      brep: Brep::new(internal_id),
    }
  }

  #[wasm_bindgen]
  pub fn set_config(&mut self, points: Vec<Vector3>) {
    self.points = points;

    self.generate_brep();
  }

  #[wasm_bindgen]
  pub fn set_transformation(&mut self, transformation: Vec<f64>) {
    if transformation.len() != 16 {
      web_sys::console::log_1(&"Transformation matrix must have 16 elements.".into());
      return;
    }

    // Set the transformation matrix in the geometry
    let transformation_matrix: Matrix4 = Matrix4::set(
      transformation[0], transformation[4], transformation[8], transformation[12],
      transformation[1], transformation[5], transformation[9], transformation[13],
      transformation[2], transformation[6], transformation[10], transformation[14],
      transformation[3], transformation[7], transformation[11], transformation[15],
    );

    for i in 0..self.points.len() {
      let mut point = self.points[i].clone();
      point.apply_matrix4(transformation_matrix.clone());
      self.points[i] = point;
    }

    // Clean BREP data structure
    // self.brep.clear();
    // self.generate_geometry();
  }

  // TODO: Implement Translate, Rotate, Scale methods
  // #[wasm_bindgen]
  // pub fn translate(&mut self, translation: Vector3) {
  //   self.position.x += translation.x;
  //   self.position.y += translation.y;
  //   self.position.z += translation.z;

  //   self.geometry.translate(translation);

  //   // TODO: Variable Geometry is used for triangulation with holes - Later
  //   // self.variable_geometry.translate(translation);
  // }

  // #[wasm_bindgen]
  // pub fn set_position(&mut self, position: openmath::Vector3D) {
  //   self.position = position;
  //   self.geometry.set_position(position);
  // }

  // #[wasm_bindgen]
  // pub fn new_with_circle(circle_arc: primitives::circle::CircleArc) -> OGPolygon {
  //   let mut polygon = OGPolygon::new(circle_arc.id());
  //   // discard the last point as it is same as the first point
  //   let mut circle_arc_points = circle_arc.get_raw_points();
  //   circle_arc_points.pop();
  //   circle_arc_points.reverse();
  //   polygon.add_vertices(circle_arc_points);
  //   polygon.triangulate();
  //   polygon
  // }

  // #[wasm_bindgen]
  // pub fn new_with_rectangle(rectangle: primitives::rectangle::OGRectangle) -> OGPolygon {
  //   let mut polygon = OGPolygon::new(rectangle.id());
  //   // discard the last point as it is same as the first point
  //   let mut rectangle_points = rectangle.get_raw_points();
  //   rectangle_points.pop();
  //   polygon.add_vertices(rectangle_points);
  //   polygon.triangulate();
  //   polygon
  // }

  // Add Set of new Vertices to the polygon
  #[wasm_bindgen]
  pub fn add_vertices(&mut self, vertices: Vec<Vector3>) {
    self.points.clear();
    self.brep.clear();

    self.set_config(vertices.clone());
  }
  
  // #[wasm_bindgen]
  // pub fn add_vertex(&mut self, vertex: Vector3) {
  //   self.geometry.add_vertex(vertex);
    
  //   // If more than 3 vertices are added, then the polygon is created
  //   if self.geometry.get_vertices().len() > 2 {
  //     self.is_polygon = true;
  //   }
  // }

  #[wasm_bindgen]
  pub fn add_holes(&mut self, holes: Vec<Vector3>) {
    let current_index = self.brep.get_vertex_count();
    self.brep.holes.push(current_index);

    for (i, point) in holes.iter().enumerate() {
      let vertex = Vertex::new((current_index + i as u32) as u32, point.clone());
      self.brep.vertices.push(vertex);

      // TODO - Can we merge Face Edges with Hole Edges, would it be wise to do so?
      // For now, keep it separate
      let edge = {
        let holes_len_u32 = holes.len() as u32;
        vec![
          current_index + i as u32,
          current_index + ((i as u32 + 1) % holes_len_u32)
        ]
      };
      self.brep.hole_edges.push(Edge::new(
        self.brep.get_hole_edge_count() as u32,
        edge[0],
        edge[1],
      ));
    }
  }

  #[wasm_bindgen]
  pub fn clean_geometry(&mut self) {
    self.brep.clear();
    self.geometry.clear();
  }

  #[wasm_bindgen]
  pub fn generate_brep(&mut self) {
    if self.points.len() < 3 {
      web_sys::console::log_1(&"Polygon must have at least 3 points to generate geometry.".into());
      return;
    }
    
    // Clear the BREP structure before generating new geometry
    self.clean_geometry();

    // Create Face for the polygon
    self.brep.faces.push(Face::new(
      self.brep.get_face_count() as u32,
      Vec::new(),
    ));

    // Add vertices, edge indices and face indices to the BREP
    for (i, point) in self.points.iter().enumerate() {
      let vertex = Vertex::new(i as u32, point.clone());
      self.brep.vertices.push(vertex.clone());

      let edge = {
        vec![i as u32, ((i + 1) % self.points.len()) as u32]
      };

      self.brep.edges.push(Edge::new(
        self.brep.get_edge_count() as u32,
        edge[0],
        edge[1],
      ));

      self.brep.insert_vertex_at_face_by_id(0, vertex.id);
    }
  }

  #[wasm_bindgen]
  pub fn generate_geometry(&mut self) {
    if self.points.len() < 3 {
      web_sys::console::log_1(&"Polygon must have at least 3 points to generate geometry.".into());
      return;
    }
  }

  #[wasm_bindgen]
  pub fn get_brep_serialized(&self) -> String {
    let serialized = serde_json::to_string(&self.brep).unwrap();
    serialized
  }

  #[wasm_bindgen]
  pub fn get_geometry_serialized(&mut self) -> String {
    let mut vertex_buffer: Vec<f64> = Vec::new();
    let faces = self.brep.faces.clone();

    for face in &faces {
      let (face_vertices, holes_vertices) = self.brep.get_vertices_and_holes_by_face_id(face.id);

      if face_vertices.len() < 3 {
        continue;
      }

      let triangles = triangulate_polygon_with_holes(&face_vertices, &holes_vertices);

      // Combine outer and hole vertices into a single list for easy lookup
      let all_vertices: Vec<Vector3> = face_vertices
        .into_iter()
        .chain(holes_vertices.into_iter().flatten())
        .collect();

      // Build the final vertex buffer for rendering
      for triangle in triangles {
        for vertex_index in triangle {
          // The indices from earcutr correspond to our combined `all_vertices` list
          let vertex = &all_vertices[vertex_index];
          vertex_buffer.push(vertex.x);
          vertex_buffer.push(vertex.y);
          vertex_buffer.push(vertex.z);
        }
      }
    }

    serde_json::to_string(&vertex_buffer).unwrap()
  }

  #[wasm_bindgen]
  pub fn get_outline_geometry_serialized(&mut self) -> String {
    let mut vertex_buffer: Vec<f64> = Vec::new();

    let edges = self.brep.edges.clone();
    for edge in edges {
      let start_index = edge.v1 as usize;
      let end_index = edge.v2 as usize;

      let start_vertex = self.brep.vertices[start_index].clone();
      let end_vertex = self.brep.vertices[end_index].clone();

      vertex_buffer.push(start_vertex.position.x);
      vertex_buffer.push(start_vertex.position.y);
      vertex_buffer.push(start_vertex.position.z);

      vertex_buffer.push(end_vertex.position.x);
      vertex_buffer.push(end_vertex.position.y);
      vertex_buffer.push(end_vertex.position.z);
    }

    if self.brep.hole_edges.len() > 0 {
      for edge in self.brep.hole_edges.clone() {
        let start_index = edge.v1 as usize;
        let end_index = edge.v2 as usize;

        let start_vertex = self.brep.vertices[start_index].clone();
        let end_vertex = self.brep.vertices[end_index].clone();

        vertex_buffer.push(start_vertex.position.x);
        vertex_buffer.push(start_vertex.position.y);
        vertex_buffer.push(start_vertex.position.z);

        vertex_buffer.push(end_vertex.position.x);
        vertex_buffer.push(end_vertex.position.y);
        vertex_buffer.push(end_vertex.position.z);
      }
    }

    let vertex_serialized = serde_json::to_string(&vertex_buffer).unwrap();
    vertex_serialized
  }

  // pub fn extrude_by_height_with_holes(&mut self, height: f64) -> String {
  //   // Create a new buffer geometry
  //   // Loop through the extruded faces with holes
  //   // Create Variable Geometry
  //   // Call Triangulate with Holes with Custom Geometry method
  //   // push to the buffer geometry
  //   // Return the buffer geometry

  //   self.extruded = true;
  //   self.extruded_height = height;

  //   let mut extrude_data = extrude_polygon_with_holes(self.geometry.clone(), height);
    
  //   let mut local_geometry: Vec<f64> = Vec::new();
  //   let faces = extrude_data.faces.clone();
  //   let all_vertices_raw = extrude_data.vertices.clone();
  //   let holes = extrude_data.holes.clone();
  //   let face_holes_map = extrude_data.face_holes_map.clone();

  //   let face_length = face_holes_map.keys().len();
  //   // extrude_data.face_length = face_length;
  //   let mut face_current_index: usize = 0;
    
  //   let mut face_map: Vec<_> = face_holes_map.keys().cloned().collect();
  //   face_map.sort();

  //   for face_index in face_map {
  //     let mut variable_geometry: BaseGeometry = BaseGeometry::new("variable_geometry".to_string());
  //     let face = faces[face_index as usize].clone();
  //     let mut face_vertices: Vec<Vector3> = Vec::new();

  //     for index in face.clone() {
  //       let v_face = all_vertices_raw[index as usize].clone();
  //       face_vertices.push(v_face);
  //     }
  //     face_vertices.reverse();
  //     variable_geometry.add_vertices(face_vertices.clone());

  //     let mut is_ccw = false;
  //     if (face_current_index > face_length - 2) {
  //       is_ccw = true;
  //       extrude_data.is_ccw_last_face = true;
  //     }

  //     // get holes for given face
  //     let holes_for_face = face_holes_map.get(&face_index).unwrap();
  //     if holes_for_face.len() > 0 {
  //       for hole_index in holes_for_face {
  //         let hole = holes[*hole_index as usize].clone();
  //         variable_geometry.add_holes(hole.clone());
  //       }
  //       self.variable_geometry = variable_geometry.clone();
        
  //       // last most processed face
  //       extrude_data.face_length = face_index as usize;

  //       let triangle_data = self.triangulate_with_holes_variable_geometry(is_ccw);
  //       let triangulated_data: HashMap<String, Vec<f64>> = serde_json::from_str(&triangle_data).unwrap();
  //       let vertices = triangulated_data.get("new_buffer").unwrap();
  //       for i in vertices {
  //         local_geometry.push(*i);
  //       }
  //     }
  //     else {
  //       // If no holes, then just triangulate the face
  //       let triangulated_face = triangulate::triangulate_polygon_by_face(face_vertices.clone());
  //       for index in triangulated_face {
  //         for i in index {
  //           let vertex = face_vertices[i as usize].clone();
  //           local_geometry.push(vertex.x);
  //           local_geometry.push(vertex.y);
  //           local_geometry.push(vertex.z);
  //         }
  //       }
  //     }
      
  //     // Destroy the variable geometry
  //     variable_geometry.reset_geometry();

  //     face_current_index += 1;
  //   }

  //   // Add BREP
  //   let brep = Geometry {
  //     vertices: extrude_data.vertices.clone(),
  //     faces: extrude_data.faces.clone(),
  //     edges: extrude_data.edges.clone()
  //   };
  //   self.brep = brep.clone();

  //   // let extrude_data_string = serde_json::to_string(&extrude_data).unwrap();
  //   // extrude_data_string
    
  //   serde_json::to_string(&local_geometry).unwrap()
  // }

  // #[wasm_bindgen]
  // pub fn extrude_by_height(&mut self, height: f64) -> String {
  //   self.extruded = true;
  //   self.extruded_height = height;


  //   // If Polygon has holes, use other extrude method
  //   if (self.geometry.get_holes().len() > 0) {
  //     return self.extrude_by_height_with_holes(height);
  //   }

  //   let extrude_data = extrude_polygon_by_buffer_geometry(self.geometry.clone(), height);
  
  //   let mut local_geometry = Vec::new();
    
  //   // let face = extrude_data.faces[0].clone();
  //   for face in extrude_data.faces.clone() {
  //     let mut face_vertices: Vec<Vector3> = Vec::new();
  //     for index in face.clone() {
  //       face_vertices.push(extrude_data.vertices[index as usize].clone());
  //     }

  //     let triangulated_face = triangulate::triangulate_polygon_by_face(face_vertices.clone());
  //     // let ccw_vertices = windingsort::ccw_test(face_vertices.clone());
  //     for index in triangulated_face {
  //       for i in index {
  //         // let vertex = ccw_vertices[i as usize];
  //         let vertex = face_vertices[i as usize].clone();
  //         local_geometry.push(vertex.x);
  //         local_geometry.push(vertex.y);
  //         local_geometry.push(vertex.z);
  //       }
  //     }
  //   }
    
  //   // let face_data_string = serde_json::to_string(&face).unwrap(); // Serialize face_data
  //   // face_data_string

  //   // let extrude_data_string = serde_json::to_string(&extrude_data).unwrap(); // Serialize extrude_data
  //   // extrude_data_string

  //   let string_data = serde_json::to_string(&local_geometry).unwrap();
  //   string_data

  //   // ABOVE LINE WORKING

  //   // // TESTING EDGES OUTLINE
  //   // let mut outline_data: Vec<Vec<Vector3D>> = Vec::new();

  //   // // for edge in extruded_raw.edges {
  //   // //   let start = vertices[edge[0] as usize].clone();
  //   // //   let end = vertices[edge[1] as usize].clone();

  //   // //   let edge_vertices = vec![start, end];
  //   // //   outline_data.push(edge_vertices);
  //   // // }

  //   // for face in faces {
  //   //   let mut face_vertices: Vec<Vector3D> = Vec::new();
  //   //   for index in face {
  //   //     let v_face = vertices[index as usize].clone();
  //   //     face_vertices.push(v_face);
  //   //   }
  //   //   outline_data.push(face_vertices);
  //   // }

  //   // serde_json::to_string(&outline_data).unwrap()
  // }
}

/// Pure Rust methods for drawing/export (not exposed to WASM)
impl OGPolygon {
  /// Convert the polygon outline to a 2D path for export.
  /// Projects from 3D to 2D using the X-Z plane (ignores Y coordinate).
  /// Returns a closed path representing the polygon boundary.
  /// Note: Holes are not included in the main path - use to_path2d_with_holes for that.
  pub fn to_path2d(&self) -> Path2D {
    let mut path = Path2D::with_closed(true);
    
    if self.points.len() < 3 {
      return path;
    }
    
    // Convert polygon points to 2D and create line segments
    let points_2d: Vec<Vec2> = self.points
      .iter()
      .map(|p| Vec2::new(p.x, p.z))
      .collect();
    
    for i in 0..points_2d.len() {
      path.add_line(points_2d[i], points_2d[(i + 1) % points_2d.len()]);
    }
    
    path
  }
  
  /// Convert the polygon to a 2D path with custom projection.
  /// 
  /// # Arguments
  /// * `x_axis` - Which 3D axis becomes 2D X: 0 = X, 1 = Y, 2 = Z
  /// * `y_axis` - Which 3D axis becomes 2D Y: 0 = X, 1 = Y, 2 = Z
  pub fn to_path2d_with_projection(&self, x_axis: u8, y_axis: u8) -> Path2D {
    let mut path = Path2D::with_closed(true);
    
    if self.points.len() < 3 {
      return path;
    }
    
    let get_axis = |p: &Vector3, axis: u8| -> f64 {
      match axis {
        0 => p.x,
        1 => p.y,
        2 => p.z,
        _ => p.x,
      }
    };
    
    let points_2d: Vec<Vec2> = self.points
      .iter()
      .map(|p| Vec2::new(get_axis(p, x_axis), get_axis(p, y_axis)))
      .collect();
    
    for i in 0..points_2d.len() {
      path.add_line(points_2d[i], points_2d[(i + 1) % points_2d.len()]);
    }
    
    path
  }
  
  /// Convert the polygon outline AND holes to multiple 2D paths for export.
  /// The first path is the outer boundary, followed by paths for each hole.
  /// Projects from 3D to 2D using the X-Z plane (ignores Y coordinate).
  pub fn to_paths2d_with_holes(&self) -> Vec<Path2D> {
    let mut paths = Vec::new();
    
    // Add main polygon outline
    paths.push(self.to_path2d());
    
    // Add holes as separate paths
    if self.brep.holes.len() > 0 {
      let vertices = &self.brep.vertices;
      let hole_edges = &self.brep.hole_edges;
      
      // Group hole edges by their starting hole index
      let mut current_hole_path = Path2D::with_closed(true);
      let mut last_end_index: Option<u32> = None;
      
      for edge in hole_edges {
        // If this edge doesn't connect to the previous one, start a new hole path
        if let Some(last_end) = last_end_index {
          if edge.v1 != last_end {
            if current_hole_path.segment_count() > 0 {
              paths.push(current_hole_path);
            }
            current_hole_path = Path2D::with_closed(true);
          }
        }
        
        let start = &vertices[edge.v1 as usize].position;
        let end = &vertices[edge.v2 as usize].position;
        
        current_hole_path.add_line(
          Vec2::new(start.x, start.z),
          Vec2::new(end.x, end.z)
        );
        
        last_end_index = Some(edge.v2);
      }
      
      // Don't forget the last hole path
      if current_hole_path.segment_count() > 0 {
        paths.push(current_hole_path);
      }
    }
    
    paths
  }
}
