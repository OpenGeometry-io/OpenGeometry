use crate::digest::sha256;
use crate::json::pretty;
use crate::kernel::{export_step, BrepEnvelope};
use crate::record::Record;
use serde_json::to_value;

pub(crate) fn record_step_exports(record: &mut Record, label: &str, brep: &BrepEnvelope) {
    for unit in ["metre", "millimetre"] {
        let title = format!("{label}.step.{unit}");
        match export_step(brep, unit) {
            Ok((text, report)) => {
                record.section(&format!("{title}.report"));
                record.field("text.sha256", sha256(text.as_bytes()));
                record.field("text.bytes", text.len());
                record.line(&pretty(to_value(&report)));
                record.block(&format!("{title}.text"), &text);
            }
            Err(error) => record.error(&title, error),
        }
    }
}
