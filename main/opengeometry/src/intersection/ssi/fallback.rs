use crate::brep::{
    Accuracy, GeometryError, GeometryStore, PatchBounds, SsiBudget, Surface, SurfaceGeometry,
};
use crate::intersection::ssi_result::SsiResult;
use crate::intersection::universal_ssi::intersect_patches_from;
use crate::math::{norm, Interval};

fn chart_domain(surface: &SurfaceGeometry) -> Result<[Interval; 2], GeometryError> {
    let chart = surface
        .charts()
        .first()
        .ok_or_else(|| GeometryError::InvalidGeometry("surface has no parameter chart".into()))?;
    Ok([
        Interval::new(chart.domain[0][0], chart.domain[0][1])?,
        Interval::new(chart.domain[1][0], chart.domain[1][1])?,
    ])
}

fn finite_target_domain(
    surface: &SurfaceGeometry,
    source_bounds: PatchBounds,
    margin: f64,
) -> Result<[Interval; 2], GeometryError> {
    if matches!(
        surface,
        SurfaceGeometry::Sphere { .. } | SurfaceGeometry::Torus { .. }
    ) {
        return chart_domain(surface);
    }
    let mut local_min = [f64::INFINITY; 3];
    let mut local_max = [f64::NEG_INFINITY; 3];
    let mut maximum_radius = 0.0_f64;
    for mask in 0..8 {
        let point = std::array::from_fn(|axis| {
            if mask & (1 << axis) == 0 {
                source_bounds.axes[axis].lo
            } else {
                source_bounds.axes[axis].hi
            }
        });
        let local = surface.frame().local(point);
        for axis in 0..3 {
            local_min[axis] = local_min[axis].min(local[axis]);
            local_max[axis] = local_max[axis].max(local[axis]);
        }
        maximum_radius = maximum_radius.max(norm(local));
    }
    let expanded = |lo: f64, hi: f64| Interval::new(lo - margin, hi + margin);
    match surface {
        SurfaceGeometry::Plane { .. } => Ok([
            expanded(local_min[0], local_max[0])?,
            expanded(local_min[1], local_max[1])?,
        ]),
        SurfaceGeometry::Cylinder { .. } => Ok([
            Interval::new(0.0, std::f64::consts::TAU)?,
            expanded(local_min[2], local_max[2])?,
        ]),
        SurfaceGeometry::Cone { .. } => Ok([
            Interval::new(0.0, std::f64::consts::TAU)?,
            Interval::new(0.0, maximum_radius + margin)?,
        ]),
        SurfaceGeometry::Sphere { .. } | SurfaceGeometry::Torus { .. } => Err(
            GeometryError::UnsupportedGeometry("SSI trim surface".into()),
        ),
    }
}

pub(super) fn universal_fallback(
    store: &mut GeometryStore,
    a: u32,
    b: u32,
    sa: &SurfaceGeometry,
    sb: &SurfaceGeometry,
    accuracy: Accuracy,
    original: GeometryError,
) -> Result<SsiResult, GeometryError> {
    let source_side = if matches!(
        sa,
        SurfaceGeometry::Sphere { .. } | SurfaceGeometry::Torus { .. }
    ) {
        0
    } else if matches!(
        sb,
        SurfaceGeometry::Sphere { .. } | SurfaceGeometry::Torus { .. }
    ) {
        1
    } else {
        return Err(original);
    };
    let mut domains = [chart_domain(sa)?, chart_domain(sb)?];
    let source_bounds = store
        .surface([a, b][source_side])?
        .enclose(domains[source_side])?;
    domains[1 - source_side] = finite_target_domain(
        store.surface([a, b][1 - source_side])?,
        source_bounds,
        accuracy.intersection,
    )?;
    intersect_patches_from(
        store,
        [a, b],
        domains,
        source_side,
        accuracy,
        SsiBudget::default(),
    )
}
