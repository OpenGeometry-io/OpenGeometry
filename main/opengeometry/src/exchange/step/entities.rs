use crate::brep::{
    CurveGeometry, Frame3, GeometryError, Orientation, PcurveGeometry, SurfaceGeometry,
};
use crate::exchange::fit_curve::FittedIntersectionCurve;
use crate::exchange::part21::Part21Writer;
use crate::math::{dot, norm};

pub(super) struct PcurveOnSurface<'a> {
    pub(super) geometry: &'a PcurveGeometry,
    pub(super) surface: &'a SurfaceGeometry,
    pub(super) surface_id: usize,
}

pub(super) fn sense(value: Orientation) -> &'static str {
    if value == Orientation::Forward {
        ".T."
    } else {
        ".F."
    }
}

pub(super) fn curve(
    writer: &mut Part21Writer,
    geometry: &CurveGeometry,
    scale: f64,
) -> Result<usize, GeometryError> {
    Ok(match geometry {
        CurveGeometry::Line { origin, direction } => line(
            writer,
            &origin.map(|v| v * scale),
            &direction.map(|v| v * scale),
        )?,
        CurveGeometry::Circle { frame, radius } => {
            let p = placement(writer, *frame, scale);
            writer.add_entity(format!("CIRCLE('',#{p},{})", real(radius * scale)))
        }
        CurveGeometry::Ellipse {
            frame,
            major_radius,
            minor_radius,
        } => {
            let p = placement(writer, *frame, scale);
            writer.add_entity(format!(
                "ELLIPSE('',#{p},{},{})",
                real(major_radius * scale),
                real(minor_radius * scale)
            ))
        }
        CurveGeometry::Intersection { .. } => {
            return Err(GeometryError::UnsupportedGeometry(
                "STEP numerical branches require bounded export-curve fitting".into(),
            ))
        }
    })
}

fn line(
    writer: &mut Part21Writer,
    origin: &[f64],
    velocity: &[f64],
) -> Result<usize, GeometryError> {
    let length = velocity.iter().fold(0.0_f64, |n, v| n.hypot(*v));
    if !length.is_finite() || length == 0.0 {
        return Err(GeometryError::UnsupportedGeometry(
            "STEP pcurve has a singular line direction".into(),
        ));
    }
    let p = point(writer, origin);
    let d = direction(
        writer,
        &velocity.iter().map(|v| v / length).collect::<Vec<_>>(),
    );
    let v = writer.add_entity(format!("VECTOR('',#{d},{})", real(length)));
    Ok(writer.add_entity(format!("LINE('',#{p},#{v})")))
}

pub(super) fn point(writer: &mut Part21Writer, value: &[f64]) -> usize {
    writer.add_entity(format!(
        "CARTESIAN_POINT('',({}))",
        value.iter().map(|v| real(*v)).collect::<Vec<_>>().join(",")
    ))
}

pub(super) fn real(value: f64) -> String {
    format!("{:.17E}", if value == 0.0 { 0.0 } else { value })
}

fn direction(writer: &mut Part21Writer, value: &[f64]) -> usize {
    writer.add_entity(format!(
        "DIRECTION('',({}))",
        value.iter().map(|v| real(*v)).collect::<Vec<_>>().join(",")
    ))
}
pub(super) fn placement(writer: &mut Part21Writer, frame: Frame3, scale: f64) -> usize {
    let p = point(writer, &frame.origin.map(|v| v * scale));
    let z = direction(writer, &frame.z);
    let x = direction(writer, &frame.x);
    writer.add_entity(format!("AXIS2_PLACEMENT_3D('',#{p},#{z},#{x})"))
}

