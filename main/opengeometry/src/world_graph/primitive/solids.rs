use super::kind::Primitive;
use super::sizes::{
    check_arc, check_corner_radius, check_slot, parameter, required_size, wrong_body_type,
};
use crate::brep::{Accuracy, BrepEnvelope, Frame3, GeometryError, GROUND};
use crate::math::{scale, Point3};
use crate::primitives::{self, PolysolidError, PolysolidJustify, PolysolidPath, ProfileEdge};
use crate::world_graph::error::{ErrorCode, GraphError};

pub(super) fn build_solid(
    shape_id: String,
    primitive: Primitive,
    accuracy: Accuracy,
) -> Result<(BrepEnvelope, Vec<String>), GraphError> {
    let result = match primitive {
        Primitive::Cuboid {
            width,
            height,
            depth,
        } => cuboid_body(shape_id, [width, height, depth], accuracy)?,
        Primitive::Cylinder { radius, height } => {
            cylinder_body(shape_id, [radius, height], accuracy)?
        }
        Primitive::CylinderSector {
            radius,
            height,
            start_angle,
            sweep_angle,
        } => sector_body(
            shape_id,
            [radius, height],
            [start_angle, sweep_angle],
            accuracy,
        )?,
        Primitive::Polyhedron { points, faces } => {
            polyhedron_body(shape_id, &points, &faces, accuracy)?
        }
        Primitive::Sphere { radius } => sphere_body(shape_id, radius, accuracy)?,
        Primitive::Cone {
            radius_bottom,
            radius_top,
            height,
        } => cone_body(shape_id, [radius_bottom, radius_top], height, accuracy)?,
        Primitive::Torus {
            major_radius,
            minor_radius,
        } => torus_body(shape_id, [major_radius, minor_radius], accuracy)?,
        Primitive::SlotPrism {
            length,
            width,
            height,
        } => slot_body(shape_id, [length, width, height], accuracy)?,
        Primitive::RoundedBox {
            width,
            height,
            depth,
            radius,
        } => rounded_box_body(shape_id, [width, height, depth, radius], accuracy)?,
        Primitive::PipeSegment {
            outer_radius,
            inner_radius,
            height,
            start_angle,
            sweep_angle,
        } => pipe_body(
            shape_id,
            [outer_radius, inner_radius, height],
            [start_angle, sweep_angle],
            accuracy,
        )?,
        Primitive::Polysolid {
            path,
            width,
            height,
            justify,
        } => polysolid_body(shape_id, &path, justify, [width, height], accuracy)?,
        _ => return Err(wrong_body_type("solid")),
    };
    Ok(result)
}

fn cuboid_body(
    shape_id: String,
    [width, height, depth]: [f64; 3],
    accuracy: Accuracy,
) -> Result<(BrepEnvelope, Vec<String>), GraphError> {
    for (name, value) in [("width", width), ("height", height), ("depth", depth)] {
        required_size(value, accuracy.geometric, name)?;
    }
    let frame = Frame3 {
        origin: [-width / 2.0, 0.0, depth / 2.0],
        ..GROUND
    };
    Ok((
        primitives::cuboid(shape_id, frame, [width, depth, height], accuracy)?,
        Vec::new(),
    ))
}

fn cylinder_body(
    shape_id: String,
    [radius, height]: [f64; 2],
    accuracy: Accuracy,
) -> Result<(BrepEnvelope, Vec<String>), GraphError> {
    required_size(radius, accuracy.geometric, "radius")?;
    required_size(height, accuracy.geometric, "height")?;
    Ok((
        primitives::cylinder(shape_id, GROUND, radius, height, accuracy)?,
        Vec::new(),
    ))
}

fn polyhedron_body(
    shape_id: String,
    points: &[Point3],
    faces: &[Vec<u32>],
    accuracy: Accuracy,
) -> Result<(BrepEnvelope, Vec<String>), GraphError> {
    let brep =
        primitives::polyhedron(shape_id, points, faces, accuracy).map_err(|error| match error {
            GeometryError::InvalidGeometry(message)
            | GeometryError::UnresolvedIntersection(message) => {
                GraphError::code(ErrorCode::InvalidParameter, message)
            }
            GeometryError::Math(error) => GraphError::code(
                ErrorCode::InvalidParameter,
                format!("polyhedron points are out of range: {error:?}"),
            ),
            other => other.into(),
        })?;
    Ok((brep, Vec::new()))
}

fn sector_body(
    shape_id: String,
    [radius, height]: [f64; 2],
    angles: [f64; 2],
    accuracy: Accuracy,
) -> Result<(BrepEnvelope, Vec<String>), GraphError> {
    check_arc("sector", radius, angles, accuracy)?;
    required_size(height, accuracy.geometric, "height")?;
    let [start_angle, sweep_angle] = angles;
    Ok((
        primitives::cylinder_sector(
            shape_id,
            GROUND,
            radius,
            height,
            start_angle,
            sweep_angle,
            accuracy,
        )?,
        Vec::new(),
    ))
}

