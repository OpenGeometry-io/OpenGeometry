mod bounded;
mod clip;
mod face_view;
#[cfg(test)]
mod tests;
mod trim;

pub(crate) use face_view::FaceView;

use crate::brep::{
    Accuracy, BrepEnvelope, Curve, Face, GeometryError, GeometryStore, Surface, SurfaceGeometry,
};
use crate::intersection::ssi_result::SsiCurve;
use crate::math::{Interval, Point3};
use crate::query::face_contains_uv;
use bounded::intersect_bounded_surfaces;
use clip::clip_face_branch;
use trim::{boundary_parameters, clip_coordinate, face_patch_bounds, is_uv_box_trim, pcurve_line};

pub(crate) struct IntersectionBranch {
    pub(crate) curve: u32,
    pub(crate) pcurves: [u32; 2],
    pub(crate) range: Interval,
    pub(crate) endpoints: [Point3; 2],
}

pub(crate) struct IntersectionGraph {
    pub(crate) geometry: GeometryStore,
    pub(crate) branches: Vec<IntersectionBranch>,
    pub(crate) contacts: Vec<Point3>,
    pub(crate) coincident: bool,
}

pub(crate) struct FacePairIntersection {
    pub(crate) faces: [u32; 2],
    pub(crate) graph: IntersectionGraph,
}

pub(crate) struct BodyIntersectionGraph {
    pub(crate) pairs: Vec<FacePairIntersection>,
}

pub(crate) fn intersect_breps(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
) -> Result<BodyIntersectionGraph, GeometryError> {
    a.validate()?;
    b.validate()?;
    let accuracy = Accuracy::combined(a.accuracy, b.accuracy);
    let bounds_a = a
        .topology
        .faces
        .iter()
        .map(|face| face_patch_bounds(a, face))
        .collect::<Result<Vec<_>, _>>()?;
    let bounds_b = b
        .topology
        .faces
        .iter()
        .map(|face| face_patch_bounds(b, face))
        .collect::<Result<Vec<_>, _>>()?;
    let mut pairs = Vec::new();
    for (face_a, bounds_a) in a.topology.faces.iter().zip(&bounds_a) {
        for (face_b, bounds_b) in b.topology.faces.iter().zip(&bounds_b) {
            if !bounds_a.overlaps(*bounds_b, accuracy.intersection) {
                continue;
            }
            let graph = intersect_faces(
                FaceView {
                    brep: a,
                    face: face_a.id,
                },
                FaceView {
                    brep: b,
                    face: face_b.id,
                },
            )
            .map_err(|error| match error {
                GeometryError::UnresolvedIntersection(message) => {
                    GeometryError::UnresolvedIntersection(format!(
                        "face pair {}:{}: {message}",
                        face_a.id, face_b.id
                    ))
                }
                other => other,
            })?;
            if graph.coincident || !graph.branches.is_empty() || !graph.contacts.is_empty() {
                pairs.push(FacePairIntersection {
                    faces: [face_a.id, face_b.id],
                    graph,
                });
            }
        }
    }
    Ok(BodyIntersectionGraph { pairs })
}

pub(crate) fn intersect_faces(
    a: FaceView<'_>,
    b: FaceView<'_>,
) -> Result<IntersectionGraph, GeometryError> {
    a.brep.validate()?;
    b.brep.validate()?;
    let face_a = a.brep.topology.faces.get(a.face as usize).ok_or_else(|| {
        GeometryError::MissingReference {
            kind: "face".into(),
            index: a.face,
        }
    })?;
    let face_b = b.brep.topology.faces.get(b.face as usize).ok_or_else(|| {
        GeometryError::MissingReference {
            kind: "face".into(),
            index: b.face,
        }
    })?;
    let planar = matches!(
        a.brep.geometry.surfaces[face_a.surface as usize],
        SurfaceGeometry::Plane { .. }
    ) && matches!(
        b.brep.geometry.surfaces[face_b.surface as usize],
        SurfaceGeometry::Plane { .. }
    );
    let boxes_only = !planar && is_uv_box_trim(a.brep, face_a)? && is_uv_box_trim(b.brep, face_b)?;
    let mut geometry = GeometryStore::new();
    geometry
        .surfaces
        .push(a.brep.geometry.surfaces[face_a.surface as usize].clone());
    geometry
        .surfaces
        .push(b.brep.geometry.surfaces[face_b.surface as usize].clone());
    let accuracy = Accuracy::combined(a.brep.accuracy, b.brep.accuracy);
    let result = intersect_bounded_surfaces(&mut geometry, [face_a, face_b], accuracy)?;
    let contacts = face_contacts(&geometry, result.contacts, [(a, face_a), (b, face_b)]);
    let mut branches = Vec::new();
    for branch in result.curves {
        if !planar {
            for range in clip_face_branch(
                &geometry,
                &branch,
                [(a, face_a), (b, face_b)],
                boxes_only,
                [face_a.trim.uv_bounds, face_b.trim.uv_bounds],
                accuracy,
            )? {
                let curve = geometry.curve(branch.curve)?;
                branches.push(IntersectionBranch {
                    curve: branch.curve,
                    pcurves: branch.pcurves,
                    range,
                    endpoints: [curve.point_at(range.lo)?, curve.point_at(range.hi)?],
                });
            }
            continue;
        }
        add_planar_branches(
            &geometry,
            &branch,
            [(a, face_a), (b, face_b)],
            accuracy,
            &mut branches,
        )?;
    }
    Ok(IntersectionGraph {
        geometry,
        branches,
        contacts,
        coincident: result.coincident,
    })
}

