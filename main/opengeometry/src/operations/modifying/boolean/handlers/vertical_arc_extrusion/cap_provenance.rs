use super::facets::Facets;
use super::section::{area, coverage, face_region};
use crate::brep::{
    BrepEnvelope, FaceProvenance, FaceRole, FaceSource, Frame3, GeometryError, GeometryQuality,
    Shell, SolidRegion, Surface, SurfaceGeometry,
};
use crate::geom2d::{intersections, winding, CurveRegion2, Pt2};
use crate::math::{dot, Point3};
use crate::operations::modifying::boolean::assembly::{
    analytic_face_mappings, assign_shell_faces, enclosing_solid,
};
use crate::operations::modifying::boolean::operands::brep_face_source;
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanReport, BooleanResult};
use crate::query::{classify_point_in_shell, face_contains_uv, PointClassification};

pub(super) fn source_for_side(
    brep: &BrepEnvelope,
    point: Point3,
    axis: Point3,
    tolerance: f64,
) -> Result<Vec<FaceSource>, GeometryError> {
    let mut sources = Vec::new();
    for face in &brep.topology.faces {
        let surface = brep.geometry.surface(face.surface)?;
        let on = match surface {
            SurfaceGeometry::Plane { frame } => {
                let local = frame.local(point);
                local[2].abs() <= tolerance && dot(frame.z, axis).abs() < 1.0 - 1e-10
            }
            SurfaceGeometry::Cylinder { frame, radius } => {
                let local = frame.local(point);
                (local[0].hypot(local[1]) - radius).abs() <= tolerance
            }
            _ => false,
        };
        if !on {
            continue;
        }
        let hint = [
            face.trim.uv_bounds[0].midpoint(),
            face.trim.uv_bounds[1].midpoint(),
        ];
        let uv = surface.project(point, Some(hint))?;
        match face_contains_uv(brep, face, uv)? {
            Some(true) => sources.push(brep_face_source(brep, face.id)),
            Some(false) => {}
            None => return Err(coverage()),
        }
    }
    Ok(sources)
}

pub(super) fn cap_provenance(
    host: &BrepEnvelope,
    cutters: &[&BrepEnvelope],
    base: Frame3,
    level: f64,
    region: &CurveRegion2,
    tolerance: f64,
) -> Result<FaceProvenance, GeometryError> {
    let mut sources = source_for_cap_region(host, base, level, region, tolerance)?;
    let mut cut = false;
    for cutter in cutters {
        let cutter_sources = source_for_cap_region(cutter, base, level, region, tolerance)?;
        cut |= !cutter_sources.is_empty();
        sources.extend(cutter_sources);
    }
    if sources.is_empty() {
        return Err(coverage());
    }
    Ok(FaceProvenance {
        sources,
        role: if cut { FaceRole::Cut } else { FaceRole::Split },
        reversed: cut,
    })
}

fn source_for_cap_region(
    brep: &BrepEnvelope,
    base: Frame3,
    level: f64,
    region: &CurveRegion2,
    tolerance: f64,
) -> Result<Vec<FaceSource>, GeometryError> {
    let mut candidates = Vec::new();
    for face in &brep.topology.faces {
        let SurfaceGeometry::Plane { frame } = brep.geometry.surface(face.surface)? else {
            continue;
        };
        if dot(frame.z, base.z).abs() > 1.0 - 1.0e-10
            && (base.local(frame.origin)[2] - level).abs() <= tolerance
        {
            candidates.push(face);
        }
    }
    let mut sources = Vec::new();
    for face in candidates {
        let source_region = face_region(brep, face, base)?;
        if cap_regions_overlap(region, &source_region, tolerance) {
            sources.push(brep_face_source(brep, face.id));
        }
    }
    Ok(sources)
}

pub(super) fn cap_regions_overlap(a: &CurveRegion2, b: &CurveRegion2, tolerance: f64) -> bool {
    for (region, other) in [(a, b), (b, a)] {
        for ring in std::iter::once(&region.outer).chain(&region.holes) {
            for edge in ring {
                let mut parameters = vec![0.0, 1.0];
                for other_ring in std::iter::once(&other.outer).chain(&other.holes) {
                    for other_edge in other_ring {
                        parameters.extend(
                            intersections(edge, other_edge, tolerance)
                                .into_iter()
                                .filter_map(|point| edge.parameter(point, tolerance)),
                        );
                    }
                }
                parameters.sort_by(f64::total_cmp);
                parameters
                    .dedup_by(|left, right| (*left - *right).abs() * edge.length() <= tolerance);
                for span in parameters.windows(2) {
                    let interval_length = (span[1] - span[0]) * edge.length();
                    if interval_length <= tolerance * 4.0 {
                        continue;
                    }
                    let midpoint = (span[0] + span[1]) / 2.0;
                    let point = edge.point(midpoint);
                    let tangent = edge.tangent(midpoint);
                    let tangent_length = tangent.x.hypot(tangent.z);
                    if tangent_length <= tolerance {
                        continue;
                    }
                    let mut offset = (interval_length * 0.1).min(1.0e-4);
                    for _ in 0..8 {
                        if offset <= tolerance * 2.0 {
                            break;
                        }
                        let probe = Pt2::new(
                            point.x + tangent.z * offset / tangent_length,
                            point.z - tangent.x * offset / tangent_length,
                        );
                        if contains_region_point(region, probe, tolerance)
                            && contains_region_point(other, probe, tolerance)
                        {
                            return true;
                        }
                        offset *= 0.5;
                    }
                }
            }
        }
    }
    false
}

