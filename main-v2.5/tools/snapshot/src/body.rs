use crate::classify::classify_grid;
use crate::display::record_display_buffers;
use crate::json::record_brep;
use crate::kernel::{BodyType, BrepEnvelope};
use crate::mesh::record_tessellations;
use crate::record::Record;
use crate::step::record_step_exports;
use crate::volume::record_volume;

pub(crate) fn record_body(record: &mut Record, label: &str, shape: &BrepEnvelope) {
    record_brep(record, label, shape);
    let body_type = shape.body_type();
    record.section(&format!("{label}.body-type"));
    record.debug("body_type", &body_type);
    let mesh = record_tessellations(record, label, shape);
    record_display_buffers(record, label, shape);
    record_step_exports(record, label, shape);
    if matches!(body_type, Ok(BodyType::Solid)) {
        classify_grid(record, label, shape);
        if let Some(mesh) = mesh {
            record_volume(record, label, shape, &mesh);
        }
    }
}
