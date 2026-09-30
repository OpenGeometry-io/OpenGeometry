use super::{
    geometry::{norm, scale as vector_scale},
    topology::{EdgeGeometry, GeometryQuality, PcurveGeometry},
    BrepEnvelope, CurveGeometry, Frame3, GeometryError, SurfaceGeometry,
};
use crate::math::interval::Interval;

fn metric(surface: &SurfaceGeometry, scale: f64) -> [f64; 2] {
    match surface {
        SurfaceGeometry::Plane { .. } => [scale; 2],
        SurfaceGeometry::Cylinder { .. } | SurfaceGeometry::Cone { .. } => [1.0, scale],
        _ => [1.0; 2],
    }
}
fn pcurve(
    geometry: &mut PcurveGeometry,
    metric: [f64; 2],
    parameter_scale: f64,
) -> Result<(), GeometryError> {
    let map = |value: &mut [f64; 2]| {
        for i in 0..2 {
            value[i] *= metric[i];
        }
    };
    match geometry {
        PcurveGeometry::Line2 { origin, direction } => {
            map(origin);
            map(direction);
            for value in direction {
                *value /= parameter_scale;
            }
        }
        PcurveGeometry::Conic2 {
            origin,
            axis_a,
            axis_b,
        } => {
            if parameter_scale != 1.0 {
                return Err(GeometryError::UnsupportedGeometry(
                    "scaled line edge uses a conic pcurve".into(),
                ));
            }
            map(origin);
            map(axis_a);
            map(axis_b);
        }
        PcurveGeometry::ProjectedCurve {
            uv_hint,
            uv_rate,
            parameter_origin,
            ..
        } => {
            map(uv_hint);
            map(uv_rate);
            for value in uv_rate {
                *value /= parameter_scale;
            }
            *parameter_origin *= parameter_scale;
        }
        PcurveGeometry::IntersectionSide { .. } => {}
    }
    Ok(())
}

