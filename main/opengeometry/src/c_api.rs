//! C API layer for integration with C/C++ (e.g., Vulkan applications)
//! Build with: `cargo build --release --features c-api`

#![allow(clippy::missing_safety_doc)]

use std::ffi::c_void;
use std::ptr;

use crate::primitives::rectangle::OGRectangle;
use crate::geometry::mesh::{ToMesh};
use openmaths::Vector3;

#[repr(C)]
pub struct OGMesh {
    pub vertex_count: u32,
    pub index_count: u32,
    pub positions: *const f32, // length = vertex_count * 3
    pub normals: *const f32,   // length = vertex_count * 3
    pub indices: *const u32,   // length = index_count
}

#[repr(C)]
pub struct OGMeshOwned {
    pub mesh: OGMesh,
    // We keep ownership of buffers so caller can read then free via og_mesh_free
    positions: Vec<f32>,
    normals: Vec<f32>,
    indices: Vec<u32>,
}

#[no_mangle]
pub extern "C" fn og_rectangle_create(id_ptr: *const u8, id_len: usize) -> *mut OGRectangle {
    let id = unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(id_ptr, id_len)) };
    let rect = OGRectangle::new(id.to_string());
    Box::into_raw(Box::new(rect))
}

#[no_mangle]
pub unsafe extern "C" fn og_rectangle_set_config(rect: *mut OGRectangle, cx: f64, cy: f64, cz: f64, width: f64, breadth: f64) {
    if let Some(r) = rect.as_mut() {
        r.set_config(Vector3::new(cx, cy, cz), width, breadth);
    }
}

#[no_mangle]
pub unsafe extern "C" fn og_rectangle_generate_geometry(rect: *mut OGRectangle) {
    if let Some(r) = rect.as_mut() { r.generate_geometry(); }
}

#[no_mangle]
pub unsafe extern "C" fn og_rectangle_to_mesh(rect: *const OGRectangle) -> *mut OGMeshOwned {
    if let Some(r) = rect.as_ref() {
        let mesh = r.to_mesh();
        let mut positions = Vec::with_capacity(mesh.positions.len()*3);
        let mut normals = Vec::with_capacity(mesh.normals.len()*3);
        for p in &mesh.positions { positions.extend_from_slice(&p[..]); }
        for n in &mesh.normals { normals.extend_from_slice(&n[..]); }
        let indices = mesh.indices.clone();
        let owned = OGMeshOwned {
            mesh: OGMesh {
                vertex_count: mesh.positions.len() as u32,
                index_count: mesh.indices.len() as u32,
                positions: positions.as_ptr(),
                normals: normals.as_ptr(),
                indices: indices.as_ptr(),
            },
            positions,
            normals,
            indices,
        };
        return Box::into_raw(Box::new(owned));
    }
    ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn og_mesh_free(mesh: *mut OGMeshOwned) {
    if !mesh.is_null() { drop(Box::from_raw(mesh)); }
}

#[no_mangle]
pub unsafe extern "C" fn og_rectangle_free(rect: *mut OGRectangle) {
    if !rect.is_null() { drop(Box::from_raw(rect)); }
}
