use crate::booleans::error::{BooleanError, BooleanErrorKind};
use crate::booleans::solid::{execute_polygon_boolean, Polygon3, Vec3f};
use crate::booleans::types::BooleanOperation;
use crate::brep::Brep;
use crate::operations::triangulate::triangulate_polygon_with_holes;
use openmaths::Vector3;

const CAP_ALIGNMENT_THRESHOLD: f64 = 0.999;

#[derive(Clone, Copy, Debug)]
pub(crate) struct PlanarContext {
    pub(crate) origin: Vec3f,
    pub(crate) normal: Vec3f,
}

/// Validates that a no-shell BRep is a coplanar planar face set.
pub(crate) fn planar_context_from_brep(
    brep: &Brep,
    tolerance: f64,
) -> Result<PlanarContext, BooleanError> {
    if brep.faces.is_empty() {
        return Err(BooleanError::new(
            BooleanErrorKind::UnsupportedOperandKind,
            "Planar booleans require at least one face",
        ));
    }

    if !brep.shells.is_empty() {
        return Err(BooleanError::new(
            BooleanErrorKind::UnsupportedOperandKind,
            "Planar booleans do not accept shell-backed operands",
        ));
    }

    let first_face = &brep.faces[0];
    let (outer, _) = brep.get_vertices_and_holes_by_face_id(first_face.id);
    if outer.len() < 3 {
        return Err(BooleanError::new(
            BooleanErrorKind::InvalidOperand,
            "Planar boolean operand has a degenerate outer loop",
        ));
    }

    let origin = Vec3f::from_vector3(&outer[0]);
    let normal = face_normal(outer.as_slice(), tolerance).ok_or_else(|| {
        BooleanError::new(
            BooleanErrorKind::InvalidOperand,
            "Unable to derive a valid plane from planar boolean operand",
        )
    })?;

    for face in &brep.faces {
        let (face_vertices, holes) = brep.get_vertices_and_holes_by_face_id(face.id);
        let local_normal = face_normal(face_vertices.as_slice(), tolerance).ok_or_else(|| {
            BooleanError::new(
                BooleanErrorKind::InvalidOperand,
                format!("Planar face {} is degenerate", face.id),
            )
        })?;

        if local_normal.dot(normal).abs() < CAP_ALIGNMENT_THRESHOLD {
            return Err(BooleanError::new(
                BooleanErrorKind::NonCoplanarPlanarOperands,
                "Planar boolean operands must remain on a single shared plane",
            ));
        }

        for point in face_vertices.iter().chain(holes.iter().flatten()) {
            let distance = Vec3f::from_vector3(point).sub(origin).dot(normal).abs();
            if distance > tolerance * 8.0 {
                return Err(BooleanError::new(
                    BooleanErrorKind::NonCoplanarPlanarOperands,
                    "Planar boolean operands must remain on a single shared plane",
                ));
            }
        }
    }

    Ok(PlanarContext { origin, normal })
}

/// Runs the planar boolean by extruding both coplanar operands into thin solids,
/// applying the shared solid CSG engine, and projecting the retained cap back to the plane.
pub(crate) fn execute_planar_boolean(
    lhs: &Brep,
    rhs: &Brep,
    operation: BooleanOperation,
    tolerance: f64,
) -> Result<(Vec<Polygon3>, usize), BooleanError> {
    let lhs_context = planar_context_from_brep(lhs, tolerance)?;
    let rhs_context = planar_context_from_brep(rhs, tolerance)?;

    if lhs_context.normal.dot(rhs_context.normal).abs() < CAP_ALIGNMENT_THRESHOLD {
        return Err(BooleanError::new(
            BooleanErrorKind::NonCoplanarPlanarOperands,
            "Planar boolean operands must be coplanar",
        ));
    }

    let rhs_plane_distance = rhs_context
        .origin
        .sub(lhs_context.origin)
        .dot(lhs_context.normal)
        .abs();
    if rhs_plane_distance > tolerance * 8.0 {
        return Err(BooleanError::new(
            BooleanErrorKind::NonCoplanarPlanarOperands,
            "Planar boolean operands must be coplanar",
        ));
    }

    let half_thickness = (tolerance * 50.0).max(1.0e-5);
    let mut triangle_count = 0;
    let lhs_polygons = thin_solid_polygons_from_planar_brep(
        lhs,
        lhs_context,
        half_thickness,
        tolerance,
        &mut triangle_count,
    )?;
    let rhs_polygons = thin_solid_polygons_from_planar_brep(
        rhs,
        PlanarContext {
            origin: lhs_context.origin,
            normal: if lhs_context.normal.dot(rhs_context.normal) < 0.0 {
                rhs_context.normal.scale(-1.0)
            } else {
                rhs_context.normal
            },
        },
        half_thickness,
        tolerance,
        &mut triangle_count,
    )?;

    let result = execute_polygon_boolean(lhs_polygons, rhs_polygons, operation, tolerance)?;
    let projected = extract_projected_cap_polygons(&result, lhs_context, half_thickness, tolerance);
    Ok((projected, triangle_count))
}

