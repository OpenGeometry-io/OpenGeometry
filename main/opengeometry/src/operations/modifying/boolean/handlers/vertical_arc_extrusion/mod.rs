mod cap_provenance;
mod facets;
mod section;
#[cfg(test)]
mod tests;

use crate::brep::{
    Accuracy, BrepEnvelope, FaceProvenance, FaceRole, Frame3, GeometryError, SurfaceGeometry,
};
use crate::geom2d::{boolean_curved_regions, CurveEdge2, CurveRegion2, PlanarBooleanOp};
use crate::math::dot;
use crate::operations::modifying::boolean::operands::all_planar;
use crate::operations::modifying::boolean::types::BooleanResult;
use cap_provenance::{cap_provenance, finish, source_for_side};
use facets::{same_side_patch, Facets, SidePatch};
use section::{append_points, coverage, face_profile, sectional_regions, split_regions};

struct SectionFrame {
    base: Frame3,
    low: f64,
    high: f64,
}

struct SectionCut<'a> {
    a: &'a BrepEnvelope,
    cutters: &'a [&'a BrepEnvelope],
    base: Frame3,
    accuracy: Accuracy,
}

pub(crate) fn subtract_vertical_arc_extrusion(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    subtract_vertical_arc_extrusion_batch(a, &[b], id)
}

pub(crate) fn subtract_vertical_arc_extrusion_batch(
    a: &BrepEnvelope,
    cutters: &[&BrepEnvelope],
    id: String,
) -> Result<BooleanResult, GeometryError> {
    if cutters.is_empty() {
        return Err(coverage());
    }
    let original_host = a
        .topology
        .faces
        .iter()
        .any(|face| face.key == "top" || face.key == "upper_cap");
    if !a
        .geometry
        .surfaces
        .iter()
        .any(|surface| matches!(surface, SurfaceGeometry::Cylinder { .. }))
    {
        return Err(coverage());
    }
    if cutters
        .iter()
        .any(|cutter| cutter.solids.len() != 1 || !all_planar(cutter))
    {
        return Err(coverage());
    }
    let accuracy = cutters.iter().fold(a.accuracy, |accuracy, cutter| {
        Accuracy::combined(accuracy, cutter.accuracy)
    });
    let SectionFrame { base, low, high } = section_frame(a, accuracy)?;
    let original_profile = if original_host {
        Some(face_profile(a, base, high - low, accuracy.intersection)?)
    } else {
        None
    };
    let cutter_profiles = cutter_profile_spans(cutters, base, accuracy)?;
    let section = SectionCut {
        a,
        cutters,
        base,
        accuracy,
    };
    let levels = section_levels(&section, original_host, low, high, &cutter_profiles);
    let mut layers = layer_regions(&section, &levels, &original_profile, &cutter_profiles)?;
    let (mut exposed_up, mut exposed_down) = exposed_regions(&layers, accuracy)?;
    split_at_shared_points(&mut layers, &mut exposed_up, &mut exposed_down, accuracy);
    let mut facets = Facets::new(id, accuracy, base)?;
    let mut side_patches = Vec::<SidePatch>::new();
    add_caps(&mut facets, &section, &[], &layers[0], 0.0, false)?;
    add_layer_faces(
        &mut facets,
        &mut side_patches,
        &section,
        &layers,
        &levels,
        &exposed_up,
        &exposed_down,
    )?;
    add_caps(
        &mut facets,
        &section,
        &[],
        layers.last().ok_or_else(coverage)?,
        high - low,
        true,
    )?;
    add_sides(&mut facets, &side_patches, accuracy)?;
    finish(facets, a, cutters)
}

fn section_frame(a: &BrepEnvelope, accuracy: Accuracy) -> Result<SectionFrame, GeometryError> {
    let host_top = host_top_frame(a)?;
    let positions = a
        .topology
        .vertices
        .iter()
        .map(|vertex| host_top.local(vertex.position)[2])
        .collect::<Vec<_>>();
    let low = positions.iter().copied().fold(f64::INFINITY, f64::min);
    let high = positions.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if !low.is_finite() || high - low <= accuracy.geometric * 4.0 {
        return Err(coverage());
    }
    let base = Frame3 {
        origin: host_top.point([0.0, 0.0, low]),
        ..host_top
    };
    Ok(SectionFrame { base, low, high })
}

