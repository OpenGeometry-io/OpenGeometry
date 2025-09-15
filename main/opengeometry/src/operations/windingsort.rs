/**
 * Code for sorting the winding order of a given vertices
 */

use openmaths::Vector3;

fn compute_signed_area(points: &[Vector3]) -> f64 {
  let n = points.len();
  let mut sum = 0.0;
  for i in 0..n {
    let p1 = &points[i];
    let p2 = &points[(i + 1) % n];
    sum += p1.x * p2.z - p2.x * p1.z;
  }
  sum / 2.0
}

pub fn ccw_test(raw_points: Vec<Vector3>) -> Vec<Vector3> {
  let mut points = raw_points;
  let area = compute_signed_area(&points);
  if area < 0.0 {
    points.reverse();
  }
  points
}

pub fn is_ccw_need(raw_points: Vec<Vector3>) -> ccw_and_flag {
  let mut points = raw_points;
  let area = compute_signed_area(&points);
  if area < 0.0 {
    points.reverse();
    ccw_and_flag { ccw: points, flag: true }
  } else {
    ccw_and_flag { ccw: points, flag: false }
  }
}

pub struct ccw_and_flag {
  pub ccw: Vec<Vector3>,
  pub flag: bool,
}