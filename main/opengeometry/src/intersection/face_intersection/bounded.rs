use super::trim::{clip_coordinate, periodic_value};
use crate::brep::{
    Accuracy, CurveGeometry, Face, GeometryError, GeometryStore, SsiBudget, Surface,
    SurfaceGeometry,
};
use crate::intersection::ssi::intersect_surfaces;
use crate::intersection::ssi_result::{SsiCurve, SsiResult};
use crate::intersection::universal_ssi::intersect_patches;
use crate::math::{add, Interval};

fn bound_plane_cylinder_generator(
    geometry: &GeometryStore,
    branch: &SsiCurve,
    faces: [&Face; 2],
    accuracy: Accuracy,
) -> Result<Option<Interval>, GeometryError> {
    let CurveGeometry::Line { origin, direction } = geometry.curves[branch.curve as usize] else {
        return Err(GeometryError::CoverageGap {
            families: ["plane/cylinder trim".into(), "nonlinear generator".into()],
        });
    };
    let mut range = [-f64::MAX.sqrt(), f64::MAX.sqrt()];
    for side in 0..2 {
        let surface = geometry.surface(side as u32)?;
        let start = surface.project(origin, None)?;
        let next = surface.project(add(origin, direction), Some(start))?;
        let periods = surface.charts()[0].periods;
        for axis in 0..2 {
            let bounds = faces[side].trim.uv_bounds[axis];
            let initial = periodic_value(start[axis], bounds, periods[axis]);
            let later = periodic_value(next[axis], bounds, periods[axis]);
            let mut rate = later - initial;
            if matches!(surface, SurfaceGeometry::Cylinder { .. })
                && axis == 0
                && rate.abs() > accuracy.intersection
            {
                return Err(GeometryError::CoverageGap {
                    families: ["plane/cylinder trim".into(), "nonaxial generator".into()],
                });
            }
            if matches!(surface, SurfaceGeometry::Cylinder { .. }) && axis == 0 {
                rate = 0.0;
            }
            if !clip_coordinate(initial, rate, bounds, &mut range) {
                return Ok(None);
            }
        }
    }
    if !range[0].is_finite() || !range[1].is_finite() || range[1] - range[0] <= accuracy.geometric {
        return Ok(None);
    }
    Ok(Some(Interval::new(range[0], range[1])?))
}

pub(super) fn intersect_bounded_surfaces(
    geometry: &mut GeometryStore,
    faces: [&Face; 2],
    accuracy: Accuracy,
) -> Result<SsiResult, GeometryError> {
    let planar_pair = geometry
        .surfaces
        .iter()
        .take(2)
        .all(|surface| matches!(surface, SurfaceGeometry::Plane { .. }));
    let plane_cylinder_pair = geometry.surfaces.len() >= 2
        && matches!(
            (&geometry.surfaces[0], &geometry.surfaces[1]),
            (
                SurfaceGeometry::Plane { .. },
                SurfaceGeometry::Cylinder { .. }
            ) | (
                SurfaceGeometry::Cylinder { .. },
                SurfaceGeometry::Plane { .. }
            )
        );
    match intersect_surfaces(geometry, 0, 1, accuracy) {
        Ok(mut result) if plane_cylinder_pair => {
            let mut bounded = Vec::new();
            for mut branch in result.curves {
                if branch.domain.is_none() {
                    branch.domain =
                        bound_plane_cylinder_generator(geometry, &branch, faces, accuracy)?;
                }
                if branch.domain.is_some() {
                    bounded.push(branch);
                }
            }
            result.curves = bounded;
            Ok(result)
        }
        Ok(result) if planar_pair || result.curves.iter().all(|curve| curve.domain.is_some()) => {
            Ok(result)
        }
        Ok(_) | Err(GeometryError::CoverageGap { .. }) => intersect_patches(
            geometry,
            [0, 1],
            [faces[0].trim.uv_bounds, faces[1].trim.uv_bounds],
            accuracy,
            SsiBudget::default(),
        ),
        Err(error) => Err(error),
    }
}