fn host_top_frame(a: &BrepEnvelope) -> Result<Frame3, GeometryError> {
    let cap_axes = a
        .topology
        .faces
        .iter()
        .filter(|face| {
            face.key == "top"
                || face.key == "upper_cap"
                || face.key.ends_with(":top")
                || face.key.starts_with("upper-cap-")
        })
        .filter_map(
            |face| match a.geometry.surfaces.get(face.surface as usize) {
                Some(SurfaceGeometry::Plane { frame }) => Some(frame.z),
                _ => None,
            },
        )
        .collect::<Vec<_>>();
    let axis = a
        .topology
        .faces
        .iter()
        .find_map(
            |face| match a.geometry.surfaces.get(face.surface as usize) {
                Some(SurfaceGeometry::Cylinder { frame, .. })
                    if cap_axes
                        .iter()
                        .any(|cap| dot(*cap, frame.z).abs() > 1.0 - 1e-10) =>
                {
                    Some(frame.z)
                }
                _ => None,
            },
        )
        .ok_or_else(coverage)?;
    a.topology
        .faces
        .iter()
        .filter_map(
            |face| match a.geometry.surfaces.get(face.surface as usize) {
                Some(SurfaceGeometry::Plane { frame }) if dot(frame.z, axis) > 1.0 - 1e-10 => {
                    Some(*frame)
                }
                _ => None,
            },
        )
        .max_by(|left, right| dot(left.origin, axis).total_cmp(&dot(right.origin, axis)))
        .ok_or_else(coverage)
}

fn cutter_profile_spans<'a>(
    cutters: &[&'a BrepEnvelope],
    base: Frame3,
    accuracy: Accuracy,
) -> Result<Vec<(&'a BrepEnvelope, f64, f64, CurveRegion2)>, GeometryError> {
    cutters
        .iter()
        .map(|cutter| {
            let cutter_levels = cutter
                .topology
                .vertices
                .iter()
                .map(|vertex| base.local(vertex.position)[2])
                .collect::<Vec<_>>();
            let lo = cutter_levels.iter().copied().fold(f64::INFINITY, f64::min);
            let hi = cutter_levels
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max);
            if !lo.is_finite()
                || hi - lo <= accuracy.geometric * 4.0
                || cutter_levels.iter().any(|level| {
                    (level - lo).abs() > accuracy.geometric
                        && (level - hi).abs() > accuracy.geometric
                })
            {
                return Err(coverage());
            }
            Ok((
                *cutter,
                lo,
                hi,
                face_profile(cutter, base, hi, accuracy.intersection)?,
            ))
        })
        .collect::<Result<Vec<_>, GeometryError>>()
}

fn section_levels(
    section: &SectionCut<'_>,
    original_host: bool,
    low: f64,
    high: f64,
    cutter_profiles: &[(&BrepEnvelope, f64, f64, CurveRegion2)],
) -> Vec<f64> {
    let &SectionCut {
        a, base, accuracy, ..
    } = section;
    let mut levels = vec![0.0, high - low];
    if !original_host {
        for vertex in &a.topology.vertices {
            let level = base.local(vertex.position)[2];
            if level > accuracy.geometric && level < high - low - accuracy.geometric {
                levels.push(level);
            }
        }
    }
    for (_, lo, hi, _) in cutter_profiles {
        for level in [*lo, *hi] {
            if level > accuracy.geometric && level < high - low - accuracy.geometric {
                levels.push(level);
            }
        }
    }
    levels.sort_by(f64::total_cmp);
    levels.dedup_by(|left, right| (*left - *right).abs() <= accuracy.geometric);
    levels
}

fn layer_regions(
    section: &SectionCut<'_>,
    levels: &[f64],
    original_profile: &Option<CurveRegion2>,
    cutter_profiles: &[(&BrepEnvelope, f64, f64, CurveRegion2)],
) -> Result<Vec<Vec<CurveRegion2>>, GeometryError> {
    let &SectionCut {
        a, base, accuracy, ..
    } = section;
    levels
        .windows(2)
        .map(|span| {
            let middle = (span[0] + span[1]) / 2.0;
            let host_regions = if let Some(profile) = original_profile {
                vec![profile.clone()]
            } else {
                sectional_regions(a, base, middle, accuracy.intersection)?
            };
            let active = cutter_profiles
                .iter()
                .filter(|(_, lo, hi, _)| middle > *lo && middle < *hi)
                .map(|(_, _, _, profile)| profile.clone())
                .collect::<Vec<_>>();
            boolean_curved_regions(
                &host_regions,
                &active,
                PlanarBooleanOp::Subtraction,
                accuracy.intersection,
            )
        })
        .collect::<Result<Vec<_>, _>>()
}

