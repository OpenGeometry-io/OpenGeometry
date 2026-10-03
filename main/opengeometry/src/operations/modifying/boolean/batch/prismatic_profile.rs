use crate::brep::{
    unit, Accuracy, BrepEnvelope, Face, FaceProvenance, FaceRole, FaceSource, Frame3,
    GeometryError, GeometryQuality, Surface, SurfaceGeometry,
};
use crate::geom2d::{
    boolean_curved_regions, reversed_ring, CurveEdge2, CurveRegion2, PlanarBooleanOp,
};
use crate::math::{add, cross, dot, scale, Interval, Point3};
use crate::operations::modifying::boolean::assembly::{
    analytic_face_mappings, append_analytic_input,
};
use crate::operations::modifying::boolean::operands::{
    brep_face_source, full_cylinder, full_planar_extrusion, unique_sources,
};
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanReport, BooleanResult};
use crate::primitives;
use crate::query::face_contains_uv;

struct ProfileLayout {
    frame: Frame3,
    across: Point3,
    geometric: f64,
    dimensions: [f64; 3],
}

pub(super) fn subtract_prismatic_profile_batch(
    host: &BrepEnvelope,
    cutters: &[BrepEnvelope],
    id: String,
) -> Result<Option<BooleanResult>, GeometryError> {
    let Some(layout) = profile_layout(host, cutters)? else {
        return Ok(None);
    };
    let dimensions = layout.dimensions;
    let host_region = CurveRegion2 {
        outer: line_ring(&[
            [0.0, 0.0],
            [0.0, dimensions[1]],
            [dimensions[0], dimensions[1]],
            [dimensions[0], 0.0],
        ]),
        holes: Vec::new(),
    };
    let Some(cut_regions) = cut_regions(cutters, &layout)? else {
        return Ok(None);
    };
    let regions = boolean_curved_regions(
        &[host_region],
        &cut_regions,
        PlanarBooleanOp::Subtraction,
        layout.geometric,
    )
    .map_err(mixed_profile_arrangement)?;
    if regions.is_empty() {
        return Ok(None);
    }
    let accuracy = profile_accuracy(host, cutters, layout.geometric);
    let mut out = profile_solid(regions, id, &layout, accuracy)?;
    let inputs = std::iter::once(host)
        .chain(cutters.iter())
        .collect::<Vec<_>>();
    set_profile_provenance(&mut out, host, cutters, &inputs, &layout)?;
    finish_profile_batch_result(out, host, cutters, inputs).map(Some)
}

fn profile_layout(
    host: &BrepEnvelope,
    cutters: &[BrepEnvelope],
) -> Result<Option<ProfileLayout>, GeometryError> {
    let Ok(prism) = full_planar_extrusion(host) else {
        return Ok(None);
    };
    if prism.contours.len() != 1
        || prism.contours[0].len() != 4
        || host.topology.vertices.len() != 8
    {
        return Ok(None);
    }
    let first_axis = cutters.iter().find_map(|cutter| {
        full_planar_extrusion(cutter)
            .ok()
            .map(|input| input.frame.z)
            .or_else(|| full_cylinder(cutter).ok().map(|input| input.frame.z))
    });
    let Some(across) = first_axis else {
        return Ok(None);
    };
    let up = prism.frame.z;
    if dot(up, across).abs() > 1.0e-10 {
        return Ok(None);
    }
    let along = unit(cross(up, across))?;
    let basis = [along, up, across];
    let mut ranges = [[f64::INFINITY, f64::NEG_INFINITY]; 3];
    for vertex in &host.topology.vertices {
        for axis in 0..3 {
            let value = dot(vertex.position, basis[axis]);
            ranges[axis][0] = ranges[axis][0].min(value);
            ranges[axis][1] = ranges[axis][1].max(value);
        }
    }
    let geometric = std::iter::once(host)
        .chain(cutters.iter())
        .map(|brep| brep.accuracy.geometric)
        .fold(0.0_f64, f64::max);
    let dimensions = ranges.map(|range| range[1] - range[0]);
    if dimensions.iter().any(|value| *value <= 4.0 * geometric)
        || host.topology.vertices.iter().any(|vertex| {
            (0..3).any(|axis| {
                let value = dot(vertex.position, basis[axis]);
                (value - ranges[axis][0]).abs() > geometric
                    && (value - ranges[axis][1]).abs() > geometric
            })
        })
    {
        return Ok(None);
    }
    let frame = Frame3 {
        origin: add(
            add(scale(along, ranges[0][0]), scale(up, ranges[1][0])),
            scale(across, ranges[2][0]),
        ),
        x: along,
        y: up,
        z: across,
    };
    frame.validate()?;
    Ok(Some(ProfileLayout {
        frame,
        across,
        geometric,
        dimensions,
    }))
}

