mod boundary;
mod cells;

use crate::brep::{Accuracy, BrepEnvelope, GeometryError, Shell, SolidRegion};
use crate::math::{norm, Point3};
use crate::operations::modifying::boolean::operands::{coverage, source, BoxInput};
use crate::operations::modifying::boolean::types::{
    BooleanOp, BooleanReport, BooleanResult, FaceMapping,
};
use boundary::{boundary_faces, BoundaryFaces};
use cells::{material_cells, material_components, GridArrangement};
use std::collections::VecDeque;

pub(crate) fn boolean_grid(
    a: BoxInput<'_>,
    b: BoxInput<'_>,
    operation: BooleanOp,
    id: String,
    canonical_boxes: bool,
) -> Result<BooleanResult, GeometryError> {
    if [a.frame.x, a.frame.y, a.frame.z] != [b.frame.x, b.frame.y, b.frame.z] {
        return Err(coverage());
    }
    let accuracy = Accuracy::combined(a.brep.accuracy, b.brep.accuracy);
    let alo = [0.0; 3];
    let ahi = a.size;
    let blo = a.frame.local(b.frame.origin);
    let bhi: Point3 = std::array::from_fn(|i| blo[i] + b.size[i]);
    if blo.into_iter().chain(bhi).any(|value| !value.is_finite()) {
        return Err(GeometryError::UnresolvedIntersection(
            "cuboid relative coordinates exceed numerical range".into(),
        ));
    }
    let mut grid: [Vec<f64>; 3] = std::array::from_fn(|i| vec![alo[i], ahi[i], blo[i], bhi[i]]);
    if !canonical_boxes {
        for input in [&a, &b] {
            for vertex in &input.brep.topology.vertices {
                let point = a.frame.local(vertex.position);
                for axis in 0..3 {
                    grid[axis].push(point[axis]);
                }
            }
        }
    }
    deduplicate_grid(&mut grid, &a, &b, accuracy)?;
    let alo = snapped_point(&grid, alo);
    let ahi = snapped_point(&grid, ahi);
    let blo = snapped_point(&grid, blo);
    let bhi = snapped_point(&grid, bhi);
    let coincident = alo == blo && ahi == bhi;
    let overlap_lo: Point3 = std::array::from_fn(|i| alo[i].max(blo[i]));
    let overlap_hi: Point3 = std::array::from_fn(|i| ahi[i].min(bhi[i]));
    let contacts = contact_points(&a, overlap_lo, overlap_hi);
    let n = grid.each_ref().map(|g| g.len() - 1);
    let count = n
        .into_iter()
        .try_fold(1usize, |product, cells| product.checked_mul(cells))
        .filter(|count| *count <= 2_000_000)
        .ok_or_else(|| {
            GeometryError::LimitExceeded(
                "rectilinear Boolean grid exceeds two million cells".into(),
            )
        })?;
    let arrangement = GridArrangement {
        a: &a,
        b: &b,
        grid: &grid,
        n,
        alo,
        ahi,
        blo,
        bhi,
        canonical_boxes,
        operation,
        accuracy,
    };
    let material = material_cells(&arrangement, count)?;
    let (components, component_count) = material_components(&material, n, count);
    let BoundaryFaces {
        brep: mut out,
        face_components,
        volumes,
    } = boundary_faces(&arrangement, &material, &components, id)?;
    add_grid_shells(&mut out, &face_components, &volumes, component_count)?;
    finish_grid_result(out, &a, &b, operation, contacts, coincident)
}

fn deduplicate_grid(
    grid: &mut [Vec<f64>; 3],
    a: &BoxInput<'_>,
    b: &BoxInput<'_>,
    accuracy: Accuracy,
) -> Result<(), GeometryError> {
    let magnitude = [a.frame.origin, b.frame.origin]
        .into_iter()
        .map(norm)
        .fold(0.0_f64, f64::max)
        .max(a.size.into_iter().fold(0.0_f64, f64::max))
        .max(b.size.into_iter().fold(0.0_f64, f64::max));
    let noise = (256.0 * f64::EPSILON * magnitude).min(accuracy.intersection / 8.0);
    for values in grid {
        values.sort_by(f64::total_cmp);
        let mut unique: Vec<f64> = Vec::new();
        for &value in values.iter() {
            if let Some(last) = unique.last() {
                let gap = value - last;
                if gap <= noise {
                    continue;
                }
                if gap <= 4.0 * accuracy.geometric {
                    return Err(GeometryError::UnresolvedIntersection(
                        "cuboid arrangement contains sub-tolerance features".into(),
                    ));
                }
            }
            unique.push(value);
        }
        *values = unique;
    }
    Ok(())
}