fn exposed_regions(
    layers: &[Vec<CurveRegion2>],
    accuracy: Accuracy,
) -> Result<(Vec<Vec<CurveRegion2>>, Vec<Vec<CurveRegion2>>), GeometryError> {
    let exposed_up = (1..layers.len())
        .map(|index| {
            boolean_curved_regions(
                &layers[index - 1],
                &layers[index],
                PlanarBooleanOp::Subtraction,
                accuracy.intersection,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let exposed_down = (1..layers.len())
        .map(|index| {
            boolean_curved_regions(
                &layers[index],
                &layers[index - 1],
                PlanarBooleanOp::Subtraction,
                accuracy.intersection,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((exposed_up, exposed_down))
}

fn split_at_shared_points(
    layers: &mut [Vec<CurveRegion2>],
    exposed_up: &mut [Vec<CurveRegion2>],
    exposed_down: &mut [Vec<CurveRegion2>],
    accuracy: Accuracy,
) {
    let mut all_points = Vec::new();
    for region in layers.iter() {
        append_points(region, &mut all_points);
    }
    for region in exposed_up.iter().chain(exposed_down.iter()) {
        append_points(region, &mut all_points);
    }
    for region in layers.iter_mut() {
        split_regions(region, &all_points, accuracy.intersection);
    }
    for region in exposed_up.iter_mut().chain(exposed_down.iter_mut()) {
        split_regions(region, &all_points, accuracy.intersection);
    }
}

fn add_caps(
    facets: &mut Facets,
    section: &SectionCut<'_>,
    cutters: &[&BrepEnvelope],
    regions: &[CurveRegion2],
    level: f64,
    up: bool,
) -> Result<(), GeometryError> {
    let &SectionCut {
        a, base, accuracy, ..
    } = section;
    for region in regions {
        let provenance = cap_provenance(a, cutters, base, level, region, accuracy.intersection)?;
        facets.cap(std::slice::from_ref(region), level, up, provenance)?;
    }
    Ok(())
}

fn add_layer_faces(
    facets: &mut Facets,
    side_patches: &mut Vec<SidePatch>,
    section: &SectionCut<'_>,
    layers: &[Vec<CurveRegion2>],
    levels: &[f64],
    exposed_up: &[Vec<CurveRegion2>],
    exposed_down: &[Vec<CurveRegion2>],
) -> Result<(), GeometryError> {
    let cutters = section.cutters;
    for (index, regions) in layers.iter().enumerate() {
        let lo = levels[index];
        let hi = levels[index + 1];
        for region in regions {
            for ring in std::iter::once(&region.outer).chain(&region.holes) {
                for edge in ring {
                    side_patches.push(side_patch(section, edge, lo, hi)?);
                }
            }
        }
        if index + 1 < layers.len() {
            let cap_level = levels[index + 1];
            add_caps(
                facets,
                section,
                cutters,
                &exposed_up[index],
                cap_level,
                true,
            )?;
            add_caps(
                facets,
                section,
                cutters,
                &exposed_down[index],
                cap_level,
                false,
            )?;
        }
    }
    Ok(())
}

fn side_patch(
    section: &SectionCut<'_>,
    edge: &CurveEdge2,
    lo: f64,
    hi: f64,
) -> Result<SidePatch, GeometryError> {
    let &SectionCut {
        a,
        cutters,
        base,
        accuracy,
    } = section;
    let p = edge.point(0.5);
    let middle = base.point([p.x, p.z, (lo + hi) / 2.0]);
    let from_a = source_for_side(a, middle, base.z, accuracy.intersection)?;
    let mut from_b = Vec::new();
    for cutter in cutters {
        from_b.extend(source_for_side(
            cutter,
            middle,
            base.z,
            accuracy.intersection,
        )?);
    }
    let cut = from_a.is_empty() && !from_b.is_empty();
    let sources = if cut { from_b } else { from_a };
    if sources.is_empty() {
        return Err(coverage());
    }
    Ok(SidePatch {
        edge: edge.clone(),
        levels: vec![lo, hi],
        provenance: FaceProvenance {
            sources,
            role: if cut { FaceRole::Cut } else { FaceRole::Split },
            reversed: cut,
        },
    })
}

fn add_sides(
    facets: &mut Facets,
    side_patches: &[SidePatch],
    accuracy: Accuracy,
) -> Result<(), GeometryError> {
    let mut consumed = vec![false; side_patches.len()];
    for first in 0..side_patches.len() {
        if consumed[first] {
            continue;
        }
        consumed[first] = true;
        let mut levels = side_patches[first].levels.clone();
        loop {
            let candidates = (first + 1..side_patches.len())
                .filter(|&index| {
                    !consumed[index]
                        && (side_patches[index].levels[0] - levels[levels.len() - 1]).abs()
                            <= accuracy.geometric
                        && same_side_patch(
                            &side_patches[first],
                            &side_patches[index],
                            accuracy.geometric / 4.0,
                        )
                })
                .collect::<Vec<_>>();
            if candidates.len() != 1 {
                break;
            }
            let next = candidates[0];
            consumed[next] = true;
            levels.push(side_patches[next].levels[1]);
        }
        facets.side(
            &side_patches[first].edge,
            &levels,
            side_patches[first].provenance.clone(),
        )?;
    }
    Ok(())
}