fn line_ring(points: &[[f64; 2]]) -> Vec<CurveEdge2> {
    points
        .iter()
        .enumerate()
        .map(|(index, point)| CurveEdge2::Line {
            from: *point,
            to: points[(index + 1) % points.len()],
        })
        .collect()
}

fn cut_regions(
    cutters: &[BrepEnvelope],
    layout: &ProfileLayout,
) -> Result<Option<Vec<CurveRegion2>>, GeometryError> {
    let mut cut_regions = Vec::with_capacity(cutters.len());
    for cutter in cutters {
        let Some(region) = cut_region(cutter, layout)? else {
            return Ok(None);
        };
        cut_regions.push(region);
    }
    Ok(Some(cut_regions))
}

fn cut_region(
    cutter: &BrepEnvelope,
    layout: &ProfileLayout,
) -> Result<Option<CurveRegion2>, GeometryError> {
    let &ProfileLayout {
        frame,
        across,
        geometric,
        dimensions,
    } = layout;
    let vertices_cover_depth = || {
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for vertex in &cutter.topology.vertices {
            let value = frame.local(vertex.position)[2];
            lo = lo.min(value);
            hi = hi.max(value);
        }
        lo <= geometric && hi >= dimensions[2] - geometric
    };
    if !vertices_cover_depth() {
        return Ok(None);
    }
    if let Ok(input) = full_planar_extrusion(cutter) {
        if dot(input.frame.z, across).abs() < 1.0 - 1.0e-10 {
            return Ok(None);
        }
        let mut rings = input.contours.iter().map(|contour| {
            line_ring(
                &contour
                    .iter()
                    .map(|point| {
                        let local = frame.local(*point);
                        [local[0], local[1]]
                    })
                    .collect::<Vec<_>>(),
            )
        });
        let outer = winding_ring(
            rings.next().ok_or_else(|| {
                GeometryError::InvalidTopology("profile cutter has no outer ring".into())
            })?,
            false,
        );
        let holes = rings.map(|ring| winding_ring(ring, true)).collect();
        Ok(Some(CurveRegion2 { outer, holes }))
    } else if let Ok(input) = full_cylinder(cutter) {
        if dot(input.frame.z, across).abs() < 1.0 - 1.0e-10 {
            return Ok(None);
        }
        let center = frame.local(input.frame.origin);
        Ok(Some(CurveRegion2 {
            outer: vec![
                CurveEdge2::Arc {
                    center: [center[0], center[1]],
                    radius: input.radius,
                    start_angle: 0.0,
                    sweep_angle: -std::f64::consts::PI,
                },
                CurveEdge2::Arc {
                    center: [center[0], center[1]],
                    radius: input.radius,
                    start_angle: -std::f64::consts::PI,
                    sweep_angle: -std::f64::consts::PI,
                },
            ],
            holes: Vec::new(),
        }))
    } else {
        Ok(None)
    }
}

fn winding_ring(mut ring: Vec<CurveEdge2>, positive: bool) -> Vec<CurveEdge2> {
    let area = ring.iter().map(CurveEdge2::twice_area).sum::<f64>();
    if (area > 0.0) != positive {
        reverse_ring_in_place(&mut ring);
    }
    ring
}

fn reverse_ring_in_place(ring: &mut Vec<CurveEdge2>) {
    *ring = reversed_ring(ring);
}

fn mixed_profile_arrangement(error: GeometryError) -> GeometryError {
    match error {
        GeometryError::UnresolvedIntersection(reason) => GeometryError::UnresolvedIntersection(
            format!("mixed profile batch arrangement: {reason}"),
        ),
        other => other,
    }
}

fn profile_accuracy(host: &BrepEnvelope, cutters: &[BrepEnvelope], geometric: f64) -> Accuracy {
    Accuracy {
        geometric,
        intersection: std::iter::once(host)
            .chain(cutters.iter())
            .map(|brep| brep.accuracy.intersection)
            .fold(0.0_f64, f64::max),
        tessellation: std::iter::once(host)
            .chain(cutters.iter())
            .map(|brep| brep.accuracy.tessellation)
            .fold(0.0_f64, f64::max),
        exchange: std::iter::once(host)
            .chain(cutters.iter())
            .map(|brep| brep.accuracy.exchange)
            .fold(0.0_f64, f64::max),
    }
}

fn profile_solid(
    regions: Vec<CurveRegion2>,
    id: String,
    layout: &ProfileLayout,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    let &ProfileLayout {
        frame, dimensions, ..
    } = layout;
    let single_region = regions.len() == 1;
    let mut parts = Vec::with_capacity(regions.len());
    for (index, region) in regions.into_iter().enumerate() {
        parts.push(primitives::arc_edged_extrusion_with_holes(
            if single_region {
                id.clone()
            } else {
                format!("{id}:part:{index}")
            },
            frame,
            profile_edges(region.outer),
            region.holes.into_iter().map(profile_edges).collect(),
            dimensions[2],
            accuracy,
        )?);
    }
    let out = if single_region {
        parts.pop().ok_or_else(|| {
            GeometryError::InvalidTopology("profile batch produced no material region".into())
        })?
    } else {
        let mut compound = BrepEnvelope::new(id, accuracy)?;
        for part in &parts {
            append_analytic_input(&mut compound, part)?;
        }
        compound
    };
    Ok(out)
}