pub(super) fn pcurve(
    writer: &mut Part21Writer,
    on_surface: &PcurveOnSurface<'_>,
    context: usize,
    scale: f64,
    lift: [i32; 2],
    fitted: Option<&FittedIntersectionCurve>,
) -> Result<usize, GeometryError> {
    let PcurveOnSurface {
        geometry,
        surface,
        surface_id,
    } = *on_surface;
    let metric = match surface {
        SurfaceGeometry::Plane { .. } => [scale, scale],
        SurfaceGeometry::Cylinder { .. } => [1.0, scale],
        SurfaceGeometry::Cone { semi_angle, .. } => [1.0, scale / semi_angle.cos()],
        _ => [1.0, 1.0],
    };
    let period = match surface {
        SurfaceGeometry::Plane { .. } => [0.0, 0.0],
        SurfaceGeometry::Torus { .. } => [std::f64::consts::TAU; 2],
        _ => [std::f64::consts::TAU, 0.0],
    };
    let map_origin = |origin: [f64; 2]| {
        std::array::from_fn::<_, 2, _>(|i| (origin[i] + f64::from(lift[i]) * period[i]) * metric[i])
    };
    let geometry_id = match geometry {
        PcurveGeometry::Line2 { origin, direction } => line(
            writer,
            &map_origin(*origin),
            &std::array::from_fn::<_, 2, _>(|i| direction[i] * metric[i]),
        )?,
        PcurveGeometry::Conic2 {
            origin,
            axis_a,
            axis_b,
        } => add_pcurve_conic(writer, *origin, *axis_a, *axis_b, metric, map_origin)?,
        PcurveGeometry::IntersectionSide { side, .. } => {
            let fitted = fitted.ok_or_else(|| {
                GeometryError::InvalidGeometry(
                    "STEP intersection pcurve is missing its certified fit".into(),
                )
            })?;
            let controls = fitted
                .pcurve(*side)
                .iter()
                .map(|control| map_origin(*control).to_vec())
                .collect::<Vec<_>>();
            bspline_curve(writer, &controls, &fitted.knots, &fitted.multiplicities)?
        }
        PcurveGeometry::ProjectedCurve { .. } => {
            return Err(GeometryError::UnsupportedGeometry(
                "STEP projected pcurves require a certified exchange representation".into(),
            ))
        }
    };
    let definition = writer.add_entity(format!(
        "DEFINITIONAL_REPRESENTATION('',(#{geometry_id}),#{context})"
    ));
    Ok(writer.add_entity(format!("PCURVE('',#{surface_id},#{definition})")))
}

fn add_pcurve_conic(
    writer: &mut Part21Writer,
    origin: [f64; 2],
    axis_a: [f64; 2],
    axis_b: [f64; 2],
    metric: [f64; 2],
    map_origin: impl Fn([f64; 2]) -> [f64; 2],
) -> Result<usize, GeometryError> {
    let a = [axis_a[0] * metric[0], axis_a[1] * metric[1], 0.0];
    let b = [axis_b[0] * metric[0], axis_b[1] * metric[1], 0.0];
    let ra = norm(a);
    let rb = norm(b);
    if !ra.is_finite()
        || !rb.is_finite()
        || ra == 0.0
        || rb == 0.0
        || dot(a.map(|v| v / ra), b.map(|v| v / rb)).abs() > 64.0 * f64::EPSILON
    {
        return Err(GeometryError::UnsupportedGeometry(
            "STEP requires an orthogonal conic pcurve parameterization".into(),
        ));
    }
    let axis = if ra >= rb { a } else { b };
    let p = point(writer, &map_origin(origin));
    let d = direction(writer, &axis[..2]);
    let frame = writer.add_entity(format!("AXIS2_PLACEMENT_2D('',#{p},#{d})"));
    Ok(if ra == rb {
        writer.add_entity(format!("CIRCLE('',#{frame},{})", real(ra)))
    } else {
        writer.add_entity(format!(
            "ELLIPSE('',#{frame},{},{})",
            real(ra.max(rb)),
            real(ra.min(rb))
        ))
    })
}

pub(super) fn bspline_curve(
    writer: &mut Part21Writer,
    controls: &[Vec<f64>],
    knots: &[f64],
    multiplicities: &[usize],
) -> Result<usize, GeometryError> {
    if controls.len() < 4 || knots.len() != multiplicities.len() {
        return Err(GeometryError::InvalidGeometry(
            "invalid fitted cubic curve shape".into(),
        ));
    }
    let points = controls
        .iter()
        .map(|control| point(writer, control))
        .collect::<Vec<_>>();
    Ok(writer.add_entity(format!(
        "B_SPLINE_CURVE_WITH_KNOTS('',3,({}),.UNSPECIFIED.,.F.,.F.,({}),({}),.UNSPECIFIED.)",
        refs(&points),
        multiplicities
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(","),
        knots
            .iter()
            .map(|value| real(*value))
            .collect::<Vec<_>>()
            .join(",")
    )))
}

pub(super) fn refs(ids: &[usize]) -> String {
    ids.iter()
        .map(|id| format!("#{id}"))
        .collect::<Vec<_>>()
        .join(",")
}
