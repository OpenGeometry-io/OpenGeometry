use crate::brep::{Accuracy, CurveGeometry, Frame3, GeometryError, IntersectionDefinition};
use crate::intersection::IntersectionGraph;
use crate::math::{add, scale, Interval};

pub(super) fn chart_seam_rotation(graph: &IntersectionGraph) -> f64 {
    let tau = std::f64::consts::TAU;
    let mut parameters = graph
        .branches
        .iter()
        .filter_map(
            |branch| match graph.geometry.curves[branch.curve as usize] {
                CurveGeometry::Intersection { definition } => Some(definition),
                _ => None,
            },
        )
        .flat_map(|definition| {
            graph.geometry.intersections[definition as usize]
                .anchors
                .iter()
                .map(|anchor| anchor.uv_a[0].rem_euclid(tau))
        })
        .collect::<Vec<_>>();
    parameters.sort_by(f64::total_cmp);
    parameters.dedup_by(|left, right| (*left - *right).abs() <= f64::EPSILON * 64.0);
    if parameters.is_empty() {
        return 0.0;
    }
    let mut largest = (0.0, parameters[0]);
    for index in 0..parameters.len() {
        let start = parameters[index];
        let end = if index + 1 < parameters.len() {
            parameters[index + 1]
        } else {
            parameters[0] + tau
        };
        if end - start > largest.0 {
            largest = (end - start, start);
        }
    }
    (largest.1 + 0.5 * largest.0).rem_euclid(tau)
}

pub(super) fn rotate_cylinder_chart(frame: Frame3, angle: f64) -> Frame3 {
    let (sine, cosine) = angle.sin_cos();
    Frame3 {
        x: add(scale(frame.x, cosine), scale(frame.y, sine)),
        y: add(scale(frame.y, cosine), scale(frame.x, -sine)),
        ..frame
    }
}

pub(super) fn rotate_first_intersection_chart(
    definition: &mut IntersectionDefinition,
    angle: f64,
    accuracy: Accuracy,
) -> Result<(), GeometryError> {
    let tau = std::f64::consts::TAU;
    let original_first = definition
        .anchors
        .first()
        .ok_or_else(|| GeometryError::InvalidGeometry("empty intersection branch".into()))?
        .uv_a[0];
    let mut previous: Option<f64> = None;
    for anchor in &mut definition.anchors {
        let mut value = (anchor.uv_a[0] - angle).rem_euclid(tau);
        if let Some(previous) = previous {
            value += ((previous - value) / tau).round() * tau;
        }
        anchor.uv_a[0] = value;
        previous = Some(value);
    }
    let lo = definition
        .anchors
        .iter()
        .map(|anchor| anchor.uv_a[0])
        .fold(f64::INFINITY, f64::min);
    let hi = definition
        .anchors
        .iter()
        .map(|anchor| anchor.uv_a[0])
        .fold(f64::NEG_INFINITY, f64::max);
    if hi - lo >= tau - accuracy.intersection {
        return Err(GeometryError::CoverageGap {
            families: [
                "cylinder through subtraction".into(),
                "intersection loop crossing every chart seam".into(),
            ],
        });
    }
    let shift = -(lo / tau).floor() * tau;
    for anchor in &mut definition.anchors {
        anchor.uv_a[0] += shift;
    }
    let chart_shift = definition.anchors[0].uv_a[0] - original_first;
    for tube in &mut definition.uv_tubes {
        tube[0] = Interval::new(tube[0].lo + chart_shift, tube[0].hi + chart_shift)?;
    }
    Ok(())
}
