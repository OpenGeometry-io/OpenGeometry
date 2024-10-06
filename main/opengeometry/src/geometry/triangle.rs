use crate::utility::openmath::Vector3D;

#[derive(Clone)]
pub struct Triangle {
  pub a: Vector3D,
  pub b: Vector3D,
  pub c: Vector3D,
}

impl Triangle {
  pub fn new() -> Triangle {
    Triangle {
      a: Vector3D::create(0.0, 0.0, 0.0),
      b: Vector3D::create(0.0, 0.0, 0.0),
      c: Vector3D::create(0.0, 0.0, 0.0),
    }
  }

  pub fn set_vertices(&mut self, a: Vector3D, b: Vector3D, c: Vector3D) {
    self.a = a;
    self.b = b;
    self.c = c;
  }

  pub fn is_point_in_triangle(&self, p : Vector3D) -> bool {
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