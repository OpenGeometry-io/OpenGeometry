use crate::brep::{FaceRole, Frame3, GeometryError};
use crate::geom2d::{signed_area2, Pt2, RingRegion};
use crate::math::scale;
use crate::operations::modifying::boolean::handlers::planar::facets::coverage;
use crate::operations::modifying::boolean::types::BooleanOp;
use crate::query::PointClassification;
use std::cell::Cell;

pub(super) struct Keeper {
    operation: BooleanOp,
    shared: Cell<bool>,
}

impl Keeper {
    pub(super) fn new(operation: BooleanOp) -> Self {
        Self {
            operation,
            shared: Cell::new(false),
        }
    }

    pub(super) fn probe_side(&self) -> f64 {
        boundary_probe_side(self.operation)
    }

    pub(super) fn host(&self, place: PointClassification) -> Option<bool> {
        self.note(place);
        host_keeps(self.operation, place)
    }

    pub(super) fn cutter(&self, place: PointClassification) -> Option<bool> {
        self.note(place);
        cutter_keeps(self.operation, place)
    }

    pub(super) fn shares_volume_or_face(&self) -> bool {
        self.shared.get()
    }

    fn note(&self, place: PointClassification) {
        if place == PointClassification::Inside {
            self.shared.set(true);
        }
    }
}

fn host_keeps(operation: BooleanOp, place: PointClassification) -> Option<bool> {
    match place {
        PointClassification::Outside => Some(operation != BooleanOp::Intersection),
        PointClassification::Inside => Some(operation == BooleanOp::Intersection),
        PointClassification::Boundary | PointClassification::Unknown => None,
    }
}

fn boundary_probe_side(operation: BooleanOp) -> f64 {
    if operation == BooleanOp::Union {
        -1.0
    } else {
        1.0
    }
}

fn cutter_keeps(operation: BooleanOp, place: PointClassification) -> Option<bool> {
    match place {
        PointClassification::Inside => Some(operation != BooleanOp::Union),
        PointClassification::Outside => Some(operation == BooleanOp::Union),
        PointClassification::Boundary => Some(operation == BooleanOp::Subtraction),
        PointClassification::Unknown => None,
    }
}

pub(super) fn turns_cutter(operation: BooleanOp) -> bool {
    operation == BooleanOp::Subtraction
}

pub(super) fn cutter_frame(operation: BooleanOp, frame: Frame3) -> Frame3 {
    if !turns_cutter(operation) {
        return frame;
    }
    Frame3 {
        origin: frame.origin,
        x: frame.x,
        y: scale(frame.y, -1.0),
        z: scale(frame.z, -1.0),
    }
}

pub(super) fn piece_role(
    operation: BooleanOp,
    cutter: bool,
    region: &RingRegion,
    base: &[Vec<Pt2>],
    tolerance: f64,
) -> FaceRole {
    if operation == BooleanOp::Subtraction {
        return if cutter {
            FaceRole::Cut
        } else {
            FaceRole::Split
        };
    }
    let area = |rings: &[Vec<Pt2>]| {
        rings
            .iter()
            .map(|ring| signed_area2(ring).abs())
            .sum::<f64>()
    };
    let region_area = signed_area2(&region.outer).abs() - area(&region.holes);
    let base_area = signed_area2(&base[0]).abs() - area(&base[1..]);
    let perimeter = base
        .iter()
        .flat_map(|ring| {
            ring.iter()
                .zip(ring.iter().cycle().skip(1))
                .map(|(a, b)| (b.x - a.x).hypot(b.z - a.z))
        })
        .sum::<f64>();
    if base_area - region_area <= tolerance * perimeter {
        FaceRole::Preserved
    } else {
        FaceRole::Split
    }
}

pub(super) fn open_boundary_gap(operation: BooleanOp, error: GeometryError) -> GeometryError {
    match error {
        GeometryError::InvalidTopology(message)
            if operation != BooleanOp::Subtraction
                && message == "layered planar Boolean has an open boundary" =>
        {
            coverage()
        }
        other => other,
    }
}
