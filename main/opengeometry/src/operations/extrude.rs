use crate::{geometry::basegeometry::BaseGeometry, utility::openmath::{Geometry, Vector3D}};

pub fn extrude_polygon_by_buffer_geometry(geom_buf: BaseGeometry, height: f64) -> Geometry {
  if (geom_buf.get_vertices().len() < 3) {
    // return String::from("Polygon should have atleast 3 vertices");
  }

  let mut buf_vertices = geom_buf.get_vertices().clone();
  let mut buf_edges: Vec<Vec<u8>> = Vec::new();
  let mut buf_faces: Vec<Vec<u8>> = Vec::new();

  let current_length = buf_vertices.len();

  for i in 0..current_length {
    let edge = {
      vec![i as u8, ((i + 1) % geom_buf.get_vertices().len()) as u8]
    };
    buf_edges.push(edge);
  }

  let mut face: Vec<u8> = Vec::new();
  for i in 0..geom_buf.get_vertices().len() {
    face.push(i as u8);
  }
  buf_faces.push(face);

  for index in 0..geom_buf.get_vertices().len() {
    let new_vertex = geom_buf.get_vertices()[index].clone().add_extrude_in_up(height, Vector3D::create(0.0, 1.0, 0.0));
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
  let face: Vec<u8> = vec![
    i as u8,
    next as u8,
    (next + current_length) as u8,
    i as u8 + current_length as u8,
    ];
    buf_faces.push(face);
  }

  // Bottom Face
  let mut face: Vec<u8> = Vec::new();
  for i in 0..current_length {
    face.push(i as u8 + current_length as u8);
  }
  buf_faces.push(face);
  
  let geometry = Geometry {
    vertices: buf_vertices,
    edges: buf_edges,
    faces: buf_faces,
  };
  geometry
}
