use crate::brep::{BrepEnvelope, GeometryQuality};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct StepReport {
    pub(super) schema_version: u32,
    pub(super) revision: String,
    pub(super) length_unit: String,
    pub(super) quality: GeometryQuality,
    pub(super) faces: usize,
    pub(super) edges: usize,
    pub(crate) solids: usize,
    pub(super) cavity_shells: usize,
    pub(super) collapsed_chart_uses: usize,
    pub(super) geometric_tolerance: f64,
    pub exchange_error_bound: f64,
    pub(super) validation_level: &'static str,
}

#[derive(Debug)]
pub(crate) struct StepBodyInput<'a> {
    pub(crate) og_id: &'a str,
    pub(crate) shape_id: &'a str,
    pub(crate) shape_revision: u64,
    pub(crate) brep: &'a BrepEnvelope,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StepBodyReport {
    pub og_id: String,
    pub(super) shape_id: String,
    pub(super) shape_revision: u64,
    pub(super) solids: usize,
    pub(super) faces: usize,
    pub(super) edges: usize,
    pub(super) cavity_shells: usize,
    pub(super) collapsed_chart_uses: usize,
    pub(super) fitted_curves: usize,
    pub pcurveless_edges: usize,
    pub(super) exchange_error_bound: f64,
    pub(super) geometric_tolerance: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StepSkipped {
    pub og_id: String,
    pub(crate) reason: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StepExportReport {
    pub(super) unit: String,
    pub up_axis: String,
    pub(super) timestamp: String,
    pub products: usize,
    pub solids: usize,
    pub faces: usize,
    pub edges: usize,
    pub(super) cavity_shells: usize,
    pub(super) collapsed_chart_uses: usize,
    pub(super) fitted_curves: usize,
    pub pcurveless_edges: usize,
    pub entities: usize,
    pub(super) bytes: usize,
    pub(super) exchange_error_bound: f64,
    pub(super) geometric_tolerance: f64,
    pub(super) validation_level: &'static str,
    pub bodies: Vec<StepBodyReport>,
    pub skipped: Vec<StepSkipped>,
}
