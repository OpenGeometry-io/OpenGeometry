use crate::digest::{bits, bits_list, sha256_words};
use crate::kernel::{bucket_floor, display_buffers, static_bucket, BrepEnvelope, DisplayBuffers};
use crate::record::Record;

const MAX_TRIANGLES: usize = 2_000_000;

pub(crate) fn record_display_buffers(record: &mut Record, label: &str, brep: &BrepEnvelope) {
    let title = format!("{label}.display");
    record.section(&title);
    match bucket_floor(brep) {
        Ok(floor) => record.field("bucket_floor", bits(floor)),
        Err(error) => record.debug("bucket_floor.error", error),
    }
    let bucket = match static_bucket(brep) {
        Ok(bucket) => bucket,
        Err(error) => {
            record.debug("static_bucket.error", error);
            return;
        }
    };
    record.field("static_bucket", bits(bucket));
    for (name, value) in [
        ("finer", bucket / 2.0),
        ("static", bucket),
        ("coarser", bucket * 2.0),
    ] {
        let section = format!("{title}.{name}");
        match display_buffers(brep, value, MAX_TRIANGLES) {
            Ok(buffers) => write_buffers(record, &section, value, &buffers),
            Err(error) => record.error(&section, error),
        }
    }
}

pub(crate) fn write_buffers(
    record: &mut Record,
    title: &str,
    requested: f64,
    buffers: &DisplayBuffers,
) {
    record.section(title);
    record.field("requested_bucket", bits(requested));
    record.field("bucket", bits(buffers.bucket));
    record.field("achieved_deflection", bits(buffers.achieved_deflection));
    record.field("origin", bits_list(&buffers.origin));
    record.field("revision", buffers.revision);
    record.field("triangles", buffers.triangles);
    record.field("positions.len", buffers.positions.len());
    record.field("positions.sha256", sha256_words(&buffers.positions));
    record.field("normals.sha256", sha256_words(&buffers.normals));
    record.field("indices.len", buffers.indices.len());
    record.field("indices.sha256", sha256_words(&buffers.indices));
    record.field("face_ranges.len", buffers.face_ranges.len());
    record.field("face_ranges.sha256", sha256_words(&buffers.face_ranges));
    record.field("outline.len", buffers.outline.len());
    record.field("outline.sha256", sha256_words(&buffers.outline));
    record.field("edge_ids.len", buffers.edge_ids.len());
    record.field("edge_ids.sha256", sha256_words(&buffers.edge_ids));
}
