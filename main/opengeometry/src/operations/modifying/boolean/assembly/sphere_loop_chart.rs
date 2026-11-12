use super::loop_pcurves::polygon_centroid;
use crate::brep::{
    unit, Frame3, GeometryError, GeometryStore, IntersectionDefinition, Surface, SurfaceGeometry,
};
use crate::math::{add, cross, dot, scale, Interval, Point3};

struct SphereCandidate {
    score: f64,
    frame: Frame3,
    coordinates: Vec<[f64; 2]>,
}

pub(crate) struct SphereLoopChart {
    pub(crate) frame: Frame3,
    pub(crate) radius: f64,
    coordinates: Vec<[f64; 2]>,
}

impl SphereLoopChart {
    fn surface(&self) -> SurfaceGeometry {
        SurfaceGeometry::Sphere {
            frame: self.frame,
            radius: self.radius,
        }
    }

    pub(crate) fn patch_sample(&self) -> Result<Point3, GeometryError> {
        self.surface().point_at(polygon_centroid(&self.coordinates))
    }
}

pub(crate) fn sphere_loop_chart(
    geometry: &GeometryStore,
    definition: &IntersectionDefinition,
    side: usize,
) -> Result<SphereLoopChart, GeometryError> {
    let (frame, radius) = match geometry.surface(definition.surfaces[side])? {
        SurfaceGeometry::Sphere { frame, radius } => (*frame, *radius),
        _ => {
            return Err(GeometryError::CoverageGap {
                families: ["periodic winding".into(), "non-spherical support".into()],
            })
        }
    };
    let SphereCandidate {
        frame, coordinates, ..
    } = best_sphere_candidate(frame, radius, definition)?.ok_or_else(winding_gap)?;
    centred_on_half_turn(frame, radius, coordinates)
}

fn winding_gap() -> GeometryError {
    GeometryError::CoverageGap {
        families: ["sphere".into(), "unresolved periodic winding".into()],
    }
}

fn best_sphere_candidate(
    frame: Frame3,
    radius: f64,
    definition: &IntersectionDefinition,
) -> Result<Option<SphereCandidate>, GeometryError> {
    let references = [frame.x, frame.y, frame.z];
    let mut best: Option<SphereCandidate> = None;
    for i in -1..=1 {
        for j in -1..=1 {
            for k in -1..=1 {
                if [i, j, k] == [0, 0, 0] {
                    continue;
                }
                let axis = unit(frame.vector([i as f64, j as f64, k as f64]))?;
                let Some(candidate) =
                    scored_sphere_candidate(frame, radius, references, axis, definition)?
                else {
                    continue;
                };
                if best
                    .as_ref()
                    .is_none_or(|best| candidate.score < best.score)
                {
                    best = Some(candidate);
                }
            }
        }
    }
    Ok(best)
}

fn centred_on_half_turn(
    frame: Frame3,
    radius: f64,
    mut coordinates: Vec<[f64; 2]>,
) -> Result<SphereLoopChart, GeometryError> {
    let (lo, hi) = coordinates
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), uv| {
            (lo.min(uv[0]), hi.max(uv[0]))
        });
    if hi - lo >= std::f64::consts::TAU {
        return Err(winding_gap());
    }
    let angle = 0.5 * (lo + hi) - std::f64::consts::PI;
    let (sin, cos) = angle.sin_cos();
    let x = add(scale(frame.x, cos), scale(frame.y, sin));
    let frame = Frame3 {
        origin: frame.origin,
        x,
        y: cross(frame.z, x),
        z: frame.z,
    };
    frame.validate()?;
    for uv in &mut coordinates {
        uv[0] -= angle;
    }
    Ok(SphereLoopChart {
        frame,
        radius,
        coordinates,
    })
}

pub(crate) fn reframe_sphere_intersection(
    geometry: &mut GeometryStore,
    definition: &mut IntersectionDefinition,
    side: usize,
    chart: &SphereLoopChart,
) -> Result<(), GeometryError> {
    geometry.surfaces[definition.surfaces[side] as usize] = chart.surface();
    let coordinates = &chart.coordinates;
    for (anchor, uv) in definition.anchors.iter_mut().zip(coordinates) {
        if side == 0 {
            anchor.uv_a = *uv;
        } else {
            anchor.uv_b = *uv;
        }
    }
    let offset = side * 2;
    for (index, tube) in definition.uv_tubes.iter_mut().enumerate() {
        for axis in 0..2 {
            let a = coordinates[index][axis];
            let b = coordinates[index + 1][axis];
            let padding = (a - b)
                .abs()
                .max(definition.residual_tolerance / chart.radius)
                * 2.0;
            tube[offset + axis] = Interval::new(a.min(b) - padding, a.max(b) + padding)?;
        }
    }
    definition.validate(geometry)
}

fn scored_sphere_candidate(
    frame: Frame3,
    radius: f64,
    references: [Point3; 3],
    axis: Point3,
    definition: &IntersectionDefinition,
) -> Result<Option<SphereCandidate>, GeometryError> {
    let reference = references
        .into_iter()
        .min_by(|left, right| dot(*left, axis).abs().total_cmp(&dot(*right, axis).abs()))
        .ok_or_else(|| GeometryError::InvalidGeometry("no reference axes".into()))?;
    let frame = Frame3::from_axis(frame.origin, axis, reference)?;
    let candidate = SurfaceGeometry::Sphere { frame, radius };
    let mut coordinates = Vec::with_capacity(definition.anchors.len());
    let mut hint = None;
    for anchor in &definition.anchors {
        let Ok(uv) = candidate.project(anchor.point, hint) else {
            return Ok(None);
        };
        coordinates.push(uv);
        hint = Some(uv);
    }
    let Some(last) = coordinates.last() else {
        return Ok(None);
    };
    let longitude_span = (last[0] - coordinates[0][0]).abs();
    let pole_proximity = coordinates
        .iter()
        .map(|uv| uv[1].abs())
        .fold(0.0_f64, f64::max);
    if longitude_span >= std::f64::consts::PI
        || pole_proximity >= std::f64::consts::FRAC_PI_2 - 1.0e-6
    {
        return Ok(None);
    }
    Ok(Some(SphereCandidate {
        score: pole_proximity + longitude_span,
        frame,
        coordinates,
    }))
}
