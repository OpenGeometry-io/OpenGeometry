use crate::{brep::{edge, Brep, Edge, Face, Vertex}, geometry::basegeometry::BaseGeometry, utility::geometry::{Geometry, Geometry_Holes}};
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

/**
 * Extrude a BREF face and return a new BREP Object
 */
pub fn extrude_brep_face(brep_face: Brep, height: f64) -> Brep {
  let mut extruded_brep = Brep::new(brep_face.id);
  // This function will take a BREP face and extrude it to create a new face
  // It will return the new face as a Geometry object
  
  if (brep_face.get_vertex_count() < 3) {
    // return String::from("Polygon should have atleast 3 vertices");
  }

  let ccw_vertices = windingsort::ccw_test(brep_face.get_flattened_vertices());

  // Bottom Face
  extruded_brep.faces.push(Face::new(
    extruded_brep.get_face_count() as u32,
    Vec::new()
  ));
  for i in 0..ccw_vertices.len() {
    let vertex = ccw_vertices[i].clone();
    extruded_brep.vertices.push(Vertex::new(
      extruded_brep.get_vertex_count() as u32,
      vertex));

    let edge = {
      vec![
        extruded_brep.get_vertex_count() - 1,
        ((extruded_brep.get_vertex_count()) % ccw_vertices.len() as u32)
      ]
    };
    extruded_brep.edges.push(Edge::new(
      extruded_brep.get_edge_count() as u32,
      edge[0],
      edge[1])
    );

    // Push Vertex Index to the face
    extruded_brep.insert_vertex_at_face_by_id(0, extruded_brep.get_vertex_count() as u32 - 1);
  }

  // Create Top Face - Index will be 1
  extruded_brep.faces.push(Face::new(
    extruded_brep.get_face_count() as u32,
    Vec::new()
  ));

  // Create Top Vertices
  let current_length = extruded_brep.get_vertex_count();
  for index in 0..ccw_vertices.len() {
    // TODO: Find a way to extrude in give direction
    let up_vertex = Vector3::new(0.0, height, 0.0);
    let new_vertex = ccw_vertices[index].clone().add(&up_vertex);
    extruded_brep.vertices.push(Vertex::new(
      extruded_brep.get_vertex_count() as u32,
      new_vertex));

    let edge = {
      vec![
        index as usize + current_length as usize,
        ( (index + 1) % ccw_vertices.len() ) as usize + current_length as usize
      ]
    };

    // Create Edge for Top Face
    extruded_brep.edges.push(Edge::new(
      extruded_brep.get_edge_count(),
      edge[0] as u32,
      edge[1] as u32)
    );

    // Push Vertex Index to the face
    extruded_brep.insert_vertex_at_face_by_id(1, extruded_brep.get_vertex_count() as u32 - 1);
  }


  // Side Faces, since vertices are already added we can just create edges and faces
  for i in 0..current_length {
    let next = (i + 1) % current_length;
    let mut face: Vec<u32> = vec![
      i as u32,
      next as u32,
      (next as u32 + current_length as u32),
      i as u32 + current_length as u32,
    ];
    face.reverse();
    extruded_brep.faces.push(Face::new(
      extruded_brep.get_face_count() as u32,
      face.clone()
    ));

    // Create Edges for Side Faces
    for j in 0..face.len() {
      let edge = {
        vec![face[j], face[(j + 1) % face.len()]]
      };
      extruded_brep.edges.push(Edge::new(
        extruded_brep.get_edge_count() as u32,
        edge[0],
        edge[1])
      );
    }
  }

  // Reverse the vertices for the top face - since top face is at 1st index
  // TODO: Find if this has any effect on the geometry
  extruded_brep.faces[1].face_indices.reverse();

  extruded_brep
}
