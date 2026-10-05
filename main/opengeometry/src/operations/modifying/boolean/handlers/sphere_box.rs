use super::containment::analytic_containment_boolean;
use crate::brep::{
    boundary, plane_boundary, Accuracy, BrepEnvelope, Builder, CurveGeometry, FaceRole, Frame3,
    GeometryError, Orientation, PcurveGeometry, SurfaceGeometry, Use,
};
use crate::math::{norm, scale, sub, Interval, Point3};
use crate::operations::modifying::boolean::assembly::{
    add_box_faces, box_source, face_provenance, finish_analytic_result, patch, SpherePatch,
};
use crate::operations::modifying::boolean::operands::{
    full_box, full_sphere, BoxInput, SphereInput,
};
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};
use crate::primitives::add_box_corners;

struct CrossedFace {
    axis: usize,
    upper: bool,
    signed_plane: f64,
}

struct CapCircle {
    cap_frame: Frame3,
    latitude: f64,
    circle_frame: Frame3,
    circle_radius: f64,
    entry_face: usize,
}

pub(crate) fn sphere_box_boolean(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let (sphere, box_, sphere_is_a) = if matches!(
        a.geometry.surfaces.first(),
        Some(SurfaceGeometry::Sphere { .. })
    ) {
        (full_sphere(a)?, full_box(b)?, true)
    } else {
        (full_sphere(b)?, full_box(a)?, false)
    };
    let clearance = 4.0
        * sphere
            .brep
            .accuracy
            .geometric
            .max(box_.brep.accuracy.geometric);
    let center = box_.frame.local(sphere.frame.origin);
    let sphere_margin = (0..3).fold(f64::INFINITY, |margin, axis| {
        margin
            .min(center[axis] - sphere.radius)
            .min(box_.size[axis] - center[axis] - sphere.radius)
    });
    let mut farthest: f64 = 0.0;
    for corner in 0..8 {
        let point = box_.frame.point(std::array::from_fn(|axis| {
            if corner & (1 << axis) == 0 {
                0.0
            } else {
                box_.size[axis]
            }
        }));
        farthest = farthest.max(norm(sub(point, sphere.frame.origin)));
    }
    let box_margin = sphere.radius - farthest;
    let sphere_inside_box = sphere_margin > clearance;
    let box_inside_sphere = box_margin > clearance;
    if sphere_inside_box || box_inside_sphere {
        let b_inside_a = if sphere_is_a {
            box_inside_sphere
        } else {
            sphere_inside_box
        };
        return analytic_containment_boolean(a, b, b_inside_a, operation, id);
    }
    if sphere_margin.abs() <= clearance || box_margin.abs() <= clearance {
        return Err(GeometryError::UnresolvedIntersection(
            "sphere/cuboid containment boundary is below geometric resolution".into(),
        ));
    }
    single_face_sphere_box_boolean(&sphere, &box_, sphere_is_a, operation, id, clearance)
}

fn single_face_sphere_box_boolean(
    sphere: &SphereInput<'_>,
    box_: &BoxInput<'_>,
    sphere_is_a: bool,
    operation: BooleanOp,
    id: String,
    clearance: f64,
) -> Result<BooleanResult, GeometryError> {
    let accuracy = Accuracy::combined(sphere.brep.accuracy, box_.brep.accuracy);
    let center = box_.frame.local(sphere.frame.origin);
    let box_axes = [box_.frame.x, box_.frame.y, box_.frame.z];
    let CrossedFace {
        axis,
        upper,
        signed_plane,
    } = crossed_face_plane(sphere, box_, center, clearance)?;
    let cap = cap_circle(sphere, box_axes, axis, upper, signed_plane, clearance)?;
    let mut builder = Builder::new(id, accuracy)?;
    let circle_vertex = builder.vertex(cap.circle_frame.point([cap.circle_radius, 0.0, 0.0]));
    let circle_edge = builder.edge(
        CurveGeometry::Circle {
            frame: cap.circle_frame,
            radius: cap.circle_radius,
        },
        Interval::new(0.0, std::f64::consts::TAU)?,
        false,
    );

    if operation == BooleanOp::Intersection || (operation == BooleanOp::Subtraction && sphere_is_a)
    {
        add_closed_sphere_cap(
            &mut builder,
            sphere,
            box_,
            operation,
            &cap,
            (circle_vertex, circle_edge),
        )?;
    } else {
        let corners = add_box_corners(&mut builder, box_.frame, box_.size, box_axes)?;
        add_box_faces(
            &mut builder,
            box_,
            &corners,
            accuracy,
            |builder, face_index, face_frame| {
                entry_hole(
                    builder,
                    face_frame,
                    face_index == cap.entry_face,
                    (circle_vertex, circle_edge),
                )
            },
            |face_index| face_index == cap.entry_face,
        )?;
        patch(
            &mut builder,
            sphere,
            &SpherePatch {
                frame: cap.cap_frame,
                latitude: cap.latitude,
                north: operation == BooleanOp::Subtraction,
                shared_edge: circle_edge,
                shared_vertex: circle_vertex,
            },
            operation == BooleanOp::Subtraction,
        )?;
    }

    let out = builder.finish_solid()?;
    finish_analytic_result(out, sphere.brep, box_.brep, sphere_is_a, operation, false)
}

