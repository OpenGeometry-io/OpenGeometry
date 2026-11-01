use super::entities::{bspline_curve, point, real, refs};
use crate::brep::{BrepEnvelope, EdgeGeometry, GeometryError, Surface, SurfaceGeometry, UVBox};
use crate::exchange::fit_bezier::{fit_curve, fit_surface, FittedSurface};
use crate::exchange::part21::Part21Writer;
use crate::math::Interval;

const MAX_PATCHES: usize = 1 << 16;
const MAX_SPANS: usize = 20_000;
const REACH: f64 = 0.1;

fn check_budget(budget: f64) -> Result<(), GeometryError> {
    if budget <= 0.0 {
        return Err(GeometryError::LimitExceeded(
            "STEP exchange budget leaves no room for B-spline fitting".into(),
        ));
    }
    Ok(())
}

pub(super) fn add_fitted_surface(
    writer: &mut Part21Writer,
    brep: &BrepEnvelope,
    surface_id: u32,
    budget: f64,
    scale: f64,
) -> Result<(usize, f64), GeometryError> {
    check_budget(budget)?;
    let surface = brep.geometry.surface(surface_id)?;
    let domain = reached_past(surface, support_domain(brep, surface_id)?)?;
    let fitted = fit_surface(surface, domain, budget, MAX_PATCHES)?;
    Ok((bspline_surface(writer, &fitted, scale), fitted.error_bound))
}

fn support_domain(brep: &BrepEnvelope, surface_id: u32) -> Result<UVBox, GeometryError> {
    brep.topology
        .faces
        .iter()
        .filter(|face| face.surface == surface_id)
        .map(|face| face.trim.uv_bounds)
        .reduce(|a, b| [a[0].hull(b[0]), a[1].hull(b[1])])
        .ok_or_else(|| GeometryError::InvalidTopology("STEP surface has no face".into()))
}

fn reached_past(surface: &SurfaceGeometry, domain: UVBox) -> Result<UVBox, GeometryError> {
    let periods = surface
        .charts()
        .first()
        .ok_or_else(|| GeometryError::InvalidGeometry("surface has no parameter chart".into()))?
        .periods;
    let side = |i: usize| {
        let d = domain[i];
        let room = periods[i].map_or(f64::INFINITY, |p| (p - d.width()) / 2.0);
        let pad = (REACH * d.width()).min(0.9 * room.max(0.0));
        Interval::new(d.lo - pad, d.hi + pad)
    };
    Ok([side(0)?, side(1)?])
}

fn bspline_surface(writer: &mut Part21Writer, fitted: &FittedSurface, scale: f64) -> usize {
    let rows = fitted
        .controls
        .iter()
        .map(|row| {
            let ids: Vec<_> = row
                .iter()
                .map(|p| point(writer, &p.map(|v| v * scale)))
                .collect();
            format!("({})", refs(&ids))
        })
        .collect::<Vec<_>>()
        .join(",");
    let list = |values: &[usize]| {
        values
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(",")
    };
    let reals = |values: &[f64]| {
        values
            .iter()
            .map(|v| real(*v))
            .collect::<Vec<_>>()
            .join(",")
    };
    writer.add_entity(format!(
        "B_SPLINE_SURFACE_WITH_KNOTS('',3,3,({rows}),.UNSPECIFIED.,.F.,.F.,.F.,({}),({}),({}),({}),.UNSPECIFIED.)",
        list(&fitted.multiplicities[0]),
        list(&fitted.multiplicities[1]),
        reals(&fitted.knots[0]),
        reals(&fitted.knots[1]),
    ))
}

pub(super) fn add_wrapped_curve(
    writer: &mut Part21Writer,
    brep: &BrepEnvelope,
    curve_id: u32,
    budget: f64,
    scale: f64,
) -> Result<(usize, f64), GeometryError> {
    check_budget(budget)?;
    let range = brep
        .topology
        .edges
        .iter()
        .filter_map(|edge| match edge.geometry {
            EdgeGeometry::Curve { curve, range } if curve == curve_id => Some(range),
            _ => None,
        })
        .reduce(Interval::hull)
        .ok_or_else(|| GeometryError::InvalidTopology("STEP curve has no edge".into()))?;
    let fitted = fit_curve(
        &brep.geometry.curves[curve_id as usize],
        range,
        budget,
        MAX_SPANS,
    )?;
    let controls = fitted
        .controls
        .iter()
        .map(|control| control.map(|value| value * scale).to_vec())
        .collect::<Vec<_>>();
    let id = bspline_curve(writer, &controls, &fitted.knots, &fitted.multiplicities)?;
    Ok((id, fitted.error_bound))
}
