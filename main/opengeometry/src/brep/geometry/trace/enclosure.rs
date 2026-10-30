use crate::brep::bounds::PatchBounds;
use crate::brep::error::GeometryError;
use crate::brep::frame::{unit, Frame3};
use crate::brep::geometry::intersection_definition::IntersectionDefinition;
use crate::brep::geometry::store::GeometryStore;
use crate::brep::geometry::surface::SurfaceGeometry;
use crate::math::{add, cross, dot, norm, scale, sub, Interval, Point3};

struct CrossedCylinders {
    host_radius: f64,
    cutter: Frame3,
    cutter_radius: f64,
    u_axis: Point3,
    v_axis: Point3,
    w_axis: Point3,
    centre: Point3,
}

struct CutterProjection {
    v_cos: f64,
    v_sin: f64,
    w_cos: f64,
    w_sin: f64,
}

pub(super) fn intersect_interval(a: Interval, b: Interval) -> Option<Interval> {
    Interval::new(a.lo.max(b.lo), a.hi.min(b.hi)).ok()
}

pub(super) fn perpendicular_cylinder_enclosure(
    definition: &IntersectionDefinition,
    store: &GeometryStore,
    segment: usize,
    range: Interval,
    parent: PatchBounds,
) -> Result<Option<(PatchBounds, f64)>, GeometryError> {
    let Some(cylinders) = crossed_cylinders(definition, store)? else {
        return Ok(None);
    };
    let CrossedCylinders {
        host_radius,
        cutter_radius,
        u_axis,
        v_axis,
        w_axis,
        centre,
        ..
    } = cylinders;

    let first = &definition.anchors[segment];
    let last = &definition.anchors[segment + 1];
    let guide_axis = unit(sub(last.point, first.point))?;
    let certificate = definition.residual_tolerance * 8.0
        + 128.0 * f64::EPSILON * host_radius.max(cutter_radius).max(norm(centre)).max(1.0);
    let theta_parent = definition.uv_tubes[segment][2];
    if theta_parent.width() >= std::f64::consts::PI {
        return Ok(None);
    }
    let u_parent = interval_dot(parent, centre, u_axis)?;
    if u_parent.contains(0.0) {
        return Ok(None);
    }
    let host_squared = Interval::new(
        (host_radius - certificate).max(0.0).powi(2),
        (host_radius + certificate).powi(2),
    )?;
    let Some(CutterProjection {
        v_cos,
        v_sin,
        w_cos,
        w_sin,
    }) = certified_projection(&cylinders, guide_axis, theta_parent, host_squared, u_parent)?
    else {
        return Ok(None);
    };

    let lo = definition.evaluate(range.lo, store)?.uv_b[0];
    let hi = definition.evaluate(range.hi, store)?.uv_b[0];
    let theta_pad = certificate / cutter_radius + 64.0 * f64::EPSILON;
    let theta = Interval::new(lo.min(hi) - theta_pad, lo.max(hi) + theta_pad)?;
    if theta.lo < theta_parent.lo - theta_pad || theta.hi > theta_parent.hi + theta_pad {
        return Ok(None);
    }
    let cosine = theta.cos()?;
    let sine = theta.sin()?;
    let v = trig(cosine, sine, v_cos, v_sin, cutter_radius)?;
    let w = trig(cosine, sine, w_cos, w_sin, cutter_radius)?;
    let Some(u) = signed_root(host_squared.sub_interval(w.square()?)?, u_parent)? else {
        return Ok(None);
    };
    let local = [u, v, w];
    let axes = [u_axis, v_axis, w_axis];

    let world = world_bounds(centre, local, axes, certificate)?;
    let Some(chord_error) = chord_error_bound(host_radius, cutter_radius, certificate, theta)
    else {
        return Ok(None);
    };
    Ok(Some((PatchBounds { axes: world }, chord_error)))
}

fn crossed_cylinders(
    definition: &IntersectionDefinition,
    store: &GeometryStore,
) -> Result<Option<CrossedCylinders>, GeometryError> {
    let (
        SurfaceGeometry::Cylinder {
            frame: host,
            radius: host_radius,
        },
        SurfaceGeometry::Cylinder {
            frame: cutter,
            radius: cutter_radius,
        },
    ) = (
        store.surface(definition.surfaces[0])?,
        store.surface(definition.surfaces[1])?,
    )
    else {
        return Ok(None);
    };
    let u_axis = cutter.z;
    let v_axis = host.z;
    if dot(u_axis, v_axis).abs() > 1e-12 {
        return Ok(None);
    }
    let w_axis = cross(u_axis, v_axis);
    let host_to_cutter = sub(cutter.origin, host.origin);
    let centre = add(host.origin, scale(v_axis, dot(host_to_cutter, v_axis)));
    let residual = sub(
        sub(centre, cutter.origin),
        scale(u_axis, dot(sub(centre, cutter.origin), u_axis)),
    );
    if norm(residual) > 1e-12 {
        return Ok(None);
    }
    Ok(Some(CrossedCylinders {
        host_radius: *host_radius,
        cutter: *cutter,
        cutter_radius: *cutter_radius,
        u_axis,
        v_axis,
        w_axis,
        centre,
    }))
}

