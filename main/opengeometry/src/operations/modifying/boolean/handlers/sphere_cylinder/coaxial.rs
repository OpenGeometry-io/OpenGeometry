use super::faces::{sphere_band, SphereBand};
use crate::brep::{Accuracy, BrepEnvelope, Builder, FaceRole, FaceSource, Frame3, GeometryError};
use crate::math::Point3;
use crate::operations::modifying::boolean::assembly::{
    append_analytic_input, circular_boundary, cylinder_band, cylinder_cap, cylinder_source,
    face_provenance, finish_analytic_result, patch, CylinderBand, SpherePatch,
};
use crate::operations::modifying::boolean::operands::{CylinderInput, SphereInput};
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};

struct CoaxialCut<'a> {
    sphere: &'a SphereInput<'a>,
    cylinder: &'a CylinderInput<'a>,
    sphere_frame: Frame3,
    z0: f64,
    z1: f64,
    latitude0: f64,
    latitude1: f64,
}

pub(super) fn coaxial_sphere_cylinder_boolean(
    sphere: &SphereInput<'_>,
    cylinder: &CylinderInput<'_>,
    sphere_is_a: bool,
    operation: BooleanOp,
    id: String,
    clearance: f64,
) -> Result<BooleanResult, GeometryError> {
    let local = cylinder.frame.local(sphere.frame.origin);
    check_coaxial_fit(sphere, cylinder, local, clearance)?;

    let accuracy = Accuracy::combined(sphere.brep.accuracy, cylinder.brep.accuracy);
    let half = ((sphere.radius - cylinder.radius) * (sphere.radius + cylinder.radius)).sqrt();
    let z0 = local[2] - half;
    let z1 = local[2] + half;
    let latitude0 = (-half / sphere.radius).asin();
    let latitude1 = (half / sphere.radius).asin();
    let sphere_frame = Frame3 {
        origin: sphere.frame.origin,
        x: cylinder.frame.x,
        y: cylinder.frame.y,
        z: cylinder.frame.z,
    };
    let cylinder_lateral = cylinder_source(cylinder, 0);
    let cylinder_caps = [cylinder_source(cylinder, 1), cylinder_source(cylinder, 2)];
    let cut = CoaxialCut {
        sphere,
        cylinder,
        sphere_frame,
        z0,
        z1,
        latitude0,
        latitude1,
    };

    if operation == BooleanOp::Subtraction && !sphere_is_a {
        let out = cylinder_minus_sphere(&cut, id, accuracy, &cylinder_lateral, &cylinder_caps)?;
        return finish_analytic_result(
            out,
            sphere.brep,
            cylinder.brep,
            sphere_is_a,
            operation,
            false,
        );
    }

    let mut builder = Builder::new(id, accuracy)?;
    let low = circular_boundary(&mut builder, cylinder.frame, cylinder.radius, z0)?;
    let high = circular_boundary(&mut builder, cylinder.frame, cylinder.radius, z1)?;
    match operation {
        BooleanOp::Intersection => {
            add_intersection_faces(&mut builder, &cut, low, high, cylinder_lateral)?;
        }
        BooleanOp::Subtraction => {
            add_sphere_minus_cylinder_faces(&mut builder, &cut, low, high, cylinder_lateral)?;
        }
        BooleanOp::Union => {
            add_union_faces(
                &mut builder,
                &cut,
                low,
                high,
                cylinder_lateral,
                &cylinder_caps,
            )?;
        }
    }
    finish_analytic_result(
        builder.finish_solid()?,
        sphere.brep,
        cylinder.brep,
        sphere_is_a,
        operation,
        false,
    )
}

