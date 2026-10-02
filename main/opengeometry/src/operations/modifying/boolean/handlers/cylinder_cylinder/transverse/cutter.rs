use super::boundaries::NumericalCylinderBoundary;
use crate::brep::{
    boundary, unit, uv_line, Accuracy, Builder, CurveGeometry, GeometryError, IntersectionSide,
    Orientation, PcurveGeometry, Surface, Use,
};
use crate::math::{norm, sub, Interval};
use crate::operations::modifying::boolean::assembly::reverse;
use crate::operations::modifying::boolean::operands::CylinderInput;

pub(super) struct CutterSeam {
    arc: u32,
    middle: u32,
    generator: u32,
    length: f64,
    first_arc_sense: Orientation,
    second_arc_sense: Orientation,
    arc_shift: f64,
}

struct SeamGenerator {
    middle: u32,
    length: f64,
    edge: u32,
}

pub(super) fn add_cutter_seam(
    builder: &mut Builder,
    cutter: &CylinderInput<'_>,
    lower: &NumericalCylinderBoundary,
    upper: &NumericalCylinderBoundary,
    accuracy: Accuracy,
) -> Result<CutterSeam, GeometryError> {
    let tau = std::f64::consts::TAU;
    let lower_end = lower.cutter_end;
    let upper_start = upper.cutter_start;
    let upper_right_u = upper_start[0];
    let lower_v = lower_end[1];
    let arc_range = Interval::new(
        lower_end[0].min(upper_right_u),
        lower_end[0].max(upper_right_u),
    )?;
    if arc_range.width() * cutter.radius <= 4.0 * accuracy.geometric {
        return Err(GeometryError::UnresolvedIntersection(
            "cylinder cut chart seam arc is below geometric resolution".into(),
        ));
    }
    let mut seam_circle_frame = cutter.frame;
    seam_circle_frame.origin = cutter.frame.point([0.0, 0.0, lower_v]);
    let seam_arc = builder.edge(
        CurveGeometry::Circle {
            frame: seam_circle_frame,
            radius: cutter.radius,
        },
        arc_range,
        true,
    );
    let SeamGenerator {
        middle: seam_middle,
        length: seam_length,
        edge: seam_generator,
    } = add_seam_generator(builder, cutter, upper, upper_right_u, lower_v, accuracy)?;
    let arc_forward = lower_end[0] < upper_right_u;
    let first_arc_sense = if arc_forward {
        Orientation::Forward
    } else {
        Orientation::Reverse
    };
    let second_arc_sense = reverse(first_arc_sense);
    let second_arc_start = if second_arc_sense == Orientation::Forward {
        arc_range.lo
    } else {
        arc_range.hi
    };
    let second_arc_end = if second_arc_sense == Orientation::Forward {
        arc_range.hi
    } else {
        arc_range.lo
    };
    let arc_shift = lower.cutter_start[0] - second_arc_end;
    let winding_error = upper.cutter_end[0] - (second_arc_start + arc_shift);
    let winding_residual = winding_error - (winding_error / tau).round() * tau;
    if winding_residual.abs() > accuracy.intersection / cutter.radius {
        return Err(GeometryError::UnresolvedIntersection(
            "cylinder cut chart seam winding is inconsistent".into(),
        ));
    }
    Ok(CutterSeam {
        arc: seam_arc,
        middle: seam_middle,
        generator: seam_generator,
        length: seam_length,
        first_arc_sense,
        second_arc_sense,
        arc_shift,
    })
}

