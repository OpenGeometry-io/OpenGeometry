mod edge_insert;
mod extrusion;
mod face_cut;
mod loop_cut;
mod topology;
mod translation;
mod vertex_remove;

pub(in crate::editor) use loop_cut::collect_closed_quad_edge_ring;
