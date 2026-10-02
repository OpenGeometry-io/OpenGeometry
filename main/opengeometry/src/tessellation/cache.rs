use super::mesh::Tessellation;
use super::tessellate::tessellate;
use crate::brep::{BrepEnvelope, GeometryError};
use std::collections::VecDeque;
use std::hash::{DefaultHasher, Hasher};
use std::io::Write;
use std::sync::Arc;

struct HashWriter(DefaultHasher);
impl Write for HashWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.write(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
struct CacheEntry {
    body: u64,
    signature: u64,
    deflection: u64,
    max_triangles: usize,
    mesh: Arc<Tessellation>,
}
pub(super) struct TessellationCache {
    entries: VecDeque<CacheEntry>,
    max_bytes: usize,
}
impl TessellationCache {
    pub(super) fn new(max_bytes: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            max_bytes,
        }
    }
    pub(super) fn tessellate(
        &mut self,
        brep: &BrepEnvelope,
        deflection: f64,
        max_triangles: usize,
    ) -> Result<Arc<Tessellation>, GeometryError> {
        brep.validate()?;
        let mut writer = HashWriter(DefaultHasher::new());
        serde_json::to_writer(&mut writer, brep)
            .map_err(|e| GeometryError::InvalidGeometry(e.to_string()))?;
        let signature = writer.0.finish();
        if let Some(i) = self.entries.iter().position(|e| {
            e.signature == signature
                && e.deflection == deflection.to_bits()
                && e.max_triangles == max_triangles
        }) {
            let entry = self
                .entries
                .remove(i)
                .ok_or_else(|| GeometryError::InvalidGeometry("cache entry missing".into()))?;
            let mesh = Arc::clone(&entry.mesh);
            self.entries.push_back(entry);
            return Ok(mesh);
        }
        let mesh = Arc::new(tessellate(brep, deflection, max_triangles)?);
        if mesh.bytes() > self.max_bytes {
            return Ok(mesh);
        }
        let mut hasher = DefaultHasher::new();
        hasher.write(brep.id.as_bytes());
        let body = hasher.finish();
        while self.entries.iter().filter(|e| e.body == body).count() >= 3 {
            if let Some(i) = self.entries.iter().position(|e| e.body == body) {
                self.entries.remove(i);
            } else {
                break;
            }
        }
        while self.entries.iter().map(|e| e.mesh.bytes()).sum::<usize>() + mesh.bytes()
            > self.max_bytes
        {
            self.entries.pop_front();
        }
        self.entries.push_back(CacheEntry {
            body,
            signature,
            deflection: deflection.to_bits(),
            max_triangles,
            mesh: Arc::clone(&mesh),
        });
        Ok(mesh)
    }
    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }
}