fn snapped_point(grid: &[Vec<f64>; 3], p: Point3) -> Point3 {
    std::array::from_fn(|i| {
        grid[i]
            .iter()
            .copied()
            .min_by(|x, y| (x - p[i]).abs().total_cmp(&(y - p[i]).abs()))
            .unwrap_or(p[i])
    })
}

fn contact_points(a: &BoxInput<'_>, overlap_lo: Point3, overlap_hi: Point3) -> Vec<Point3> {
    let mut contacts = Vec::new();
    if (0..3).all(|i| overlap_lo[i] <= overlap_hi[i])
        && (0..3).any(|i| overlap_lo[i] == overlap_hi[i])
    {
        for mask in 0..8 {
            let point = a.frame.point(std::array::from_fn(|i| {
                if mask & (1 << i) == 0 {
                    overlap_lo[i]
                } else {
                    overlap_hi[i]
                }
            }));
            if !contacts.contains(&point) {
                contacts.push(point);
            }
        }
    }
    contacts
}

fn add_grid_shells(
    out: &mut BrepEnvelope,
    face_components: &[usize],
    volumes: &[f64],
    component_count: usize,
) -> Result<(), GeometryError> {
    let mut assigned = vec![false; out.topology.faces.len()];
    let mut outer = vec![None; component_count];
    let mut cavities = vec![Vec::new(); component_count];
    for start in 0..out.topology.faces.len() {
        if assigned[start] {
            continue;
        }
        let shell = out.topology.shells.len() as u32;
        let component = face_components[start];
        let mut faces = Vec::new();
        let mut volume = 0.0;
        assigned[start] = true;
        let mut queue = VecDeque::from([start]);
        while let Some(face) = queue.pop_front() {
            volume += volumes[face];
            faces.push(face as u32);
            out.topology.faces[face].shell_ref = Some(shell);
            for h in out
                .topology
                .halfedges
                .iter()
                .filter(|h| h.face == Some(face as u32))
            {
                let twin = h.twin.ok_or_else(|| {
                    GeometryError::InvalidTopology("open cuboid boolean boundary".into())
                })?;
                let adjacent = out.topology.halfedges[twin as usize].face.ok_or_else(|| {
                    GeometryError::InvalidTopology("cuboid twin has no face".into())
                })? as usize;
                if face_components[adjacent] != component {
                    return Err(GeometryError::InvalidTopology(
                        "cuboid contact incorrectly sewed separate material components".into(),
                    ));
                }
                if !assigned[adjacent] {
                    assigned[adjacent] = true;
                    queue.push_back(adjacent);
                }
            }
        }
        out.topology.shells.push(Shell {
            id: shell,
            faces,
            is_closed: true,
        });
        if volume > 0.0 {
            if outer[component].replace(shell).is_some() {
                return Err(GeometryError::UnresolvedIntersection(
                    "cuboid material component has multiple outer shells".into(),
                ));
            }
        } else if volume < 0.0 {
            cavities[component].push(shell);
        } else {
            return Err(GeometryError::UnresolvedIntersection(
                "cuboid shell volume is unresolved".into(),
            ));
        }
    }
    for (component, shell) in outer.into_iter().enumerate() {
        out.solids.push(SolidRegion {
            outer_shell: shell.ok_or_else(|| {
                GeometryError::InvalidTopology("cuboid material has no outer shell".into())
            })?,
            cavity_shells: cavities[component].clone(),
        });
    }
    Ok(())
}

fn finish_grid_result(
    mut out: BrepEnvelope,
    a: &BoxInput<'_>,
    b: &BoxInput<'_>,
    operation: BooleanOp,
    contacts: Vec<Point3>,
    coincident: bool,
) -> Result<BooleanResult, GeometryError> {
    out.revision = a
        .brep
        .revision
        .max(b.brep.revision)
        .checked_add(1)
        .ok_or_else(|| GeometryError::LimitExceeded("boolean revision overflow".into()))?;
    out.validate()?;
    let face_mappings = [a, b]
        .into_iter()
        .flat_map(|input| (0..input.brep.topology.faces.len()).map(move |face| source(input, face)))
        .map(|source| FaceMapping {
            result_faces: out
                .topology
                .faces
                .iter()
                .filter(|face| {
                    face.provenance.sources.iter().any(|s| {
                        s.entity == source.entity
                            && s.body == source.body
                            && s.key == source.key
                            && s.face == source.face
                    })
                })
                .map(|face| face.id)
                .collect(),
            source,
        })
        .collect();
    Ok(BooleanResult {
        report: BooleanReport {
            operation,
            quality: out.quality.clone(),
            contacts,
            coincident,
            face_mappings,
        },
        brep: out,
    })
}