pub fn placed(
    input: &BrepEnvelope,
    placement: Frame3,
    scale: f64,
) -> Result<BrepEnvelope, GeometryError> {
    input.validate()?;
    placement.validate()?;
    if !scale.is_finite() || scale <= 0.0 {
        return Err(GeometryError::InvalidGeometry(
            "placement scale must be positive and finite".into(),
        ));
    }
    let mut output = input.clone();
    output.revision = input
        .revision
        .checked_add(1)
        .ok_or_else(|| GeometryError::LimitExceeded("BRep revision overflow".into()))?;
    let transform_frame = |frame: &mut Frame3| {
        frame.origin = placement.point(vector_scale(frame.origin, scale));
        frame.x = placement.vector(frame.x);
        frame.y = placement.vector(frame.y);
        frame.z = placement.vector(frame.z);
    };
    let metrics: Vec<_> = input
        .geometry
        .surfaces
        .iter()
        .map(|surface| metric(surface, scale))
        .collect();
    for surface in &mut output.geometry.surfaces {
        match surface {
            SurfaceGeometry::Plane { frame } | SurfaceGeometry::Cone { frame, .. } => {
                transform_frame(frame)
            }
            SurfaceGeometry::Sphere { frame, radius }
            | SurfaceGeometry::Cylinder { frame, radius } => {
                transform_frame(frame);
                *radius *= scale;
            }
            SurfaceGeometry::Torus {
                frame,
                major_radius,
                minor_radius,
            } => {
                transform_frame(frame);
                *major_radius *= scale;
                *minor_radius *= scale;
            }
        }
    }
    for curve in &mut output.geometry.curves {
        match curve {
            CurveGeometry::Line { origin, direction } => {
                *origin = placement.point(vector_scale(*origin, scale));
                *direction = placement.vector(*direction);
            }
            CurveGeometry::Circle { frame, radius } => {
                transform_frame(frame);
                *radius *= scale;
            }
            CurveGeometry::Ellipse {
                frame,
                major_radius,
                minor_radius,
            } => {
                transform_frame(frame);
                *major_radius *= scale;
                *minor_radius *= scale;
            }
            CurveGeometry::Intersection { .. } => {}
        }
    }
    for vertex in &mut output.topology.vertices {
        vertex.position = placement.point(vector_scale(vertex.position, scale));
        vertex.tolerance *= scale;
    }
    for edge in &mut output.topology.edges {
        edge.tolerance *= scale;
        if let EdgeGeometry::Curve { curve, range } = &mut edge.geometry {
            if matches!(
                input.geometry.curves[*curve as usize],
                CurveGeometry::Line { .. }
            ) {
                *range = Interval::new(range.lo * scale, range.hi * scale)?;
            }
        }
    }
    let mut pcurve_metrics = vec![None; input.geometry.pcurves.len()];
    for use_ in &mut output.topology.halfedges {
        let (Some(face), Some(pc)) = (use_.face, use_.geometry_use.pcurve) else {
            continue;
        };
        let metric = metrics[input.topology.faces[face as usize].surface as usize];
        let parameter_scale = match input.topology.edges[use_.edge as usize].geometry {
            EdgeGeometry::Curve { curve, .. }
                if matches!(
                    input.geometry.curves[curve as usize],
                    CurveGeometry::Line { .. }
                ) =>
            {
                scale
            }
            _ => 1.0,
        };
        match pcurve_metrics[pc as usize] {
            None => {
                pcurve_metrics[pc as usize] = Some((metric, parameter_scale));
            }
            Some(previous) if previous != (metric, parameter_scale) => {
                let mut copy = input.geometry.pcurves[pc as usize].clone();
                pcurve(&mut copy, metric, parameter_scale)?;
                use_.geometry_use.pcurve = Some(output.geometry.pcurves.len() as u32);
                output.geometry.pcurves.push(copy);
            }
            _ => {}
        }
    }
    for (id, original) in input.geometry.pcurves.iter().enumerate() {
        let (mut metric, parameter_scale) = pcurve_metrics[id].unwrap_or(([1.0; 2], 1.0));
        if let PcurveGeometry::ProjectedCurve { surface, .. } = original {
            metric = metrics[*surface as usize];
        }
        pcurve(&mut output.geometry.pcurves[id], metric, parameter_scale)?;
    }
    for face in &mut output.topology.faces {
        for (i, metric) in metrics[face.surface as usize].iter().enumerate() {
            face.trim.uv_bounds[i] = Interval::new(
                face.trim.uv_bounds[i].lo * metric,
                face.trim.uv_bounds[i].hi * metric,
            )?;
        }
    }
    for definition in &mut output.geometry.intersections {
        let a = metrics[definition.surfaces[0] as usize];
        let b = metrics[definition.surfaces[1] as usize];
        for anchor in &mut definition.anchors {
            anchor.point = placement.point(vector_scale(anchor.point, scale));
            for i in 0..2 {
                anchor.uv_a[i] *= a[i];
                anchor.uv_b[i] *= b[i];
            }
        }
        for tube in &mut definition.uv_tubes {
            for (i, metric) in a.into_iter().chain(b).enumerate() {
                tube[i] = Interval::new(tube[i].lo * metric, tube[i].hi * metric)?;
            }
        }
        definition.residual_tolerance *= scale;
    }
    output.accuracy.geometric *= scale;
    output.accuracy.intersection *= scale;
    output.accuracy.tessellation *= scale;
    output.accuracy.exchange *= scale;
    if let GeometryQuality::Approximate { max_error } = &mut output.quality {
        *max_error *= scale;
    }
    output.validate()?;
    if let Some(bounds) = output.bounds()? {
        let magnitude = norm(std::array::from_fn(|i| {
            bounds.axes[i].lo.abs().max(bounds.axes[i].hi.abs())
        }));
        if !magnitude.is_finite() || magnitude * 64.0 * f64::EPSILON > output.accuracy.geometric {
            return Err(GeometryError::LimitExceeded(
                "placement coordinate precision exceeds geometric tolerance".into(),
            ));
        }
    }
    Ok(output)
}
