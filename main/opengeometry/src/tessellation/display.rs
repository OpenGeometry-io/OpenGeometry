use super::mesh::Tessellation;
use super::tessellate::{coordinate_magnitude, tessellate};
use crate::brep::{BrepEnvelope, GeometryError};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq)]
pub struct DisplayBuffers {
    pub positions: Vec<f32>,
    pub normals: Vec<f32>,
    pub indices: Vec<u32>,
    pub face_ranges: Vec<u32>,
    pub outline: Vec<f32>,
    pub edge_ids: Vec<u32>,
    pub origin: [f64; 3],
    pub revision: u64,
    pub bucket: f64,
    pub achieved_deflection: f64,
    pub triangles: u32,
}

pub fn static_bucket(brep: &BrepEnvelope) -> Result<f64, GeometryError> {
    let floor = bucket_floor(brep)?;
    let bounds = brep
        .bounds()?
        .ok_or_else(|| GeometryError::InvalidGeometry("display shape has no bounds".into()))?;
    let diagonal = bounds
        .axes
        .iter()
        .map(|axis| axis.width().powi(2))
        .sum::<f64>()
        .sqrt();
    deflection_bucket((diagonal / 500.0).min(1.0).max(floor))
}

pub fn bucket_floor(brep: &BrepEnvelope) -> Result<f64, GeometryError> {
    brep.validate()?;
    let magnitude = coordinate_magnitude(brep)?;
    let bounds = brep
        .bounds()?
        .ok_or_else(|| GeometryError::InvalidGeometry("display shape has no bounds".into()))?;
    let half_diagonal = bounds
        .axes
        .iter()
        .map(|axis| axis.width().powi(2))
        .sum::<f64>()
        .sqrt()
        / 2.0;
    let floor = 2.0 * (brep.accuracy.geometric + 64.0 * f64::EPSILON * magnitude) / 0.75;
    let encoding = 4.0 * 2.0_f64.powi(-24) * 3.0_f64.sqrt() * half_diagonal;
    let required = floor.max(encoding);
    if !required.is_finite() || required <= 0.0 {
        return Err(limit("display precision floor is out of range"));
    }
    let mut bucket = 2.0_f64.powf(required.log2().ceil());
    if 0.75 * bucket <= 2.0 * (brep.accuracy.geometric + 64.0 * f64::EPSILON * magnitude) {
        bucket *= 2.0;
    }
    if !bucket.is_finite() || bucket <= 0.0 {
        return Err(limit("display precision floor is out of range"));
    }
    Ok(bucket)
}

pub(super) fn limit(message: &str) -> GeometryError {
    GeometryError::LimitExceeded(message.into())
}

pub fn deflection_bucket(target: f64) -> Result<f64, GeometryError> {
    if !target.is_finite() || target <= 0.0 {
        return Err(GeometryError::InvalidGeometry(
            "invalid display deflection".into(),
        ));
    }
    let bucket = 2.0_f64.powf(target.log2().floor());
    if !bucket.is_finite() || bucket <= 0.0 {
        return Err(limit("display deflection is out of range"));
    }
    Ok(bucket)
}

pub fn display_buffers(
    brep: &BrepEnvelope,
    bucket: f64,
    max_triangles: usize,
) -> Result<DisplayBuffers, GeometryError> {
    if !bucket.is_finite() || bucket <= 0.0 || bucket < bucket_floor(brep)? {
        return Err(limit("display bucket is below the precision floor"));
    }
    if max_triangles == 0 || max_triangles > 2_000_000 {
        return Err(limit("display triangle budget"));
    }
    let mesh = tessellate(brep, 0.75 * bucket, max_triangles)?;
    check_display_mesh(brep, &mesh)?;
    let origin = display_origin(&mesh);
    let mut encoding_error = 0.0;
    let positions = encode(&mesh.positions, origin, &mut encoding_error)?;
    let outline = encode(&mesh.outline_positions, origin, &mut encoding_error)?;
    if !encoding_error.is_finite()
        || encoding_error > bucket / 4.0
        || 0.75 * bucket + encoding_error > bucket
    {
        return Err(limit("display encoding exceeds requested deflection"));
    }
    let face_ranges = display_face_ranges(&mesh)?;
    Ok(DisplayBuffers {
        positions,
        normals: mesh.normals,
        indices: mesh.indices,
        face_ranges,
        outline,
        edge_ids: mesh.outline_edge_ids,
        origin,
        revision: mesh.revision,
        bucket,
        achieved_deflection: 0.75 * bucket + encoding_error,
        triangles: mesh.triangle_face_ids.len() as u32,
    })
}

