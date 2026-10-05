mod arcs;
mod coextensive;
mod contained;
mod embedded;
mod parallel;
mod transverse;

use super::disjoint::separate_boolean;
use crate::brep::{Accuracy, BrepEnvelope, FaceProvenance, FaceRole, FaceSource, GeometryError};
use crate::math::{dot, norm, scale, sub, Point3};
use crate::operations::modifying::boolean::assembly::{
    append_cylinder_segment, cylinder_source, face_provenance, finish_cylinder_result,
};
use crate::operations::modifying::boolean::operands::{full_cylinder, CylinderInput};
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};
use coextensive::coextensive_cylinder_radii;
use contained::contained_cylinders;
use embedded::{embedded_transverse_cylinder_boolean, EmbeddedPlacement};
use parallel::{parallel_cylinder_boolean, sliced_parallel_cylinder_boolean, ParallelSlice};
use transverse::transverse_cylinder_through_subtraction;

struct AxialPlacement {
    radial: Point3,
    radial_distance: f64,
    a_span: [f64; 2],
    b_span: [f64; 2],
    span_error: f64,
    roundoff: f64,
}

struct AxialOverlap {
    alignment: f64,
    overlap_span: [f64; 2],
    clearance: f64,
}

struct Segment {
    lo: f64,
    hi: f64,
    provenances: [FaceProvenance; 3],
}

struct CoaxialSources {
    a_lateral: FaceSource,
    b_lateral: FaceSource,
    a_caps: [FaceSource; 2],
    b_caps: [FaceSource; 2],
}

pub(crate) fn boolean_cylinders(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let a = full_cylinder(a)?;
    let b = full_cylinder(b)?;
    let accuracy = Accuracy::combined(a.brep.accuracy, b.brep.accuracy);
    let alignment = dot(a.frame.z, b.frame.z);
    if 1.0 - alignment.abs() > 1e-12 {
        if operation == BooleanOp::Subtraction {
            return transverse_cylinder_through_subtraction(&a, &b, id, accuracy);
        }
        return Err(GeometryError::CoverageGap {
            families: ["cylinder".into(), "nonparallel cylinder".into()],
        });
    }
    let placement = axial_placement(&a, &b, alignment);
    if placement.radial_distance > placement.roundoff {
        return offset_axis_boolean(&a, &b, alignment, &placement, operation, id, accuracy);
    }
    if a.radius != b.radius {
        return coaxial_unequal_radii_boolean(
            &a, &b, alignment, &placement, operation, id, accuracy,
        );
    }
    coaxial_equal_radii_boolean(&a, &b, alignment, &placement, operation, id, accuracy)
}

fn axial_placement(a: &CylinderInput<'_>, b: &CylinderInput<'_>, alignment: f64) -> AxialPlacement {
    let delta = sub(b.frame.origin, a.frame.origin);
    let axial = dot(delta, a.frame.z);
    let radial = sub(delta, scale(a.frame.z, axial));
    let a_span = [0.0, a.height];
    let b_end = axial + alignment.signum() * b.height;
    let b_span = [axial.min(b_end), axial.max(b_end)];
    let span_error = (a_span[0] - b_span[0])
        .abs()
        .max((a_span[1] - b_span[1]).abs());
    let scale = a
        .frame
        .origin
        .into_iter()
        .chain(b.frame.origin)
        .chain([a.radius, b.radius, a.height, b.height])
        .fold(1.0_f64, |largest, value| largest.max(value.abs()));
    let roundoff = 128.0 * f64::EPSILON * scale;
    let radial_distance = norm(radial);
    AxialPlacement {
        radial,
        radial_distance,
        a_span,
        b_span,
        span_error,
        roundoff,
    }
}

fn offset_axis_boolean(
    a: &CylinderInput<'_>,
    b: &CylinderInput<'_>,
    alignment: f64,
    placement: &AxialPlacement,
    operation: BooleanOp,
    id: String,
    accuracy: Accuracy,
) -> Result<BooleanResult, GeometryError> {
    if placement.radial_distance <= accuracy.intersection {
        return Err(GeometryError::UnresolvedIntersection(
            "cylinder axes differ within the intersection budget".into(),
        ));
    }
    if placement.span_error <= placement.roundoff {
        return parallel_cylinder_boolean(a, b, alignment, operation, id, accuracy);
    }
    let clearance = 4.0 * accuracy.geometric;
    let overlap_span = [
        placement.a_span[0].max(placement.b_span[0]),
        placement.a_span[1].min(placement.b_span[1]),
    ];
    let overlap = overlap_span[1] - overlap_span[0];
    if overlap.abs() <= clearance {
        return Err(GeometryError::UnresolvedIntersection(
            "parallel cylinder axial contact is below geometric resolution".into(),
        ));
    }
    if overlap < 0.0 {
        return separate_boolean(a.brep, b.brep, operation, id, Vec::new());
    }
    let axial_overlap = AxialOverlap {
        alignment,
        overlap_span,
        clearance,
    };
    offset_axis_overlap_boolean(a, b, placement, &axial_overlap, operation, id, accuracy)
}

