#![cfg(not(target_arch = "wasm32"))]

mod batch;
mod batch_case;
mod batch_route;
mod batch_staged;
mod body;
mod boolean_case;
mod boolean_fixtures;
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
mod contract;
mod coverage;
mod creating;
mod digest;
mod display;
mod extras;
mod fine_solids;
mod fixtures;
mod golden_file;
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
mod planar_shapes;
mod profiles;
mod record;
mod runner;
mod shell;
mod step;
mod volume;

use golden_file::Mode;
use record::{Failure, Findings};
use runner::Case;
use std::collections::BTreeSet;
use std::env;
use std::path::PathBuf;

fn corpus() -> Result<Vec<Case>, Failure> {
    Ok([
        builders_solids::cases(),
        builders_profiles::cases(),
        matrix::cases()?,
        boolean_fixtures::cases(),
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

fn check_findings(
    expected: &BTreeSet<String>,
    findings: Findings,
    uncovered: &[String],
) -> Result<(), Failure> {
    let seen = findings.covered.union(&findings.staged_handlers);
    let unscanned = coverage::unscanned_names(expected, seen);
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

fn environment_mode() -> Mode {
    let variable = |name| env::var(name).ok();
    golden_file::mode(
        variable("OG_GOLDEN_UPDATE").as_deref(),
        variable("CI").as_deref(),
        env::var_os("OG_GOLDEN_DUMP").map(PathBuf::from),
    )
}

#[test]
fn every_golden_scene_matches_the_recorded_golden_for_this_target() {
    let mode = environment_mode();
    let expected = coverage::expected_names().unwrap();
    let cases = corpus().unwrap();
    let run = runner::run(&cases).unwrap();
    let listings = coverage::listings(&expected, &run.findings.covered);
    println!(
        "{} cases, {} expected names, {} covered names, {} uncovered entries, {} fixture disagreements",
        cases.len(),
        expected.len(),
        run.findings.covered.len(),
        listings.uncovered.len(),
        run.findings.disagreements.len()
    );
    let mut texts = run.records;
    texts.extend(listings.files.map(|(name, text)| (name.to_string(), text)));
    if let Mode::Dump(directory) = &mode {
        golden_file::dump(directory, &texts).unwrap();
    }
    check_findings(&expected, run.findings, &listings.uncovered).unwrap();
    if matches!(mode, Mode::Dump(_)) {
        return;
    }
    texts.extend(extras::records().unwrap());
    let mut actual = golden_file::digests(&texts);
    actual.insert(
        extras::SWEEP_KEY.to_string(),
        extras::sweep_digest(&extras::sweep_outputs().unwrap()),
    );
    let target = golden_file::current_target();
    if mode == Mode::Record {
        let file = golden_file::record(&target, &actual).unwrap();
        println!("recorded {} keys into {}", actual.len(), file.display());
        return;
    }
    if mode == Mode::Build {
        println!(
            "{} scenes built; the golden comparison does not run on CI",
            actual.len()
        );
        return;
    }
    let Some(golden) = golden_file::committed(&target).unwrap() else {
        println!(
            "{} scenes built; no golden is recorded for {target}, so nothing is compared",
            actual.len()
        );
        return;
    };
    if let Err(mismatch) = golden_file::compare(&golden, &actual) {
        let directory = golden_file::write_differing(&target, &mismatch.differing, &texts).unwrap();
        panic!(
            "{target}: {mismatch}\nthe texts of the differing scenes are in {}",
            directory.display()
        );
    }
    println!("{} scenes match the golden for {target}", actual.len());
}
