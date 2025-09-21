use openmaths::Vector3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Triangle {
  pub a: Vector3,
  pub b: Vector3,
  pub c: Vector3,
}

impl Triangle {
  pub fn new() -> Triangle {
    Triangle {
      a: Vector3::new(0.0, 0.0, 0.0),
      b: Vector3::new(0.0, 0.0, 0.0),
      c: Vector3::new(0.0, 0.0, 0.0),
    }
  }

  pub fn new_with_vertices(a: Vector3, b: Vector3, c: Vector3) -> Triangle {
    Triangle { a, b, c }
  }

  pub fn set_vertices(&mut self, a: Vector3, b: Vector3, c: Vector3) {
    self.a = a;
    self.b = b;
    self.c = c;
  }

  pub fn is_point_in_triangle(&self, p : Vector3) -> bool {
    let v1 = Vector3::new(1.0, 0.0, 0.0);
    let crso = v1.cross(&self.a);

    let ab = self.b.clone().subtract(&self.a);
    let bc = self.c.clone().subtract(&self.b);
    let ca = self.a.clone().subtract(&self.c);

    let ap = p.clone().subtract(&self.a);
    let bp = p.clone().subtract(&self.b);
    let cp = p.clone().subtract(&self.c);

    let cross_abp = ab.clone().cross(&ap);
    let cross_bcp = bc.clone().cross(&bp);
    let cross_cap = ca.clone().cross(&cp);

    if (
        cross_abp.y > 0.0 &&
        cross_bcp.y > 0.0 &&
        cross_cap.y > 0.0
      ) || (
        cross_abp.y < 0.0 &&
        cross_bcp.y < 0.0 &&
        cross_cap.y < 0.0
      ) {
        return true;
      }
    false
  }
}