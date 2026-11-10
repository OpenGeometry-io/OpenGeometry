use super::linear_extrusion::linear_extrusion;
use crate::brep::{Accuracy, BrepEnvelope, Frame3, GeometryError};
use crate::geom2d::{
    offset_path, offset_polyline_regions, CurveEdge2, OffsetError, OffsetOptions, Pt2,
};
use crate::primitives::profile::{
    profile_edge_intersections, profile_self_intersects, ProfileEdge,
};
use crate::primitives::profile_wire::{profile_segment_edges, ProfileSegment};
use serde::{Deserialize, Serialize};

const MITER_LIMIT: f64 = 4.0;
const STROKE_TOLERANCE: f64 = 1.0e-6;
const BEVEL_BELOW_DEGREES: f64 = 28.955_024_371_859_85;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolysolidPath {
    pub start: [f64; 2],
    pub segments: Vec<ProfileSegment>,
    pub closed: bool,
}

#[derive(Debug)]
pub enum PolysolidError {
    Parameter(String),
    Geometry(GeometryError),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PolysolidJustify {
    #[default]
    Centre,
    Left,
    Right,
}

pub fn polysolid(
    id: String,
    frame: Frame3,
    path: &PolysolidPath,
    justify: PolysolidJustify,
    [width, height]: [f64; 2],
    accuracy: Accuracy,
) -> Result<BrepEnvelope, PolysolidError> {
    let g = accuracy.geometric;
    let centre = path_points(path, g)?;
    check_legs(&centre, path.closed, width, g, "")?;
    let edges = profile_segment_edges(path.start, &path.segments, path.closed);
    check_crossing(&edges, path.closed, g)?;
    let points = justified(&centre, path.closed, justify, width)?;
    check_legs(&points, path.closed, width, g, " after justify")?;
    let (outer, holes) = stroke(&points, path.closed, width)?;
    linear_extrusion(id, frame, outer, holes, height, accuracy).map_err(PolysolidError::Geometry)
}

fn path_points(path: &PolysolidPath, g: f64) -> Result<Vec<[f64; 2]>, PolysolidError> {
    if let Some(index) = path
        .segments
        .iter()
        .position(|segment| segment.arc.is_some())
    {
        return Err(parameter(format!(
            "polysolid path segment {index} is an arc; only straight legs are accepted"
        )));
    }
    let values = path.segments.iter().flat_map(|segment| segment.to);
    if !path.start.into_iter().chain(values).all(f64::is_finite) {
        return Err(parameter("polysolid path points must be finite".into()));
    }
    if path.segments.len() < if path.closed { 3 } else { 1 } {
        return Err(parameter(if path.closed {
            "closed polysolid path needs at least three points".into()
        } else {
            "polysolid path needs at least two points".into()
        }));
    }
    let last = path.segments[path.segments.len() - 1].to;
    if path.closed != (distance(last, path.start) <= 4.0 * g) {
        return Err(parameter(if path.closed {
            "closed polysolid path must end at its start".into()
        } else {
            "open polysolid path must not end at its start".into()
        }));
    }
    let mut points = vec![path.start];
    let count = if path.closed {
        path.segments.len() - 1
    } else {
        path.segments.len()
    };
    points.extend(path.segments[..count].iter().map(|segment| segment.to));
    Ok(points)
}

fn check_legs(
    points: &[[f64; 2]],
    closed: bool,
    width: f64,
    g: f64,
    stage: &str,
) -> Result<(), PolysolidError> {
    let legs = if closed {
        points.len()
    } else {
        points.len().saturating_sub(1)
    };
    for index in 0..legs {
        let length = distance(points[index], points[(index + 1) % points.len()]);
        if length <= 4.0 * g {
            return Err(parameter(format!(
                "polysolid path leg {index} has zero length{stage}"
            )));
        }
        if length < width / 2.0 {
            return Err(parameter(format!(
                "polysolid path leg {index} is shorter than half the width{stage}"
            )));
        }
    }
    Ok(())
}

fn check_crossing(edges: &[ProfileEdge], closed: bool, g: f64) -> Result<(), PolysolidError> {
    if !profile_self_intersects(edges, closed, g) {
        return Ok(());
    }
    let last = edges.len() - 1;
    let pair = (0..edges.len())
        .flat_map(|first| (first + 2..edges.len()).map(move |second| (first, second)))
        .filter(|&(first, second)| !(closed && first == 0 && second == last))
        .find(|&(first, second)| {
            !profile_edge_intersections(&edges[first], &edges[second], g).is_empty()
        });
    Err(parameter(match pair {
        Some((first, second)) => {
            format!("polysolid path crosses itself at legs {first} and {second}")
        }
        None => "polysolid path crosses itself where two legs overlap".into(),
    }))
}

fn justified(
    points: &[[f64; 2]],
    closed: bool,
    justify: PolysolidJustify,
    width: f64,
) -> Result<Vec<[f64; 2]>, PolysolidError> {
    let shift = match justify {
        PolysolidJustify::Centre => return Ok(points.to_vec()),
        PolysolidJustify::Left => width / 2.0,
        PolysolidJustify::Right => -width / 2.0,
    };
    let options = OffsetOptions {
        bevel: true,
        acute_threshold_degrees: BEVEL_BELOW_DEGREES,
    };
    let offset = offset_path(
        &plane_points(points),
        shift,
        Some(closed),
        options,
        STROKE_TOLERANCE,
    );
    line_points(&offset.edges, closed).map_err(PolysolidError::Geometry)
}

fn stroke(
    points: &[[f64; 2]],
    closed: bool,
    width: f64,
) -> Result<(Vec<[f64; 2]>, Vec<Vec<[f64; 2]>>), PolysolidError> {
    let regions = offset_polyline_regions(
        &plane_points(points),
        width,
        closed,
        MITER_LIMIT,
        STROKE_TOLERANCE,
    )
    .map_err(|error| match error {
        OffsetError::InvalidParameter(message) => parameter(message),
        OffsetError::InvalidTopology(message) => {
            PolysolidError::Geometry(GeometryError::InvalidTopology(message))
        }
        OffsetError::Clip(error) => PolysolidError::Geometry(error),
    })?;
    let [region] = regions.as_slice() else {
        return Err(parameter(format!(
            "polysolid stroke gives {} regions instead of one",
            regions.len()
        )));
    };
    let holes = region
        .holes
        .iter()
        .map(|hole| line_points(hole, true))
        .collect::<Result<Vec<_>, _>>()
        .map_err(PolysolidError::Geometry)?;
    let outer = line_points(&region.outer, true).map_err(PolysolidError::Geometry)?;
    Ok((outer, holes))
}

fn line_points(edges: &[CurveEdge2], closed: bool) -> Result<Vec<[f64; 2]>, GeometryError> {
    let mut points = Vec::with_capacity(edges.len() + 1);
    for (index, edge) in edges.iter().enumerate() {
        let &CurveEdge2::Line { from, to } = edge else {
            return Err(GeometryError::InvalidGeometry(
                "polysolid outline has an arc edge".into(),
            ));
        };
        points.push(from);
        if !closed && index + 1 == edges.len() {
            points.push(to);
        }
    }
    Ok(points)
}

fn plane_points(points: &[[f64; 2]]) -> Vec<Pt2> {
    points
        .iter()
        .map(|point| Pt2::new(point[0], point[1]))
        .collect()
}

fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    (b[0] - a[0]).hypot(b[1] - a[1])
}

fn parameter(message: String) -> PolysolidError {
    PolysolidError::Parameter(message)
}
