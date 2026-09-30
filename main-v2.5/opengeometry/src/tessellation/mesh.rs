use crate::brep::GeometryError;
use crate::math::{cross, dot, norm, sub, Point3};

#[derive(Clone, Debug)]
pub struct Tessellation {
    pub positions: Vec<f64>,
    pub normals: Vec<f32>,
    pub indices: Vec<u32>,
    pub triangle_face_ids: Vec<u32>,
    pub outline_positions: Vec<f64>,
    pub outline_edge_ids: Vec<u32>,
    pub revision: u64,
    pub achieved_deflection: f64,
}
impl Tessellation {
    pub(super) fn new(revision: u64, deflection: f64) -> Self {
        Self {
            positions: Vec::new(),
            normals: Vec::new(),
            indices: Vec::new(),
            triangle_face_ids: Vec::new(),
            outline_positions: Vec::new(),
            outline_edge_ids: Vec::new(),
            revision,
            achieved_deflection: deflection,
        }
    }
    pub(super) fn vertex(&mut self, p: Point3, n: Point3) -> Result<u32, GeometryError> {
        if p.into_iter().chain(n).any(|v| !v.is_finite()) {
            return Err(GeometryError::InvalidGeometry(
                "nonfinite tessellation output".into(),
            ));
        }
        let id = u32::try_from(self.positions.len() / 3)
            .map_err(|_| GeometryError::LimitExceeded("tessellation vertices".into()))?;
        self.positions.extend(p);
        self.normals.extend(n.map(|v| v as f32));
        Ok(id)
    }
    pub(super) fn point(&self, id: u32) -> Point3 {
        let i = id as usize * 3;
        [
            self.positions[i],
            self.positions[i + 1],
            self.positions[i + 2],
        ]
    }
    pub(super) fn triangle(
        &mut self,
        mut ids: [u32; 3],
        face: u32,
        normal: Point3,
        max_triangles: usize,
    ) -> Result<(), GeometryError> {
        let [a, b, c] = ids.map(|id| self.point(id));
        let direction = cross(sub(b, a), sub(c, a));
        if direction.iter().any(|v| !v.is_finite()) {
            return Err(GeometryError::UnresolvedTessellation(
                "triangle orientation exceeds arithmetic range".into(),
            ));
        }
        if norm(direction) == 0.0 {
            return Ok(());
        }
        if self.triangle_face_ids.len() >= max_triangles {
            return Err(GeometryError::LimitExceeded(
                "tessellation triangles".into(),
            ));
        }
        let orientation = dot(direction, normal);
        if !orientation.is_finite() || orientation == 0.0 {
            return Err(GeometryError::UnresolvedTessellation(
                "triangle orientation could not be resolved".into(),
            ));
        }
        if orientation < 0.0 {
            ids.swap(1, 2);
        }
        self.indices.extend(ids);
        self.triangle_face_ids.push(face);
        Ok(())
    }
    #[cfg(test)]
    pub(super) fn bytes(&self) -> usize {
        self.positions.len() * 8
            + self.normals.len() * 4
            + self.indices.len() * 4
            + self.triangle_face_ids.len() * 4
            + self.outline_positions.len() * 8
            + self.outline_edge_ids.len() * 4
    }
}
