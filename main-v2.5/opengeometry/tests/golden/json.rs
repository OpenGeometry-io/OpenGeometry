use crate::digest::sha256;
use crate::kernel::BrepEnvelope;
use crate::record::Record;
use serde_json::Value;

pub(crate) fn pretty(value: serde_json::Result<Value>) -> String {
    match value.and_then(|value| serde_json::to_string_pretty(&value)) {
        Ok(text) => text,
        Err(error) => format!("serialization error: {error:?}"),
    }
}

pub(crate) fn pretty_text(json: &str) -> String {
    pretty(serde_json::from_str(json))
}

pub(crate) fn brep_sha256(brep: &BrepEnvelope) -> String {
    match brep.to_json() {
        Ok(json) => sha256(json.as_bytes()),
        Err(error) => format!("error: {error:?}"),
    }
}

pub(crate) fn record_brep(record: &mut Record, label: &str, brep: &BrepEnvelope) {
    match brep.to_json() {
        Ok(json) => {
            record.section(&format!("{label}.brep"));
            record.field("sha256", sha256(json.as_bytes()));
            record.field("bytes", json.len());
            record.block(&format!("{label}.brep.json"), &pretty_text(&json));
        }
        Err(error) => record.error(&format!("{label}.brep"), error),
    }
}