fn profile_edges(ring: Vec<CurveEdge2>) -> Vec<primitives::ProfileEdge> {
    ring.into_iter()
        .map(|edge| match edge {
            CurveEdge2::Line { from, to } => primitives::ProfileEdge::Line { from, to },
            CurveEdge2::Arc {
                center,
                radius,
                start_angle,
                sweep_angle,
            } => primitives::ProfileEdge::Arc {
                center,
                radius,
                start_angle,
                sweep_angle,
            },
        })
        .collect()
}

fn set_profile_provenance(
    out: &mut BrepEnvelope,
    host: &BrepEnvelope,
    cutters: &[BrepEnvelope],
    inputs: &[&BrepEnvelope],
    layout: &ProfileLayout,
) -> Result<(), GeometryError> {
    for result_face in &mut out.topology.faces {
        let surface = out.geometry.surface(result_face.surface)?;
        let sources = profile_face_sources(surface, result_face, host, inputs, layout)?;
        if sources.is_empty() {
            return Err(GeometryError::InvalidTopology(format!(
                "profile batch output face {} has no source",
                result_face.key
            )));
        }
        let cut = sources
            .iter()
            .any(|source| cutters.iter().any(|cutter| cutter.id == source.entity));
        result_face.provenance = FaceProvenance {
            sources: unique_sources(sources),
            role: if cut { FaceRole::Cut } else { FaceRole::Split },
            reversed: cut,
        };
    }
    Ok(())
}

fn profile_face_sources(
    surface: &SurfaceGeometry,
    result_face: &Face,
    host: &BrepEnvelope,
    inputs: &[&BrepEnvelope],
    layout: &ProfileLayout,
) -> Result<Vec<FaceSource>, GeometryError> {
    let &ProfileLayout {
        across, geometric, ..
    } = layout;
    let mut sources = Vec::new();
    let cap = matches!(surface, SurfaceGeometry::Plane { frame: plane }
        if dot(plane.z, across).abs() > 1.0 - 1.0e-10);
    if cap {
        let position = dot(surface.frame().origin, across);
        for face in &host.topology.faces {
            let SurfaceGeometry::Plane {
                frame: source_frame,
            } = host.geometry.surface(face.surface)?
            else {
                continue;
            };
            if dot(source_frame.z, across).abs() > 1.0 - 1.0e-10
                && (dot(source_frame.origin, across) - position).abs() <= geometric
            {
                sources.push(brep_face_source(host, face.id));
            }
        }
    } else {
        let uv = result_face.trim.uv_bounds.map(Interval::midpoint);
        let sample = surface.point_at(uv)?;
        for input in inputs {
            for face in &input.topology.faces {
                let source_surface = input.geometry.surface(face.surface)?;
                let on = match source_surface {
                    SurfaceGeometry::Plane {
                        frame: source_frame,
                    } => {
                        source_frame.local(sample)[2].abs() <= geometric
                            && dot(source_frame.z, across).abs() < 1.0 - 1.0e-10
                    }
                    SurfaceGeometry::Cylinder {
                        frame: source_frame,
                        radius,
                    } => {
                        let local = source_frame.local(sample);
                        (local[0].hypot(local[1]) - radius).abs() <= geometric
                    }
                    _ => false,
                };
                if !on {
                    continue;
                }
                let hint = face.trim.uv_bounds.map(Interval::midpoint);
                let source_uv = source_surface.project(sample, Some(hint))?;
                if face_contains_uv(input, face, source_uv)? == Some(true) {
                    sources.push(brep_face_source(input, face.id));
                }
            }
        }
    }
    Ok(sources)
}

fn finish_profile_batch_result(
    mut out: BrepEnvelope,
    host: &BrepEnvelope,
    cutters: &[BrepEnvelope],
    inputs: Vec<&BrepEnvelope>,
) -> Result<BooleanResult, GeometryError> {
    out.revision = std::iter::once(host)
        .chain(cutters.iter())
        .map(|brep| brep.revision)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| GeometryError::LimitExceeded("boolean revision overflow".into()))?;
    out.validate()?;
    Ok(BooleanResult {
        report: BooleanReport {
            operation: BooleanOp::Subtraction,
            quality: GeometryQuality::Analytic,
            contacts: Vec::new(),
            coincident: false,
            face_mappings: analytic_face_mappings(&out, inputs),
        },
        brep: out,
    })
}