fn offset_axis_overlap_boolean(
    a: &CylinderInput<'_>,
    b: &CylinderInput<'_>,
    placement: &AxialPlacement,
    axial_overlap: &AxialOverlap,
    operation: BooleanOp,
    id: String,
    accuracy: Accuracy,
) -> Result<BooleanResult, GeometryError> {
    let AxialPlacement {
        radial,
        a_span,
        b_span,
        roundoff,
        ..
    } = *placement;
    let AxialOverlap {
        alignment,
        overlap_span,
        clearance,
    } = *axial_overlap;
    if operation == BooleanOp::Intersection {
        return sliced_parallel_cylinder_boolean(
            a,
            b,
            &ParallelSlice {
                alignment,
                radial,
                a_span,
                b_span,
                span: overlap_span,
            },
            operation,
            id,
            accuracy,
            roundoff,
        );
    }
    if operation == BooleanOp::Subtraction {
        let b_covers_a = b_span[0] <= a_span[0] + roundoff && b_span[1] >= a_span[1] - roundoff;
        if b_covers_a {
            return sliced_parallel_cylinder_boolean(
                a,
                b,
                &ParallelSlice {
                    alignment,
                    radial,
                    a_span,
                    b_span,
                    span: a_span,
                },
                operation,
                id,
                accuracy,
                roundoff,
            );
        }
        let b_nearly_covers_a =
            b_span[0] <= a_span[0] + clearance && b_span[1] >= a_span[1] - clearance;
        if b_nearly_covers_a {
            return Err(GeometryError::UnresolvedIntersection(
                "parallel cylinder subtraction leaves a sub-tolerance axial segment".into(),
            ));
        }
    }
    embedded_offset_axis_boolean(a, b, placement, axial_overlap, operation, id, accuracy)
}

fn embedded_offset_axis_boolean(
    a: &CylinderInput<'_>,
    b: &CylinderInput<'_>,
    placement: &AxialPlacement,
    axial_overlap: &AxialOverlap,
    operation: BooleanOp,
    id: String,
    accuracy: Accuracy,
) -> Result<BooleanResult, GeometryError> {
    let AxialPlacement {
        radial,
        radial_distance,
        a_span,
        b_span,
        ..
    } = *placement;
    let AxialOverlap {
        alignment,
        clearance,
        ..
    } = *axial_overlap;
    let b_embedded_in_a = b_span[0] > a_span[0] + clearance && b_span[1] < a_span[1] - clearance;
    let transverse_overlap = radial_distance > (a.radius - b.radius).abs() + clearance
        && radial_distance < a.radius + b.radius - clearance;
    if b_embedded_in_a
        && transverse_overlap
        && matches!(operation, BooleanOp::Union | BooleanOp::Subtraction)
    {
        return embedded_transverse_cylinder_boolean(
            a,
            b,
            &EmbeddedPlacement {
                alignment,
                radial,
                b_span,
            },
            operation,
            id,
            accuracy,
        );
    }
    let a_embedded_in_b = a_span[0] > b_span[0] + clearance && a_span[1] < b_span[1] - clearance;
    if a_embedded_in_b && transverse_overlap && operation == BooleanOp::Union {
        return boolean_cylinders(b.brep, a.brep, operation, id);
    }
    Err(GeometryError::CoverageGap {
        families: [
            "parallel cylinder".into(),
            "non-coextensive parallel cylinder".into(),
        ],
    })
}