fn check_display_mesh(brep: &BrepEnvelope, mesh: &Tessellation) -> Result<(), GeometryError> {
    if mesh.positions.len() % 3 != 0
        || mesh.normals.len() != mesh.positions.len()
        || mesh.indices.len() % 3 != 0
        || mesh.indices.len() > 6_000_000
        || mesh.triangle_face_ids.len() != mesh.indices.len() / 3
        || mesh.outline_positions.len() % 6 != 0
        || mesh.outline_positions.len() > 12_000_000
        || mesh.outline_edge_ids.len() != mesh.outline_positions.len() / 6
    {
        return Err(limit("invalid or oversized display arrays"));
    }
    let vertex_count = mesh.positions.len() / 3;
    if mesh
        .indices
        .iter()
        .any(|&index| index as usize >= vertex_count)
        || mesh
            .triangle_face_ids
            .iter()
            .any(|&face| face as usize >= brep.topology.faces.len())
        || mesh
            .outline_edge_ids
            .iter()
            .any(|&edge| edge as usize >= brep.topology.edges.len())
        || mesh
            .positions
            .iter()
            .chain(&mesh.outline_positions)
            .any(|value| !value.is_finite())
        || mesh.normals.iter().any(|value| !value.is_finite())
    {
        return Err(GeometryError::InvalidGeometry(
            "invalid display coordinates or topology mapping".into(),
        ));
    }
    Ok(())
}

fn display_origin(mesh: &Tessellation) -> [f64; 3] {
    let reference = if mesh.positions.is_empty() {
        &mesh.outline_positions
    } else {
        &mesh.positions
    };
    let mut low = [f64::INFINITY; 3];
    let mut high = [f64::NEG_INFINITY; 3];
    for point in reference.chunks_exact(3) {
        for axis in 0..3 {
            low[axis] = low[axis].min(point[axis]);
            high[axis] = high[axis].max(point[axis]);
        }
    }
    if reference.is_empty() {
        [0.0; 3]
    } else {
        std::array::from_fn(|axis| low[axis] / 2.0 + high[axis] / 2.0)
    }
}

pub(super) fn encode(
    values: &[f64],
    origin: [f64; 3],
    error: &mut f64,
) -> Result<Vec<f32>, GeometryError> {
    let mut encoded = Vec::with_capacity(values.len());
    for point in values.chunks_exact(3) {
        let mut errors = [0.0; 3];
        for axis in 0..3 {
            let local = point[axis] - origin[axis];
            let rounded = local as f32;
            if !local.is_finite() || !rounded.is_finite() {
                return Err(limit("display coordinate encoding is nonfinite"));
            }
            errors[axis] = (local - f64::from(rounded)).abs();
            encoded.push(rounded);
        }
        *error = error.max(errors[0].hypot(errors[1]).hypot(errors[2]));
    }
    Ok(encoded)
}

fn display_face_ranges(mesh: &Tessellation) -> Result<Vec<u32>, GeometryError> {
    let mut face_ranges = Vec::new();
    let mut seen = BTreeSet::new();
    for (triangle, &face) in mesh.triangle_face_ids.iter().enumerate() {
        if face_ranges.len() >= 3 && face_ranges[face_ranges.len() - 3] == face {
            let last = face_ranges.len() - 1;
            face_ranges[last] += 1;
            continue;
        }
        if !seen.insert(face) {
            return Err(GeometryError::InvalidTopology(
                "face triangles are not contiguous".into(),
            ));
        }
        face_ranges.extend([face, triangle as u32, 1]);
    }
    Ok(face_ranges)
}
