use crate::brep::{
    period_lifted_uv, BrepEnvelope, Face, GeometryError, PcurveGeometry, Surface, SurfaceGeometry,
};
use crate::math::Interval;

pub(super) struct Rect {
    pub(super) halfedges: [u32; 4],
}
pub(super) fn rectangle(brep: &BrepEnvelope, face: &Face) -> Result<Rect, GeometryError> {
    let [u, v] = face.trim.uv_bounds;
    let mut sides = [None; 4];
    let start = brep.topology.loops[face.trim.outer as usize].start_halfedge;
    let mut current = start;
    let mut count = 0;
    for _ in 0..128 {
        let h = &brep.topology.halfedges[current as usize];
        let edge = &brep.topology.edges[h.edge as usize];
        let range = edge.geometry.range();
        let pcurve = h
            .geometry_use
            .pcurve
            .ok_or_else(|| GeometryError::InvalidTopology("missing face pcurve".into()))?;
        if !matches!(
            brep.geometry.pcurves[pcurve as usize],
            PcurveGeometry::Line2 { .. }
        ) {
            return Err(GeometryError::UnsupportedGeometry(
                "nonrectangular curved face trimming".into(),
            ));
        }
        let periods =
            brep.geometry.surface(face.surface)?.charts()[face.trim.chart as usize].periods;
        let a = period_lifted_uv(
            brep.geometry.pcurve_at(pcurve, range.lo)?,
            periods,
            h.geometry_use.periodic_lift,
        );
        let b = period_lifted_uv(
            brep.geometry.pcurve_at(pcurve, range.hi)?,
            periods,
            h.geometry_use.periodic_lift,
        );
        let close = |x: f64, y: f64| (x - y).abs() <= 1e-12;
        let covers_u =
            (close(a[0], u.lo) && close(b[0], u.hi)) || (close(a[0], u.hi) && close(b[0], u.lo));
        let covers_v =
            (close(a[1], v.lo) && close(b[1], v.hi)) || (close(a[1], v.hi) && close(b[1], v.lo));
        let side = if covers_u && close(a[1], v.lo) && close(b[1], v.lo) {
            0
        } else if covers_v && close(a[0], u.hi) && close(b[0], u.hi) {
            1
        } else if covers_u && close(a[1], v.hi) && close(b[1], v.hi) {
            2
        } else if covers_v && close(a[0], u.lo) && close(b[0], u.lo) {
            3
        } else {
            return Err(GeometryError::UnsupportedGeometry(
                "curved trim is not a chart rectangle".into(),
            ));
        };
        if sides[side].replace(current).is_some() {
            return Err(GeometryError::UnsupportedGeometry(
                "repeated rectangle boundary".into(),
            ));
        }
        count += 1;
        current = h
            .next
            .ok_or_else(|| GeometryError::InvalidTopology("open face loop".into()))?;
        if current == start {
            break;
        }
    }
    if count != 4 || sides.iter().any(Option::is_none) {
        return Err(GeometryError::UnsupportedGeometry(
            "curved trim is not a four-sided rectangle".into(),
        ));
    }
    Ok(Rect {
        halfedges: [
            sides[0].ok_or_else(|| GeometryError::InvalidTopology("bottom edge missing".into()))?,
            sides[1].ok_or_else(|| GeometryError::InvalidTopology("right edge missing".into()))?,
            sides[2].ok_or_else(|| GeometryError::InvalidTopology("top edge missing".into()))?,
            sides[3].ok_or_else(|| GeometryError::InvalidTopology("left edge missing".into()))?,
        ],
    })
}

pub(super) fn grid_size(
    surface: &SurfaceGeometry,
    bounds: [Interval; 2],
    error: f64,
    max_triangles: usize,
) -> Result<[usize; 2], GeometryError> {
    let u = bounds[0].width();
    let v = bounds[1].width();
    let (nu, nv) = match surface {
        SurfaceGeometry::Cylinder { radius, .. } => (
            count(u * (radius / (8.0 * error)).sqrt(), 3, max_triangles)?,
            1,
        ),
        SurfaceGeometry::Cone { semi_angle, .. } => (
            count(
                u * (bounds[1].hi * semi_angle.tan() / (8.0 * error)).sqrt(),
                3,
                max_triangles,
            )?,
            1,
        ),
        SurfaceGeometry::Sphere { radius, .. } => (
            count(u * (2.0 * radius / error).sqrt(), 3, max_triangles)?,
            count(v * (2.0 * radius / error).sqrt(), 2, max_triangles)?,
        ),
        SurfaceGeometry::Torus {
            major_radius: r,
            minor_radius: a,
            ..
        } => (
            count(u * ((r + 2.0 * a) / error).sqrt(), 3, max_triangles)?,
            count(v * (2.0 * a / error).sqrt(), 3, max_triangles)?,
        ),
        SurfaceGeometry::Plane { .. } => {
            return Err(GeometryError::InvalidGeometry(
                "plane does not use a curved grid".into(),
            ))
        }
    };
    if nu
        .checked_mul(nv)
        .and_then(|n| n.checked_mul(2))
        .is_none_or(|n| n > max_triangles)
    {
        return Err(GeometryError::LimitExceeded(
            "tessellation grid triangles".into(),
        ));
    }
    Ok([nu, nv])
}

pub(super) fn count(
    value: f64,
    minimum: usize,
    max_triangles: usize,
) -> Result<usize, GeometryError> {
    let value = value.ceil().max(minimum as f64);
    if !value.is_finite() || value > max_triangles as f64 {
        return Err(GeometryError::LimitExceeded(
            "tessellation subdivisions".into(),
        ));
    }
    Ok(value as usize)
}
