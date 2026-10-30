use super::chart::{conic, gap, line_pcurve, segment, Chart, HalfSpaceUses};
use crate::brep::Orientation::{Forward, Reverse};
use crate::brep::{
    boundary, unit, uv_line, Builder, CurveGeometry, Frame3, GeometryError, PcurveGeometry, Use,
};
use crate::math::{add, cross, scale, Interval};
use std::f64::consts::TAU;

struct EllipseCut {
    k: f64,
    m: f64,
    major: f64,
    minor: f64,
    curve: CurveGeometry,
    cut_frame: Frame3,
    cuts_top: bool,
    cuts_bottom: bool,
    t_top: f64,
    t_bottom: f64,
}

#[derive(Default)]
struct Loops {
    side: Vec<Use>,
    cut: Vec<Use>,
    bottom: Option<Vec<Use>>,
}

fn unresolved<T>(reason: &str) -> Result<T, GeometryError> {
    Err(GeometryError::UnresolvedIntersection(reason.into()))
}

pub(super) fn ellipse_uses(
    builder: &mut Builder,
    chart: &Chart,
    sliver: f64,
) -> Result<HalfSpaceUses, GeometryError> {
    let pad = builder.brep.accuracy.geometric;
    let cut = ellipse_cut(chart, sliver, pad)?;
    let h = chart.height;
    let (start, end, seam) = if cut.cuts_top {
        let first = builder.vertex(chart.point(cut.t_top, h));
        (first, builder.vertex(chart.point(TAU - cut.t_top, h)), None)
    } else {
        let apex = builder.vertex(chart.point(0.0, cut.m + cut.k));
        (apex, apex, Some(builder.vertex(chart.point(0.0, h))))
    };
    let mut loops = Loops::default();
    if cut.cuts_bottom {
        add_bottom_crossing(builder, chart, &cut, [start, end], &mut loops)?;
    } else {
        let (edge, pcurve) = ellipse_edge(builder, &cut, cut.t_top, TAU - cut.t_top)?;
        loops.side.push(boundary(edge, start, end, Forward, pcurve));
        loops
            .cut
            .push(boundary(edge, end, start, Reverse, ellipse_on_cut(&cut)));
    }
    let top = match seam {
        Some(rim) => add_full_top(builder, chart, &cut, [start, rim], &mut loops)?,
        None => add_top_chord(builder, chart, &cut, [start, end], &mut loops)?,
    };
    let r = chart.radius;
    Ok(HalfSpaceUses {
        side: loops.side,
        side_bounds: [[cut.t_top, TAU - cut.t_top], [(cut.m - cut.k).max(0.0), h]],
        bottom: loops.bottom,
        top,
        top_partial: seam.is_none(),
        cut: loops.cut,
        cut_frame: cut.cut_frame,
        cut_bounds: [[-cut.major - pad, cut.major + pad], [-r - pad, r + pad]],
    })
}

fn ellipse_cut(chart: &Chart, sliver: f64, geometric: f64) -> Result<EllipseCut, GeometryError> {
    let (radius, height) = (chart.radius, chart.height);
    let k = chart.a * radius / chart.c;
    let m = chart.e / chart.c;
    if !(k.is_finite() && m.is_finite()) || (m.abs() + k) * 16.0 * f64::EPSILON > 0.25 * geometric {
        return unresolved("cut plane is nearly parallel to the cylinder axis");
    }
    let (low, high) = (m - k, m + k);
    let rim_gap = |dw: f64| dw.abs() * (radius / k).min(1.0);
    if high <= 0.0 || low >= height || rim_gap(high) <= sliver || rim_gap(low - height) <= sliver {
        return Err(gap());
    }
    if rim_gap(high - height) <= sliver || rim_gap(low) <= sliver {
        return unresolved("cut ellipse meets a cylinder rim within the sliver limit");
    }
    let (cuts_top, cuts_bottom) = (high > height, low < 0.0);
    let t_top = ((height - m) / k).clamp(-1.0, 1.0).acos();
    let t_bottom = (-m / k).clamp(-1.0, 1.0).acos();
    if cuts_top && cuts_bottom && t_bottom - t_top <= 1.0e-9 {
        return unresolved("cut ellipse arc is below angular resolution");
    }
    let frame = chart.frame;
    let major = radius.hypot(k);
    let g = unit(add(scale(frame.x, radius), scale(frame.z, k)))?;
    let center = frame.point([0.0, 0.0, m]);
    let curve = CurveGeometry::Ellipse {
        frame: Frame3 {
            origin: center,
            x: g,
            y: frame.y,
            z: unit(cross(g, frame.y))?,
        },
        major_radius: major,
        minor_radius: radius,
    };
    let cut_frame = Frame3 {
        origin: center,
        x: g,
        y: scale(frame.y, -1.0),
        z: unit(cross(frame.y, g))?,
    };
    cut_frame.validate()?;
    Ok(EllipseCut {
        k,
        m,
        major,
        minor: radius,
        curve,
        cut_frame,
        cuts_top,
        cuts_bottom,
        t_top,
        t_bottom,
    })
}

