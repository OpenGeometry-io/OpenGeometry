use crate::bindings::{errors, OGWorldGraph};
use crate::geom2d::{
    boolean_curved_regions, boolean_oriented_regions, offset_ring_variable, signed_area2,
    CurveRegion2, OffsetError, OffsetHole, PlanarBooleanOp, Pt2, RingRegion,
};
use crate::world_graph::{ErrorCode, GraphError};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsValue;

const REGION_TOLERANCE: f64 = 1.0e-6;
const POINT_LIMIT: usize = 100_000;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegionJson {
    outer: Vec<[f64; 3]>,
    holes: Vec<Vec<[f64; 3]>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OffsetHoleJson {
    ring: Vec<f64>,
    distances: Vec<f64>,
}

#[derive(Serialize)]
struct PointJson {
    x: f64,
    y: f64,
    z: f64,
}

#[derive(Serialize)]
struct RegionOutJson {
    outer: Vec<PointJson>,
    holes: Vec<Vec<PointJson>>,
}

#[wasm_bindgen]
impl OGWorldGraph {
    #[wasm_bindgen(js_name = booleanRegions2D)]
    pub fn boolean_regions_2d(
        &self,
        a_json: &str,
        b_json: &str,
        operation: &str,
    ) -> Result<String, JsValue> {
        let regions = regions(a_json, b_json, operation).map_err(errors::json)?;
        serde_json::to_string(&regions).map_err(errors::serialization)
    }

    #[wasm_bindgen(js_name = booleanCurvedRegions2D)]
    pub fn boolean_curved_regions_2d(
        &self,
        a_json: &str,
        b_json: &str,
        operation: &str,
    ) -> Result<String, JsValue> {
        let regions = curved_regions(a_json, b_json, operation).map_err(errors::json)?;
        serde_json::to_string(&regions).map_err(errors::serialization)
    }

    #[wasm_bindgen(js_name = offsetRingVariable)]
    pub fn offset_ring_variable(
        &self,
        ring: Vec<f64>,
        distances: Vec<f64>,
        holes_json: &str,
    ) -> Result<String, JsValue> {
        let regions = offset_ring(&ring, &distances, holes_json).map_err(errors::json)?;
        serde_json::to_string(&regions).map_err(errors::serialization)
    }
}

fn offset_ring(
    ring: &[f64],
    distances: &[f64],
    holes_json: &str,
) -> Result<Vec<CurveRegion2>, GraphError> {
    let parsed: Vec<OffsetHoleJson> = serde_json::from_str(holes_json).map_err(|error| {
        GraphError::code(
            ErrorCode::InvalidParameter,
            format!("Invalid offset holes JSON: {error}"),
        )
    })?;
    check_size(
        (ring.len() + parsed.iter().map(|hole| hole.ring.len()).sum::<usize>()) / 3,
        "offset points",
    )?;
    let mut holes = Vec::with_capacity(parsed.len());
    for (index, hole) in parsed.into_iter().enumerate() {
        holes.push(OffsetHole {
            ring: flat_ring(&hole.ring, &format!("holes[{index}].ring"))?,
            distances: hole.distances,
        });
    }
    offset_ring_variable(
        &flat_ring(ring, "ring")?,
        distances,
        &holes,
        REGION_TOLERANCE,
    )
    .map_err(|error| match error {
        OffsetError::InvalidParameter(message) => {
            GraphError::code(ErrorCode::InvalidParameter, message)
        }
        OffsetError::InvalidTopology(message) => {
            GraphError::code(ErrorCode::InvalidTopology, message)
        }
        OffsetError::Clip(error) => GraphError::from(error),
    })
}

fn flat_ring(flat: &[f64], label: &str) -> Result<Vec<Pt2>, GraphError> {
    if flat.len() % 3 != 0 || !flat.iter().all(|value| value.is_finite()) {
        return Err(GraphError::code(
            ErrorCode::InvalidParameter,
            format!("{label} must be a flat [x, y, z, ...] list of finite numbers"),
        ));
    }
    Ok(flat
        .chunks_exact(3)
        .map(|point| Pt2::new(point[0], point[2]))
        .collect())
}

fn regions(a_json: &str, b_json: &str, operation: &str) -> Result<Vec<RegionOutJson>, GraphError> {
    let operation = operation_of(operation)?;
    let a: Vec<RegionJson> = parsed(a_json)?;
    let b: Vec<RegionJson> = parsed(b_json)?;
    check_size(
        a.iter()
            .chain(&b)
            .map(|region| region.outer.len() + region.holes.iter().map(Vec::len).sum::<usize>())
            .sum(),
        "Boolean region points",
    )?;
    let base_y = a
        .iter()
        .chain(&b)
        .find_map(|region| region.outer.first().map(|point| point[1]))
        .unwrap_or(0.0);
    let result = boolean_oriented_regions(
        &contours(&a, base_y)?,
        &contours(&b, base_y)?,
        operation,
        REGION_TOLERANCE,
    )
    .map_err(|error| GraphError::code(ErrorCode::InvalidTopology, error.to_string()))?;
    Ok(result
        .iter()
        .map(|region| finished(region, base_y))
        .collect())
}

fn curved_regions(
    a_json: &str,
    b_json: &str,
    operation: &str,
) -> Result<Vec<CurveRegion2>, GraphError> {
    let operation = operation_of(operation)?;
    let a: Vec<CurveRegion2> = parsed(a_json)?;
    let b: Vec<CurveRegion2> = parsed(b_json)?;
    check_size(
        a.iter()
            .chain(&b)
            .map(|region| region.outer.len() + region.holes.iter().map(Vec::len).sum::<usize>())
            .sum(),
        "Boolean region edges",
    )?;
    boolean_curved_regions(&a, &b, operation, REGION_TOLERANCE).map_err(GraphError::from)
}

fn operation_of(text: &str) -> Result<PlanarBooleanOp, GraphError> {
    match text {
        "union" => Ok(PlanarBooleanOp::Union),
        "intersection" => Ok(PlanarBooleanOp::Intersection),
        "subtraction" => Ok(PlanarBooleanOp::Subtraction),
        _ => Err(GraphError::code(
            ErrorCode::InvalidParameter,
            "Unknown 2D Boolean operation",
        )),
    }
}

fn parsed<T: DeserializeOwned>(json: &str) -> Result<T, GraphError> {
    serde_json::from_str(json).map_err(|error| {
        GraphError::code(
            ErrorCode::InvalidParameter,
            format!("Invalid Boolean regions JSON: {error}"),
        )
    })
}

fn check_size(count: usize, items: &str) -> Result<(), GraphError> {
    if count > POINT_LIMIT {
        return Err(GraphError::code(
            ErrorCode::LimitExceeded,
            format!("2D {items}, {count} in all, more than {POINT_LIMIT}"),
        ));
    }
    Ok(())
}

fn contours(regions: &[RegionJson], base_y: f64) -> Result<Vec<Vec<Pt2>>, GraphError> {
    let mut loops = Vec::new();
    for region in regions {
        for (ring, outer) in std::iter::once((&region.outer, true))
            .chain(region.holes.iter().map(|hole| (hole, false)))
        {
            loops.push(checked_ring(ring, outer, base_y)?);
        }
    }
    Ok(loops)
}

fn checked_ring(ring: &[[f64; 3]], outer: bool, base_y: f64) -> Result<Vec<Pt2>, GraphError> {
    if ring.len() < 3
        || ring.iter().any(|point| {
            !point.iter().all(|value| value.is_finite())
                || (point[1] - base_y).abs() > REGION_TOLERANCE
        })
    {
        return Err(GraphError::code(
            ErrorCode::InvalidParameter,
            "Boolean region ring must contain at least three finite coplanar points",
        ));
    }
    let points: Vec<Pt2> = ring
        .iter()
        .map(|point| Pt2::new(point[0], point[2]))
        .collect();
    let area = signed_area2(&points);
    if area.abs() <= REGION_TOLERANCE * REGION_TOLERANCE || (area < 0.0) != outer {
        return Err(GraphError::code(
            ErrorCode::InvalidParameter,
            "Boolean region ring has degenerate or incorrect winding",
        ));
    }
    Ok(points)
}

fn finished(region: &RingRegion, base_y: f64) -> RegionOutJson {
    RegionOutJson {
        outer: lifted(&region.outer, base_y, true),
        holes: region
            .holes
            .iter()
            .map(|hole| lifted(hole, base_y, false))
            .collect(),
    }
}

fn lifted(ring: &[Pt2], base_y: f64, clockwise: bool) -> Vec<PointJson> {
    let mut points = ring.to_vec();
    if (signed_area2(&points) < 0.0) != clockwise {
        points.reverse();
    }
    let start = canonical_start(&points);
    points.rotate_left(start);
    points
        .iter()
        .map(|point| PointJson {
            x: point.x,
            y: base_y,
            z: point.z,
        })
        .collect()
}

fn canonical_start(ring: &[Pt2]) -> usize {
    let mut start = 0;
    for (index, point) in ring.iter().enumerate().skip(1) {
        let dx = point.x - ring[start].x;
        if dx < -1.0e-9 || (dx.abs() <= 1.0e-9 && point.z < ring[start].z - 1.0e-9) {
            start = index;
        }
    }
    start
}
