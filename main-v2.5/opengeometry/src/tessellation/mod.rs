#[cfg(test)]
mod cache;
mod curved_face;
mod deviation;
pub mod display;
mod edges;
mod grid;
mod mesh;
mod planar_face;
mod snapshot_store;
mod tessellate;
#[cfg(test)]
mod tests;
mod trim;
mod trimmed_face;

pub use mesh::Tessellation;
pub use snapshot_store::SnapshotStore;
pub use tessellate::tessellate;
