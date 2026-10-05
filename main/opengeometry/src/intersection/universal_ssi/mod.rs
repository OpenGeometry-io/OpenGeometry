mod branches;
mod cells;
mod correction;
mod implicit_bounds;
mod subdivision;
#[cfg(test)]
mod tests;

use super::ssi_result::SsiResult;
use crate::brep::{
    Accuracy, GeometryError, GeometryStore, PatchBounds, SsiBudget, Surface, SurfaceGeometry, UVBox,
};
use crate::math::{dot, find, norm, sub, union, Interval, Point3};
use branches::{branch_definition, normalized_key, segment_endpoint_tangent};
use cells::{cell_segments, stationary_event, Root, Segment, StationaryEvent};
use implicit_bounds::{implicit_interval, surface_scale};
use std::collections::{HashMap, HashSet};
use subdivision::{bounds_diameter, split};

pub(crate) fn intersect_patches(
    store: &mut GeometryStore,
    surfaces: [u32; 2],
    domains: [UVBox; 2],
    accuracy: Accuracy,
    budget: SsiBudget,
) -> Result<SsiResult, GeometryError> {
    intersect_patches_from(store, surfaces, domains, 0, accuracy, budget)
}

pub(super) fn intersect_patches_from(
    store: &mut GeometryStore,
    surfaces: [u32; 2],
    domains: [UVBox; 2],
    source_side: usize,
    accuracy: Accuracy,
    budget: SsiBudget,
) -> Result<SsiResult, GeometryError> {
    if source_side > 1 {
        return Err(GeometryError::InvalidGeometry(
            "universal SSI source side must be zero or one".into(),
        ));
    }
    let ordered_surfaces = if source_side == 0 {
        surfaces
    } else {
        [surfaces[1], surfaces[0]]
    };
    let ordered_domains = if source_side == 0 {
        domains
    } else {
        [domains[1], domains[0]]
    };
    accuracy.validate()?;
    budget.validate()?;
    check_patch_domains(ordered_domains)?;
    let source = store.surface(ordered_surfaces[0])?.clone();
    let target = store.surface(ordered_surfaces[1])?.clone();
    source.validate()?;
    target.validate()?;
    let source_bounds = source.enclose(ordered_domains[0])?;
    let target_bounds = target.enclose(ordered_domains[1])?;
    let scale = bounds_diameter(source_bounds)
        .max(surface_scale(&source))
        .max(surface_scale(&target))
        .max(accuracy.geometric);
    let target_cell = (scale / 64.0).max(16.0 * accuracy.geometric);
    let (segments, contacts) = patch_segments(
        &source,
        &target,
        ordered_domains,
        target_bounds,
        target_cell,
        accuracy,
        budget,
    )?;
    let mut result = SsiResult {
        contacts,
        ..SsiResult::default()
    };
    if segments.is_empty() {
        return Ok(result);
    }

    let unique = unique_segments(&source, segments, ordered_domains, accuracy, scale);
    let mut parent = stitched_endpoints(&source, &target, &unique, accuracy, target_cell)?;
    add_branch_curves(
        store,
        &unique,
        &mut parent,
        ordered_surfaces,
        accuracy,
        budget,
        &mut result,
    )?;
    if source_side == 1 {
        for curve in &mut result.curves {
            curve.pcurves.swap(0, 1);
        }
    }
    Ok(result)
}

fn check_patch_domains(ordered_domains: [UVBox; 2]) -> Result<(), GeometryError> {
    for domain in ordered_domains {
        for axis in domain {
            Interval::new(axis.lo, axis.hi)?;
            if axis.width() <= 0.0 {
                return Err(GeometryError::InvalidGeometry(
                    "universal SSI patch domain has no area".into(),
                ));
            }
        }
    }
    Ok(())
}