fn face_contacts(
    geometry: &GeometryStore,
    contacts: Vec<Point3>,
    faces: [(FaceView<'_>, &Face); 2],
) -> Vec<Point3> {
    let [(a, face_a), (b, face_b)] = faces;
    contacts
        .into_iter()
        .filter_map(|point| {
            let ua = geometry.surfaces[0].project(point, None).ok()?;
            let ub = geometry.surfaces[1].project(point, None).ok()?;
            (face_contains_uv(a.brep, face_a, ua).ok()? == Some(true)
                && face_contains_uv(b.brep, face_b, ub).ok()? == Some(true))
            .then_some(point)
        })
        .collect()
}

fn add_planar_branches(
    geometry: &GeometryStore,
    branch: &SsiCurve,
    faces: [(FaceView<'_>, &Face); 2],
    accuracy: Accuracy,
    branches: &mut Vec<IntersectionBranch>,
) -> Result<(), GeometryError> {
    let [(a, face_a), (b, face_b)] = faces;
    let (origin_a, direction_a) = pcurve_line(geometry, branch.pcurves[0])?;
    let (origin_b, direction_b) = pcurve_line(geometry, branch.pcurves[1])?;
    let mut range = branch
        .domain
        .map_or([-f64::MAX.sqrt(), f64::MAX.sqrt()], |range| {
            [range.lo, range.hi]
        });
    for (origin, direction, face) in [
        (origin_a, direction_a, face_a),
        (origin_b, direction_b, face_b),
    ] {
        for axis in 0..2 {
            if !clip_coordinate(
                origin[axis],
                direction[axis],
                face.trim.uv_bounds[axis],
                &mut range,
            ) {
                range[0] = 1.0;
                range[1] = 0.0;
                break;
            }
        }
    }
    if range[0] > range[1] {
        return Ok(());
    }
    let mut splits = vec![range[0], range[1]];
    boundary_parameters(a.brep, a.face, origin_a, direction_a, &mut splits)?;
    boundary_parameters(b.brep, b.face, origin_b, direction_b, &mut splits)?;
    splits.retain(|value| *value >= range[0] && *value <= range[1] && value.is_finite());
    splits.sort_by(f64::total_cmp);
    splits.dedup_by(|left, right| (*left - *right).abs() <= accuracy.intersection);
    for interval in splits.windows(2) {
        if interval[1] - interval[0] <= accuracy.geometric {
            continue;
        }
        let midpoint = interval[0] / 2.0 + interval[1] / 2.0;
        let uv_a = geometry.pcurve_at(branch.pcurves[0], midpoint)?;
        let uv_b = geometry.pcurve_at(branch.pcurves[1], midpoint)?;
        if face_contains_uv(a.brep, face_a, uv_a)? != Some(true)
            || face_contains_uv(b.brep, face_b, uv_b)? != Some(true)
        {
            continue;
        }
        let range = Interval::new(interval[0], interval[1])?;
        let curve = geometry.curve(branch.curve)?;
        branches.push(IntersectionBranch {
            curve: branch.curve,
            pcurves: branch.pcurves,
            range,
            endpoints: [curve.point_at(range.lo)?, curve.point_at(range.hi)?],
        });
    }
    Ok(())
}
