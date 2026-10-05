use crate::brep::{Accuracy, BrepEnvelope, FaceProvenance, FaceRole, FaceSource, GeometryError};
use crate::geom2d::{boolean_oriented_regions, PlanarBooleanOp, Pt2, RingRegion};
use crate::math::{dot, sub, Point3};
use crate::operations::modifying::boolean::assembly::{
    append_analytic_input, finish_analytic_result,
};
use crate::operations::modifying::boolean::operands::{
    brep_face_source, full_planar_extrusion, prismatic_face_provenance, prismatic_gap,
    prismatic_profile_sources, profiles_match, region_profiles, unique_sources,
    PrismaticAxialSegment, PrismaticInput,
};
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};
use crate::primitives;

struct AxialProfile<'a> {
    a_contours: &'a [Vec<Pt2>],
    a_span: [f64; 2],
    b_span: [f64; 2],
}

pub(crate) fn boolean_planar_extrusions(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let a = full_planar_extrusion(a)?;
    let b = full_planar_extrusion(b)?;
    let accuracy = Accuracy::combined(a.brep.accuracy, b.brep.accuracy);
    if dot(a.frame.z, b.frame.z) < 1.0 - 1.0e-12 {
        return Err(prismatic_gap());
    }
    let to_common = |contours: &[Vec<Point3>]| {
        contours
            .iter()
            .map(|ring| {
                ring.iter()
                    .map(|point| {
                        let local = a.frame.local(*point);
                        Pt2::new(local[0], local[1])
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    };
    let a_contours = to_common(&a.contours);
    let b_contours = to_common(&b.contours);
    let b_lo = dot(sub(b.frame.origin, a.frame.origin), a.frame.z);
    let b_hi = b_lo + b.height;
    if b.contours
        .iter()
        .flatten()
        .any(|point| (a.frame.local(*point)[2] - b_lo).abs() > accuracy.geometric)
    {
        return Err(prismatic_gap());
    }
    let a_span = [0.0, a.height];
    let b_span = [b_lo, b_hi];
    let coextensive =
        b_lo.abs() <= accuracy.geometric && (a.height - b_hi).abs() <= accuracy.geometric;
    if !coextensive {
        if !profiles_match(&a_contours, &b_contours, accuracy.geometric) {
            return Err(prismatic_gap());
        }
        return axial_profile_boolean(
            &a,
            &b,
            &AxialProfile {
                a_contours: &a_contours,
                a_span,
                b_span,
            },
            operation,
            id,
            accuracy,
        );
    }
    let planar_operation = match operation {
        BooleanOp::Union => PlanarBooleanOp::Union,
        BooleanOp::Intersection => PlanarBooleanOp::Intersection,
        BooleanOp::Subtraction => PlanarBooleanOp::Subtraction,
    };
    let regions = boolean_oriented_regions(
        &a_contours,
        &b_contours,
        planar_operation,
        accuracy.intersection,
    );
    let out = region_solid(&a, &b, &regions, operation, id, accuracy)?;
    finish_analytic_result(out, a.brep, b.brep, true, operation, false)
}

fn axial_profile_boolean(
    a: &PrismaticInput<'_>,
    b: &PrismaticInput<'_>,
    profile: &AxialProfile<'_>,
    operation: BooleanOp,
    id: String,
    accuracy: Accuracy,
) -> Result<BooleanResult, GeometryError> {
    let AxialProfile {
        a_contours,
        a_span,
        b_span,
    } = *profile;
    let a_caps = [brep_face_source(a.brep, 1), brep_face_source(a.brep, 0)];
    let b_caps = [brep_face_source(b.brep, 1), brep_face_source(b.brep, 0)];
    let overlap = a_span[1].min(b_span[1]) - a_span[0].max(b_span[0]);
    if overlap != 0.0 && overlap.abs() <= 4.0 * accuracy.geometric {
        return Err(GeometryError::UnresolvedIntersection(
            "profile extrusion axial overlap or clearance is below geometric resolution".into(),
        ));
    }
    let segments = axial_segments(
        operation, overlap, a_span, b_span, &a_caps, &b_caps, accuracy,
    );
    let out = axial_solid(a, b, a_contours, segments, id, accuracy)?;
    finish_analytic_result(out, a.brep, b.brep, true, operation, overlap >= 0.0)
}

fn axial_segments(
    operation: BooleanOp,
    overlap: f64,
    a_span: [f64; 2],
    b_span: [f64; 2],
    a_caps: &[FaceSource; 2],
    b_caps: &[FaceSource; 2],
    accuracy: Accuracy,
) -> Vec<PrismaticAxialSegment> {
    let mut segments = Vec::new();
    match operation {
        BooleanOp::Union if overlap >= 0.0 => {
            let lo = a_span[0].min(b_span[0]);
            let hi = a_span[1].max(b_span[1]);
            segments.push(coincident_segment(
                lo, hi, a_span, b_span, a_caps, b_caps, accuracy,
            ));
        }
        BooleanOp::Union => {
            for (span, caps, include_a_sides, include_b_sides) in [
                (a_span, a_caps.clone(), true, false),
                (b_span, b_caps.clone(), false, true),
            ] {
                segments.push(PrismaticAxialSegment {
                    lo: span[0],
                    hi: span[1],
                    lower: preserved_cap(vec![caps[0].clone()]),
                    upper: preserved_cap(vec![caps[1].clone()]),
                    include_a_sides,
                    include_b_sides,
                    side_role: FaceRole::Preserved,
                });
            }
        }
        BooleanOp::Intersection if overlap > 0.0 => {
            let lo = a_span[0].max(b_span[0]);
            let hi = a_span[1].min(b_span[1]);
            segments.push(coincident_segment(
                lo, hi, a_span, b_span, a_caps, b_caps, accuracy,
            ));
        }
        BooleanOp::Intersection => {}
        BooleanOp::Subtraction if overlap <= 0.0 => {
            segments.push(PrismaticAxialSegment {
                lo: a_span[0],
                hi: a_span[1],
                lower: preserved_cap(vec![a_caps[0].clone()]),
                upper: preserved_cap(vec![a_caps[1].clone()]),
                include_a_sides: true,
                include_b_sides: false,
                side_role: FaceRole::Preserved,
            });
        }
        BooleanOp::Subtraction => {
            if b_span[0] > a_span[0] {
                segments.push(segment_below_cutter(a_span, b_span, a_caps, b_caps));
            }
            if b_span[1] < a_span[1] {
                segments.push(segment_above_cutter(a_span, b_span, a_caps, b_caps));
            }
        }
    }
    segments
}

fn coincident_segment(
    lo: f64,
    hi: f64,
    a_span: [f64; 2],
    b_span: [f64; 2],
    a_caps: &[FaceSource; 2],
    b_caps: &[FaceSource; 2],
    accuracy: Accuracy,
) -> PrismaticAxialSegment {
    let lower = [
        cap_at(lo, a_span, a_caps, accuracy),
        cap_at(lo, b_span, b_caps, accuracy),
    ]
    .into_iter()
    .flatten()
    .collect();
    let upper = [
        cap_at(hi, a_span, a_caps, accuracy),
        cap_at(hi, b_span, b_caps, accuracy),
    ]
    .into_iter()
    .flatten()
    .collect();
    PrismaticAxialSegment {
        lo,
        hi,
        lower: preserved_cap(lower),
        upper: preserved_cap(upper),
        include_a_sides: true,
        include_b_sides: true,
        side_role: FaceRole::Coincident,
    }
}

fn cap_at(
    boundary: f64,
    span: [f64; 2],
    caps: &[FaceSource; 2],
    accuracy: Accuracy,
) -> Option<FaceSource> {
    if (boundary - span[0]).abs() <= accuracy.geometric {
        Some(caps[0].clone())
    } else if (boundary - span[1]).abs() <= accuracy.geometric {
        Some(caps[1].clone())
    } else {
        None
    }
}

fn preserved_cap(sources: Vec<FaceSource>) -> FaceProvenance {
    let role = if sources.len() > 1 {
        FaceRole::Coincident
    } else {
        FaceRole::Preserved
    };
    FaceProvenance {
        sources,
        role,
        reversed: false,
    }
}

fn segment_below_cutter(
    a_span: [f64; 2],
    b_span: [f64; 2],
    a_caps: &[FaceSource; 2],
    b_caps: &[FaceSource; 2],
) -> PrismaticAxialSegment {
    PrismaticAxialSegment {
        lo: a_span[0],
        hi: b_span[0].min(a_span[1]),
        lower: preserved_cap(vec![a_caps[0].clone()]),
        upper: FaceProvenance {
            sources: vec![b_caps[0].clone()],
            role: FaceRole::Cut,
            reversed: true,
        },
        include_a_sides: true,
        include_b_sides: false,
        side_role: FaceRole::Split,
    }
}

fn segment_above_cutter(
    a_span: [f64; 2],
    b_span: [f64; 2],
    a_caps: &[FaceSource; 2],
    b_caps: &[FaceSource; 2],
) -> PrismaticAxialSegment {
    PrismaticAxialSegment {
        lo: b_span[1].max(a_span[0]),
        hi: a_span[1],
        lower: FaceProvenance {
            sources: vec![b_caps[1].clone()],
            role: FaceRole::Cut,
            reversed: true,
        },
        upper: preserved_cap(vec![a_caps[1].clone()]),
        include_a_sides: true,
        include_b_sides: false,
        side_role: FaceRole::Split,
    }
}

fn axial_solid(
    a: &PrismaticInput<'_>,
    b: &PrismaticInput<'_>,
    a_contours: &[Vec<Pt2>],
    segments: Vec<PrismaticAxialSegment>,
    id: String,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    let outer = a_contours[0]
        .iter()
        .map(|point| [point.x, point.z])
        .collect::<Vec<_>>();
    let holes = a_contours[1..]
        .iter()
        .map(|ring| ring.iter().map(|point| [point.x, point.z]).collect())
        .collect::<Vec<Vec<_>>>();
    let mut out = BrepEnvelope::new(id, accuracy)?;
    for (index, segment) in segments.into_iter().enumerate() {
        let mut frame = a.frame;
        frame.origin = a.frame.point([0.0, 0.0, segment.lo]);
        let part = primitives::linear_extrusion(
            format!("{}:axial-region-{index}", out.id),
            frame,
            outer.clone(),
            holes.clone(),
            segment.hi - segment.lo,
            accuracy,
        )?;
        let face_offset = out.topology.faces.len() as u32;
        append_analytic_input(&mut out, &part)?;
        out.topology.faces[face_offset as usize].provenance = segment.upper;
        out.topology.faces[(face_offset + 1) as usize].provenance = segment.lower;
        let mut local_face = 2_u32;
        for ring in a_contours {
            for edge in 0..ring.len() {
                let from = ring[edge];
                let to = ring[(edge + 1) % ring.len()];
                let mut sources = Vec::new();
                if segment.include_a_sides {
                    sources.extend(prismatic_profile_sources(
                        a,
                        a.frame,
                        from,
                        to,
                        accuracy.geometric,
                    ));
                }
                if segment.include_b_sides {
                    sources.extend(prismatic_profile_sources(
                        b,
                        a.frame,
                        from,
                        to,
                        accuracy.geometric,
                    ));
                }
                let sources = unique_sources(sources);
                if sources.is_empty() {
                    return Err(GeometryError::InvalidTopology(
                        "axial profile boolean produced a side without source ancestry".into(),
                    ));
                }
                out.topology.faces[(face_offset + local_face) as usize].provenance =
                    FaceProvenance {
                        sources,
                        role: segment.side_role,
                        reversed: false,
                    };
                local_face += 1;
            }
        }
    }
    Ok(out)
}

fn region_solid(
    a: &PrismaticInput<'_>,
    b: &PrismaticInput<'_>,
    regions: &[RingRegion],
    operation: BooleanOp,
    id: String,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    let mut out = BrepEnvelope::new(id, accuracy)?;
    for (region_index, region) in regions.iter().enumerate() {
        let (outer, holes) = region_profiles(region);
        let part = primitives::linear_extrusion(
            format!("{}:region-{region_index}", out.id),
            a.frame,
            outer,
            holes,
            a.height,
            accuracy,
        )?;
        let face_offset = out.topology.faces.len() as u32;
        append_analytic_input(&mut out, &part)?;
        let cap_sources = if operation == BooleanOp::Subtraction {
            [
                vec![brep_face_source(a.brep, 0)],
                vec![brep_face_source(a.brep, 1)],
            ]
        } else {
            [
                vec![brep_face_source(a.brep, 0), brep_face_source(b.brep, 0)],
                vec![brep_face_source(a.brep, 1), brep_face_source(b.brep, 1)],
            ]
        };
        for local in 0..2 {
            out.topology.faces[(face_offset + local) as usize].provenance = FaceProvenance {
                sources: cap_sources[local as usize].clone(),
                role: FaceRole::Split,
                reversed: false,
            };
        }
        let mut local_face = 2_u32;
        for ring in std::iter::once(&region.outer).chain(&region.holes) {
            for index in 0..ring.len() {
                let from = a.frame.point([ring[index].x, ring[index].z, 0.0]);
                let next = ring[(index + 1) % ring.len()];
                let to = a.frame.point([next.x, next.z, 0.0]);
                out.topology.faces[(face_offset + local_face) as usize].provenance =
                    prismatic_face_provenance(a, b, from, to, operation, accuracy.geometric)?;
                local_face += 1;
            }
        }
    }
    Ok(out)
}