fn check_coaxial_fit(
    sphere: &SphereInput<'_>,
    cylinder: &CylinderInput<'_>,
    local: Point3,
    clearance: f64,
) -> Result<(), GeometryError> {
    let radial = local[0].hypot(local[1]);
    let scale = sphere
        .radius
        .max(cylinder.radius)
        .max(cylinder.height)
        .max(1.0);
    let roundoff = 128.0 * f64::EPSILON * scale;
    if radial > roundoff {
        return Err(if radial <= clearance {
            GeometryError::UnresolvedIntersection(
                "sphere/cylinder axes differ below geometric resolution".into(),
            )
        } else {
            GeometryError::CoverageGap {
                families: ["sphere".into(), "noncoaxial cylinder".into()],
            }
        });
    }
    let radius_gap = sphere.radius - cylinder.radius;
    if radius_gap <= clearance {
        return Err(if radius_gap.abs() <= clearance {
            GeometryError::UnresolvedIntersection(
                "sphere/cylinder tangency is below geometric resolution".into(),
            )
        } else {
            GeometryError::CoverageGap {
                families: ["sphere".into(), "cylinder".into()],
            }
        });
    }
    let sphere_low = local[2] - sphere.radius;
    let sphere_high = local[2] + sphere.radius;
    let lower_clearance = sphere_low;
    let upper_clearance = cylinder.height - sphere_high;
    if lower_clearance <= clearance || upper_clearance <= clearance {
        let near = lower_clearance.abs() <= clearance || upper_clearance.abs() <= clearance;
        return Err(if near {
            GeometryError::UnresolvedIntersection(
                "sphere/cylinder cap event is below geometric resolution".into(),
            )
        } else {
            GeometryError::CoverageGap {
                families: ["sphere".into(), "finite cylinder cap".into()],
            }
        });
    }
    Ok(())
}

fn cylinder_minus_sphere(
    cut: &CoaxialCut<'_>,
    id: String,
    accuracy: Accuracy,
    cylinder_lateral: &FaceSource,
    cylinder_caps: &[FaceSource; 2],
) -> Result<BrepEnvelope, GeometryError> {
    let mut out = BrepEnvelope::new(id, accuracy)?;
    for north in [false, true] {
        let mut builder = Builder::new(
            format!("{}:{}", out.id, if north { "upper" } else { "lower" }),
            accuracy,
        )?;
        let intersection = circular_boundary(
            &mut builder,
            cut.cylinder.frame,
            cut.cylinder.radius,
            if north { cut.z1 } else { cut.z0 },
        )?;
        let end = circular_boundary(
            &mut builder,
            cut.cylinder.frame,
            cut.cylinder.radius,
            if north { cut.cylinder.height } else { 0.0 },
        )?;
        if north {
            add_upper_piece_faces(
                &mut builder,
                cut,
                intersection,
                end,
                cylinder_lateral,
                cylinder_caps,
            )?;
        } else {
            add_lower_piece_faces(
                &mut builder,
                cut,
                intersection,
                end,
                cylinder_lateral,
                cylinder_caps,
            )?;
        }
        let part = builder.finish_solid()?;
        let provenances: Vec<_> = part
            .topology
            .faces
            .iter()
            .map(|face| face.provenance.clone())
            .collect();
        let face_offset = out.topology.faces.len();
        append_analytic_input(&mut out, &part)?;
        for (face, provenance) in out.topology.faces[face_offset..]
            .iter_mut()
            .zip(provenances)
        {
            face.provenance = provenance;
        }
    }
    Ok(out)
}

fn add_upper_piece_faces(
    builder: &mut Builder,
    cut: &CoaxialCut<'_>,
    intersection: (u32, u32),
    end: (u32, u32),
    cylinder_lateral: &FaceSource,
    cylinder_caps: &[FaceSource; 2],
) -> Result<(), GeometryError> {
    cylinder_band(
        builder,
        &CylinderBand {
            frame: cut.cylinder.frame,
            radius: cut.cylinder.radius,
            z0: cut.z1,
            z1: cut.cylinder.height,
            low: intersection,
            high: end,
        },
        face_provenance(vec![cylinder_lateral.clone()], FaceRole::Split, false),
        false,
    )?;
    cylinder_cap(
        builder,
        cut.cylinder.frame,
        cut.cylinder.radius,
        cut.cylinder.height,
        end,
        true,
        face_provenance(vec![cylinder_caps[1].clone()], FaceRole::Preserved, false),
    )?;
    patch(
        builder,
        cut.sphere,
        &SpherePatch {
            frame: cut.sphere_frame,
            latitude: cut.latitude1,
            north: true,
            shared_edge: intersection.1,
            shared_vertex: intersection.0,
        },
        true,
    )
}

fn add_lower_piece_faces(
    builder: &mut Builder,
    cut: &CoaxialCut<'_>,
    intersection: (u32, u32),
    end: (u32, u32),
    cylinder_lateral: &FaceSource,
    cylinder_caps: &[FaceSource; 2],
) -> Result<(), GeometryError> {
    cylinder_band(
        builder,
        &CylinderBand {
            frame: cut.cylinder.frame,
            radius: cut.cylinder.radius,
            z0: 0.0,
            z1: cut.z0,
            low: end,
            high: intersection,
        },
        face_provenance(vec![cylinder_lateral.clone()], FaceRole::Split, false),
        false,
    )?;
    cylinder_cap(
        builder,
        cut.cylinder.frame,
        cut.cylinder.radius,
        0.0,
        end,
        false,
        face_provenance(vec![cylinder_caps[0].clone()], FaceRole::Preserved, false),
    )?;
    patch(
        builder,
        cut.sphere,
        &SpherePatch {
            frame: cut.sphere_frame,
            latitude: cut.latitude0,
            north: false,
            shared_edge: intersection.1,
            shared_vertex: intersection.0,
        },
        true,
    )
}

