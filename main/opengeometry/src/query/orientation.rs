use super::classification::PointClassification;
use super::classify::Classifier;
use super::face_trim::FaceTrim;
use crate::brep::{unit, BrepEnvelope, Face, GeometryError, Surface, UV};
use crate::math::{add, scale, sub, Point3};

const GRIDS: [usize; 2] = [6, 24];
const PROBE_OFFSET: f64 = 100.0;
const SPACINGS_PER_OFFSET: f64 = 256.0;

pub fn validate_orientation(brep: &BrepEnvelope) -> Result<(), GeometryError> {
    brep.validate()?;
    let offset = PROBE_OFFSET * brep.accuracy.geometric;
    let frames_resolve = brep
        .geometry
        .surfaces
        .iter()
        .all(|surface| resolves(offset, surface.frame().origin));
    if brep.solids.is_empty() || !frames_resolve {
        return Ok(());
    }
    let classifier = Classifier::new(brep);
    match brep
        .topology
        .faces
        .iter()
        .find(|face| face_is_inverted(brep, &classifier, face))
    {
        Some(face) => Err(GeometryError::InvalidTopology(format!(
            "face {} ('{}') has its material normal pointing into the solid",
            face.id, face.key
        ))),
        None => Ok(()),
    }
}

fn face_is_inverted(brep: &BrepEnvelope, classifier: &Classifier, face: &Face) -> bool {
    let Ok(trim) = FaceTrim::new(brep, face) else {
        return false;
    };
    probe_points(brep, face, &trim).is_some_and(|points| {
        points
            .into_iter()
            .all(|uv| probe(brep, classifier, face, uv) == Some(true))
    })
}

fn probe(brep: &BrepEnvelope, classifier: &Classifier, face: &Face, uv: UV) -> Option<bool> {
    let surface = brep.geometry.surface(face.surface).ok()?;
    let normal = unit(surface.normal_at(uv).ok()?).ok()?;
    let on_face = surface.point_at(uv).ok()?;
    let offset = PROBE_OFFSET * brep.accuracy.geometric;
    if !resolves(offset, on_face) {
        return None;
    }
    let step = scale(normal, face.sense.multiplier() * offset);
    let front = classifier.classify(add(on_face, step)).ok()?;
    let back = classifier.classify(sub(on_face, step)).ok()?;
    match (front, back) {
        (PointClassification::Inside, PointClassification::Outside) => Some(true),
        (PointClassification::Outside, PointClassification::Inside) => Some(false),
        _ => None,
    }
}

fn resolves(offset: f64, at: Point3) -> bool {
    let reach = at
        .iter()
        .fold(0.0_f64, |reach, value| reach.max(value.abs()));
    offset > SPACINGS_PER_OFFSET * f64::EPSILON * reach
}

fn probe_points(brep: &BrepEnvelope, face: &Face, trim: &FaceTrim) -> Option<[UV; 2]> {
    GRIDS.into_iter().find_map(|grid| {
        let at = |i: usize, j: usize| -> UV {
            std::array::from_fn(|axis| {
                let bounds = face.trim.uv_bounds[axis];
                let index = [i, j][axis] as f64;
                bounds.lo + bounds.width() * (index + 0.5) / grid as f64
            })
        };
        let inside: Vec<Vec<bool>> = (0..grid)
            .map(|i| {
                (0..grid)
                    .map(|j| trim.contains(brep, face, at(i, j)) == Ok(Some(true)))
                    .collect()
            })
            .collect();
        let mut ranked: Vec<(usize, usize, usize)> = (0..grid * grid)
            .map(|index| (index / grid, index % grid))
            .filter(|&(i, j)| inside[i][j])
            .map(|(i, j)| (depth(&inside, i, j), i, j))
            .collect();
        ranked.sort_by_key(|&(depth, i, j)| (std::cmp::Reverse(depth), i, j));
        match ranked[..] {
            [(_, i, j), (_, k, l), ..] => Some([at(i, j), at(k, l)]),
            _ => None,
        }
    })
}

fn depth(inside: &[Vec<bool>], i: usize, j: usize) -> usize {
    let grid = inside.len();
    let border = [i + 1, j + 1, grid - i, grid - j].into_iter().min();
    let outside = (0..grid * grid)
        .map(|index| (index / grid, index % grid))
        .filter(|&(k, l)| !inside[k][l])
        .map(|(k, l)| i.abs_diff(k).max(j.abs_diff(l)));
    border.into_iter().chain(outside).min().unwrap_or(0)
}
