use super::charts::rotate_first_intersection_chart;
use crate::brep::{Accuracy, Builder, Curve, CurveGeometry, GeometryError};
use crate::intersection::IntersectionGraph;
use crate::math::{norm, sub, Interval};

pub(super) struct NumericalCylinderBoundary {
    pub(super) definition: u32,
    pub(super) edge: u32,
    pub(super) vertex: u32,
    pub(super) cutter_start: [f64; 2],
    pub(super) cutter_end: [f64; 2],
    pub(super) cutter_middle: [f64; 2],
}

pub(super) fn add_numerical_boundaries(
    builder: &mut Builder,
    graph: &IntersectionGraph,
    seam_rotation: f64,
    accuracy: Accuracy,
) -> Result<Vec<NumericalCylinderBoundary>, GeometryError> {
    let mut numerical = Vec::with_capacity(2);
    for branch in &graph.branches {
        let CurveGeometry::Intersection { definition } =
            graph.geometry.curves[branch.curve as usize]
        else {
            return Err(GeometryError::InvalidGeometry(
                "universal face graph did not retain its intersection definition".into(),
            ));
        };
        let mut definition = graph.geometry.intersections[definition as usize].clone();
        definition.surfaces = [0, 3];
        rotate_first_intersection_chart(&mut definition, seam_rotation, accuracy)?;
        let definition_id = builder.brep.geometry.intersections.len() as u32;
        builder.brep.geometry.intersections.push(definition);
        let curve = CurveGeometry::Intersection {
            definition: definition_id,
        };
        let start = graph
            .geometry
            .curve(branch.curve)?
            .point_at(branch.range.lo)?;
        let end = graph
            .geometry
            .curve(branch.curve)?
            .point_at(branch.range.hi)?;
        if norm(sub(start, end)) > accuracy.intersection {
            return Err(GeometryError::CoverageGap {
                families: [
                    "cylinder through subtraction".into(),
                    "open lateral intersection branch".into(),
                ],
            });
        }
        let vertex = builder.vertex(start);
        let edge = builder.edge(curve, branch.range, false);
        numerical.push(NumericalCylinderBoundary {
            definition: definition_id,
            edge,
            vertex,
            cutter_start: graph
                .geometry
                .pcurve_at(branch.pcurves[1], branch.range.lo)?,
            cutter_end: graph
                .geometry
                .pcurve_at(branch.pcurves[1], branch.range.hi)?,
            cutter_middle: graph
                .geometry
                .pcurve_at(branch.pcurves[1], branch.range.midpoint())?,
        });
    }
    Ok(numerical)
}

pub(super) fn align_cutter_boundaries(
    builder: &mut Builder,
    numerical: &mut [NumericalCylinderBoundary],
) -> Result<(), GeometryError> {
    let tau = std::f64::consts::TAU;
    numerical.sort_by(|left, right| left.cutter_middle[1].total_cmp(&right.cutter_middle[1]));
    let upper_shift =
        ((numerical[0].cutter_end[0] - numerical[1].cutter_start[0]) / tau).round() * tau;
    {
        let upper = &mut numerical[1];
        upper.cutter_start[0] += upper_shift;
        upper.cutter_end[0] += upper_shift;
        upper.cutter_middle[0] += upper_shift;
        let definition = &mut builder.brep.geometry.intersections[upper.definition as usize];
        for anchor in &mut definition.anchors {
            anchor.uv_b[0] += upper_shift;
        }
        for tube in &mut definition.uv_tubes {
            tube[2] = Interval::new(tube[2].lo + upper_shift, tube[2].hi + upper_shift)?;
        }
    }
    Ok(())
}