fn crossed_face_plane(
    sphere: &SphereInput<'_>,
    box_: &BoxInput<'_>,
    center: Point3,
    clearance: f64,
) -> Result<CrossedFace, GeometryError> {
    let mut crossings = Vec::new();
    for axis in 0..3 {
        for upper in [false, true] {
            let signed_plane = if upper {
                center[axis] - box_.size[axis]
            } else {
                -center[axis]
            };
            let separation = signed_plane.abs() - sphere.radius;
            if separation.abs() <= clearance {
                return Err(GeometryError::UnresolvedIntersection(
                    "sphere is tangent to a cuboid face within geometric resolution".into(),
                ));
            }
            if separation < 0.0 {
                crossings.push((axis, upper, signed_plane));
            } else if signed_plane > sphere.radius {
                return Err(GeometryError::CoverageGap {
                    families: ["sphere".into(), "cuboid".into()],
                });
            }
        }
    }
    let [(axis, upper, signed_plane)] = crossings.as_slice() else {
        return Err(GeometryError::CoverageGap {
            families: ["sphere".into(), "multi-face cuboid intersection".into()],
        });
    };
    Ok(CrossedFace {
        axis: *axis,
        upper: *upper,
        signed_plane: *signed_plane,
    })
}

fn cap_circle(
    sphere: &SphereInput<'_>,
    box_axes: [Point3; 3],
    axis: usize,
    upper: bool,
    signed_plane: f64,
    clearance: f64,
) -> Result<CapCircle, GeometryError> {
    let inward = if upper {
        scale(box_axes[axis], -1.0)
    } else {
        box_axes[axis]
    };
    let tangent_axis = match axis {
        0 => box_axes[1],
        _ => box_axes[0],
    };
    let cap_frame = Frame3::from_axis(sphere.frame.origin, inward, tangent_axis)?;
    let latitude = (signed_plane / sphere.radius).asin();
    let circle_radius =
        ((sphere.radius - signed_plane.abs()) * (sphere.radius + signed_plane.abs())).sqrt();
    if circle_radius <= clearance {
        return Err(GeometryError::UnresolvedIntersection(
            "sphere/cuboid intersection circle is below geometric resolution".into(),
        ));
    }
    let circle_frame = Frame3 {
        origin: cap_frame.point([0.0, 0.0, signed_plane]),
        ..cap_frame
    };
    let entry_face = match axis {
        0 => 2 + usize::from(upper),
        1 => 4 + usize::from(upper),
        _ => usize::from(upper),
    };
    Ok(CapCircle {
        cap_frame,
        latitude,
        circle_frame,
        circle_radius,
        entry_face,
    })
}

fn add_closed_sphere_cap(
    builder: &mut Builder,
    sphere: &SphereInput<'_>,
    box_: &BoxInput<'_>,
    operation: BooleanOp,
    cap: &CapCircle,
    circle: (u32, u32),
) -> Result<(), GeometryError> {
    let (circle_vertex, circle_edge) = circle;
    let keep_inside_box = operation == BooleanOp::Intersection;
    let disk_frame = if keep_inside_box {
        Frame3 {
            x: cap.circle_frame.x,
            y: scale(cap.circle_frame.y, -1.0),
            z: scale(cap.circle_frame.z, -1.0),
            ..cap.circle_frame
        }
    } else {
        cap.circle_frame
    };
    builder.face(
        "cuboid:cap",
        SurfaceGeometry::Plane { frame: disk_frame },
        [[-cap.circle_radius, cap.circle_radius]; 2],
        vec![boundary(
            circle_edge,
            circle_vertex,
            circle_vertex,
            if keep_inside_box {
                Orientation::Reverse
            } else {
                Orientation::Forward
            },
            PcurveGeometry::Conic2 {
                origin: [0.0; 2],
                axis_a: [cap.circle_radius, 0.0],
                axis_b: [
                    0.0,
                    if keep_inside_box {
                        -cap.circle_radius
                    } else {
                        cap.circle_radius
                    },
                ],
            },
        )],
    )?;
    builder.brep.topology.faces[0].provenance = face_provenance(
        vec![box_source(box_, cap.entry_face)],
        if keep_inside_box {
            FaceRole::Split
        } else {
            FaceRole::Cut
        },
        !keep_inside_box,
    );
    patch(
        builder,
        sphere,
        &SpherePatch {
            frame: cap.cap_frame,
            latitude: cap.latitude,
            north: keep_inside_box,
            shared_edge: circle_edge,
            shared_vertex: circle_vertex,
        },
        false,
    )
}

fn entry_hole(
    builder: &Builder,
    face_frame: Frame3,
    is_entry_face: bool,
    circle: (u32, u32),
) -> Result<Vec<Vec<Use>>, GeometryError> {
    let (circle_vertex, circle_edge) = circle;
    Ok(if is_entry_face {
        vec![vec![plane_boundary(
            builder,
            face_frame,
            circle_edge,
            circle_vertex,
            circle_vertex,
            Orientation::Forward,
        )?]]
    } else {
        Vec::new()
    })
}
