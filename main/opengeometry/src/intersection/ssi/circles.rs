use crate::brep::{CurveGeometry, Frame3, GeometryError};
use crate::intersection::ssi_result::GeometryResult;
use crate::math::{Interval, Point3};

pub(super) fn circle(
    center: Point3,
    normal: Point3,
    radius: f64,
) -> Result<GeometryResult, GeometryError> {
    if !radius.is_finite() || radius <= 0.0 {
        return Err(GeometryError::UnresolvedIntersection(
            "positive intersection radius is not representable".into(),
        ));
    }
    let reference = if normal[0].abs() < 0.8 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let frame = Frame3::from_axis(center, normal, reference)?;
    Ok(GeometryResult::Curves(vec![(
        CurveGeometry::Circle { frame, radius },
        Some(Interval::new(0.0, std::f64::consts::TAU)?),
    )]))
}

pub(super) fn axial_circles(
    normal: Point3,
    sections: impl IntoIterator<Item = (Point3, f64)>,
) -> Result<GeometryResult, GeometryError> {
    let mut curves = Vec::new();
    for (center, radius) in sections {
        let GeometryResult::Curves(mut section) = circle(center, normal, radius)? else {
            return Err(GeometryError::UnresolvedIntersection(
                "axial circle construction returned a non-curve".into(),
            ));
        };
        curves.append(&mut section);
    }
    Ok(if curves.is_empty() {
        GeometryResult::Empty
    } else {
        GeometryResult::Curves(curves)
    })
}