fn ellipse_on_cut(cut: &EllipseCut) -> PcurveGeometry {
    conic(cut.major, -cut.minor)
}

fn ellipse_edge(
    builder: &mut Builder,
    cut: &EllipseCut,
    lo: f64,
    hi: f64,
) -> Result<(u32, PcurveGeometry), GeometryError> {
    let curve = builder.brep.geometry.curves.len() as u32;
    let edge = builder.edge(cut.curve.clone(), Interval::new(lo, hi)?, false);
    let pcurve = PcurveGeometry::ProjectedCurve {
        curve,
        surface: 0,
        chart: 0,
        uv_hint: [lo, cut.m + cut.k * lo.cos()],
        uv_rate: [1.0, -cut.k * lo.sin()],
        parameter_origin: lo,
    };
    Ok((edge, pcurve))
}

fn add_bottom_crossing(
    builder: &mut Builder,
    chart: &Chart,
    cut: &EllipseCut,
    [start, end]: [u32; 2],
    loops: &mut Loops,
) -> Result<(), GeometryError> {
    let (lo, hi, tb) = (cut.t_top, TAU - cut.t_top, cut.t_bottom);
    let (pb1, pb2) = (chart.point(tb, 0.0), chart.point(TAU - tb, 0.0));
    let (vb1, vb2) = (builder.vertex(pb1), builder.vertex(pb2));
    let (e1, p1) = ellipse_edge(builder, cut, lo, tb)?;
    let range = Interval::new(tb, TAU - tb)?;
    let arc = builder.edge(chart.circle(0.0), range, false);
    let (e2, p2) = ellipse_edge(builder, cut, TAU - tb, hi)?;
    let (cb, cb_dir) = segment(builder, pb1, pb2)?;
    let r = chart.radius;
    let rim = uv_line([0.0, 0.0], [1.0, 0.0]);
    let (on_cap, on_cut) = (
        line_pcurve(chart.cap_bottom(), pb1, cb_dir),
        line_pcurve(cut.cut_frame, pb1, cb_dir),
    );
    loops.side.extend([
        boundary(e1, start, vb1, Forward, p1),
        boundary(arc, vb1, vb2, Forward, rim),
        boundary(e2, vb2, end, Forward, p2),
    ]);
    loops.bottom = Some(vec![
        boundary(arc, vb2, vb1, Reverse, conic(r, -r)),
        boundary(cb, vb1, vb2, Forward, on_cap),
    ]);
    loops.cut.extend([
        boundary(e2, end, vb2, Reverse, ellipse_on_cut(cut)),
        boundary(cb, vb2, vb1, Reverse, on_cut),
        boundary(e1, vb1, start, Reverse, ellipse_on_cut(cut)),
    ]);
    Ok(())
}

fn add_full_top(
    builder: &mut Builder,
    chart: &Chart,
    cut: &EllipseCut,
    [apex, rim]: [u32; 2],
    loops: &mut Loops,
) -> Result<Vec<Use>, GeometryError> {
    let (h, top) = (chart.height, cut.m + cut.k);
    let circle = builder.edge(chart.circle(h), Interval::new(0.0, TAU)?, false);
    let seam = builder.edge(
        CurveGeometry::Line {
            origin: chart.point(0.0, top),
            direction: chart.frame.z,
        },
        Interval::new(0.0, h - top)?,
        true,
    );
    loops.side.extend([
        boundary(seam, apex, rim, Forward, uv_line([TAU, top], [0.0, 1.0])),
        boundary(circle, rim, rim, Reverse, uv_line([0.0, h], [1.0, 0.0])),
        boundary(seam, rim, apex, Reverse, uv_line([0.0, top], [0.0, 1.0])),
    ]);
    let r = chart.radius;
    Ok(vec![boundary(circle, rim, rim, Forward, conic(r, r))])
}

fn add_top_chord(
    builder: &mut Builder,
    chart: &Chart,
    cut: &EllipseCut,
    [start, end]: [u32; 2],
    loops: &mut Loops,
) -> Result<Vec<Use>, GeometryError> {
    let (h, lo, hi) = (chart.height, cut.t_top, TAU - cut.t_top);
    let arc = builder.edge(chart.circle(h), Interval::new(lo, hi)?, false);
    let (start_p, end_p) = (chart.point(lo, h), chart.point(hi, h));
    let (chord, direction) = segment(builder, end_p, start_p)?;
    let rim = uv_line([0.0, h], [1.0, 0.0]);
    let on_cut = line_pcurve(cut.cut_frame, end_p, direction);
    let on_cap = line_pcurve(chart.cap_top(), end_p, direction);
    loops.side.push(boundary(arc, end, start, Reverse, rim));
    loops.cut.push(boundary(chord, start, end, Reverse, on_cut));
    let r = chart.radius;
    Ok(vec![
        boundary(arc, start, end, Forward, conic(r, r)),
        boundary(chord, end, start, Forward, on_cap),
    ])
}
