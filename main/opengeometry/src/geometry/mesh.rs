use openmaths::Vector3;

/// CPU-side mesh buffers suitable for transfer to graphics APIs.
pub struct MeshBuffers {
    pub positions: Vec<[f32;3]>,
    pub normals: Vec<[f32;3]>,
    pub indices: Vec<u32>,
}

impl MeshBuffers {
    pub fn empty() -> Self { Self { positions: Vec::new(), normals: Vec::new(), indices: Vec::new() } }
    pub fn vertex_count(&self) -> usize { self.positions.len() }
    pub fn index_count(&self) -> usize { self.indices.len() }
}

/// Trait for converting primitives/BReps into mesh buffers.
pub trait ToMesh {
    fn to_mesh(&self) -> MeshBuffers;
}

/// Utility to compute a flat normal for a planar polygon (assumes non-degenerate and coplanar)
pub fn polygon_flat_normal(verts: &[Vector3]) -> [f32;3] {
    if verts.len() < 3 { return [0.0, 1.0, 0.0]; }
    // Newell's method (robust for simple polygons)
    let mut nx = 0.0f64; let mut ny = 0.0f64; let mut nz = 0.0f64;
    for i in 0..verts.len() {
        let current = &verts[i];
        let next = &verts[(i+1)%verts.len()];
        nx += (current.y - next.y) * (current.z + next.z);
        ny += (current.z - next.z) * (current.x + next.x);
        nz += (current.x - next.x) * (current.y + next.y);
    }
    let len = (nx*nx + ny*ny + nz*nz).sqrt();
    if len == 0.0 { return [0.0,1.0,0.0]; }
    [ (nx/len) as f32, (ny/len) as f32, (nz/len) as f32 ]
}
