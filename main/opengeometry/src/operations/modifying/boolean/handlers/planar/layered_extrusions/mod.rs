mod regions;

use super::facets::{coverage, finish, PlanarFacets};
use crate::brep::{
    unit, Accuracy, BrepEnvelope, CurveGeometry, FaceProvenance, FaceRole, FaceSource, Frame3,
    GeometryError, SurfaceGeometry,
};
use crate::geom2d::{Pt2, RingRegion};
use crate::math::{cross, dot, scale, sub, Point3};
use crate::operations::modifying::boolean::operands::{
    all_planar, brep_face_source, full_planar_extrusion,
};
use crate::operations::modifying::boolean::types::BooleanResult;
use crate::query::face_contains_uv;
use regions::{contours, layer_boundaries, layer_regions, LayerBoundaries};

struct LayerSlab<'a> {
    base: Frame3,
    regions: &'a [RingRegion],
    vertices: &'a [Pt2],
    lo: f64,
    hi: f64,
}

pub(crate) fn subtract_layered_extrusions(
    host: &BrepEnvelope,
    cutter: &BrepEnvelope,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    host.validate()?;
    let b = full_planar_extrusion(cutter)?;
    if !all_planar(host)
        || host
            .geometry
            .curves
            .iter()
            .any(|curve| !matches!(curve, CurveGeometry::Line { .. }))
        || host.solids.is_empty()
    {
        return Err(coverage());
    }
    let accuracy = Accuracy::combined(host.accuracy, cutter.accuracy);
    let host_positions = host
        .topology
        .vertices
        .iter()
        .map(|vertex| b.frame.local(vertex.position)[2])
        .collect::<Vec<_>>();
    let host_lo = host_positions.iter().copied().fold(f64::INFINITY, f64::min);
    let host_hi = host_positions
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    if !host_lo.is_finite() || host_hi - host_lo <= 4.0 * accuracy.geometric {
        return Err(coverage());
    }
    let frame = Frame3 {
        origin: b.frame.point([0.0, 0.0, host_lo]),
        ..b.frame
    };
    check_layer_alignment(host, frame)?;
    let b_contours = contours(&b, frame);
    let b_lo = dot(sub(b.frame.origin, frame.origin), frame.z);
    let b_hi = b_lo + b.height;
    let host_height = host_hi - host_lo;
    if b_hi <= accuracy.geometric || b_lo >= host_height - accuracy.geometric {
        return Err(coverage());
    }
    let levels = layer_levels(&host_positions, host_lo, b_lo, b_hi, host_height, accuracy);
    let layers = layer_regions(host, frame, &levels, &b_contours, b_lo, b_hi, accuracy)?;
    let boundaries = layer_boundaries(&layers, accuracy);
    let mut facets = PlanarFacets::new(id, accuracy)?;
    cap(
        &mut facets,
        frame,
        0.0,
        false,
        &layers[0],
        &boundaries.vertices,
        &[(host, FaceRole::Split)],
    )?;
    add_layer_faces(
        &mut facets,
        frame,
        &levels,
        &layers,
        &boundaries,
        host,
        cutter,
    )?;
    cap(
        &mut facets,
        frame,
        host_height,
        true,
        layers.last().ok_or_else(coverage)?,
        &boundaries.vertices,
        &[(host, FaceRole::Split)],
    )?;
    finish(facets, host, cutter)
}

fn check_layer_alignment(host: &BrepEnvelope, frame: Frame3) -> Result<(), GeometryError> {
    for face in &host.topology.faces {
        let SurfaceGeometry::Plane { frame: face_frame } = host
            .geometry
            .surfaces
            .get(face.surface as usize)
            .ok_or_else(coverage)?
        else {
            return Err(coverage());
        };
        let alignment = dot(face_frame.z, frame.z).abs();
        if alignment > 1.0e-10 && alignment < 1.0 - 1.0e-10 {
            return Err(coverage());
        }
    }
    Ok(())
}

fn layer_levels(
    host_positions: &[f64],
    host_lo: f64,
    b_lo: f64,
    b_hi: f64,
    host_height: f64,
    accuracy: Accuracy,
) -> Vec<f64> {
    let mut levels = host_positions
        .iter()
        .map(|level| level - host_lo)
        .collect::<Vec<_>>();
    for level in [b_lo, b_hi] {
        if level > accuracy.geometric && level < host_height - accuracy.geometric {
            levels.push(level);
        }
    }
    levels.sort_by(f64::total_cmp);
    levels.dedup_by(|left, right| (*left - *right).abs() <= accuracy.geometric);
    levels
}

