use crate::{geometry::basegeometry::BaseGeometry, utility::geometry::{Geometry, Geometry_Holes}};
use super::{triangulate, windingsort};
use openmaths::Vector3;

pub fn extrude_polygon_by_buffer_geometry(geom_buf: BaseGeometry, height: f64) -> Geometry {
  if (geom_buf.get_vertices().len() < 3) {
    // return String::from("Polygon should have atleast 3 vertices");
  }

  let ccw_vertices = windingsort::ccw_test(geom_buf.get_vertices());

  let mut buf_vertices = ccw_vertices.clone();
  let mut buf_edges: Vec<Vec<u8>> = Vec::new();
  let mut buf_faces: Vec<Vec<u8>> = Vec::new();

  let current_length = buf_vertices.len();

  // Bottom Face
  for i in 0..current_length {
    let edge = {
      vec![i as u8, ((i + 1) % ccw_vertices.len()) as u8]
    };
    buf_edges.push(edge);
  }

  let mut face: Vec<u8> = Vec::new();
  for i in 0..ccw_vertices.len() {
    face.push(i as u8);
  }
  // face.reverse();
  buf_faces.push(face);

  for index in 0..ccw_vertices.len() {
    let up_vertex = Vector3::new(0.0, height, 0.0);
    let new_vertex = ccw_vertices[index].clone().add(&up_vertex);
    buf_vertices.push(new_vertex);

    let edge = {
      vec![index as u8, buf_vertices.len() as u8 - 1]
    };
    
    buf_edges.push(edge);
  }

  for i in current_length..buf_vertices.len() {
    if i < buf_vertices.len() - 1 {
      let edge = {
        vec![i as u8, (i + 1) as u8]
      };
      buf_edges.push(edge);
    } else {
      let edge = {
        vec![i as u8, (current_length) as u8]
      };
      buf_edges.push(edge);
    }
  }

  // Side Faces
  for i in 0..current_length {
  let next = (i + 1) % current_length;
  let mut face: Vec<u8> = vec![
    i as u8,
    next as u8,
    (next + current_length) as u8,
    i as u8 + current_length as u8,
    ];
    face.reverse();
    buf_faces.push(face);
  }

  // Top Face
  let mut face: Vec<u8> = Vec::new();
  for i in 0..current_length {
    face.push(i as u8 + current_length as u8);
  }
  face.reverse();
  buf_faces.push(face);
  
  let geometry = Geometry {
    vertices: buf_vertices,
    edges: buf_edges,
    faces: buf_faces,
  };
  geometry
}


