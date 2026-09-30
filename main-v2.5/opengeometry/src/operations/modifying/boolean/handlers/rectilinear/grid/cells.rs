use crate::brep::{Accuracy, GeometryError};
use crate::math::Point3;
use crate::operations::modifying::boolean::operands::{classified_inside, BoxInput, GridPoint};
use crate::operations::modifying::boolean::types::BooleanOp;
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
                    classified_inside(a.brep, a.frame.point(center))?
                };
                let ib = if canonical_boxes {
                    inside(center, blo, bhi)
                } else {
                    classified_inside(b.brep, a.frame.point(center))?
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
