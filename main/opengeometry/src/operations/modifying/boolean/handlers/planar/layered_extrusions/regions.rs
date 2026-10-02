use crate::brep::{Accuracy, BrepEnvelope, Frame3, GeometryError, SurfaceGeometry};
use crate::geom2d::{boolean_oriented_regions, PlanarBooleanOp, Pt2, RingRegion};
use crate::math::{cross, dot, scale, sub};
use crate::operations::modifying::boolean::handlers::planar::facets::{coverage, loop_positions};
use crate::operations::modifying::boolean::operands::PrismaticInput;

pub(super) struct LayerBoundaries {
    pub(super) exposed_up: Vec<Vec<RingRegion>>,
    pub(super) exposed_down: Vec<Vec<RingRegion>>,
    pub(super) vertices: Vec<Pt2>,
}

pub(super) fn contours(input: &PrismaticInput<'_>, frame: Frame3) -> Vec<Vec<Pt2>> {
    input
        .contours
        .iter()
        .map(|ring| {
            ring.iter()
                .map(|point| {
                    let local = frame.local(*point);
                    Pt2::new(local[0], local[1])
                })
                .collect()
        })
        .collect()
}

pub(super) fn layer_regions(
    host: &BrepEnvelope,
    frame: Frame3,
    levels: &[f64],
    b_contours: &[Vec<Pt2>],
    b_lo: f64,
    b_hi: f64,
    accuracy: Accuracy,
) -> Result<Vec<Vec<RingRegion>>, GeometryError> {
    levels
        .windows(2)
        .map(|span| -> Result<Vec<RingRegion>, GeometryError> {
            let middle = (span[0] + span[1]) / 2.0;
            let host_regions = slice_regions(host, frame, middle, accuracy.intersection)?;
            Ok(boolean_oriented_regions(
                &region_contours(&host_regions),
                if middle > b_lo && middle < b_hi {
                    b_contours
                } else {
                    &[]
                },
                PlanarBooleanOp::Subtraction,
                accuracy.intersection,
            ))
        })
        .collect::<Result<Vec<_>, _>>()
}

fn slice_regions(
    brep: &BrepEnvelope,
    frame: Frame3,
    level: f64,
    tolerance: f64,
) -> Result<Vec<RingRegion>, GeometryError> {
    let segments = slice_segments(brep, frame, level, tolerance)?;
    let loops = segment_loops(segments, brep, tolerance)?;
    Ok(boolean_oriented_regions(
        &loops,
        &[],
        PlanarBooleanOp::Union,
        tolerance,
    ))
}

fn slice_segments(
    brep: &BrepEnvelope,
    frame: Frame3,
    level: f64,
    tolerance: f64,
) -> Result<Vec<(Pt2, Pt2)>, GeometryError> {
    let mut segments = Vec::<(Pt2, Pt2)>::new();
    for face in &brep.topology.faces {
        let SurfaceGeometry::Plane { frame: face_frame } = brep
            .geometry
            .surfaces
            .get(face.surface as usize)
            .ok_or_else(coverage)?
        else {
            return Err(coverage());
        };
        if dot(face_frame.z, frame.z).abs() > 1.0e-10 {
            continue;
        }
        if !face.trim.holes.is_empty() {
            return Err(coverage());
        }
        let points = loop_positions(brep, face.trim.outer)?;
        let mut crossings = Vec::<Pt2>::new();
        for index in 0..points.len() {
            let from = frame.local(points[index]);
            let to = frame.local(points[(index + 1) % points.len()]);
            if (from[2] < level && to[2] > level) || (from[2] > level && to[2] < level) {
                let fraction = (level - from[2]) / (to[2] - from[2]);
                let point = Pt2::new(
                    from[0] + fraction * (to[0] - from[0]),
                    from[1] + fraction * (to[1] - from[1]),
                );
                if crossings
                    .iter()
                    .all(|existing| (existing.x - point.x).hypot(existing.z - point.z) > tolerance)
                {
                    crossings.push(point);
                }
            }
        }
        if crossings.is_empty() {
            continue;
        }
        if crossings.len() != 2 {
            return Err(coverage());
        }
        let mut from = crossings[0];
        let mut to = crossings[1];
        let edge = sub(
            frame.point([to.x, to.z, level]),
            frame.point([from.x, from.z, level]),
        );
        let outward = cross(edge, frame.z);
        let actual = scale(face_frame.z, face.sense.multiplier());
        if dot(outward, actual) < 0.0 {
            std::mem::swap(&mut from, &mut to);
        }
        segments.push((from, to));
    }
    Ok(segments)
}

fn segment_loops(
    mut segments: Vec<(Pt2, Pt2)>,
    brep: &BrepEnvelope,
    tolerance: f64,
) -> Result<Vec<Vec<Pt2>>, GeometryError> {
    let mut loops = Vec::new();
    while let Some((from, mut to)) = segments.pop() {
        let mut ring = vec![from];
        while (to.x - from.x).hypot(to.z - from.z) > tolerance {
            ring.push(to);
            let Some(next) = segments
                .iter()
                .position(|(start, _)| (start.x - to.x).hypot(start.z - to.z) <= tolerance)
            else {
                return Err(coverage());
            };
            let (_, end) = segments.swap_remove(next);
            to = end;
            if ring.len() > brep.topology.halfedges.len() {
                return Err(coverage());
            }
        }
        if ring.len() < 3 {
            return Err(coverage());
        }
        loops.push(ring);
    }
    Ok(loops)
}

fn region_contours(regions: &[RingRegion]) -> Vec<Vec<Pt2>> {
    regions
        .iter()
        .flat_map(|region| {
            std::iter::once(region.outer.clone()).chain(region.holes.iter().cloned())
        })
        .collect()
}

pub(super) fn layer_boundaries(layers: &[Vec<RingRegion>], accuracy: Accuracy) -> LayerBoundaries {
    let mut exposed_up = Vec::new();
    let mut exposed_down = Vec::new();
    for index in 1..layers.len() {
        let below = region_contours(&layers[index - 1]);
        let above = region_contours(&layers[index]);
        exposed_up.push(boolean_oriented_regions(
            &below,
            &above,
            PlanarBooleanOp::Subtraction,
            accuracy.intersection,
        ));
        exposed_down.push(boolean_oriented_regions(
            &above,
            &below,
            PlanarBooleanOp::Subtraction,
            accuracy.intersection,
        ));
    }
    let mut vertices = Vec::new();
    for layer in layers {
        region_points(layer, &mut vertices);
    }
    for regions in exposed_up.iter().chain(&exposed_down) {
        region_points(regions, &mut vertices);
    }
    LayerBoundaries {
        exposed_up,
        exposed_down,
        vertices,
    }
}

fn region_points(regions: &[RingRegion], out: &mut Vec<Pt2>) {
    for region in regions {
        out.extend(region.outer.iter().copied());
        for hole in &region.holes {
            out.extend(hole.iter().copied());
        }
    }
}
