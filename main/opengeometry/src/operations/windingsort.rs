/**
 * Simple Sorting Points to form a polygon and get them in winding order
 * The Algorithm works for 2D points only and Y-Axis is ignored - Need to be fixed/decided which axis to ignore
 */

use crate::utility::openmath::Vector3D;

fn compute_center(points: &[Vector3D]) -> Vector3D {
  let n = points.len();
  let sum_x: f64 = points.iter().map(|p| p.x).sum();
  let sum_z: f64 = points.iter().map(|p| p.z).sum();
  Vector3D {
    x: sum_x / n as f64,
    y: 0.0,
    z: sum_z / n as f64,
  }
}

pub fn sort_points_by_angle(points: &mut Vec<Vector3D>, center: Vector3D) {
  points.sort_by(|&p1, &p2| {
    let angle1 = (p1.z - center.z).atan2(p1.x - center.x);
    let angle2 = (p2.z - center.z).atan2(p2.x - center.x);
    angle1.partial_cmp(&angle2).unwrap()
  });
}

// main
pub fn wind_points(unsort: Vec<Vector3D>) -> Vec<Vector3D> {
  let mut points = unsort;
  let center = compute_center(&points);
  sort_points_by_angle(&mut points, center);

  points
}



// Test for 3D Coords
//  use crate::utility::openmath::Vector3D;

//  fn compute_center(points: &[Vector3D]) -> Vector3D {
//    let n = points.len();
//    let sum_x: f64 = points.iter().map(|p| p.x).sum();
//    let sum_y: f64 = points.iter().map(|p| p.y).sum();
//    let sum_z: f64 = points.iter().map(|p| p.z).sum();
//    Vector3D {
//        x: sum_x / n as f64,
//        y: sum_y / n as f64,
//        z: sum_z / n as f64,
//    }
//  }
 
//  pub fn sort_points_by_angle(points: &mut Vec<Vector3D>, center: Vector3D) {
//      points.sort_by(|&p1, &p2| {
//          let theta1 = (p1.z - center.z).atan2((p1.y - center.y).hypot(p1.x - center.x));
//          let theta2 = (p2.z - center.z).atan2((p2.y - center.y).hypot(p2.x - center.x));
//          theta1.partial_cmp(&theta2).unwrap()
//      });
//  }
 
//  // main
//  pub fn connect_points(points: &mut Vec<Vector3D>) {
//      let center = compute_center(points);
//      sort_points_by_angle(points, center);
//  }
 
 
