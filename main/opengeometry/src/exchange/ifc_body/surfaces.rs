use super::entities::{enumeration, real, reference, references, Entities, IfcValue};
use crate::brep::{BrepEnvelope, Frame3, GeometryError, SurfaceGeometry};
use crate::math::scale;

pub(super) fn add_surfaces(
    entities: &mut Entities,
    brep: &BrepEnvelope,
) -> Result<Vec<Option<u32>>, GeometryError> {
    let mut surfaces = vec![None; brep.geometry.surfaces.len()];
    for face in &brep.topology.faces {
        if surfaces[face.surface as usize].is_some() {
            continue;
        }
        let surface = brep.geometry.surface(face.surface)?;
        let position = entities.placement(*surface.frame());
        let id = match surface {
            SurfaceGeometry::Plane { .. } => entities.add("IfcPlane", vec![reference(position)]),
            SurfaceGeometry::Sphere { radius, .. } => entities.add(
                "IfcSphericalSurface",
                vec![reference(position), real(*radius)],
            ),
            SurfaceGeometry::Cylinder { radius, .. } => entities.add(
                "IfcCylindricalSurface",
                vec![reference(position), real(*radius)],
            ),
            SurfaceGeometry::Torus {
                major_radius,
                minor_radius,
                ..
            } => entities.add(
                "IfcToroidalSurface",
                vec![
                    reference(position),
                    real(*major_radius),
                    real(*minor_radius),
                ],
            ),
            SurfaceGeometry::Cone { frame, semi_angle } => {
                let height = cone_height(brep, face.surface)?;
                add_cone(entities, *frame, *semi_angle, height)
            }
        };
        surfaces[face.surface as usize] = Some(id);
    }
    Ok(surfaces)
}

fn cone_height(brep: &BrepEnvelope, surface: u32) -> Result<f64, GeometryError> {
    let height = brep
        .topology
        .faces
        .iter()
        .filter(|face| face.surface == surface)
        .map(|face| face.trim.uv_bounds[1].hi)
        .fold(0.0_f64, f64::max);
    if !height.is_finite() || height <= 0.0 {
        return Err(GeometryError::InvalidGeometry(
            "IFC cone profile requires a positive finite height".into(),
        ));
    }
    Ok(height)
}

fn add_cone(entities: &mut Entities, frame: Frame3, semi_angle: f64, height: f64) -> u32 {
    let p0 = entities.point(&[0.0, 0.0]);
    let p1 = entities.point(&[height * semi_angle.tan(), height]);
    let line = entities.add("IfcPolyline", vec![references([p0, p1])]);
    let profile = entities.add(
        "IfcArbitraryOpenProfileDef",
        vec![enumeration("CURVE"), IfcValue::Null, reference(line)],
    );
    let profile_frame = Frame3 {
        y: frame.z,
        z: scale(frame.y, -1.0),
        ..frame
    };
    let profile_position = entities.placement(profile_frame);
    let origin = entities.point(&[0.0; 3]);
    let axis = entities.direction(&[0.0, 1.0, 0.0]);
    let axis = entities.add(
        "IfcAxis1Placement",
        vec![reference(origin), reference(axis)],
    );
    entities.add(
        "IfcSurfaceOfRevolution",
        vec![
            reference(profile),
            reference(profile_position),
            reference(axis),
        ],
    )
}
