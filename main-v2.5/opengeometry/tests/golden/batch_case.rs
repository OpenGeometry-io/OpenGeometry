use crate::batch_route::{batch_route, BatchOutcome};
use crate::boolean_case::result_body;
use crate::json::brep_sha256;
use crate::kernel::{subtract_planar_cutters_with_handlers, BrepEnvelope};
use crate::record::{Failure, Record};
use crate::runner::Case;
use std::sync::Arc;

pub(crate) type Inputs =
    Arc<dyn Fn() -> Result<(BrepEnvelope, Vec<BrepEnvelope>), Failure> + Send + Sync>;

pub(crate) type BatchBuilt = Result<(BrepEnvelope, Vec<BrepEnvelope>), Failure>;

pub(crate) type Batch = fn() -> BatchBuilt;

pub(crate) type BatchEntry = (&'static str, &'static str, Batch);

pub(crate) fn inputs(
    build: impl Fn() -> Result<(BrepEnvelope, Vec<BrepEnvelope>), Failure> + Send + Sync + 'static,
) -> Inputs {
    Arc::new(build)
}

pub(crate) fn reversed(inputs: &Inputs) -> Inputs {
    let inputs = Arc::clone(inputs);
    Arc::new(move || {
        let (host, mut cutters) = inputs()?;
        cutters.reverse();
        Ok((host, cutters))
    })
}

fn record_inputs(record: &mut Record, host: &BrepEnvelope, cutters: &[BrepEnvelope]) {
    record.section("inputs");
    record.field("host", &host.id);
    record.field("host.sha256", brep_sha256(host));
    record.field("cutters", cutters.len());
    for cutter in cutters {
        record.field(&format!("cutter {}", cutter.id), brep_sha256(cutter));
    }
}

pub(crate) fn batch(name: &str, id: &str, inputs: &Inputs) -> Case {
    let inputs = Arc::clone(inputs);
    let id = id.to_string();
    Case::new(format!("batch.{name}"), move |record| {
        let (host, cutters) = inputs()?;
        record_inputs(record, &host, &cutters);
        record.section("batch");
        record.field("id", &id);
        match subtract_planar_cutters_with_handlers(&host, &cutters, id.clone()) {
            Ok((result, handlers)) => {
                let route = batch_route(cutters.len(), &BatchOutcome::Succeeded(&handlers));
                record.field("route", route);
                record.debug("handlers", &handlers);
                record.cover(route);
                record.cover_all(&handlers);
                result_body(record, Ok(result));
            }
            Err(error) => {
                let route = batch_route(cutters.len(), &BatchOutcome::Failed(&error));
                record.field("route", route);
                record.cover(route);
                result_body(record, Err(error));
            }
        }
        Ok(())
    })
}

pub(crate) fn batch_table(entries: &[BatchEntry]) -> Vec<Case> {
    entries
        .iter()
        .map(|(name, id, build)| batch(name, id, &inputs(*build)))
        .collect()
}

pub(crate) fn both_orders(entries: &[BatchEntry]) -> Vec<Case> {
    entries
        .iter()
        .flat_map(|(name, id, build)| {
            let forward = inputs(*build);
            let backward = reversed(&forward);
            [
                batch(&format!("{name}.forward"), id, &forward),
                batch(&format!("{name}.reverse"), id, &backward),
            ]
        })
        .collect()
}