fn interval_dot(
    bounds: PatchBounds,
    origin: Point3,
    axis: Point3,
) -> Result<Interval, GeometryError> {
    let mut value = Interval::point(0.0)?;
    for coordinate in 0..3 {
        value = value.add_interval(
            bounds.axes[coordinate]
                .sub_interval(Interval::point(origin[coordinate])?)?
                .mul_interval(Interval::point(axis[coordinate])?)?,
        )?;
    }
    Ok(value)
}

fn certified_projection(
    cylinders: &CrossedCylinders,
    guide_axis: Point3,
    theta_parent: Interval,
    host_squared: Interval,
    u_parent: Interval,
) -> Result<Option<CutterProjection>, GeometryError> {
    let CrossedCylinders {
        cutter,
        cutter_radius,
        u_axis,
        v_axis,
        w_axis,
        ..
    } = *cylinders;
    let v_cos = dot(cutter.x, v_axis);
    let v_sin = dot(cutter.y, v_axis);
    let w_cos = dot(cutter.x, w_axis);
    let w_sin = dot(cutter.y, w_axis);
    let parent_cosine = theta_parent.cos()?;
    let parent_sine = theta_parent.sin()?;
    let parent_w = trig(parent_cosine, parent_sine, w_cos, w_sin, cutter_radius)?;
    let parent_dw = trig(parent_sine, parent_cosine, -w_cos, w_sin, cutter_radius)?;
    let parent_dv = trig(parent_sine, parent_cosine, -v_cos, v_sin, cutter_radius)?;
    let Some(parent_u) = signed_root(host_squared.sub_interval(parent_w.square()?)?, u_parent)?
    else {
        return Ok(None);
    };
    if parent_u.contains(0.0) {
        return Ok(None);
    }
    let parent_du = Interval::point(-1.0)?
        .mul_interval(parent_w)?
        .mul_interval(parent_dw)?
        .div_interval(parent_u)?;
    let derivative = parent_du
        .mul_interval(Interval::point(dot(guide_axis, u_axis))?)?
        .add_interval(parent_dv.mul_interval(Interval::point(dot(guide_axis, v_axis))?)?)?
        .add_interval(parent_dw.mul_interval(Interval::point(dot(guide_axis, w_axis))?)?)?;
    if derivative.contains(0.0) {
        return Ok(None);
    }
    Ok(Some(CutterProjection {
        v_cos,
        v_sin,
        w_cos,
        w_sin,
    }))
}

fn trig(
    cosine: Interval,
    sine: Interval,
    along_cosine: f64,
    along_sine: f64,
    cutter_radius: f64,
) -> Result<Interval, GeometryError> {
    Ok(cosine
        .mul_interval(Interval::point(along_cosine)?)?
        .add_interval(sine.mul_interval(Interval::point(along_sine)?)?)?
        .mul_interval(Interval::point(cutter_radius)?)?)
}

fn signed_root(radial: Interval, sign: Interval) -> Result<Option<Interval>, GeometryError> {
    if radial.hi < 0.0 {
        return Ok(None);
    }
    let root = Interval::new(radial.lo.max(0.0), radial.hi)?.sqrt()?;
    Ok(Some(if sign.lo >= 0.0 {
        root
    } else if sign.hi <= 0.0 {
        Interval::new(-root.hi, -root.lo)?
    } else {
        Interval::new(-root.hi, root.hi)?
    }))
}

fn world_bounds(
    centre: Point3,
    local: [Interval; 3],
    axes: [Point3; 3],
    certificate: f64,
) -> Result<[Interval; 3], GeometryError> {
    let mut world = [Interval::point(0.0)?; 3];
    for coordinate in 0..3 {
        let mut value = Interval::point(centre[coordinate])?;
        for axis in 0..3 {
            value = value.add_interval(
                local[axis].mul_interval(Interval::point(axes[axis][coordinate])?)?,
            )?;
        }
        world[coordinate] = value.add_interval(Interval::new(-certificate, certificate)?)?;
    }
    Ok(world)
}

fn chord_error_bound(
    host_radius: f64,
    cutter_radius: f64,
    certificate: f64,
    theta: Interval,
) -> Option<f64> {
    let minimum_u = ((host_radius - certificate).powi(2) - (cutter_radius + certificate).powi(2))
        .max(0.0)
        .sqrt();
    if minimum_u <= certificate {
        return None;
    }
    let radius = cutter_radius + certificate;
    let axial_curvature = radius.powi(2) / minimum_u + radius.powi(4) / minimum_u.powi(3);
    let host_angle_curvature = radius / minimum_u + radius.powi(3) / minimum_u.powi(3);

    let host_support_curvature =
        host_radius * (host_angle_curvature + (radius / minimum_u).powi(2));
    let curvature = (radius + axial_curvature).max(host_support_curvature);
    Some(curvature * theta.width().powi(2) / 8.0 + certificate * 4.0)
}
