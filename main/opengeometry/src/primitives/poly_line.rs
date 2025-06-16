/*
 * Poly Line
 * Definition - A Polyline is a connected sequence of line segments created as a single object.
 */

use crate::{operations::windingsort, utility::openmath};
use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};
use crate::utility::openmath::{Geometry, Vector3D};

/*
* Data structure to hold the offset points and the flag indicating if the points are in counter-clockwise order
* This is used to return the offset points and their treatment status
*/
#[derive(Serialize)]
struct Data {
  untreated: Vec<openmath::Vector3D>,
  treated: Vec<openmath::Vector3D>,
  flag: bool,
}

 #[wasm_bindgen]
 #[derive(Clone, Serialize, Deserialize)]
pub struct OGPolyLine {
  id: String,
  points: Vec<openmath::Vector3D>,
  backup_points: Vec<openmath::Vector3D>,
  is_closed: bool,
  brep : Geometry,
  position: openmath::Vector3D,
}

impl Drop for OGPolyLine {
  fn drop(&mut self) {
    self.points.clear();
    self.is_closed = false;
    self.brep.clear();
    self.id.clear();
    web_sys::console::log_1(&"Clearing Polyline...".into());
  }
}
 
#[wasm_bindgen]
impl OGPolyLine {
  #[wasm_bindgen(setter)]
  pub fn set_id(&mut self, id: String) {
    self.id = id;
  }
 
  #[wasm_bindgen(getter)]
  pub fn id(&self) -> String {
    self.id.clone()
  }
 
  #[wasm_bindgen(constructor)]
  pub fn new(id: String) -> OGPolyLine {
    OGPolyLine {
      id,
      points: Vec::new(),
      backup_points: Vec::new(),
      is_closed: false,
      brep: Geometry::new(),
      position: openmath::Vector3D { x: 0.0, y: 0.0, z: 0.0 },
    }
  }
  
  #[wasm_bindgen]
  pub fn clone(&self) -> OGPolyLine {
    OGPolyLine {
      id: self.id.clone(),
      points: self.points.clone(),
      backup_points: self.backup_points.clone(),
      is_closed: self.is_closed,
      brep: self.brep.clone(),
      position: self.position.clone(),
    }
  }

  #[wasm_bindgen]
  pub fn translate(&mut self, translation: openmath::Vector3D) {

    self.points.clear();

    for i in 0..self.backup_points.len() {
      let point = &mut self.backup_points[i].clone();
      point.x += translation.x;
      point.y += translation.y;
      point.z += translation.z;

      self.points.push(point.clone());
      self.brep.vertices.push(point.clone());
    }
  
    self.check_closed_test();
    self.generate_brep();
  }

  #[wasm_bindgen]
  pub fn set_position(&mut self, position: openmath::Vector3D) {
    self.position = position;
  }
  
  #[wasm_bindgen]
  pub fn add_multiple_points(&mut self, points: Vec<openmath::Vector3D>) {
    self.points.clear();
    for point in points {
      self.points.push(point);
      self.backup_points.push(point);
    }

    self.check_closed_test();

    self.generate_brep();
  }

  #[wasm_bindgen]
  pub fn add_point(&mut self, point: openmath::Vector3D) {
    self.points.push(point);
    self.backup_points.push(point);
    self.check_closed_test();
  }

  // If we use Drop, we don't need to implement this  
  // // Dispose
  // #[wasm_bindgen]
  // pub fn dispose_points(&mut self) {
  //   self.points.clear();
  // }
  
  // // TODO: Implement Destroy
  // #[wasm_bindgen]
  // pub fn destroy(&mut self) {
  //   self.points.clear();
  //   self.is_closed = false;
  //   self.brep.clear();
  //   self.id.clear();
  // }
  
  // Get Points for the Circle
  #[wasm_bindgen]
  pub fn get_points(&self) -> String {
    serde_json::to_string(&self.points).unwrap()
  }

  pub fn get_raw_points(&self) -> Vec<openmath::Vector3D> {
    self.points.clone()
  }

