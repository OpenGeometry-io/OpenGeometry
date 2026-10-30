use crate::brep::{Accuracy, BrepEnvelope, GeometryError};
use crate::math::Point3;
use crate::operations::modifying::boolean::operands::{BoxInput, GridPoint};
use crate::operations::modifying::boolean::types::BooleanOp;
use crate::query::{classify_point_in_shell, PointClassification};
use std::collections::VecDeque;

pub(super) struct GridArrangement<'a> {
    pub(super) a: &'a BoxInput<'a>,
    pub(super) b: &'a BoxInput<'a>,
    pub(super) grid: &'a [Vec<f64>; 3],
    pub(super) n: GridPoint,
    pub(super) alo: Point3,
    pub(super) ahi: Point3,
    pub(super) blo: Point3,
    pub(super) bhi: Point3,
    pub(super) canonical_boxes: bool,
    pub(super) operation: BooleanOp,
    pub(super) accuracy: Accuracy,
}

pub(super) fn material_cells(
    arrangement: &GridArrangement<'_>,
    count: usize,
) -> Result<Vec<bool>, GeometryError> {
    let &GridArrangement {
        a,
        b,
        grid,
        n,
        alo,
        ahi,
        blo,
        bhi,
        canonical_boxes,
        operation,
        ..
    } = arrangement;
    let mut material = vec![false; count];
    let solids = if canonical_boxes {
        [Vec::new(), Vec::new()]
    } else {
        [solid_shells(a.brep), solid_shells(b.brep)]
    };
    for x in 0..n[0] {
        for y in 0..n[1] {
            for z in 0..n[2] {
                let p = [x, y, z];
                let center = std::array::from_fn(|i| {
                    grid[i][p[i]] + (grid[i][p[i] + 1] - grid[i][p[i]]) / 2.0
                });
                let ia = if canonical_boxes {
                    inside(center, alo, ahi)
                } else {
                    inside_solids(a.brep, &solids[0], a.frame.point(center))?
                };
                let ib = if canonical_boxes {
                    inside(center, blo, bhi)
                } else {
                    inside_solids(b.brep, &solids[1], a.frame.point(center))?
                };
                material[flat_index(p, n)] = match operation {
                    BooleanOp::Union => ia || ib,
                    BooleanOp::Intersection => ia && ib,
                    BooleanOp::Subtraction => ia && !ib,
                };
            }
        }
    }
    Ok(material)
}

struct SolidShells {
    shells: Vec<Vec<u32>>,
    lo: Point3,
    hi: Point3,
}

fn solid_shells(brep: &BrepEnvelope) -> Vec<SolidShells> {
    let pad = brep.accuracy.geometric;
    brep.solids
        .iter()
        .map(|solid| {
            let shells = std::iter::once(solid.outer_shell)
                .chain(solid.cavity_shells.iter().copied())
                .map(|shell| brep.topology.shells[shell as usize].faces.clone())
                .collect::<Vec<_>>();
            let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
            for halfedge in &brep.topology.halfedges {
                if halfedge.face.is_some_and(|face| shells[0].contains(&face)) {
                    let point = brep.topology.vertices[halfedge.from as usize].position;
                    for axis in 0..3 {
                        lo[axis] = lo[axis].min(point[axis] - pad);
                        hi[axis] = hi[axis].max(point[axis] + pad);
                    }
                }
            }
            SolidShells { shells, lo, hi }
        })
        .collect()
}

fn inside_solids(
    brep: &BrepEnvelope,
    solids: &[SolidShells],
    point: Point3,
) -> Result<bool, GeometryError> {
    for solid in solids {
        if (0..3).any(|axis| point[axis] < solid.lo[axis] || point[axis] > solid.hi[axis]) {
            continue;
        }
        let mut inside = inside_shell(brep, &solid.shells[0], point)?;
        for cavity in &solid.shells[1..] {
            inside = inside && !inside_shell(brep, cavity, point)?;
        }
        if inside {
            return Ok(true);
        }
    }
    Ok(false)
}

fn inside_shell(brep: &BrepEnvelope, faces: &[u32], point: Point3) -> Result<bool, GeometryError> {
    match classify_point_in_shell(brep, faces, point)? {
        PointClassification::Inside => Ok(true),
        PointClassification::Outside => Ok(false),
        _ => Err(GeometryError::UnresolvedIntersection(
            "rectilinear arrangement cell classification is unresolved".into(),
        )),
    }
}

fn inside(point: Point3, lo: Point3, hi: Point3) -> bool {
    (0..3).all(|i| point[i] > lo[i] && point[i] < hi[i])
}

pub(super) fn flat_index(p: GridPoint, n: GridPoint) -> usize {
    (p[0] * n[1] + p[1]) * n[2] + p[2]
}

pub(super) fn material_components(
    material: &[bool],
    n: GridPoint,
    count: usize,
) -> (Vec<usize>, usize) {
    let mut components = vec![usize::MAX; count];
    let mut component_count = 0;
    for x in 0..n[0] {
        for y in 0..n[1] {
            for z in 0..n[2] {
                let p = [x, y, z];
                let index = flat_index(p, n);
                if !material[index] || components[index] != usize::MAX {
                    continue;
                }
                components[index] = component_count;
                let mut queue = VecDeque::from([p]);
                while let Some(cell) = queue.pop_front() {
                    for axis in 0..3 {
                        for upper in [false, true] {
                            if let Some(adjacent) = neighbor(cell, axis, upper, n) {
                                let j = flat_index(adjacent, n);
                                if material[j] && components[j] == usize::MAX {
                                    components[j] = component_count;
                                    queue.push_back(adjacent);
                                }
                            }
                        }
                    }
                }
                component_count += 1;
            }
        }
    }
    (components, component_count)
}

pub(super) fn neighbor(
    mut p: GridPoint,
    axis: usize,
    upper: bool,
    n: GridPoint,
) -> Option<GridPoint> {
    if upper {
        p[axis] += 1;
        if p[axis] >= n[axis] {
            return None;
        }
    } else {
        p[axis] = p[axis].checked_sub(1)?;
    }
    Some(p)
}