/// Extrudes each planar face into a thin watertight shell so the shared solid
/// boolean engine can process it.
fn thin_solid_polygons_from_planar_brep(
    brep: &Brep,
    context: PlanarContext,
    half_thickness: f64,
    tolerance: f64,
    triangle_count: &mut usize,
) -> Result<Vec<Polygon3>, BooleanError> {
    let mut polygons = Vec::new();

    for face in &brep.faces {
        let (outer, holes) = brep.get_vertices_and_holes_by_face_id(face.id);
        if outer.len() < 3 {
            continue;
        }

        let top_outer = offset_points(outer.as_slice(), context.normal, half_thickness);
        let bottom_outer = offset_points(outer.as_slice(), context.normal, -half_thickness);
        let top_holes: Vec<Vec<Vec3f>> = holes
            .iter()
            .map(|hole| offset_points(hole.as_slice(), context.normal, half_thickness))
            .collect();
        let bottom_holes: Vec<Vec<Vec3f>> = holes
            .iter()
            .map(|hole| offset_points(hole.as_slice(), context.normal, -half_thickness))
            .collect();

        let triangles = triangulate_polygon_with_holes(&outer, &holes);
        *triangle_count += triangles.len();

        let flat_top: Vec<Vec3f> = top_outer
            .iter()
            .copied()
            .chain(top_holes.iter().flatten().copied())
            .collect();
        let flat_bottom: Vec<Vec3f> = bottom_outer
            .iter()
            .copied()
            .chain(bottom_holes.iter().flatten().copied())
            .collect();

        for triangle in &triangles {
            add_oriented_polygon(
                &mut polygons,
                vec![
                    flat_top[triangle[0]],
                    flat_top[triangle[1]],
                    flat_top[triangle[2]],
                ],
                context.normal,
                tolerance,
            )?;
            add_oriented_polygon(
                &mut polygons,
                vec![
                    flat_bottom[triangle[0]],
                    flat_bottom[triangle[1]],
                    flat_bottom[triangle[2]],
                ],
                context.normal.scale(-1.0),
                tolerance,
            )?;
        }

        add_side_polygons(
            &mut polygons,
            bottom_outer.as_slice(),
            top_outer.as_slice(),
            context.normal,
            false,
            tolerance,
        )?;

        for (bottom_hole, top_hole) in bottom_holes.iter().zip(top_holes.iter()) {
            add_side_polygons(
                &mut polygons,
                bottom_hole.as_slice(),
                top_hole.as_slice(),
                context.normal,
                true,
                tolerance,
            )?;
        }
    }

    Ok(polygons)
}

