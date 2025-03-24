/*
 * Poly Line
 * Definition - A Polyline is a connected sequence of line segments created as a single object.
 */

 use crate::utility::openmath;
 use wasm_bindgen::prelude::*;
 use serde::{Serialize, Deserialize};
 
 #[wasm_bindgen]
 #[derive(Clone, Serialize, Deserialize)]
 pub struct OGPolyLine {
   id: String,
   points: Vec<openmath::Vector3D>,
   is_closed: bool
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
        is_closed: false
      }
    }
  
    #[wasm_bindgen]
    pub fn clone(&self) -> OGPolyLine {
      OGPolyLine {
        id: self.id.clone(),
        points: self.points.clone(),
        is_closed: self.is_closed
      }
    }
  
    #[wasm_bindgen]
    pub fn set_config(&mut self, points: Vec<openmath::Vector3D>) {
      self.points.clear();
      for point in points {
        self.points.push(point);
      }

      self.check_closed_test();
    }

    #[wasm_bindgen]
    pub fn add_point(&mut self, point: openmath::Vector3D) {
      self.points.push(point);
      self.check_closed_test();
    }
  
    // Dispose
    #[wasm_bindgen]
    pub fn dispose_points(&mut self) {
      self.points.clear();
    }
  
    // TODO: Implement Destroy
    #[wasm_bindgen]
    pub fn destroy(&mut self) {
      self.points.clear();
    }
  
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
 }