fn patch_segments(
    source: &SurfaceGeometry,
    target: &SurfaceGeometry,
    ordered_domains: [UVBox; 2],
    target_bounds: PatchBounds,
    target_cell: f64,
    accuracy: Accuracy,
    budget: SsiBudget,
) -> Result<(Vec<Segment>, Vec<Point3>), GeometryError> {
    let mut stack = vec![(ordered_domains[0], 0usize)];
    let mut visits = 0usize;
    let mut segments = Vec::new();
    let mut contacts = Vec::new();
    let mut unresolved = false;
    while let Some((domain, depth)) = stack.pop() {
        visits = visits
            .checked_add(1)
            .ok_or_else(|| GeometryError::LimitExceeded("SSI patch visits".into()))?;
        if visits > budget.max_patch_pairs {
            return Err(GeometryError::UnresolvedIntersection(
                "universal SSI patch-pair visit budget exhausted".into(),
            ));
        }
        let bounds = source.enclose(domain)?;
        if !bounds.overlaps(target_bounds, accuracy.intersection) {
            continue;
        }
        if !implicit_interval(target, bounds)?.contains(0.0) {
            continue;
        }
        if bounds_diameter(bounds) <= target_cell || depth == budget.max_subdivision_depth {
            let found = cell_segments(source, target, domain, accuracy.intersection)?;
            if found.is_empty() {
                match stationary_event(source, target, domain, accuracy.intersection)? {
                    Some(StationaryEvent::Contact(contact)) => {
                        if contacts
                            .iter()
                            .all(|existing| norm(sub(*existing, contact)) > accuracy.intersection)
                        {
                            contacts.push(contact);
                        }
                    }
                    Some(StationaryEvent::InteriorCritical)
                        if depth == budget.max_subdivision_depth =>
                    {
                        unresolved = true;
                    }
                    Some(StationaryEvent::InteriorCritical) => {
                        let axis = usize::from(domain[1].width() > domain[0].width());
                        let children = split(domain, axis)?;
                        stack.push((children[1], depth + 1));
                        stack.push((children[0], depth + 1));
                    }
                    None => {}
                }
            } else {
                segments.extend(found);
            }
            continue;
        }
        let axis = usize::from(domain[1].width() > domain[0].width());
        let children = split(domain, axis)?;
        stack.push((children[1], depth + 1));
        stack.push((children[0], depth + 1));
    }
    if unresolved {
        return Err(GeometryError::UnresolvedIntersection(
            "universal SSI could not certify an interval cell containing a possible tangency or sub-cell branch".into(),
        ));
    }
    Ok((segments, contacts))
}

fn unique_segments(
    source: &SurfaceGeometry,
    segments: Vec<Segment>,
    ordered_domains: [UVBox; 2],
    accuracy: Accuracy,
    scale: f64,
) -> Vec<Segment> {
    let periods = source.charts()[0].periods;
    let quantum = [
        (ordered_domains[0][0].width() / 1_000_000.0).max(accuracy.geometric / scale),
        (ordered_domains[0][1].width() / 1_000_000.0).max(accuracy.geometric / scale),
    ];
    let mut unique = Vec::new();
    let mut segment_keys = HashSet::new();
    for segment in segments {
        let mut keys = segment
            .ends
            .map(|root| normalized_key(root.uv_a, ordered_domains[0], periods, quantum));
        if keys[0] == keys[1] {
            continue;
        }
        if keys[1] < keys[0] {
            keys.swap(0, 1);
        }
        if segment_keys.insert(keys) {
            unique.push(segment);
        }
    }
    unique
}