/// Pulls the retained cap polygons back onto the original construction plane.
fn extract_projected_cap_polygons(
    polygons: &[Polygon3],
    context: PlanarContext,
    half_thickness: f64,
    tolerance: f64,
) -> Vec<Polygon3> {
    let top_origin = context.origin.add(context.normal.scale(half_thickness));
    let mut projected = collect_cap_polygons(
        polygons,
        context.normal,
        top_origin,
        context.normal.scale(half_thickness),
        context.normal,
        tolerance,
    );

    if !projected.is_empty() {
        return projected;
    }

    let bottom_origin = context.origin.add(context.normal.scale(-half_thickness));
    projected = collect_cap_polygons(
        polygons,
        context.normal.scale(-1.0),
        bottom_origin,
        context.normal.scale(-half_thickness),
        context.normal,
        tolerance,
    );

    projected
}

/// Filters polygons that lie on a chosen cap plane and projects them back to the
/// original planar operand space.
fn collect_cap_polygons(
    polygons: &[Polygon3],
    target_normal: Vec3f,
    target_origin: Vec3f,
    offset: Vec3f,
    final_normal: Vec3f,
    tolerance: f64,
) -> Vec<Polygon3> {
    let mut projected = Vec::new();

    for polygon in polygons {
        if polygon.plane.normal.dot(target_normal) < CAP_ALIGNMENT_THRESHOLD {
            continue;
        }

        let on_target_plane = polygon.vertices.iter().all(|vertex| {
            vertex.position.sub(target_origin).dot(target_normal).abs() <= tolerance * 12.0
        });
        if !on_target_plane {
            continue;
        }

        let positions = polygon
            .vertices
            .iter()
            .map(|vertex| vertex.position.sub(offset))
            .collect::<Vec<_>>();
        let Some(mut projected_polygon) = Polygon3::from_positions(positions, tolerance) else {
            continue;
        };

        if projected_polygon.plane.normal.dot(final_normal) < 0.0 {
            projected_polygon = projected_polygon.flipped();
        }
        projected.push(projected_polygon);
    }

    projected
}

/// Builds the vertical side quads that close each extruded ring.
fn add_side_polygons(
    output: &mut Vec<Polygon3>,
    bottom_ring: &[Vec3f],
    top_ring: &[Vec3f],
    normal: Vec3f,
    is_hole: bool,
    tolerance: f64,
) -> Result<(), BooleanError> {
    for index in 0..bottom_ring.len() {
        let next = (index + 1) % bottom_ring.len();
        let edge = top_ring[next].sub(top_ring[index]);
        let desired = if is_hole {
            edge.cross(normal)
        } else {
            normal.cross(edge)
        };

        add_oriented_polygon(
            output,
            vec![
                bottom_ring[index],
                bottom_ring[next],
                top_ring[next],
                top_ring[index],
            ],
            desired,
            tolerance,
        )?;
    }

    Ok(())
}

/// Inserts a polygon after aligning its winding with the requested normal.
fn add_oriented_polygon(
    output: &mut Vec<Polygon3>,
    positions: Vec<Vec3f>,
    desired_normal: Vec3f,
    tolerance: f64,
) -> Result<(), BooleanError> {
    let Some(mut polygon) = Polygon3::from_positions(positions, tolerance) else {
        return Ok(());
    };

    if desired_normal.norm_sq() > 0.0 && polygon.plane.normal.dot(desired_normal) < 0.0 {
        polygon = polygon.flipped();
    }

    output.push(polygon);
    Ok(())
}

/// Offsets a vertex ring along the plane normal to create the thin-solid caps.
fn offset_points(points: &[Vector3], normal: Vec3f, distance: f64) -> Vec<Vec3f> {
    points
        .iter()
        .map(|point| Vec3f::from_vector3(point).add(normal.scale(distance)))
        .collect()
}

/// Derives a stable face normal from the first non-degenerate triangle in the loop.
fn face_normal(vertices: &[Vector3], tolerance: f64) -> Option<Vec3f> {
    if vertices.len() < 3 {
        return None;
    }

    let origin = Vec3f::from_vector3(&vertices[0]);
    for index in 1..(vertices.len() - 1) {
        let a = Vec3f::from_vector3(&vertices[index]).sub(origin);
        let b = Vec3f::from_vector3(&vertices[index + 1]).sub(origin);
        let normal = a.cross(b).normalized(tolerance);
        if normal.is_some() {
            return normal;
        }
    }

    None
}
