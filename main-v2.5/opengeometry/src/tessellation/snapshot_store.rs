use super::display::{display_buffers, limit, DisplayBuffers};
use crate::brep::{BrepEnvelope, GeometryError};
use std::collections::BTreeMap;

#[derive(Default)]
pub struct SnapshotStore {
    next_slot: u32,
    snapshots: BTreeMap<u32, BrepEnvelope>,
}

impl SnapshotStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load(&mut self, bytes: &[u8]) -> Result<u32, GeometryError> {
        if bytes.len() > 64 * 1024 * 1024 {
            return Err(limit("snapshot exceeds 64 MiB"));
        }
        let json = std::str::from_utf8(bytes)
            .map_err(|_| GeometryError::InvalidGeometry("snapshot is not UTF-8".into()))?;
        let brep = BrepEnvelope::from_json(json)?;
        let slot = self.next_slot;
        self.next_slot = self
            .next_slot
            .checked_add(1)
            .ok_or_else(|| limit("tessellator slots"))?;
        self.snapshots.insert(slot, brep);
        Ok(slot)
    }

    pub fn buffers(
        &self,
        slot: u32,
        bucket: f64,
        max_triangles: usize,
    ) -> Result<DisplayBuffers, GeometryError> {
        let brep = self
            .snapshots
            .get(&slot)
            .ok_or_else(|| GeometryError::MissingReference {
                kind: "snapshot slot".into(),
                index: slot,
            })?;
        display_buffers(brep, bucket, max_triangles)
    }

    pub fn release(&mut self, slot: u32) -> bool {
        self.snapshots.remove(&slot).is_some()
    }
}
