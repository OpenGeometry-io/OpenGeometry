use crate::kernel::GeometryError;

pub(crate) enum BatchOutcome<'a> {
    Succeeded(&'a [String]),
    Failed(&'a GeometryError),
}

struct BatchRoute {
    name: &'static str,
    taken: fn(usize, &BatchOutcome) -> bool,
}

fn coverage_gap<'a>(outcome: &'a BatchOutcome) -> Option<&'a [String; 2]> {
    match outcome {
        BatchOutcome::Failed(GeometryError::CoverageGap { families }) => Some(families),
        _ => None,
    }
}

fn mixed_batch_gap<'a>(outcome: &'a BatchOutcome) -> Option<&'a String> {
    coverage_gap(outcome)
        .filter(|families| families[0] == "mixed cutter batch")
        .map(|families| &families[1])
}

fn nested_handler(outcome: &BatchOutcome, name: &str) -> bool {
    match outcome {
        BatchOutcome::Succeeded(handlers) => handlers.get(1).is_some_and(|handler| handler == name),
        BatchOutcome::Failed(_) => false,
    }
}

fn cutter_count_rejected(_: usize, outcome: &BatchOutcome) -> bool {
    matches!(
        outcome,
        BatchOutcome::Failed(GeometryError::InvalidGeometry(message))
            if message.starts_with("planar batch subtraction requires")
    )
}

fn overlapping_rejected(_: usize, outcome: &BatchOutcome) -> bool {
    coverage_gap(outcome).is_some_and(|families| families[0] == "overlapping planar batch cutters")
}

fn mixed_planar_stage_gap(_: usize, outcome: &BatchOutcome) -> bool {
    mixed_batch_gap(outcome).is_some_and(|stage| stage == "planar stage")
}

fn mixed_cylindrical_stage_gap(_: usize, outcome: &BatchOutcome) -> bool {
    mixed_batch_gap(outcome).is_some_and(|stage| stage != "planar stage")
}

fn single_cutter(cutters: usize, _: &BatchOutcome) -> bool {
    cutters == 1
}

fn mixed_staged(_: usize, outcome: &BatchOutcome) -> bool {
    nested_handler(outcome, "subtract_planar_cutters")
}

fn prismatic_profile(_: usize, outcome: &BatchOutcome) -> bool {
    nested_handler(outcome, "subtract_prismatic_profile_batch")
}

fn vertical_arc(_: usize, outcome: &BatchOutcome) -> bool {
    nested_handler(outcome, "subtract_vertical_arc_extrusion_batch")
}

fn rectilinear(_: usize, outcome: &BatchOutcome) -> bool {
    nested_handler(outcome, "boolean_rectilinear")
}

const BATCH_ROUTES: [BatchRoute; 9] = [
    BatchRoute {
        name: "batch-route:cutter-count-rejected",
        taken: cutter_count_rejected,
    },
    BatchRoute {
        name: "batch-route:overlapping-rejected",
        taken: overlapping_rejected,
    },
    BatchRoute {
        name: "batch-route:mixed-planar-stage-gap",
        taken: mixed_planar_stage_gap,
    },
    BatchRoute {
        name: "batch-route:mixed-cylindrical-stage-gap",
        taken: mixed_cylindrical_stage_gap,
    },
    BatchRoute {
        name: "batch-route:single-cutter",
        taken: single_cutter,
    },
    BatchRoute {
        name: "batch-route:mixed-staged",
        taken: mixed_staged,
    },
    BatchRoute {
        name: "batch-route:prismatic-profile",
        taken: prismatic_profile,
    },
    BatchRoute {
        name: "batch-route:vertical-arc",
        taken: vertical_arc,
    },
    BatchRoute {
        name: "batch-route:rectilinear",
        taken: rectilinear,
    },
];

pub(crate) fn batch_route(cutters: usize, outcome: &BatchOutcome) -> &'static str {
    match BATCH_ROUTES
        .iter()
        .find(|route| (route.taken)(cutters, outcome))
    {
        Some(route) => route.name,
        None if matches!(outcome, BatchOutcome::Succeeded(_)) => "batch-route:other-success",
        None => "batch-route:other-failure",
    }
}

pub(crate) fn batch_route_names() -> impl Iterator<Item = &'static str> {
    BATCH_ROUTES.iter().map(|route| route.name)
}