fn add_intersection_faces(
    builder: &mut Builder,
    cut: &CoaxialCut<'_>,
    low: (u32, u32),
    high: (u32, u32),
    cylinder_lateral: FaceSource,
) -> Result<(), GeometryError> {
    patch(
        builder,
        cut.sphere,
        &SpherePatch {
            frame: cut.sphere_frame,
            latitude: cut.latitude0,
            north: false,
            shared_edge: low.1,
            shared_vertex: low.0,
        },
        false,
    )?;
    cylinder_band(
        builder,
        &CylinderBand {
            frame: cut.cylinder.frame,
            radius: cut.cylinder.radius,
            z0: cut.z0,
            z1: cut.z1,
            low,
            high,
        },
        face_provenance(vec![cylinder_lateral], FaceRole::Split, false),
        false,
    )?;
    patch(
        builder,
        cut.sphere,
        &SpherePatch {
            frame: cut.sphere_frame,
            latitude: cut.latitude1,
            north: true,
            shared_edge: high.1,
            shared_vertex: high.0,
        },
        false,
    )
}

fn add_sphere_minus_cylinder_faces(
    builder: &mut Builder,
    cut: &CoaxialCut<'_>,
    low: (u32, u32),
    high: (u32, u32),
    cylinder_lateral: FaceSource,
) -> Result<(), GeometryError> {
    sphere_band(
        builder,
        cut.sphere,
        &SphereBand {
            frame: cut.sphere_frame,
            latitude0: cut.latitude0,
            latitude1: cut.latitude1,
            low,
            high,
        },
        false,
    )?;
    cylinder_band(
        builder,
        &CylinderBand {
            frame: cut.cylinder.frame,
            radius: cut.cylinder.radius,
            z0: cut.z0,
            z1: cut.z1,
            low,
            high,
        },
        face_provenance(vec![cylinder_lateral], FaceRole::Cut, true),
        true,
    )
}

fn add_union_faces(
    builder: &mut Builder,
    cut: &CoaxialCut<'_>,
    low: (u32, u32),
    high: (u32, u32),
    cylinder_lateral: FaceSource,
    cylinder_caps: &[FaceSource; 2],
) -> Result<(), GeometryError> {
    let bottom = circular_boundary(builder, cut.cylinder.frame, cut.cylinder.radius, 0.0)?;
    let top = circular_boundary(
        builder,
        cut.cylinder.frame,
        cut.cylinder.radius,
        cut.cylinder.height,
    )?;
    cylinder_band(
        builder,
        &CylinderBand {
            frame: cut.cylinder.frame,
            radius: cut.cylinder.radius,
            z0: 0.0,
            z1: cut.z0,
            low: bottom,
            high: low,
        },
        face_provenance(vec![cylinder_lateral.clone()], FaceRole::Split, false),
        false,
    )?;
    cylinder_cap(
        builder,
        cut.cylinder.frame,
        cut.cylinder.radius,
        0.0,
        bottom,
        false,
        face_provenance(vec![cylinder_caps[0].clone()], FaceRole::Preserved, false),
    )?;
    sphere_band(
        builder,
        cut.sphere,
        &SphereBand {
            frame: cut.sphere_frame,
            latitude0: cut.latitude0,
            latitude1: cut.latitude1,
            low,
            high,
        },
        false,
    )?;
    cylinder_band(
        builder,
        &CylinderBand {
            frame: cut.cylinder.frame,
            radius: cut.cylinder.radius,
            z0: cut.z1,
            z1: cut.cylinder.height,
            low: high,
            high: top,
        },
        face_provenance(vec![cylinder_lateral], FaceRole::Split, false),
        false,
    )?;
    cylinder_cap(
        builder,
        cut.cylinder.frame,
        cut.cylinder.radius,
        cut.cylinder.height,
        top,
        true,
        face_provenance(vec![cylinder_caps[1].clone()], FaceRole::Preserved, false),
    )
}