fn sphere_body(
    shape_id: String,
    radius: f64,
    accuracy: Accuracy,
) -> Result<(BrepEnvelope, Vec<String>), GraphError> {
    required_size(radius, accuracy.geometric, "radius")?;
    Ok((
        primitives::sphere(shape_id, GROUND, radius, accuracy)?,
        Vec::new(),
    ))
}

fn cone_body(
    shape_id: String,
    [bottom, top]: [f64; 2],
    height: f64,
    accuracy: Accuracy,
) -> Result<(BrepEnvelope, Vec<String>), GraphError> {
    required_size(height, accuracy.geometric, "height")?;
    for (name, radius) in [("radiusBottom", bottom), ("radiusTop", top)] {
        if radius != 0.0 {
            required_size(radius, accuracy.geometric, name)?;
        }
    }
    if bottom == 0.0 && top == 0.0 {
        return Err(parameter("cone needs at least one nonzero radius"));
    }
    let brep = if top == 0.0 {
        primitives::cone(shape_id, GROUND, bottom, height, accuracy)?
    } else if bottom == 0.0 {
        let inverted = Frame3 {
            origin: [0.0, height, 0.0],
            x: GROUND.x,
            y: scale(GROUND.y, -1.0),
            z: scale(GROUND.z, -1.0),
        };
        primitives::cone(shape_id, inverted, top, height, accuracy)?
    } else if (bottom - top).abs() <= 4.0 * accuracy.geometric {
        primitives::cylinder(shape_id, GROUND, bottom, height, accuracy)?
    } else {
        primitives::frustum(shape_id, GROUND, bottom, top, height, accuracy)?
    };
    Ok((brep, Vec::new()))
}

fn torus_body(
    shape_id: String,
    [major, minor]: [f64; 2],
    accuracy: Accuracy,
) -> Result<(BrepEnvelope, Vec<String>), GraphError> {
    required_size(major, accuracy.geometric, "majorRadius")?;
    required_size(minor, accuracy.geometric, "minorRadius")?;
    if major - minor <= 4.0 * accuracy.geometric {
        return Err(parameter(
            "torus minor radius must be below its major radius",
        ));
    }
    let frame = Frame3 {
        origin: [0.0, minor, 0.0],
        ..GROUND
    };
    Ok((
        primitives::torus(shape_id, frame, major, minor, accuracy)?,
        Vec::new(),
    ))
}

fn slot_body(
    shape_id: String,
    [length, width, height]: [f64; 3],
    accuracy: Accuracy,
) -> Result<(BrepEnvelope, Vec<String>), GraphError> {
    check_slot(length, width, accuracy.geometric)?;
    required_size(height, accuracy.geometric, "height")?;
    let outline = primitives::slot_outline(length, width);
    outline_body(shape_id, outline, height, accuracy)
}

fn rounded_box_body(
    shape_id: String,
    [width, height, depth, radius]: [f64; 4],
    accuracy: Accuracy,
) -> Result<(BrepEnvelope, Vec<String>), GraphError> {
    required_size(height, accuracy.geometric, "height")?;
    let sides = [("width", width), ("depth", depth)];
    check_corner_radius("rounded box", sides, radius, accuracy.geometric)?;
    let outline = primitives::rounded_rectangle_outline(width, depth, radius);
    outline_body(shape_id, outline, height, accuracy)
}

fn pipe_body(
    shape_id: String,
    [outer, inner, height]: [f64; 3],
    angles: [f64; 2],
    accuracy: Accuracy,
) -> Result<(BrepEnvelope, Vec<String>), GraphError> {
    for (name, value) in [
        ("outerRadius", outer),
        ("innerRadius", inner),
        ("height", height),
    ] {
        required_size(value, accuracy.geometric, name)?;
    }
    check_arc("pipe segment", inner, angles, accuracy)?;
    if outer - inner <= 4.0 * accuracy.geometric {
        return Err(parameter(
            "pipe segment inner radius must be below its outer radius",
        ));
    }
    let outline = primitives::ring_sector_outline([outer, inner], angles);
    outline_body(shape_id, outline, height, accuracy)
}

fn outline_body(
    shape_id: String,
    outline: Vec<ProfileEdge>,
    height: f64,
    accuracy: Accuracy,
) -> Result<(BrepEnvelope, Vec<String>), GraphError> {
    Ok((
        primitives::arc_edged_extrusion(shape_id, GROUND, outline, height, accuracy)?,
        Vec::new(),
    ))
}

fn polysolid_body(
    shape_id: String,
    path: &PolysolidPath,
    justify: PolysolidJustify,
    [width, height]: [f64; 2],
    accuracy: Accuracy,
) -> Result<(BrepEnvelope, Vec<String>), GraphError> {
    required_size(width, accuracy.geometric, "width")?;
    required_size(height, accuracy.geometric, "height")?;
    let brep = primitives::polysolid(shape_id, GROUND, path, justify, [width, height], accuracy)
        .map_err(|error| match error {
            PolysolidError::Parameter(message) => parameter(&message),
            PolysolidError::Geometry(error) => error.into(),
        })?;
    Ok((brep, Vec::new()))
}