fn contains_region_point(region: &CurveRegion2, point: Pt2, tolerance: f64) -> bool {
    winding(point, &region.outer, tolerance) != 0
        && region
            .holes
            .iter()
            .all(|hole| winding(point, hole, tolerance) == 0)
}

pub(super) fn finish(
    mut facets: Facets,
    a: &BrepEnvelope,
    cutters: &[&BrepEnvelope],
) -> Result<BooleanResult, GeometryError> {
    let mut assigned = vec![false; facets.builder.brep.topology.faces.len()];
    let mut outer_shells = Vec::new();
    let mut cavity_shells = Vec::new();
    for start in 0..assigned.len() {
        if assigned[start] {
            continue;
        }
        let shell = facets.builder.brep.topology.shells.len() as u32;
        let faces = assign_shell_faces(
            &mut facets.builder.brep,
            &mut assigned,
            start,
            shell,
            coverage,
            coverage,
        )?;
        let signed_volume = signed_shell_volume(&facets, &faces)?;
        if !signed_volume.is_finite() || signed_volume.abs() <= facets.accuracy.geometric.powi(3) {
            return Err(coverage());
        }
        facets.builder.brep.topology.shells.push(Shell {
            id: shell,
            faces,
            is_closed: true,
        });
        if signed_volume > 0.0 {
            outer_shells.push((shell, signed_volume));
        } else {
            cavity_shells.push(shell);
        }
    }
    for &(outer_shell, _) in &outer_shells {
        facets.builder.brep.solids.push(SolidRegion {
            outer_shell,
            cavity_shells: Vec::new(),
        });
    }
    for cavity_shell in cavity_shells {
        let index = enclosing_solid(
            &facets.builder.brep,
            &outer_shells,
            cavity_shell,
            |brep, faces, point| match classify_point_in_shell(brep, faces, point)? {
                PointClassification::Inside => Ok(true),
                PointClassification::Outside => Ok(false),
                PointClassification::Boundary | PointClassification::Unknown => Err(coverage()),
            },
            coverage,
        )?;
        facets.builder.brep.solids[index]
            .cavity_shells
            .push(cavity_shell);
    }
    finish_arc_extrusion_result(facets.builder.brep, a, cutters)
}

fn signed_shell_volume(facets: &Facets, faces: &[u32]) -> Result<f64, GeometryError> {
    let mut signed_volume = 0.0;
    for &face_id in faces {
        let face = &facets.builder.brep.topology.faces[face_id as usize];
        let SurfaceGeometry::Plane { frame } = facets
            .builder
            .brep
            .geometry
            .surfaces
            .get(face.surface as usize)
            .ok_or_else(coverage)?
        else {
            continue;
        };
        let outward = dot(frame.z, facets.base.z) * face.sense.multiplier();
        if outward.abs() <= 1.0 - 1e-10 {
            continue;
        }
        let region = face_region(&facets.builder.brep, face, facets.base)?;
        let cap_area = area(&region.outer).abs()
            - region
                .holes
                .iter()
                .map(|hole| area(hole).abs())
                .sum::<f64>();
        if !cap_area.is_finite() || cap_area <= facets.accuracy.geometric.powi(2) {
            return Err(coverage());
        }
        signed_volume += facets.base.local(frame.origin)[2] * outward * cap_area;
    }
    Ok(signed_volume)
}

fn finish_arc_extrusion_result(
    mut out: BrepEnvelope,
    a: &BrepEnvelope,
    cutters: &[&BrepEnvelope],
) -> Result<BooleanResult, GeometryError> {
    out.revision = cutters
        .iter()
        .fold(a.revision, |revision, cutter| revision.max(cutter.revision))
        .checked_add(1)
        .ok_or_else(coverage)?;
    out.validate()?;
    let face_mappings =
        analytic_face_mappings(&out, std::iter::once(a).chain(cutters.iter().copied()));
    Ok(BooleanResult {
        report: BooleanReport {
            operation: BooleanOp::Subtraction,
            quality: GeometryQuality::Analytic,
            contacts: Vec::new(),
            coincident: true,
            face_mappings,
        },
        brep: out,
    })
}
