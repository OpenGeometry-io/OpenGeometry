/**
 * Code for sorting the winding order of a given vertices
 */

use crate::utility::openmath::Vector3D;

fn compute_signed_area(points: &[Vector3D]) -> f64 {
  let n = points.len();
  let mut sum = 0.0;
  for i in 0..n {
    let p1 = &points[i];
    let p2 = &points[(i + 1) % n];
    sum += p1.x * p2.z - p2.x * p1.z;
  }
  sum / 2.0
}


pub fn ccw_test(raw_points: Vec<Vector3D>) -> Vec<Vector3D> {
  let mut points = raw_points;
  let area = compute_signed_area(&points);
  if area < 0.0 {
    points.reverse();
  }
  points
}