fn add_seam_generator(
    builder: &mut Builder,
    cutter: &CylinderInput<'_>,
    upper: &NumericalCylinderBoundary,
    upper_right_u: f64,
    lower_v: f64,
    accuracy: Accuracy,
) -> Result<SeamGenerator, GeometryError> {
    let seam_middle_point = cutter
        .brep
        .geometry
        .surface(0)?
        .point_at([upper_right_u, lower_v])?;
    let seam_middle = builder.vertex(seam_middle_point);
    let seam_vector = sub(
        builder.brep.topology.vertices[upper.vertex as usize].position,
        seam_middle_point,
    );
    let seam_length = norm(seam_vector);
    if seam_length <= 4.0 * accuracy.geometric {
        return Err(GeometryError::UnresolvedIntersection(
            "cylinder cut chart seam generator is below geometric resolution".into(),
        ));
    }
    let seam_generator = builder.edge(
        CurveGeometry::Line {
            origin: seam_middle_point,
            direction: unit(seam_vector)?,
        },
        Interval::new(0.0, seam_length)?,
        true,
    );
    Ok(SeamGenerator {
        middle: seam_middle,
        length: seam_length,
        edge: seam_generator,
    })
}

pub(super) fn cutter_face_bounds(
    builder: &Builder,
    numerical: &[NumericalCylinderBoundary],
    lower: &NumericalCylinderBoundary,
    upper: &NumericalCylinderBoundary,
) -> [[f64; 2]; 2] {
    let mut cutter_u_bounds = [f64::INFINITY, f64::NEG_INFINITY];
    let mut cutter_v_bounds = [f64::INFINITY, f64::NEG_INFINITY];
    for boundary in numerical {
        let definition = &builder.brep.geometry.intersections[boundary.definition as usize];
        for anchor in &definition.anchors {
            cutter_u_bounds[0] = cutter_u_bounds[0].min(anchor.uv_b[0]);
            cutter_u_bounds[1] = cutter_u_bounds[1].max(anchor.uv_b[0]);
            cutter_v_bounds[0] = cutter_v_bounds[0].min(anchor.uv_b[1]);
            cutter_v_bounds[1] = cutter_v_bounds[1].max(anchor.uv_b[1]);
        }
    }
    for value in [
        upper.cutter_start[0],
        upper.cutter_end[0],
        lower.cutter_start[0],
    ] {
        cutter_u_bounds[0] = cutter_u_bounds[0].min(value);
        cutter_u_bounds[1] = cutter_u_bounds[1].max(value);
    }
    for value in [lower.cutter_end[1], upper.cutter_start[1]] {
        cutter_v_bounds[0] = cutter_v_bounds[0].min(value);
        cutter_v_bounds[1] = cutter_v_bounds[1].max(value);
    }
    [cutter_u_bounds, cutter_v_bounds]
}

pub(super) fn cutter_face_uses(
    lower: &NumericalCylinderBoundary,
    upper: &NumericalCylinderBoundary,
    seam: &CutterSeam,
) -> Vec<Use> {
    vec![
        boundary(
            lower.edge,
            lower.vertex,
            lower.vertex,
            Orientation::Forward,
            PcurveGeometry::IntersectionSide {
                definition: lower.definition,
                side: IntersectionSide::B,
            },
        ),
        boundary(
            seam.arc,
            lower.vertex,
            seam.middle,
            seam.first_arc_sense,
            uv_line([0.0, lower.cutter_end[1]], [1.0, 0.0]),
        ),
        boundary(
            seam.generator,
            seam.middle,
            upper.vertex,
            Orientation::Forward,
            uv_line(
                [upper.cutter_start[0], lower.cutter_end[1]],
                [
                    0.0,
                    (upper.cutter_start[1] - lower.cutter_end[1]) / seam.length,
                ],
            ),
        ),
        boundary(
            upper.edge,
            upper.vertex,
            upper.vertex,
            Orientation::Forward,
            PcurveGeometry::IntersectionSide {
                definition: upper.definition,
                side: IntersectionSide::B,
            },
        ),
        boundary(
            seam.generator,
            upper.vertex,
            seam.middle,
            Orientation::Reverse,
            uv_line(
                [upper.cutter_end[0], lower.cutter_end[1]],
                [
                    0.0,
                    (upper.cutter_start[1] - lower.cutter_end[1]) / seam.length,
                ],
            ),
        ),
        boundary(
            seam.arc,
            seam.middle,
            lower.vertex,
            seam.second_arc_sense,
            uv_line([seam.arc_shift, lower.cutter_end[1]], [1.0, 0.0]),
        ),
    ]
}