fn cap(
    facets: &mut PlanarFacets,
    base: Frame3,
    level: f64,
    up: bool,
    regions: &[RingRegion],
    vertices: &[Pt2],
    source_breps: &[(&BrepEnvelope, FaceRole)],
) -> Result<(), GeometryError> {
    let origin = base.point([0.0, 0.0, level]);
    let frame = if up {
        Frame3 { origin, ..base }
    } else {
        Frame3 {
            origin,
            x: base.x,
            y: scale(base.y, -1.0),
            z: scale(base.z, -1.0),
        }
    };
    for region in regions {
        let sample = region_sample(region)?;
        let mut selected = None;
        for &(brep, role) in source_breps {
            let sources = cap_sources(
                brep,
                base.point([sample.x, sample.z, level]),
                frame.z,
                role == FaceRole::Cut,
                facets.accuracy.intersection,
            )?;
            if !sources.is_empty() {
                selected = Some((sources, role));
                break;
            }
        }
        let (sources, role) = selected.ok_or_else(|| {
            GeometryError::InvalidTopology(format!(
                "layered planar cap has no source face at level {level}"
            ))
        })?;
        let mut outer = split_ring(&region.outer, vertices, facets.accuracy.geometric / 4.0)
            .into_iter()
            .map(|point| base.point([point.x, point.z, level]))
            .collect::<Vec<_>>();
        let mut holes = region
            .holes
            .iter()
            .map(|ring| {
                split_ring(ring, vertices, facets.accuracy.geometric / 4.0)
                    .into_iter()
                    .map(|point| base.point([point.x, point.z, level]))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        if !up {
            outer.reverse();
            for hole in &mut holes {
                hole.reverse();
            }
        }
        facets.face(
            if up { "upper-cap" } else { "lower-cap" },
            frame,
            outer,
            holes,
            FaceProvenance {
                sources,
                role,
                reversed: role == FaceRole::Cut,
            },
        )?;
    }
    Ok(())
}

fn region_sample(region: &RingRegion) -> Result<Pt2, GeometryError> {
    let mut coordinates = Vec::new();
    let mut holes = Vec::new();
    for point in &region.outer {
        coordinates.extend([point.x, point.z]);
    }
    for hole in &region.holes {
        holes.push(coordinates.len() / 2);
        for point in hole {
            coordinates.extend([point.x, point.z]);
        }
    }
    let triangles = earcutr::earcut(&coordinates, &holes, 2);
    let triangle = triangles
        .chunks_exact(3)
        .max_by(|left, right| {
            let area = |triangle: &[usize]| {
                let point =
                    |index: usize| Pt2::new(coordinates[index * 2], coordinates[index * 2 + 1]);
                let a = point(triangle[0]);
                let b = point(triangle[1]);
                let c = point(triangle[2]);
                ((b.x - a.x) * (c.z - a.z) - (b.z - a.z) * (c.x - a.x)).abs()
            };
            area(left).total_cmp(&area(right))
        })
        .ok_or_else(coverage)?;
    Ok(Pt2::new(
        triangle
            .iter()
            .map(|&index| coordinates[index * 2])
            .sum::<f64>()
            / 3.0,
        triangle
            .iter()
            .map(|&index| coordinates[index * 2 + 1])
            .sum::<f64>()
            / 3.0,
    ))
}

fn cap_sources(
    brep: &BrepEnvelope,
    point: Point3,
    outward: Point3,
    cut: bool,
    tolerance: f64,
) -> Result<Vec<FaceSource>, GeometryError> {
    let mut sources = Vec::new();
    for face in &brep.topology.faces {
        let SurfaceGeometry::Plane { frame } = brep
            .geometry
            .surfaces
            .get(face.surface as usize)
            .ok_or_else(coverage)?
        else {
            return Err(coverage());
        };
        let local = frame.local(point);
        if local[2].abs() > tolerance {
            continue;
        }
        let actual = scale(frame.z, face.sense.multiplier());
        let alignment = dot(actual, outward);
        if (cut && alignment > -1.0 + 1.0e-10) || (!cut && alignment < 1.0 - 1.0e-10) {
            continue;
        }
        if face_contains_uv(brep, face, [local[0], local[1]])? == Some(true) {
            sources.push(brep_face_source(brep, face.id));
        }
    }
    Ok(sources)
}

fn split_ring(ring: &[Pt2], vertices: &[Pt2], tolerance: f64) -> Vec<Pt2> {
    let mut split = Vec::new();
    for index in 0..ring.len() {
        let from = ring[index];
        let to = ring[(index + 1) % ring.len()];
        let delta = Pt2::new(to.x - from.x, to.z - from.z);
        let length2 = delta.x * delta.x + delta.z * delta.z;
        if length2 <= tolerance * tolerance {
            continue;
        }
        split.push(from);
        let mut interior = Vec::new();
        for &point in vertices {
            let offset = Pt2::new(point.x - from.x, point.z - from.z);
            let t = (offset.x * delta.x + offset.z * delta.z) / length2;
            let distance = (offset.x * delta.z - offset.z * delta.x).abs() / length2.sqrt();
            if t > tolerance / length2.sqrt()
                && t < 1.0 - tolerance / length2.sqrt()
                && distance <= tolerance
            {
                interior.push((t, point));
            }
        }
        interior.sort_by(|left, right| left.0.total_cmp(&right.0));
        for (_, point) in interior {
            if split
                .last()
                .is_none_or(|last| (last.x - point.x).hypot(last.z - point.z) > tolerance)
            {
                split.push(point);
            }
        }
    }
    split
}

fn add_layer_faces(
    facets: &mut PlanarFacets,
    frame: Frame3,
    levels: &[f64],
    layers: &[Vec<RingRegion>],
    boundaries: &LayerBoundaries,
    host: &BrepEnvelope,
    cutter: &BrepEnvelope,
) -> Result<(), GeometryError> {
    for (index, layer) in layers.iter().enumerate() {
        side_faces(
            facets,
            host,
            cutter,
            &LayerSlab {
                base: frame,
                regions: layer,
                vertices: &boundaries.vertices,
                lo: levels[index],
                hi: levels[index + 1],
            },
        )?;
        if index + 1 < layers.len() {
            cap(
                facets,
                frame,
                levels[index + 1],
                true,
                &boundaries.exposed_up[index],
                &boundaries.vertices,
                &[(host, FaceRole::Split), (cutter, FaceRole::Cut)],
            )?;
            cap(
                facets,
                frame,
                levels[index + 1],
                false,
                &boundaries.exposed_down[index],
                &boundaries.vertices,
                &[(host, FaceRole::Split), (cutter, FaceRole::Cut)],
            )?;
        }
    }
    Ok(())
}

fn side_faces(
    facets: &mut PlanarFacets,
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    slab: &LayerSlab<'_>,
) -> Result<(), GeometryError> {
    let LayerSlab {
        base,
        regions,
        vertices,
        lo,
        hi,
    } = *slab;
    for region in regions {
        for ring in std::iter::once(&region.outer).chain(&region.holes) {
            let ring = split_ring(ring, vertices, facets.accuracy.geometric / 4.0);
            for index in 0..ring.len() {
                let from = ring[index];
                let to = ring[(index + 1) % ring.len()];
                let lower_from = base.point([from.x, from.z, lo]);
                let lower_to = base.point([to.x, to.z, lo]);
                let upper_to = base.point([to.x, to.z, hi]);
                let upper_from = base.point([from.x, from.z, hi]);
                let normal = unit(cross(
                    sub(lower_to, lower_from),
                    sub(upper_from, lower_from),
                ))?;
                let face_frame = Frame3::from_axis(lower_from, normal, sub(lower_to, lower_from))?;
                let midpoint = scale(
                    [
                        lower_from[0] + lower_to[0] + upper_to[0] + upper_from[0],
                        lower_from[1] + lower_to[1] + upper_to[1] + upper_from[1],
                        lower_from[2] + lower_to[2] + upper_to[2] + upper_from[2],
                    ],
                    0.25,
                );
                let a_sources = side_sources(a, midpoint, normal, facets.accuracy.intersection)?;
                let b_sources = side_sources(b, midpoint, normal, facets.accuracy.intersection)?;
                let is_cut = a_sources.is_empty() && !b_sources.is_empty();
                let sources = if is_cut { b_sources } else { a_sources };
                if sources.is_empty() {
                    return Err(GeometryError::InvalidTopology(
                        "layered planar side has no source face".into(),
                    ));
                }
                facets.face(
                    if is_cut { "cut-side" } else { "host-side" },
                    face_frame,
                    vec![lower_from, lower_to, upper_to, upper_from],
                    Vec::new(),
                    FaceProvenance {
                        sources,
                        role: if is_cut {
                            FaceRole::Cut
                        } else {
                            FaceRole::Split
                        },
                        reversed: is_cut,
                    },
                )?;
            }
        }
    }
    Ok(())
}

fn side_sources(
    brep: &BrepEnvelope,
    point: Point3,
    outward: Point3,
    tolerance: f64,
) -> Result<Vec<FaceSource>, GeometryError> {
    let mut sources = Vec::new();
    for face in &brep.topology.faces {
        let SurfaceGeometry::Plane { frame } = brep
            .geometry
            .surfaces
            .get(face.surface as usize)
            .ok_or_else(coverage)?
        else {
            return Err(coverage());
        };
        let local = frame.local(point);
        if local[2].abs() > tolerance || dot(frame.z, outward).abs() < 1.0 - 1.0e-10 {
            continue;
        }
        if face_contains_uv(brep, face, [local[0], local[1]])? == Some(true) {
            sources.push(brep_face_source(brep, face.id));
        }
    }
    Ok(sources)
}
