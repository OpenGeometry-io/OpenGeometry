use super::brep_builder::{Builder, Use};
use crate::brep::error::{missing, GeometryError};
use crate::brep::frame::Frame3;
use crate::brep::geometry::CurveGeometry;
use crate::brep::pcurve::PcurveGeometry;
use crate::brep::topology::{EdgeGeometry, Orientation};
use crate::math::{dot, scale, sub, Point3};

pub(crate) fn dimensions(values: &[f64]) -> Result<(), GeometryError> {
    if values.iter().any(|&x| !x.is_finite() || x <= 0.0) {
        return Err(GeometryError::InvalidGeometry(
            "primitive dimensions must be positive and finite".into(),
        ));
    }
    Ok(())
}

pub fn plane_boundary(
    builder: &Builder,
    frame: Frame3,
    edge: u32,
    from: u32,
    to: u32,
    sense: Orientation,
) -> Result<Use, GeometryError> {
    let brep = &builder.brep;
    let geometry = &brep
        .topology
        .edges
        .get(edge as usize)
        .ok_or_else(|| missing("edge", edge))?
        .geometry;
    let curve = match *geometry {
        EdgeGeometry::Curve { curve, .. } => brep
            .geometry
            .curves
            .get(curve as usize)
            .ok_or_else(|| missing("curve", curve))?,
        _ => {
            return Err(GeometryError::InvalidGeometry(
                "collapsed planar boundary".into(),
            ))
        }
    };
    let local = |p: Point3| [dot(p, frame.x), dot(p, frame.y)];
    let pcurve = match curve {
        CurveGeometry::Line { origin, direction } => {
            uv_line(local(sub(*origin, frame.origin)), local(*direction))
        }
        CurveGeometry::Circle { frame: c, radius } => uv_conic(
            local(sub(c.origin, frame.origin)),
            local(scale(c.x, *radius)),
            local(scale(c.y, *radius)),
        ),
        CurveGeometry::Ellipse {
            frame: c,
            major_radius,
            minor_radius,
        } => uv_conic(
            local(sub(c.origin, frame.origin)),
            local(scale(c.x, *major_radius)),
            local(scale(c.y, *minor_radius)),
        ),
        _ => {
            return Err(GeometryError::UnsupportedGeometry(
                "planar primitive boundary".into(),
            ))
        }
    };
    Ok(boundary(edge, from, to, sense, pcurve))
}

#[derive(Clone, Debug, PartialEq)]
pub struct UvCurve(PcurveGeometry);

impl From<PcurveGeometry> for UvCurve {
    fn from(pcurve: PcurveGeometry) -> Self {
        Self(pcurve)
    }
}

impl From<UvCurve> for PcurveGeometry {
    fn from(curve: UvCurve) -> Self {
        curve.0
    }
}

pub fn uv_line(origin: [f64; 2], direction: [f64; 2]) -> UvCurve {
    UvCurve(PcurveGeometry::Line2 { origin, direction })
}

pub fn uv_conic(origin: [f64; 2], axis_a: [f64; 2], axis_b: [f64; 2]) -> UvCurve {
    UvCurve(PcurveGeometry::Conic2 {
        origin,
        axis_a,
        axis_b,
    })
}

pub fn boundary(
    edge: u32,
    from: u32,
    to: u32,
    sense: Orientation,
    pcurve: impl Into<UvCurve>,
) -> Use {
    Use {
        edge,
        from,
        to,
        sense,
        pcurve: pcurve.into().0,
    }
}

pub(crate) fn padded_uv_bounds(
    points: impl IntoIterator<Item = impl AsRef<[f64]>>,
    pad: f64,
) -> [[f64; 2]; 2] {
    let mut bounds = [[f64::INFINITY, f64::NEG_INFINITY]; 2];
    for point in points {
        let uv = point.as_ref();
        for axis in 0..2 {
            bounds[axis][0] = bounds[axis][0].min(uv[axis]);
            bounds[axis][1] = bounds[axis][1].max(uv[axis]);
        }
    }
    for bound in &mut bounds {
        bound[0] -= pad;
        bound[1] += pad;
    }
    bounds
}
