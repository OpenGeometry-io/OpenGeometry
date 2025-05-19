use std::collections::HashMap;

use crate::operations::extrude::{extrude_polygon_by_buffer_geometry, extrude_polygon_with_holes};
use crate::operations::triangulate::triangulate_polygon_buffer_geometry;
use crate::operations::windingsort;
use crate::{geometry, primitives};
use crate::utility::openmath::{Geometry, Vector3D};
use crate::{operations::triangulate, utility::openmath};
use crate::geometry::basegeometry::{self, BaseGeometry};
use serde_json::ser;
use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct OGPolygon {
  id: String,
  geometry: basegeometry::BaseGeometry,
  pub extruded: bool,
  pub extruded_height: f64,
  pub is_polygon: bool,
  pub position: openmath::Vector3D,
  pub rotation: openmath::Vector3D,
  pub scale: openmath::Vector3D,
  buffer: Vec<f64>,
  variable_geometry: basegeometry::BaseGeometry,
  brep: Geometry,
}

/**
 * A Polygon created with a `id` will have a BaseGeometry with same `id`. Feels like a good decision as of now.
 */

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
    OGPolygon {
      id: id.clone(),
      geometry : basegeometry::BaseGeometry::new(id.clone()),
      extruded : false,
      extruded_height : 0.0,
      is_polygon : false,
      position : openmath::Vector3D::create(0.0, 0.0, 0.0),
      rotation : openmath::Vector3D::create(0.0, 0.0, 0.0),
      scale : openmath::Vector3D::create(1.0, 1.0, 1.0),
      buffer : Vec::new(),
      variable_geometry: basegeometry::BaseGeometry::new(id.clone()),
      brep : Geometry::new(),
    }
  }

  #[wasm_bindgen]
  pub fn new_with_circle(circle_arc: primitives::circle::CircleArc) -> OGPolygon {
    let mut polygon = OGPolygon::new(circle_arc.id());
    // discard the last point as it is same as the first point
    let mut circle_arc_points = circle_arc.get_raw_points();
    circle_arc_points.pop();
    circle_arc_points.reverse();
    polygon.add_vertices(circle_arc_points);
    polygon.triangulate();
    polygon
  }

  #[wasm_bindgen]
  pub fn new_with_rectangle(rectangle: primitives::rectangle::OGRectangle) -> OGPolygon {
    let mut polygon = OGPolygon::new(rectangle.id());
    // discard the last point as it is same as the first point
    let mut rectangle_points = rectangle.get_raw_points();
    rectangle_points.pop();
    polygon.add_vertices(rectangle_points);
    polygon.triangulate();
    polygon
  }

  #[wasm_bindgen]
  pub fn add_vertices(&mut self, vertices: Vec<openmath::Vector3D>) {
    self.geometry.add_vertices(vertices);
  }
  
  #[wasm_bindgen]
  pub fn add_vertex(&mut self, vertex: openmath::Vector3D) {
    self.geometry.add_vertex(vertex);
    
    // If more than 3 vertices are added, then the polygon is created
    if self.geometry.get_vertices().len() > 2 {
      self.is_polygon = true;
    }
  }

  #[wasm_bindgen]
  pub fn add_holes(&mut self, holes: Vec<openmath::Vector3D>) {
    self.geometry.add_holes(holes);
  }

  #[wasm_bindgen]
  pub fn triangulate(&mut self) -> String {
    self.is_polygon = true;
    
    let indices = triangulate_polygon_buffer_geometry(self.geometry.clone());

    // This is important as the current vertices are not in the same order as the indices, Genius Vishwajeet
    let ccw_vertices = windingsort::ccw_test(self.geometry.get_vertices());
  
    // Should we do this? Store the ccw vertices in the geometry or we CCW the vertices every time we need to use them?    
    // self.geometry.add_vertices(ccw_vertices.clone());

    for index in indices {
      for i in index {
        let vertex = ccw_vertices[i as usize];
        // let vertex = self.geometry.get_vertices()[i as usize];
        self.buffer.push(vertex.x);
        self.buffer.push(vertex.y);
        self.buffer.push(vertex.z);
      }
    }
    serde_json::to_string(&self.buffer).unwrap()
  }

  #[wasm_bindgen]
  pub fn triangulate_with_holes_variable_geometry(&mut self, is_ccw: bool) -> String {
    // Step 1 - Flatten the geometry - Works
    let flat_data = triangulate::flatten_buffer_geometry(self.variable_geometry.clone());
    let vertices = flat_data.vertices.clone();
    let holes = flat_data.holes;
    let dimension = flat_data.dimension;

    // Step 2 - Find the left most point in the first hole
    let start_index_in_vertices = holes[0] * 3;
    let mut end_index_in_vertices = 0;

    if holes.len() > 1 {
      end_index_in_vertices = holes[1] * 3;
    } else {
      // if only one hole is present
      end_index_in_vertices = vertices.len() as u32;
    }

    let right_most_index = triangulate::find_right_most_point_index(vertices.clone(), start_index_in_vertices, end_index_in_vertices);

    // Step 3 - Find Ray Casting with the outer edges
    let right_point = Vector3D::create(
      vertices[right_most_index as usize],
      vertices[right_most_index as usize + 1],
      vertices[right_most_index as usize + 2]
    );
    
    // Step 4 is inside
    let ray_edge = triangulate::check_vertex_collision_with_flat_vertices(
      vertices.clone(),
      right_point,
      0,
      start_index_in_vertices + 1
    );

    let mut new_vertices_processed: Vec<f64> = Vec::new();
    // Step 5 - Create Bridge
    let bridge_start_index = ray_edge[0][0];
    let bridge_end_index = right_most_index;
    let bridge_start = Vector3D::create(
      vertices[bridge_start_index as usize],
      vertices[bridge_start_index as usize + 1],
      vertices[bridge_start_index as usize + 2]
    );
    let bridge_end = Vector3D::create(
      vertices[bridge_end_index as usize],
      vertices[bridge_end_index as usize + 1],
      vertices[bridge_end_index as usize + 2]
    );

    let hole_one = self.variable_geometry.get_holes()[0].clone();
    let hole_vertex_nodes = triangulate::create_vertex_nodes(hole_one.clone(), start_index_in_vertices, end_index_in_vertices - 1, true);

    // Before Bridge Vertices
    for i in 0..bridge_start_index+3 {
      let vertex = vertices[i as usize];
      new_vertices_processed.push(vertex);
    }
    // Insert Bridge Vertices
    let mut vertex_nodes_data: Vec<f64> = Vec::new();
    let mut vertex_next_nodes_data: Vec<f64> = Vec::new();
    for node in hole_vertex_nodes {
      vertex_nodes_data.push(node.vertex.x);
      vertex_nodes_data.push(node.vertex.y);
      vertex_nodes_data.push(node.vertex.z);

      vertex_next_nodes_data.push(node.next_index as f64);
    }
    for i in vertex_next_nodes_data.iter() {
      let index = *i as usize;
      let x = vertices[index];
      let y = vertices[index + 1];
      let z = vertices[index + 2];

      new_vertices_processed.push(x);
      new_vertices_processed.push(y);
      new_vertices_processed.push(z);
    }
    // Start Index Of Hole-Bridge again to complete the loop, i.e. vertex_next_nodes
    let start_index_of_bridge_to_hole = vertex_next_nodes_data[0] as usize;
    let bridge_to_hole_x = vertices[start_index_of_bridge_to_hole];
    let bridge_to_hole_y = vertices[start_index_of_bridge_to_hole + 1];
    let bridge_to_hole_z = vertices[start_index_of_bridge_to_hole + 2];
    new_vertices_processed.push(bridge_to_hole_x);
    new_vertices_processed.push(bridge_to_hole_y);
    new_vertices_processed.push(bridge_to_hole_z);
    // // Back To Bridge
    let bridge_x = vertices[bridge_start_index as usize];
    let bridge_y = vertices[bridge_start_index as usize + 1];
    let bridge_z = vertices[bridge_start_index as usize + 2];
    new_vertices_processed.push(bridge_x);
    new_vertices_processed.push(bridge_y);
    new_vertices_processed.push(bridge_z);
    // After Bridge Vertices
    let before_hole_start_index = holes[0] * 3;
    for i in bridge_start_index..before_hole_start_index as u32 {
      let vertex = vertices[i as usize];
      new_vertices_processed.push(vertex);
    }
    let mut new_buffergeometry = basegeometry::BaseGeometry::new("new_buffergeometry".to_string());
    let og_vertices: Vec<Vector3D> = new_vertices_processed.chunks(3)
      .map(|chunk| Vector3D::create(chunk[0], chunk[1], chunk[2]))
      .collect();
    new_buffergeometry.add_vertices(og_vertices);
    let new_tricut = triangulate_polygon_buffer_geometry(new_buffergeometry.clone());
    // let ccw_vertices = windingsort::ccw_test(new_buffergeometry.get_vertices());
    let mut new_buffer: Vec<f64> = Vec::new();
    for index in new_tricut {
      for i in index {
        // let vertex = ccw_vertices[i as usize];
        let vertex = new_buffergeometry.get_vertices()[i as usize];
        new_buffer.push(vertex.x);
        new_buffer.push(vertex.y);
        new_buffer.push(vertex.z);
      }
    }

    let mut reverse_triangles:Vec<f64> = Vec::new();
    if is_ccw {
      // read the triangles in reverse order but in start from end - 3 to end then push them and 
      let mut rev_buf = new_buffer.clone();
      rev_buf.reverse();
      let index = 3;
      let mut i = 0;

      while i < rev_buf.len() {
        let x = rev_buf[i + 2];
        let y = rev_buf[i + 1];
        let z = rev_buf[i];
        reverse_triangles.push(x);
        reverse_triangles.push(y);
        reverse_triangles.push(z);

        i += index;
      }

      new_buffer = reverse_triangles;
    }

    let mut data = HashMap::new();
    data.insert("vertices", vertices);
    data.insert("holes", holes.into_iter().map(|x| x as f64).collect());
    data.insert("dimension", vec![dimension as f64]);
    data.insert("start_index_in_vertices", vec![start_index_in_vertices as f64]);
    data.insert("end_index_in_vertices", vec![end_index_in_vertices as f64]);
    data.insert("right_most_index", vec![right_most_index as f64]);
    data.insert("right_point", vec![right_point.x, right_point.y, right_point.z]);
    data.insert("bridge_start", vec![bridge_start.x, bridge_start.y, bridge_start.z]);
    data.insert("bridge_end", vec![bridge_end.x, bridge_end.y, bridge_end.z]);
    data.insert("new_vertices_processed", new_vertices_processed);
    data.insert("vertex_nodes", vertex_nodes_data);
    data.insert("vertex_next_nodes", vertex_next_nodes_data);
    data.insert("new_buffer", new_buffer);
    let mut edge_data: Vec<f64> = Vec::new();
    for edge in ray_edge {
      for i in edge {
        edge_data.push(i as f64);
      }
    }
    data.insert("ray_edge", edge_data);
    let string_data = serde_json::to_string(&data).unwrap();
    string_data
  }

  #[wasm_bindgen]
  pub fn triangulate_with_holes(&mut self) -> String {
    // Step 1 - Flatten the geometry - Works
    let flat_data = triangulate::flatten_buffer_geometry(self.geometry.clone());
    let vertices = flat_data.vertices.clone();
    let holes = flat_data.holes;
    let dimension = flat_data.dimension;

    // Step 2 - Find the left most point in the first hole
    let start_index_in_vertices = holes[0] * 3;
    let mut end_index_in_vertices = 0;

    if holes.len() > 1 {
      end_index_in_vertices = holes[1] * 3;
    } else {
      // if only one hole is present
      end_index_in_vertices = vertices.len() as u32;
    }

    let right_most_index = triangulate::find_right_most_point_index(vertices.clone(), start_index_in_vertices, end_index_in_vertices);

    // Step 3 - Find Ray Casting with the outer edges
    let right_point = Vector3D::create(
      vertices[right_most_index as usize],
      vertices[right_most_index as usize + 1],
      vertices[right_most_index as usize + 2]
    );
    
    // Step 4 is inside
    let ray_edge = triangulate::check_vertex_collision_with_flat_vertices(
      vertices.clone(),
      right_point,
      0,
      start_index_in_vertices + 1
    );

    let mut new_vertices_processed: Vec<f64> = Vec::new();
    // Step 5 - Create Bridge
    let bridge_start_index = ray_edge[0][0];
    let bridge_end_index = right_most_index;
    let bridge_start = Vector3D::create(
      vertices[bridge_start_index as usize],
      vertices[bridge_start_index as usize + 1],
      vertices[bridge_start_index as usize + 2]
    );
    let bridge_end = Vector3D::create(
      vertices[bridge_end_index as usize],
      vertices[bridge_end_index as usize + 1],
      vertices[bridge_end_index as usize + 2]
    );

    let hole_one = self.geometry.get_holes()[0].clone();
    let hole_vertex_nodes = triangulate::create_vertex_nodes(hole_one.clone(), start_index_in_vertices, end_index_in_vertices - 1, true);

    // Before Bridge Vertices
    for i in 0..bridge_start_index+3 {
      let vertex = vertices[i as usize];
      new_vertices_processed.push(vertex);
    }

    // Insert Bridge Vertices
    let mut vertex_nodes_data: Vec<f64> = Vec::new();
    let mut vertex_next_nodes_data: Vec<f64> = Vec::new();
    for node in hole_vertex_nodes {
      vertex_nodes_data.push(node.vertex.x);
      vertex_nodes_data.push(node.vertex.y);
      vertex_nodes_data.push(node.vertex.z);

      vertex_next_nodes_data.push(node.next_index as f64);
    }
    for i in vertex_next_nodes_data.iter() {
      let index = *i as usize;
      let x = vertices[index];
      let y = vertices[index + 1];
      let z = vertices[index + 2];

      new_vertices_processed.push(x);
      new_vertices_processed.push(y);
      new_vertices_processed.push(z);
    }

    // Start Index Of Hole-Bridge again to complete the loop, i.e. vertex_next_nodes
    let start_index_of_bridge_to_hole = vertex_next_nodes_data[0] as usize;
    let bridge_to_hole_x = vertices[start_index_of_bridge_to_hole];
    let bridge_to_hole_y = vertices[start_index_of_bridge_to_hole + 1];
    let bridge_to_hole_z = vertices[start_index_of_bridge_to_hole + 2];
    new_vertices_processed.push(bridge_to_hole_x);
    new_vertices_processed.push(bridge_to_hole_y);
    new_vertices_processed.push(bridge_to_hole_z);

    // // Back To Bridge
    let bridge_x = vertices[bridge_start_index as usize];
    let bridge_y = vertices[bridge_start_index as usize + 1];
    let bridge_z = vertices[bridge_start_index as usize + 2];
    new_vertices_processed.push(bridge_x);
    new_vertices_processed.push(bridge_y);
    new_vertices_processed.push(bridge_z);

    // After Bridge Vertices
    let before_hole_start_index = holes[0] * 3;
    for i in bridge_start_index..before_hole_start_index as u32 {
      let vertex = vertices[i as usize];
      new_vertices_processed.push(vertex);
    }

    let mut new_buffergeometry = basegeometry::BaseGeometry::new("new_buffergeometry".to_string());
    let og_vertices: Vec<Vector3D> = new_vertices_processed.chunks(3)
      .map(|chunk| Vector3D::create(chunk[0], chunk[1], chunk[2]))
      .collect();
    new_buffergeometry.add_vertices(og_vertices);
    let new_tricut = triangulate_polygon_buffer_geometry(new_buffergeometry.clone());
    let ccw_vertices = windingsort::ccw_test(new_buffergeometry.get_vertices());
    let mut new_buffer: Vec<f64> = Vec::new();
    
    for index in new_tricut {
      for i in index {
        let vertex = ccw_vertices[i as usize];
        new_buffer.push(vertex.x);
        new_buffer.push(vertex.y);
        new_buffer.push(vertex.z);
      }
    }

    let mut data = HashMap::new();
    data.insert("vertices", vertices);
    data.insert("holes", holes.into_iter().map(|x| x as f64).collect());
    data.insert("dimension", vec![dimension as f64]);
    data.insert("start_index_in_vertices", vec![start_index_in_vertices as f64]);
    data.insert("end_index_in_vertices", vec![end_index_in_vertices as f64]);
    data.insert("right_most_index", vec![right_most_index as f64]);
    data.insert("right_point", vec![right_point.x, right_point.y, right_point.z]);
    data.insert("bridge_start", vec![bridge_start.x, bridge_start.y, bridge_start.z]);
    data.insert("bridge_end", vec![bridge_end.x, bridge_end.y, bridge_end.z]);
    
    data.insert("new_vertices_processed", new_vertices_processed);
    data.insert("vertex_nodes", vertex_nodes_data);
    data.insert("vertex_next_nodes", vertex_next_nodes_data);

    data.insert("new_buffer", new_buffer);

    let mut edge_data: Vec<f64> = Vec::new();
    for edge in ray_edge {
      for i in edge {
        edge_data.push(i as f64);
      }
    }
    data.insert("ray_edge", edge_data);

    serde_json::to_string(&data).unwrap()
  }

  #[wasm_bindgen]
  pub fn get_buffer_flush(&self) -> String {
    serde_json::to_string(&self.buffer).unwrap()
  }

  #[wasm_bindgen]
  pub fn clear_vertices(&mut self) {
    self.geometry.reset_geometry();
  }

  #[wasm_bindgen]
  pub fn reset_polygon(&mut self) {
    // Reset the geometry
  }

  pub fn extrude_by_height_with_holes(&mut self, height: f64) -> String {
    // Create a new buffer geometry
    // Loop through the extruded faces with holes
    // Create Variable Geometry
    // Call Triangulate with Holes with Custom Geometry method
    // push to the buffer geometry
    // Return the buffer geometry

    self.extruded = true;
    self.extruded_height = height;

    let mut extrude_data = extrude_polygon_with_holes(self.geometry.clone(), height);
    
    let mut local_geometry: Vec<f64> = Vec::new();
    let faces = extrude_data.faces.clone();
    let all_vertices_raw = extrude_data.vertices.clone();
    let holes = extrude_data.holes.clone();
    let face_holes_map = extrude_data.face_holes_map.clone();

    let face_length = face_holes_map.keys().len();
    // extrude_data.face_length = face_length;
    let mut face_current_index: usize = 0;
    
    let mut face_map: Vec<_> = face_holes_map.keys().cloned().collect();
    face_map.sort();

    for face_index in face_map {
      let mut variable_geometry: BaseGeometry = BaseGeometry::new("variable_geometry".to_string());
      let face = faces[face_index as usize].clone();
      let mut face_vertices: Vec<Vector3D> = Vec::new();

      for index in face.clone() {
        let v_face = all_vertices_raw[index as usize].clone();
        face_vertices.push(v_face);
      }
      face_vertices.reverse();
      variable_geometry.add_vertices(face_vertices.clone());

      let mut is_ccw = false;
      if (face_current_index > face_length - 2) {
        is_ccw = true;
        extrude_data.is_ccw_last_face = true;
      }

      // get holes for given face
      let holes_for_face = face_holes_map.get(&face_index).unwrap();
      if holes_for_face.len() > 0 {
        for hole_index in holes_for_face {
          let hole = holes[*hole_index as usize].clone();
          variable_geometry.add_holes(hole.clone());
        }
        self.variable_geometry = variable_geometry.clone();
        
        // last most processed face
        extrude_data.face_length = face_index as usize;

        let triangle_data = self.triangulate_with_holes_variable_geometry(is_ccw);
        let triangulated_data: HashMap<String, Vec<f64>> = serde_json::from_str(&triangle_data).unwrap();
        let vertices = triangulated_data.get("new_buffer").unwrap();
        for i in vertices {
          local_geometry.push(*i);
        }
      }
      else {
        // If no holes, then just triangulate the face
        let triangulated_face = triangulate::triangulate_polygon_by_face(face_vertices.clone());
        for index in triangulated_face {
          for i in index {
            let vertex = face_vertices[i as usize].clone();
            local_geometry.push(vertex.x);
            local_geometry.push(vertex.y);
            local_geometry.push(vertex.z);
          }
        }
      }
      
      // Destroy the variable geometry
      variable_geometry.reset_geometry();

      face_current_index += 1;
    }

    // Add BREP
    let brep = Geometry {
      vertices: extrude_data.vertices.clone(),
      faces: extrude_data.faces.clone(),
      edges: extrude_data.edges.clone()
    };
    self.brep = brep.clone();

    // let extrude_data_string = serde_json::to_string(&extrude_data).unwrap();
    // extrude_data_string
    
    serde_json::to_string(&local_geometry).unwrap()
  }

  #[wasm_bindgen]
  pub fn extrude_by_height(&mut self, height: f64) -> String {
    self.extruded = true;
    self.extruded_height = height;


    // If Polygon has holes, use other extrude method
    if (self.geometry.get_holes().len() > 0) {
      return self.extrude_by_height_with_holes(height);
    }

    let extrude_data = extrude_polygon_by_buffer_geometry(self.geometry.clone(), height);
  
    let mut local_geometry = Vec::new();
    
    // let face = extrude_data.faces[0].clone();
    for face in extrude_data.faces.clone() {
      let mut face_vertices: Vec<Vector3D> = Vec::new();
      for index in face.clone() {
        face_vertices.push(extrude_data.vertices[index as usize].clone());
      }

      let triangulated_face = triangulate::triangulate_polygon_by_face(face_vertices.clone());
      // let ccw_vertices = windingsort::ccw_test(face_vertices.clone());
      for index in triangulated_face {
        for i in index {
          // let vertex = ccw_vertices[i as usize];
          let vertex = face_vertices[i as usize].clone();
          local_geometry.push(vertex.x);
          local_geometry.push(vertex.y);
          local_geometry.push(vertex.z);
        }
      }
    }
    
    // let face_data_string = serde_json::to_string(&face).unwrap(); // Serialize face_data
    // face_data_string

    // let extrude_data_string = serde_json::to_string(&extrude_data).unwrap(); // Serialize extrude_data
    // extrude_data_string

    let string_data = serde_json::to_string(&local_geometry).unwrap();
    string_data

    // ABOVE LINE WORKING

    // // TESTING EDGES OUTLINE
    // let mut outline_data: Vec<Vec<Vector3D>> = Vec::new();

    // // for edge in extruded_raw.edges {
    // //   let start = vertices[edge[0] as usize].clone();
    // //   let end = vertices[edge[1] as usize].clone();

    // //   let edge_vertices = vec![start, end];
    // //   outline_data.push(edge_vertices);
    // // }

    // for face in faces {
    //   let mut face_vertices: Vec<Vector3D> = Vec::new();
    //   for index in face {
    //     let v_face = vertices[index as usize].clone();
    //     face_vertices.push(v_face);
    //   }
    //   outline_data.push(face_vertices);
    // }

    // serde_json::to_string(&outline_data).unwrap()
  }

  #[wasm_bindgen]
  pub fn get_outlines(&self) -> String {
    let height = self.extruded_height;
    if height == 0.0 {
      return "Please extrude the polygon first".to_string();
    }

    let mut outline_data: Vec<Vec<Vector3D>> = Vec::new();
    let extruded_raw = extrude_polygon_by_buffer_geometry(self.geometry.clone(), height);
    let faces = extruded_raw.faces;
    let vertices = extruded_raw.vertices;

    for face in faces {
      let mut face_vertices: Vec<Vector3D> = Vec::new();
      for index in face {
        let v_face = vertices[index as usize].clone();
        face_vertices.push(v_face);
      }
      outline_data.push(face_vertices);
    }
    serde_json::to_string(&outline_data).unwrap()
  }

  #[wasm_bindgen]
  pub fn get_geometry(&self) -> String {
    let geometry = self.geometry.get_geometry();
    geometry
  }

  #[wasm_bindgen]
  pub fn get_brep_data(&self) -> String {
    let geometry = self.brep.get_geometry();
    geometry
  }

  #[wasm_bindgen]
  pub fn outline_edges(&mut self) -> String {
    let mut outline_points: Vec<f64> = Vec::new();

    for edge in self.brep.edges.clone() {
      let start_index = edge[0] as usize;
      let end_index = edge[1] as usize;

      let start_point = self.brep.vertices[start_index].clone();
      let end_point = self.brep.vertices[end_index].clone();

      outline_points.push(start_point.x);
      outline_points.push(start_point.y);
      outline_points.push(start_point.z);

      outline_points.push(end_point.x);
      outline_points.push(end_point.y);
      outline_points.push(end_point.z);
    }

    let outline_data_string = serde_json::to_string(&outline_points).unwrap();
    outline_data_string
  }
}
