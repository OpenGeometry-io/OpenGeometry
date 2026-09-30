use crate::brep::{GeometryError, PatchBounds, SurfaceGeometry};
use crate::math::{Interval, Point3};

fn local_intervals(
    surface: &SurfaceGeometry,
    bounds: PatchBounds,
) -> Result<[Interval; 3], GeometryError> {
    let frame = surface.frame();
    let mut local = [Interval::point(0.0)?; 3];
    for (axis, direction) in [frame.x, frame.y, frame.z].into_iter().enumerate() {
        let mut coordinate = Interval::point(0.0)?;
        for world_axis in 0..3 {
            coordinate = coordinate.add_interval(
                bounds.axes[world_axis]
                    .sub_interval(Interval::point(frame.origin[world_axis])?)?
                    .mul_interval(Interval::point(direction[world_axis])?)?,
            )?;
        }
        local[axis] = coordinate;
    }
    Ok(local)
}

pub(super) fn implicit_interval(
    surface: &SurfaceGeometry,
    bounds: PatchBounds,
) -> Result<Interval, GeometryError> {
    let [x, y, z] = local_intervals(surface, bounds)?;
    let x2 = x.square()?;
    let y2 = y.square()?;
    let z2 = z.square()?;
    Ok(match surface {
        SurfaceGeometry::Plane { .. } => z,
        SurfaceGeometry::Sphere { radius, .. } => x2
            .add_interval(y2)?
            .add_interval(z2)?
            .sub_interval(Interval::point(radius * radius)?)?,
        SurfaceGeometry::Cylinder { radius, .. } => x2
            .add_interval(y2)?
            .sub_interval(Interval::point(radius * radius)?)?,
        SurfaceGeometry::Cone { semi_angle, .. } => {
            if z.hi < 0.0 {
                return Interval::point(1.0).map_err(Into::into);
            }
            let slope = semi_angle.tan();
            x2.add_interval(y2)?
                .sub_interval(z2.mul_interval(Interval::point(slope * slope)?)?)?
        }
        SurfaceGeometry::Torus {
            major_radius,
            minor_radius,
            ..
        } => {
            let radial = x2.add_interval(y2)?;
            Interval::new(radial.lo.max(0.0), radial.hi)?
                .sqrt()?
                .sub_interval(Interval::point(*major_radius)?)?
                .square()?
                .add_interval(z2)?
                .sub_interval(Interval::point(minor_radius * minor_radius)?)?
        }
    })
}

pub(super) fn implicit_value(
    surface: &SurfaceGeometry,
    point: Point3,
) -> Result<f64, GeometryError> {
    let [x, y, z] = surface.frame().local(point);
    let value = match surface {
        SurfaceGeometry::Plane { .. } => z,
        SurfaceGeometry::Sphere { radius, .. } => {
            x.mul_add(x, y.mul_add(y, z * z)) - radius * radius
        }
        SurfaceGeometry::Cylinder { radius, .. } => x.mul_add(x, y * y) - radius * radius,
        SurfaceGeometry::Cone { semi_angle, .. } => {
            if z < 0.0 {
                return Ok(x.mul_add(x, y.mul_add(y, z * z)) + 1.0);
            }
            x.mul_add(x, y * y) - z * z * semi_angle.tan().powi(2)
        }
        SurfaceGeometry::Torus {
            major_radius,
            minor_radius,
            ..
        } => {
            let radial = x.mul_add(x, y * y);
            (radial.sqrt() - major_radius).powi(2) + z * z - minor_radius * minor_radius
        }
    };
    if value.is_finite() {
        Ok(value)
    } else {
        Err(GeometryError::UnresolvedIntersection(
            "universal SSI implicit evaluation exceeded numerical range".into(),
        ))
    }
}

pub(super) fn surface_scale(surface: &SurfaceGeometry) -> f64 {
    match surface {
        SurfaceGeometry::Plane { .. } => 1.0,
        SurfaceGeometry::Sphere { radius, .. } | SurfaceGeometry::Cylinder { radius, .. } => {
            *radius
        }
        SurfaceGeometry::Cone { .. } => 1.0,
        SurfaceGeometry::Torus {
            major_radius,
            minor_radius,
            ..
        } => major_radius + minor_radius,
    }
}