  #[wasm_bindgen]
  pub fn is_closed(&self) -> bool {
    self.is_closed
  }

  // Simple Check to see if the Polyline is closed
  // This can be made better    
  pub fn check_closed_test(&mut self) {
    if self.points.len() > 2 {
      if self.points[0].x == self.points[self.points.len() - 1].x &&
        self.points[0].y == self.points[self.points.len() - 1].y &&
        self.points[0].z == self.points[self.points.len() - 1].z {
        self.is_closed = true;
      }
    }
  }

  pub fn generate_brep(&mut self) {
    self.brep.clear();
    if self.points.len() > 1 {
      for i in 0..self.points.len() - 1 {
        self.brep.add_edge(vec![i as u8, (i + 1) as u8]);
      }
    }
    self.brep.vertices = self.points.clone();
  }

  pub fn get_brep_data(&self) -> String {
    let mut geometry = self.brep.get_geometry_raw().clone();

    // Add the position to the geometry
    for vertex in &mut geometry.vertices {
      vertex.x += self.position.x;
      vertex.y += self.position.y;
      vertex.z += self.position.z;
    }

    // Serialize the geometry to JSON
    serde_json::to_string(&geometry).unwrap()
  }

  // Paper - https://seant23.wordpress.com/wp-content/uploads/2010/11/anoffsetalgorithm.pdf
  // Paper has coverage for curves as well, but we will only implement for polylines
  #[wasm_bindgen]
  pub fn get_offset(&self, distance: f64) -> String {
    let n = self.points.len();
    if n < 2 {
        return serde_json::to_string(&Vec::<openmath::Vector3D>::new()).unwrap();
    }

    let mut offset_points = Vec::new();

    for i in 0..n {
      let prev = if i == 0 {
        self.points[i]
      } else {
        self.points[i - 1]
      };
      let curr = self.points[i];
      let next = if i == n - 1 {
        self.points[i]
      } else {
        self.points[i + 1]
      };

      let v1 = curr.subtract(&prev).normalize();
      let v2 = next.subtract(&curr).normalize();

      let perp1 = Vector3D { x: -v1.z, y: 0.0, z: v1.x };
      let perp2 = Vector3D { x: -v2.z, y: 0.0, z: v2.x };

      let offset_point = if i == 0 {
        // Start point: move perpendicular to first segment
        curr.add(&perp2.multiply(distance))
      } else if i == n - 1 {
        // End point: move perpendicular to last segment
        curr.add(&perp1.multiply(distance))
      } else {
        // Middle: compute bisector intersection
        let a1 = prev.add(&perp1.multiply(distance));
        let a2 = curr.add(&perp1.multiply(distance));
        let b1 = curr.add(&perp2.multiply(distance));
        let b2 = next.add(&perp2.multiply(distance));

        Self::calculate_2D_interesection(&a1, &a2, &b1, &b2)
            .unwrap_or(curr.add(&perp1.multiply(distance)))
      };

      offset_points.push(offset_point);
    }

    let ccw_test = windingsort::is_ccw_need(offset_points.clone());

    let data = Data {
      untreated: offset_points.clone(),
      treated: ccw_test.ccw.clone(),
      flag: ccw_test.flag,
    };

    serde_json::to_string(&data).unwrap()
  }

  pub fn calculate_2D_interesection(
    point_a: &Vector3D,
    point_b: &Vector3D,
    point_c: &Vector3D,
    point_d: &Vector3D,
  ) -> Option<Vector3D> {
    let dx1 = point_b.x - point_a.x;
    let dz1 = point_b.z - point_a.z;
    let dx2 = point_d.x - point_c.x;
    let dz2 = point_d.z - point_c.z;

    let det = dx1 * dz2 - dz1 * dx2;
    if det.abs() < 1e-8 {
      return None; // Parallel lines
    }

    let dx = point_c.x - point_a.x;
    let dz = point_c.z - point_a.z;

    let t = (dx * dz2 - dz * dx2) / det;

    Some(Vector3D {
      x: point_a.x + t * dx1,
      y: 0.0,
      z: point_a.z + t * dz1,
    })
  }
}
