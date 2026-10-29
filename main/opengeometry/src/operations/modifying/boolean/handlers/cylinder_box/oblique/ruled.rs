use super::chart::{conic, gap, line_pcurve, segment, Chart, HalfSpaceUses};
use crate::brep::Orientation::{Forward, Reverse};
use crate::brep::{boundary, uv_line, Builder, Frame3, GeometryError, Use};
use crate::math::{scale, Interval, Point3};
use std::f64::consts::TAU;

struct Rulings {
    angles: [f64; 2],
    corners: [Point3; 4],
    vertices: [u32; 4],
    arcs: [u32; 2],
    lines: [u32; 2],
    chords: [(u32, Point3); 2],
}

pub(super) fn ruled_uses(
    builder: &mut Builder,
    chart: &Chart,
    sliver: f64,
) -> Result<HalfSpaceUses, GeometryError> {
    let u0 = -chart.e / chart.a;
    if u0.abs() >= chart.radius - sliver {
        return Err(gap());
    }
    let rulings = add_rulings(builder, chart, u0)?;
    let pad = builder.brep.accuracy.geometric;
    let half = chart.radius * rulings.angles[0].sin();
    let frame = chart.frame;
    let cut_frame = Frame3 {
        origin: frame.point([u0, 0.0, 0.0]),
        x: frame.z,
        y: scale(frame.y, -1.0),
        z: frame.x,
    };
    let [side, bottom, top, cut] = ruled_loops(chart, &rulings, cut_frame);
    Ok(HalfSpaceUses {
        side,
        side_bounds: [rulings.angles, [0.0, chart.height]],
        bottom: Some(bottom),
        top,
        top_partial: true,
        cut,
        cut_frame,
        cut_bounds: [[-pad, chart.height + pad], [-half - pad, half + pad]],
    })
}

fn add_rulings(builder: &mut Builder, chart: &Chart, u0: f64) -> Result<Rulings, GeometryError> {
    let t1 = (u0 / chart.radius).acos();
    let t2 = TAU - t1;
    let corners = [
        chart.point(t1, 0.0),
        chart.point(t2, 0.0),
        chart.point(t1, chart.height),
        chart.point(t2, chart.height),
    ];
    let vertices = corners.map(|p| builder.vertex(p));
    let range = Interval::new(t1, t2)?;
    let arcs = [
        builder.edge(chart.circle(0.0), range, false),
        builder.edge(chart.circle(chart.height), range, false),
    ];
    let lines = [
        segment(builder, corners[0], corners[2])?.0,
        segment(builder, corners[1], corners[3])?.0,
    ];
    let chords = [
        segment(builder, corners[0], corners[1])?,
        segment(builder, corners[3], corners[2])?,
    ];
    Ok(Rulings {
        angles: [t1, t2],
        corners,
        vertices,
        arcs,
        lines,
        chords,
    })
}

fn ruled_loops(chart: &Chart, rulings: &Rulings, cut_frame: Frame3) -> [Vec<Use>; 4] {
    let [vb1, vb2, vt1, vt2] = rulings.vertices;
    let [pb1, pb2, _, pt2] = rulings.corners;
    let [t1, t2] = rulings.angles;
    let [low, high] = rulings.arcs;
    let [l1, l2] = rulings.lines;
    let [(cb, cb_dir), (ct, ct_dir)] = rulings.chords;
    let (r, h, z) = (chart.radius, chart.height, chart.frame.z);
    let on_low_cap = line_pcurve(chart.cap_bottom(), pb1, cb_dir);
    let on_high_cap = line_pcurve(chart.cap_top(), pt2, ct_dir);
    let side = vec![
        boundary(low, vb1, vb2, Forward, uv_line([0.0, 0.0], [1.0, 0.0])),
        boundary(l2, vb2, vt2, Forward, uv_line([t2, 0.0], [0.0, 1.0])),
        boundary(high, vt2, vt1, Reverse, uv_line([0.0, h], [1.0, 0.0])),
        boundary(l1, vt1, vb1, Reverse, uv_line([t1, 0.0], [0.0, 1.0])),
    ];
    let bottom = vec![
        boundary(low, vb2, vb1, Reverse, conic(r, -r)),
        boundary(cb, vb1, vb2, Forward, on_low_cap),
    ];
    let top = vec![
        boundary(high, vt1, vt2, Forward, conic(r, r)),
        boundary(ct, vt2, vt1, Forward, on_high_cap),
    ];
    let cut = vec![
        boundary(l2, vt2, vb2, Reverse, line_pcurve(cut_frame, pb2, z)),
        boundary(cb, vb2, vb1, Reverse, line_pcurve(cut_frame, pb1, cb_dir)),
        boundary(l1, vb1, vt1, Forward, line_pcurve(cut_frame, pb1, z)),
        boundary(ct, vt1, vt2, Reverse, line_pcurve(cut_frame, pt2, ct_dir)),
    ];
    [side, bottom, top, cut]
}