fn stitched_endpoints(
    source: &SurfaceGeometry,
    target: &SurfaceGeometry,
    unique: &[Segment],
    accuracy: Accuracy,
    target_cell: f64,
) -> Result<Vec<usize>, GeometryError> {
    let endpoint_count = unique.len() * 2;
    let mut parent = (0..endpoint_count).collect::<Vec<_>>();
    let stitch_tolerance =
        (4.0 * accuracy.geometric.max(accuracy.intersection)).max(target_cell * 0.01);
    let mut buckets: HashMap<[i64; 3], Vec<usize>> = HashMap::new();
    for endpoint in 0..endpoint_count {
        let root = unique[endpoint / 2].ends[endpoint % 2];
        let key = root
            .point
            .map(|coordinate| (coordinate / stitch_tolerance).floor() as i64);
        let tangent = segment_endpoint_tangent(source, target, unique[endpoint / 2], endpoint % 2)?;
        for x in (key[0] - 1)..=(key[0] + 1) {
            for y in (key[1] - 1)..=(key[1] + 1) {
                for z in (key[2] - 1)..=(key[2] + 1) {
                    for &candidate in buckets.get(&[x, y, z]).into_iter().flatten() {
                        let other = unique[candidate / 2].ends[candidate % 2];
                        if norm(sub(root.point, other.point)) > stitch_tolerance {
                            continue;
                        }
                        let other_tangent = segment_endpoint_tangent(
                            source,
                            target,
                            unique[candidate / 2],
                            candidate % 2,
                        )?;
                        if dot(tangent, other_tangent).abs() >= 0.99 {
                            union(&mut parent, endpoint, candidate);
                        }
                    }
                }
            }
        }
        buckets.entry(key).or_default().push(endpoint);
    }
    Ok(parent)
}

fn add_branch_curves(
    store: &mut GeometryStore,
    unique: &[Segment],
    parent: &mut [usize],
    ordered_surfaces: [u32; 2],
    accuracy: Accuracy,
    budget: SsiBudget,
    result: &mut SsiResult,
) -> Result<(), GeometryError> {
    let collapsed_segments = (0..unique.len())
        .map(|index| find(parent, index * 2) == find(parent, index * 2 + 1))
        .collect::<Vec<_>>();
    let mut adjacency: HashMap<usize, Vec<(usize, usize)>> = HashMap::new();
    for index in 0..unique.len() {
        if collapsed_segments[index] {
            continue;
        }
        for end in 0..2 {
            let node = find(parent, index * 2 + end);
            adjacency.entry(node).or_default().push((index, end));
        }
    }
    if adjacency.values().any(|edges| edges.len() > 2) {
        return Err(GeometryError::UnresolvedIntersection(
            "universal SSI found a singular branch junction".into(),
        ));
    }
    let mut used = collapsed_segments;
    for start in 0..unique.len() {
        if used[start] {
            continue;
        }
        let endpoint = (0..2)
            .find(|&end| {
                let node = find(parent, start * 2 + end);
                adjacency.get(&node).is_some_and(|edges| edges.len() == 1)
            })
            .unwrap_or(0);
        let (branch, source_domains) = branch_roots(
            unique, parent, &adjacency, &mut used, start, endpoint, budget,
        )?;
        if branch.len() >= 2 {
            result.curves.push(branch_definition(
                branch,
                source_domains,
                ordered_surfaces,
                store,
                accuracy,
            )?);
        }
    }
    Ok(())
}

fn branch_roots(
    unique: &[Segment],
    parent: &mut [usize],
    adjacency: &HashMap<usize, Vec<(usize, usize)>>,
    used: &mut [bool],
    start: usize,
    endpoint: usize,
    budget: SsiBudget,
) -> Result<(Vec<Root>, Vec<UVBox>), GeometryError> {
    let mut branch = Vec::new();
    let mut source_domains = Vec::new();
    let mut current = start;
    let mut from_end = endpoint;
    loop {
        if used[current] {
            break;
        }
        used[current] = true;
        let segment = unique[current];
        if branch.is_empty() {
            branch.push(segment.ends[from_end]);
        }
        let to_end = 1 - from_end;
        branch.push(segment.ends[to_end]);
        source_domains.push(segment.domain);
        if branch.len() > budget.max_steps_per_branch {
            return Err(GeometryError::UnresolvedIntersection(
                "universal SSI branch step budget exhausted".into(),
            ));
        }
        let node = find(parent, current * 2 + to_end);
        let Some(next) = adjacency
            .get(&node)
            .and_then(|edges| edges.iter().copied().find(|(index, _)| !used[*index]))
        else {
            break;
        };
        current = next.0;
        from_end = next.1;
    }
    Ok((branch, source_domains))
}
