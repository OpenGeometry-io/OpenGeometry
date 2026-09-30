mod batch;
mod batch_case;
mod batch_route;
mod batch_staged;
mod body;
mod boolean_case;
mod booleans_curved;
mod booleans_cylinders;
mod booleans_direct;
mod booleans_oblique;
mod booleans_planar;
mod booleans_spheres;
mod builder_case;
mod builders_profiles;
mod builders_solids;
mod classify;
mod coverage;
mod creating;
mod digest;
mod display;
mod fine_solids;
mod fixtures;
mod graph_case;
mod graph_operate;
mod graph_record;
mod graph_scenarios;
mod graph_session;
mod graph_state;
mod handler_source;
mod json;
mod kernel;
mod matrix;
mod mesh;
mod parity_fixtures;
mod planar_shapes;
mod profiles;
mod record;
mod runner;
mod shell;
mod step;
mod volume;

use record::Failure;
use runner::Case;
use std::path::PathBuf;

fn corpus() -> Result<Vec<Case>, Failure> {
    Ok([
        builders_solids::cases(),
        builders_profiles::cases(),
        matrix::cases()?,
        parity_fixtures::cases(),
        booleans_cylinders::cases(),
        booleans_spheres::cases(),
        booleans_direct::cases(),
        booleans_planar::cases(),
        booleans_oblique::cases(),
        booleans_curved::cases(),
        batch::cases(),
        batch_staged::cases(),
        shell::cases(),
        creating::cases(),
        graph_session::cases(),
        graph_operate::cases(),
        graph_scenarios::cases(),
    ]
    .into_iter()
    .flatten()
    .collect())
}

fn main() -> Result<(), Failure> {
    let output = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: cargo run --offline -- <out-dir>")?;
    let expected = coverage::expected_names()?;
    let cases = corpus()?;
    let findings = runner::run(&cases, &output)?;
    let uncovered = coverage::write_listings(&output, &expected, &findings.covered)?;
    println!(
        "{} cases, {} expected names, {} covered names, {} uncovered entries, {} fixture disagreements",
        cases.len(),
        expected.len(),
        findings.covered.len(),
        uncovered.len(),
        findings.disagreements.len()
    );
    let seen = findings.covered.union(&findings.staged_handlers);
    let unscanned = coverage::unscanned_names(&expected, seen);
    if !unscanned.is_empty() {
        return Err(format!(
            "every handler seen at runtime must be in the scanned set, only batch-route:other-* is exempt; not scanned: {}",
            unscanned.join(", ")
        )
        .into());
    }
    if !findings.broken_cases.is_empty() {
        let broken = findings.broken_cases.into_iter().collect::<Vec<_>>();
        return Err(format!(
            "every matrix case must finish without panicking or returning an error and record the result and handlers verdicts; broken: {}",
            broken.join(", ")
        )
        .into());
    }
    if !uncovered.is_empty() {
        return Err(format!("uncovered handlers or routes: {}", uncovered.join(", ")).into());
    }
    if !findings.disagreements.is_empty() {
        let disagreements = findings.disagreements.into_iter().collect::<Vec<_>>();
        return Err(format!("fixture disagreements: {}", disagreements.join(", ")).into());
    }
    Ok(())
}
