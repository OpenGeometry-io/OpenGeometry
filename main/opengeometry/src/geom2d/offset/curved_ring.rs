use super::cutters::{bands, disk};
use super::ring::OffsetError;
use super::tidy::{merged, oriented_ring, sealed, sliver};
use crate::brep::GeometryError;
use crate::geom2d::boolean2d::PlanarBooleanOp;
use crate::geom2d::curved_boolean2d::{boolean_curved_regions, CurveEdge2, CurveRegion2};
use crate::geom2d::poly2d::Pt2;

pub(crate) fn inset_curved_region(
    region: &CurveRegion2,
    distance: f64,
    eps: f64,
) -> Result<Vec<CurveRegion2>, GeometryError> {
    if !distance.is_finite() || distance <= 4.0 * eps {
        return Err(GeometryError::UnresolvedIntersection(
            "curved inset distance is below resolution".into(),
        ));
    }
    let at = centre(region);
    let region = moved(region, [-at[0], -at[1]]);
    let cutters: Vec<CurveRegion2> = std::iter::once(&region.outer)
        .chain(&region.holes)
        .flatten()
        .flat_map(|edge| bands(edge, distance, eps))
        .collect();
    let regions = boolean_curved_regions(
        &[based(&region)],
        &cutters,
        PlanarBooleanOp::Subtraction,
        eps,
    )?;
    Ok(finished(regions, at, eps))
}

pub(crate) fn offset_curved_ring_variable(
    region: &CurveRegion2,
    distances: &[f64],
    hole_distances: &[Vec<f64>],
    eps: f64,
) -> Result<Vec<CurveRegion2>, OffsetError> {
    if hole_distances.len() != region.holes.len() {
        return Err(OffsetError::InvalidParameter(format!(
            "Expected one distance list per hole: the region has {} holes but {} lists were given",
            region.holes.len(),
            hole_distances.len()
        )));
    }
    let at = centre(region);
    let region = moved(region, [-at[0], -at[1]]);
    let lists = std::iter::once(distances).chain(hole_distances.iter().map(Vec::as_slice));
    let mut cutters = Vec::new();
    for (index, (ring, list)) in std::iter::once(&region.outer)
        .chain(&region.holes)
        .zip(lists)
        .enumerate()
    {
        let label = match index {
            0 => "ring".to_string(),
            _ => format!("holes[{}]", index - 1),
        };
        checked(&label, ring, list)?;
        for (edge, &distance) in ring.iter().zip(list).filter(|(_, d)| **d > 4.0 * eps) {
            let end = edge.point(1.0);
            cutters.extend(bands(edge, distance, eps));
            cutters.push(disk([end.x, end.z], distance));
        }
    }
    let regions = boolean_curved_regions(
        &[based(&region)],
        &cutters,
        PlanarBooleanOp::Subtraction,
        eps,
    )
    .map_err(OffsetError::Clip)?;
    Ok(finished(regions, at, eps))
}

fn checked(label: &str, ring: &[CurveEdge2], distances: &[f64]) -> Result<(), OffsetError> {
    if distances.len() != ring.len() {
        return Err(OffsetError::InvalidParameter(format!(
            "Expected one distance per {label} edge: the ring has {} edges but {} distances were given",
            ring.len(),
            distances.len()
        )));
    }
    for (index, (edge, &distance)) in ring.iter().zip(distances).enumerate() {
        if !distance.is_finite() || distance < 0.0 {
            return Err(OffsetError::InvalidParameter(format!(
                "{label} distances[{index}] = {distance} is invalid: every per-edge distance must be a finite value >= 0"
            )));
        }
        if let CurveEdge2::Arc { radius, .. } = edge {
            if distance > *radius {
                return Err(OffsetError::InvalidParameter(format!(
                    "{label} distances[{index}] = {distance} passes the centre of its arc of radius {radius}"
                )));
            }
        }
    }
    Ok(())
}

fn based(region: &CurveRegion2) -> CurveRegion2 {
    CurveRegion2 {
        outer: oriented_ring(&region.outer, false),
        holes: region
            .holes
            .iter()
            .map(|hole| oriented_ring(hole, true))
            .collect(),
    }
}

fn finished(regions: Vec<CurveRegion2>, at: [f64; 2], eps: f64) -> Vec<CurveRegion2> {
    regions
        .into_iter()
        .filter(|region| !sliver(&region.outer, eps))
        .map(|region| {
            let sealed_region = CurveRegion2 {
                outer: sealed(merged(region.outer, eps), eps),
                holes: region
                    .holes
                    .into_iter()
                    .map(|hole| sealed(merged(hole, eps), eps))
                    .collect(),
            };
            moved(&sealed_region, at)
        })
        .collect()
}

fn samples(region: &CurveRegion2) -> impl Iterator<Item = Pt2> + '_ {
    std::iter::once(&region.outer)
        .chain(&region.holes)
        .flatten()
        .flat_map(|edge| [edge.point(0.0), edge.point(0.5), edge.point(1.0)])
}

fn centre(region: &CurveRegion2) -> [f64; 2] {
    let (low, high) = samples(region).fold(
        ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]),
        |(low, high), point| {
            (
                [low[0].min(point.x), low[1].min(point.z)],
                [high[0].max(point.x), high[1].max(point.z)],
            )
        },
    );
    [(low[0] + high[0]) / 2.0, (low[1] + high[1]) / 2.0]
}

fn moved(region: &CurveRegion2, by: [f64; 2]) -> CurveRegion2 {
    let shift = |point: &mut [f64; 2]| *point = [point[0] + by[0], point[1] + by[1]];
    let ring = |ring: &Vec<CurveEdge2>| {
        let mut ring = ring.clone();
        for edge in &mut ring {
            match edge {
                CurveEdge2::Line { from, to } => {
                    shift(from);
                    shift(to);
                }
                CurveEdge2::Arc { center, .. } => shift(center),
            }
        }
        ring
    };
    CurveRegion2 {
        outer: ring(&region.outer),
        holes: region.holes.iter().map(ring).collect(),
    }
}

pub(crate) fn inset_tolerance(section: &CurveRegion2, intersection: f64) -> f64 {
    let at = centre(section);
    let extent = samples(section).fold(0.0, |extent: f64, point| {
        extent
            .max((point.x - at[0]).abs())
            .max((point.z - at[1]).abs())
    });
    (1e-7 * extent).max(intersection)
}
