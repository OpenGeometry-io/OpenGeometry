//! Work-plane / sketch coordinate system (debt item D6).
//!
//! A sketch is conceptually "a coordinate frame + 2D geometry." The kernel
//! historically had no such object: every consumer re-derived the world
//! transform of a profile by hand (baking Euler angles into vertex positions),
//! duplicating error-prone math. [`WorkPlane`] is the first-class datum frame —
//! an origin, a normal, and two in-plane axes — that lifts 2D `(u, v)`
//! coordinates to 3D world space, so sketches can be authored on any plane.

use openmaths::Vector3;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct WorkPlane {
    origin: Vector3,
    normal: Vector3,
    u_axis: Vector3,
    v_axis: Vector3,
}

#[wasm_bindgen]
impl WorkPlane {
    /// Builds a work plane from an origin and normal, choosing a stable in-plane
    /// `u` axis automatically (`v = normal × u`).
    #[wasm_bindgen(js_name = fromOriginNormal)]
    pub fn from_origin_normal(origin: Vector3, normal: Vector3) -> WorkPlane {
        let n = normalize(normal, Vector3::new(0.0, 1.0, 0.0));
        let u = normalize(any_perpendicular(n), Vector3::new(1.0, 0.0, 0.0));
        let v = normalize(cross(n, u), Vector3::new(0.0, 0.0, 1.0));
        WorkPlane {
            origin,
            normal: n,
            u_axis: u,
            v_axis: v,
        }
    }

    /// Builds a work plane from an origin, normal, and a preferred `u` axis. The
    /// `u` axis is projected onto the plane and orthonormalized; `v = normal × u`.
    #[wasm_bindgen(constructor)]
    pub fn new(origin: Vector3, normal: Vector3, u_hint: Vector3) -> WorkPlane {
        let n = normalize(normal, Vector3::new(0.0, 1.0, 0.0));
        // Remove the normal component from the u hint so u lies in the plane.
        let dot = u_hint.x * n.x + u_hint.y * n.y + u_hint.z * n.z;
        let projected = Vector3::new(
            u_hint.x - n.x * dot,
            u_hint.y - n.y * dot,
            u_hint.z - n.z * dot,
        );
        let u = normalize(projected, any_perpendicular(n));
        let v = normalize(cross(n, u), Vector3::new(0.0, 0.0, 1.0));
        WorkPlane {
            origin,
            normal: n,
            u_axis: u,
            v_axis: v,
        }
    }

    /// Lifts a single 2D in-plane coordinate to 3D world space.
    #[wasm_bindgen(js_name = liftPoint)]
    pub fn lift_point(&self, u: f64, v: f64) -> Vector3 {
        Vector3::new(
            self.origin.x + self.u_axis.x * u + self.v_axis.x * v,
            self.origin.y + self.u_axis.y * u + self.v_axis.y * v,
            self.origin.z + self.u_axis.z * u + self.v_axis.z * v,
        )
    }

    /// Lifts a flat `[u0,v0,u1,v1,…]` buffer to a flat `[x0,y0,z0,…]` world buffer.
    #[wasm_bindgen(js_name = liftPoints)]
    pub fn lift_points_flat(&self, uv: Vec<f64>) -> Vec<f64> {
        let mut out = Vec::with_capacity(uv.len() / 2 * 3);
        for pair in uv.chunks_exact(2) {
            let p = self.lift_point(pair[0], pair[1]);
            out.push(p.x);
            out.push(p.y);
            out.push(p.z);
        }
        out
    }

    #[wasm_bindgen(getter)]
    pub fn origin(&self) -> Vector3 {
        self.origin
    }

    #[wasm_bindgen(getter)]
    pub fn normal(&self) -> Vector3 {
        self.normal
    }
}

impl WorkPlane {
    pub fn u_axis(&self) -> Vector3 {
        self.u_axis
    }
    pub fn v_axis(&self) -> Vector3 {
        self.v_axis
    }
    /// Lifts owned 2D points to world `Vector3`s.
    pub fn lift(&self, points: &[(f64, f64)]) -> Vec<Vector3> {
        points
            .iter()
            .map(|(u, v)| self.lift_point(*u, *v))
            .collect()
    }
}

fn cross(a: Vector3, b: Vector3) -> Vector3 {
    Vector3::new(
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    )
}

fn normalize(v: Vector3, fallback: Vector3) -> Vector3 {
    let len = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
    if len <= 1.0e-12 {
        fallback
    } else {
        Vector3::new(v.x / len, v.y / len, v.z / len)
    }
}

fn any_perpendicular(n: Vector3) -> Vector3 {
    if n.x.abs() <= n.y.abs() && n.x.abs() <= n.z.abs() {
        Vector3::new(0.0, -n.z, n.y)
    } else if n.y.abs() <= n.z.abs() {
        Vector3::new(-n.z, 0.0, n.x)
    } else {
        Vector3::new(-n.y, n.x, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: Vector3, b: Vector3) -> bool {
        (a.x - b.x).abs() < 1.0e-9 && (a.y - b.y).abs() < 1.0e-9 && (a.z - b.z).abs() < 1.0e-9
    }

    #[test]
    fn xy_plane_lifts_into_world() {
        let plane =
            WorkPlane::from_origin_normal(Vector3::new(0.0, 0.0, 0.0), Vector3::new(0.0, 0.0, 1.0));
        // Origin maps to origin; axes are orthonormal in the z=0 plane.
        assert!(approx(
            plane.lift_point(0.0, 0.0),
            Vector3::new(0.0, 0.0, 0.0)
        ));
        let p = plane.lift_point(1.0, 0.0);
        assert!((p.z).abs() < 1.0e-9, "stays in plane");
    }

    #[test]
    fn tilted_plane_positions_by_frame_not_by_hand() {
        // A plane with normal +X and origin offset: a (u,v) sketch point lands
        // on x = origin.x, lifted purely by the frame.
        let plane = WorkPlane::new(
            Vector3::new(5.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
        );
        let p = plane.lift_point(2.0, 3.0);
        assert!((p.x - 5.0).abs() < 1.0e-9, "in-plane points keep x=5");
    }

    #[test]
    fn axes_are_orthonormal() {
        let plane =
            WorkPlane::from_origin_normal(Vector3::new(1.0, 2.0, 3.0), Vector3::new(1.0, 1.0, 1.0));
        let u = plane.u_axis();
        let v = plane.v_axis();
        let n = plane.normal();
        let dot = |a: Vector3, b: Vector3| a.x * b.x + a.y * b.y + a.z * b.z;
        assert!(dot(u, v).abs() < 1.0e-9);
        assert!(dot(u, n).abs() < 1.0e-9);
        assert!(dot(v, n).abs() < 1.0e-9);
        assert!((dot(u, u) - 1.0).abs() < 1.0e-9);
    }
}