fn coaxial_unequal_radii_boolean(
    a: &CylinderInput<'_>,
    b: &CylinderInput<'_>,
    alignment: f64,
    placement: &AxialPlacement,
    operation: BooleanOp,
    id: String,
    accuracy: Accuracy,
) -> Result<BooleanResult, GeometryError> {
    if (a.radius - b.radius).abs() <= accuracy.intersection {
        return Err(GeometryError::UnresolvedIntersection(
            "cylinder radii differ within the intersection budget".into(),
        ));
    }
    if placement.span_error <= placement.roundoff {
        return coextensive_cylinder_radii(a, b, alignment, operation, id, accuracy);
    }
    let clearance = 4.0 * accuracy.geometric;
    let b_inside_a = b.radius + clearance < a.radius
        && placement.b_span[0] > placement.a_span[0] + clearance
        && placement.b_span[1] < placement.a_span[1] - clearance;
    let a_inside_b = a.radius + clearance < b.radius
        && placement.a_span[0] > placement.b_span[0] + clearance
        && placement.a_span[1] < placement.b_span[1] - clearance;
    if b_inside_a || a_inside_b {
        return contained_cylinders(a, b, b_inside_a, operation, id, accuracy);
    }
    let near_radial_contact = (a.radius - b.radius).abs() <= clearance;
    let near_axial_contact = [
        (placement.a_span[0] - placement.b_span[0]).abs(),
        (placement.a_span[0] - placement.b_span[1]).abs(),
        (placement.a_span[1] - placement.b_span[0]).abs(),
        (placement.a_span[1] - placement.b_span[1]).abs(),
    ]
    .into_iter()
    .any(|gap| gap <= clearance);
    if near_radial_contact || near_axial_contact {
        return Err(GeometryError::UnresolvedIntersection(
            "cylinder containment boundary is below geometric resolution".into(),
        ));
    }
    if placement.span_error <= 4.0 * accuracy.geometric {
        return Err(GeometryError::UnresolvedIntersection(
            "cylinder axial spans differ below geometric resolution".into(),
        ));
    }
    Err(GeometryError::CoverageGap {
        families: [
            "cylinder".into(),
            "different-radius coaxial cylinder with unequal axial spans".into(),
        ],
    })
}

fn coaxial_equal_radii_boolean(
    a: &CylinderInput<'_>,
    b: &CylinderInput<'_>,
    alignment: f64,
    placement: &AxialPlacement,
    operation: BooleanOp,
    id: String,
    accuracy: Accuracy,
) -> Result<BooleanResult, GeometryError> {
    let overlap =
        placement.a_span[1].min(placement.b_span[1]) - placement.a_span[0].max(placement.b_span[0]);
    if overlap != 0.0 && overlap.abs() <= 4.0 * accuracy.geometric {
        return Err(GeometryError::UnresolvedIntersection(
            "cylinder axial overlap or clearance is below geometric resolution".into(),
        ));
    }

    let a_lateral = cylinder_source(a, 0);
    let b_lateral = cylinder_source(b, 0);
    let a_caps = [cylinder_source(a, 1), cylinder_source(a, 2)];
    let b_caps = if alignment > 0.0 {
        [cylinder_source(b, 1), cylinder_source(b, 2)]
    } else {
        [cylinder_source(b, 2), cylinder_source(b, 1)]
    };
    let face_sources = CoaxialSources {
        a_lateral,
        b_lateral,
        a_caps,
        b_caps,
    };
    let cap_at = |boundary: f64, low: bool, include_a: bool, include_b: bool| {
        let mut sources = Vec::new();
        if include_a
            && boundary
                == if low {
                    placement.a_span[0]
                } else {
                    placement.a_span[1]
                }
        {
            sources.push(face_sources.a_caps[usize::from(!low)].clone());
        }
        if include_b
            && boundary
                == if low {
                    placement.b_span[0]
                } else {
                    placement.b_span[1]
                }
        {
            sources.push(face_sources.b_caps[usize::from(!low)].clone());
        }
        sources
    };

    let mut segments = Vec::new();
    let coincident = overlap >= 0.0;
    add_coaxial_segments(
        &mut segments,
        operation,
        overlap,
        placement.a_span,
        placement.b_span,
        &face_sources,
        cap_at,
    );
    coaxial_segments_boolean(a, b, segments, coincident, operation, id, accuracy)
}

