use crate::digest::{bits, sha256_words, triples};
use crate::kernel::{tessellate, BrepEnvelope, Tessellation};
use crate::record::Record;

pub(crate) const VOLUME_DEFLECTION: f64 = 0.01;
const DEFLECTIONS: [f64; 2] = [VOLUME_DEFLECTION, 0.05];
const MAX_TRIANGLES: usize = 2_000_000;

pub(crate) fn record_tessellations(
    record: &mut Record,
    label: &str,
    brep: &BrepEnvelope,
) -> Option<Tessellation> {
    let mut volume_mesh = None;
    for deflection in DEFLECTIONS {
        let title = format!("{label}.tessellate@{deflection}");
        match tessellate(brep, deflection, MAX_TRIANGLES) {
            Ok(mesh) => {
                write_mesh(record, &title, &mesh);
                if deflection.to_bits() == VOLUME_DEFLECTION.to_bits() {
                    volume_mesh = Some(mesh);
                }
            }
            Err(error) => record.error(&title, error),
        }
    }
    volume_mesh
}

fn write_mesh(record: &mut Record, title: &str, mesh: &Tessellation) {
    record.section(title);
    record.field("revision", mesh.revision);
    record.field("achieved_deflection", bits(mesh.achieved_deflection));
    record.field("vertices", mesh.positions.len() / 3);
    record.field("triangles", mesh.triangle_face_ids.len());
    record.field("outline_segments", mesh.outline_edge_ids.len());
    record.field("positions.sha256", sha256_words(&mesh.positions));
    record.field("normals.sha256", sha256_words(&mesh.normals));
    record.field("indices.sha256", sha256_words(&mesh.indices));
    record.field(
        "triangle_face_ids.sha256",
        sha256_words(&mesh.triangle_face_ids),
    );
    record.field(
        "outline_positions.sha256",
        sha256_words(&mesh.outline_positions),
    );
    record.field(
        "outline_edge_ids.sha256",
        sha256_words(&mesh.outline_edge_ids),
    );
    record.block(&format!("{title}.positions"), &triples(&mesh.positions));
}
