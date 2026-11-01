use super::classification::PointClassification;
use super::classify::classify_point_validated;
use super::face_trim::face_contains_uv;
use crate::brep::{unit, BrepEnvelope, Face, GeometryError, Surface, UV};
use crate::math::{add, norm, scale, sub};

const GRID: usize = 6;
const PROBE_OFFSET: f64 = 100.0;
const RELIABLE_TOLERANCE: f64 = 1e-12;

pub fn validate_orientation(brep: &BrepEnvelope) -> Result<(), GeometryError> {
    brep.validate()?;
    validate_orientation_validated(brep)
}

pub(super) fn validate_orientation_validated(brep: &BrepEnvelope) -> Result<(), GeometryError> {
    if brep.solids.is_empty() {
        return Ok(());
    }
    match check_faces(brep) {
        Err(GeometryError::Math(_)) => Ok(()),
        outcome => outcome,
    }
}

fn check_faces(brep: &BrepEnvelope) -> Result<(), GeometryError> {
    let Some(bounds) = brep.bounds_unchecked()? else {
        return Ok(());
    };
    let diagonal = norm(std::array::from_fn(|axis| bounds.axes[axis].width()));
    let coordinates = bounds
        .axes
        .iter()
        .flat_map(|axis| [axis.lo, axis.hi])
        .chain(brep.geometry.surfaces.iter().flat_map(|s| s.frame().origin))
        .fold(diagonal, |scale, value| scale.max(value.abs()));
    if !coordinates.is_finite() || brep.accuracy.geometric < RELIABLE_TOLERANCE * coordinates {
        return Ok(());
    }
    for face in &brep.topology.faces {
        if face_is_inverted(brep, face)? {
            return Err(GeometryError::InvalidTopology(format!(
                "face {} ('{}') has its material normal pointing into the solid",
                face.id, face.key
            )));
        }
    }
    Ok(())
}

fn face_is_inverted(brep: &BrepEnvelope, face: &Face) -> Result<bool, GeometryError> {
    let [first, second] = probe_points(brep, face)?;
    let Some(first) = first else {
        return Ok(false);
    };
    if probe(brep, face, first)? != Some(true) {
        return Ok(false);
    }
    let Some(second) = second else {
        return Ok(true);
    };
    Ok(probe(brep, face, second)? == Some(true))
}

fn probe(brep: &BrepEnvelope, face: &Face, uv: UV) -> Result<Option<bool>, GeometryError> {
    let surface = brep.geometry.surface(face.surface)?;
    let Ok(normal) = surface.normal_at(uv) else {
        return Ok(None);
    };
    let step = scale(
        unit(normal)?,
        face.sense.multiplier() * PROBE_OFFSET * brep.accuracy.geometric,
    );
    let on_face = surface.point_at(uv)?;
    let front = classify_point_validated(brep, add(on_face, step))?;
    let back = classify_point_validated(brep, sub(on_face, step))?;
    Ok(match (front, back) {
        (PointClassification::Inside, PointClassification::Outside) => Some(true),
        (PointClassification::Outside, PointClassification::Inside) => Some(false),
        _ => None,
    })
}

fn probe_points(brep: &BrepEnvelope, face: &Face) -> Result<[Option<UV>; 2], GeometryError> {
    let cell =
        |i: usize, j: usize| -> UV { [i, j].map(|index| (index as f64 + 0.5) / GRID as f64) };
    let at = |fraction: UV| -> UV {
        std::array::from_fn(|axis| {
            let bounds = face.trim.uv_bounds[axis];
            bounds.lo + bounds.width() * fraction[axis]
        })
    };
    let mut inside = [[false; GRID]; GRID];
    for (i, row) in inside.iter_mut().enumerate() {
        for (j, value) in row.iter_mut().enumerate() {
            *value = face_contains_uv(brep, face, at(cell(i, j)))? == Some(true);
        }
    }
    let mut ranked: Vec<(usize, usize, usize)> = (0..GRID * GRID)
        .map(|index| (index / GRID, index % GRID))
        .filter(|&(i, j)| inside[i][j])
        .map(|(i, j)| (depth(&inside, i, j), i, j))
        .collect();
    ranked.sort_by_key(|&(depth, i, j)| (std::cmp::Reverse(depth), i, j));
    Ok([0, 1].map(|rank| ranked.get(rank).map(|&(_, i, j)| at(cell(i, j)))))
}

fn depth(inside: &[[bool; GRID]; GRID], i: usize, j: usize) -> usize {
    let border = [i + 1, j + 1, GRID - i, GRID - j].into_iter().min();
    let outside = (0..GRID * GRID)
        .map(|index| (index / GRID, index % GRID))
        .filter(|&(k, l)| !inside[k][l])
        .map(|(k, l)| i.abs_diff(k).max(j.abs_diff(l)));
    border.into_iter().chain(outside).min().unwrap_or(0)
}