fn add_coaxial_segments(
    segments: &mut Vec<Segment>,
    operation: BooleanOp,
    overlap: f64,
    a_span: [f64; 2],
    b_span: [f64; 2],
    sources: &CoaxialSources,
    cap_at: impl Fn(f64, bool, bool, bool) -> Vec<FaceSource>,
) {
    match operation {
        BooleanOp::Union if overlap >= 0.0 => {
            let lo = a_span[0].min(b_span[0]);
            let hi = a_span[1].max(b_span[1]);
            let lower = cap_at(lo, true, true, true);
            let upper = cap_at(hi, false, true, true);
            segments.push(coincident_segment(
                lo,
                hi,
                lower,
                upper,
                &sources.a_lateral,
                &sources.b_lateral,
            ));
        }
        BooleanOp::Union => {
            for (span, lateral, caps) in [
                (a_span, sources.a_lateral.clone(), sources.a_caps.clone()),
                (b_span, sources.b_lateral.clone(), sources.b_caps.clone()),
            ] {
                segments.push(Segment {
                    lo: span[0],
                    hi: span[1],
                    provenances: [
                        face_provenance(vec![lateral], FaceRole::Preserved, false),
                        face_provenance(vec![caps[0].clone()], FaceRole::Preserved, false),
                        face_provenance(vec![caps[1].clone()], FaceRole::Preserved, false),
                    ],
                });
            }
        }
        BooleanOp::Intersection if overlap > 0.0 => {
            let lo = a_span[0].max(b_span[0]);
            let hi = a_span[1].min(b_span[1]);
            let lower = cap_at(lo, true, lo == a_span[0], lo == b_span[0]);
            let upper = cap_at(hi, false, hi == a_span[1], hi == b_span[1]);
            segments.push(coincident_segment(
                lo,
                hi,
                lower,
                upper,
                &sources.a_lateral,
                &sources.b_lateral,
            ));
        }
        BooleanOp::Intersection => {}
        BooleanOp::Subtraction if overlap <= 0.0 => {
            segments.push(Segment {
                lo: a_span[0],
                hi: a_span[1],
                provenances: [
                    face_provenance(vec![sources.a_lateral.clone()], FaceRole::Preserved, false),
                    face_provenance(vec![sources.a_caps[0].clone()], FaceRole::Preserved, false),
                    face_provenance(vec![sources.a_caps[1].clone()], FaceRole::Preserved, false),
                ],
            });
        }
        BooleanOp::Subtraction => {
            add_subtraction_segments(
                segments,
                a_span,
                b_span,
                &sources.a_lateral,
                &sources.a_caps,
                &sources.b_caps,
            );
        }
    }
}

fn coincident_segment(
    lo: f64,
    hi: f64,
    lower: Vec<FaceSource>,
    upper: Vec<FaceSource>,
    a_lateral: &FaceSource,
    b_lateral: &FaceSource,
) -> Segment {
    Segment {
        lo,
        hi,
        provenances: [
            face_provenance(
                vec![a_lateral.clone(), b_lateral.clone()],
                FaceRole::Coincident,
                false,
            ),
            face_provenance(
                lower.clone(),
                if lower.len() > 1 {
                    FaceRole::Coincident
                } else {
                    FaceRole::Preserved
                },
                false,
            ),
            face_provenance(
                upper.clone(),
                if upper.len() > 1 {
                    FaceRole::Coincident
                } else {
                    FaceRole::Preserved
                },
                false,
            ),
        ],
    }
}

fn add_subtraction_segments(
    segments: &mut Vec<Segment>,
    a_span: [f64; 2],
    b_span: [f64; 2],
    a_lateral: &FaceSource,
    a_caps: &[FaceSource; 2],
    b_caps: &[FaceSource; 2],
) {
    if b_span[0] > a_span[0] {
        segments.push(Segment {
            lo: a_span[0],
            hi: b_span[0].min(a_span[1]),
            provenances: [
                face_provenance(vec![a_lateral.clone()], FaceRole::Split, false),
                face_provenance(vec![a_caps[0].clone()], FaceRole::Preserved, false),
                face_provenance(vec![b_caps[0].clone()], FaceRole::Cut, true),
            ],
        });
    }
    if b_span[1] < a_span[1] {
        segments.push(Segment {
            lo: b_span[1].max(a_span[0]),
            hi: a_span[1],
            provenances: [
                face_provenance(vec![a_lateral.clone()], FaceRole::Split, false),
                face_provenance(vec![b_caps[1].clone()], FaceRole::Cut, true),
                face_provenance(vec![a_caps[1].clone()], FaceRole::Preserved, false),
            ],
        });
    }
}

fn coaxial_segments_boolean(
    a: &CylinderInput<'_>,
    b: &CylinderInput<'_>,
    segments: Vec<Segment>,
    coincident: bool,
    operation: BooleanOp,
    id: String,
    accuracy: Accuracy,
) -> Result<BooleanResult, GeometryError> {
    let mut out = BrepEnvelope::new(id, accuracy)?;
    for (index, segment) in segments.into_iter().enumerate() {
        let mut frame = a.frame;
        frame.origin = a.frame.point([0.0, 0.0, segment.lo]);
        append_cylinder_segment(
            &mut out,
            frame,
            a.radius,
            segment.hi - segment.lo,
            segment.provenances,
            index,
        )?;
    }
    finish_cylinder_result(out, a, b, operation, coincident)
}