pub fn extrude_polygon_with_holes(mut geom_buf: BaseGeometry, height: f64) -> Geometry_Holes {
  if (geom_buf.get_vertices().len() < 3) {
    // return String::from("Polygon should have atleast 3 vertices");
  }

  let flat_data = triangulate::flatten_buffer_geometry(geom_buf.clone());
  let vertices = flat_data.vertices.clone();
  let holes = flat_data.holes.clone();

  let mut start_hole_index = 0;
  if holes.len() > 0 {
    start_hole_index = holes[0] * 3;
  }

  let mut temp_vertices: Vec<Vector3> = Vec::new();
  let mut i: usize = 0;
  while i < start_hole_index as usize {
    let x = vertices[i];
    let y = vertices[i + 1];
    let z = vertices[i + 2];
    let vertex = Vector3::new(x, y, z);
    temp_vertices.push(vertex);
    i += 3;
  }

  let ccw_vertices = windingsort::ccw_test(temp_vertices);
  let mut buf_vertices = ccw_vertices.clone();
  let mut buf_edges: Vec<Vec<u8>> = Vec::new();
  let mut buf_faces: Vec<Vec<u8>> = Vec::new();
  let mut buf_holes: Vec<Vec<Vector3>> = Vec::new();
  let mut face_holes_map: std::collections::HashMap<u8, Vec<u8>> = std::collections::HashMap::new();

  let mut hole_index = 0;
  let mut face_index = 0;

  let mut current_length = buf_vertices.len();

  // Bottom Face
  // TODO: Extra Edge for holes
  for i in 0..current_length {
    let edge = {
      vec![i as u8, ((i + 1) % ccw_vertices.len()) as u8]
    };
    buf_edges.push(edge);
  }

  let mut face: Vec<u8> = Vec::new();
  for i in 0..ccw_vertices.len() {
    face.push(i as u8);
  }
  face.reverse();
  buf_faces.push(face);

  if geom_buf.get_holes().len() > 0 {
    let mut holes_for_face = Vec::new();
    for hole in geom_buf.get_holes() {
      let mut clone_hole = hole.clone();
      clone_hole.reverse();
      buf_holes.push(clone_hole);
      // Bottom Face hence 0
      holes_for_face.push(hole_index);
      hole_index += 1;
    }
    face_holes_map.insert(face_index, holes_for_face);
    face_index += 1;
  }

  // Side Face Refined
  for index in 0..ccw_vertices.len() {
    let up_vertex = Vector3::new(0.0, height, 0.0);
    let new_vertex = ccw_vertices[index].clone().add(&up_vertex);
    buf_vertices.push(new_vertex);

    let edge = {
      vec![index as u8, buf_vertices.len() as u8 - 1]
    };
    
    buf_edges.push(edge);
  }

  // Extruded Top Face Edges
  for i in current_length..buf_vertices.len() {
    if i < buf_vertices.len() - 1 {
      let edge = {
        vec![i as u8, (i + 1) as u8]
      };
      buf_edges.push(edge);
    } else {
      let edge = {
        vec![i as u8, (current_length) as u8]
      };
      buf_edges.push(edge);
    }
  }

  // Bottom Holes - Holes can now be added to buf_vertices since we follow the order that holes come after normal vertices
  let mut hole_index_start = buf_vertices.len();
  let mut i = start_hole_index as usize;
  while i < vertices.len() - 2 {
    let x = vertices[i];
    let y = vertices[i + 1];
    let z = vertices[i + 2];
    let vertex = Vector3::new(x, y, z);
    buf_vertices.push(vertex);
    i += 3;
  }
  // Create Edges for holes
  for hole in geom_buf.get_holes() {
    for i in 0..hole.len() {
      let edge = {
        vec![(hole_index_start + i) as u8, ((i + 1) % hole.len() + hole_index_start) as u8]
      };
      buf_edges.push(edge);
    }
  }

  // Side Faces
  for i in 0..current_length {
    let next = (i + 1) % current_length;
    let mut face: Vec<u8> = vec![
      i as u8,
      next as u8,
      (next + current_length) as u8,
      i as u8 + current_length as u8,
    ];
    // face.reverse();
    buf_faces.push(face);

    face_holes_map.insert(face_index, Vec::new());
    face_index += 1;
  }

  // let mut hole_index_start_buf_vertices = buf_vertices.len();

  // TODO: Add Side Faces Of Holes
  // Add Extrude for Holes
  if geom_buf.get_holes().len() > 0 {
    for hole in geom_buf.get_holes() {

      let hole_index_constant = hole_index_start;

      // Hole Side Face Edges Vertical
      for vertex in hole.clone() {
        let up_vertex = Vector3::new(0.0, height, 0.0);
        let new_vertex = vertex.clone().add(&up_vertex);
        buf_vertices.push(new_vertex);

        let edge = {
          vec![hole_index_start as u8, buf_vertices.len() as u8 - 1]
        };
        buf_edges.push(edge);
        hole_index_start += 1;
      }

      // Add Edges for extruded holes
      for i in 0..hole.len() {
        let edge = {
          vec![(hole_index_start + i) as u8, ((i + 1) % hole.len() + hole_index_start) as u8]
        };
        buf_edges.push(edge);
      }

      // Hole Side Face Actuals
      let hole_end_index = buf_vertices.len() - hole.len();
      for i in 0..hole.len() {
        let next = (i + 1) % hole.len();
        let mut face: Vec<u8> = vec![
          hole_index_constant as u8 + i as u8,
          hole_index_constant as u8 + next as u8,
          hole_end_index as u8 + next as u8,
          hole_end_index as u8 + i as u8,
        ];
        face.reverse();
        buf_faces.push(face);

        // // Add to Face Holes Map
        face_holes_map.insert(face_index, Vec::new());
        face_index += 1;     
      }
    }
  }

  // Top Face
  let mut face: Vec<u8> = Vec::new();
  for i in 0..current_length {
    face.push(i as u8 + current_length as u8);
  }
  face.reverse();
  buf_faces.push(face);
  face_holes_map.insert(face_index, Vec::new());

  if geom_buf.get_holes().len() > 0 {
    for hole in geom_buf.get_holes() {
      let mut extruded_hole: Vec<Vector3> = Vec::new();
      for i in 0..hole.len() {
        let index = hole_index_start + i;
        extruded_hole.push(buf_vertices[index as usize].clone());
      }
      extruded_hole.reverse();
      buf_holes.push(extruded_hole.clone());
      // Top Face hence 1
      let mut holes_for_face = Vec::new();
      holes_for_face.push(hole_index);
      hole_index += 1;
      face_holes_map.insert(face_index, holes_for_face);
      face_index += 1;
    }
  }
  
  let brep_geom: Geometry_Holes = Geometry_Holes {
    vertices: buf_vertices.clone(),
    edges: buf_edges.clone(),
    faces: buf_faces.clone(),
    holes: buf_holes.clone(),
    face_holes_map: face_holes_map.clone(),
    is_ccw_last_face: false,
    face_length: 0
  };
  brep_geom
}