use crate::brep::{
    unit, Accuracy, BrepEnvelope, Builder, CurveGeometry, FaceRole, Frame3, GeometryError,
    GeometryQuality, GeometryStore, SolidRegion, SurfaceGeometry,
};
use crate::intersection::intersect_surfaces;
use crate::math::{norm, sphere_sphere_relation, sub, Interval, Point3, Sign};
use crate::operations::modifying::boolean::assembly::{append_sphere, patch, source, SpherePatch};
use crate::operations::modifying::boolean::operands::{full_sphere, gap, SphereInput};
use crate::operations::modifying::boolean::types::{
    BooleanOp, BooleanReport, BooleanResult, FaceMapping,
};

pub fn boolean_spheres(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let (a, b) = sphere_operands(a, b)?;
    let accuracy = Accuracy::combined(a.brep.accuracy, b.brep.accuracy);
    let mut out = BrepEnvelope::new(id, accuracy)?;
    let delta = sub(b.frame.origin, a.frame.origin);
    let distance = norm(delta);
    let [outer, inner] =
        sphere_sphere_relation(a.frame.origin, a.radius, b.frame.origin, b.radius)?;
    let coincident = distance == 0.0 && a.radius == b.radius;
    let mut supports = sphere_supports(&a, &b);
    let intersections = intersect_surfaces(&mut supports, 0, 1, accuracy)?;
    let mut keep = |input: &SphereInput<'_>| -> Result<(), GeometryError> {
        let shell = append_sphere(&mut out, input, false)?;
        out.solids.push(SolidRegion {
            outer_shell: shell,
            cavity_shells: Vec::new(),
        });
        Ok(())
    };
    if coincident {
        if operation != BooleanOp::Subtraction {
            keep(&a)?;
            out.topology.faces[0].provenance.role = FaceRole::Coincident;
            out.topology.faces[0].provenance.sources.push(source(&b));
        }
    } else if outer != Sign::Negative {
        match operation {
            BooleanOp::Union => {
                keep(&a)?;
                keep(&b)?;
            }
            BooleanOp::Intersection => {}
            BooleanOp::Subtraction => keep(&a)?,
        }
    } else if inner != Sign::Positive {
        let a_outer = a.radius > b.radius;
        match operation {
            BooleanOp::Union => keep(if a_outer { &a } else { &b })?,
            BooleanOp::Intersection => keep(if a_outer { &b } else { &a })?,
            BooleanOp::Subtraction if a_outer => {
                if inner == Sign::Zero {
                    return Err(GeometryError::UnresolvedIntersection(
                        "internally tangent cavity boundaries require contact topology".into(),
                    ));
                }
                if a.radius - b.radius - distance <= 4.0 * accuracy.geometric {
                    return Err(GeometryError::UnresolvedIntersection(
                        "cavity clearance is below geometric resolution".into(),
                    ));
                }
                let shell = append_sphere(&mut out, &a, false)?;
                let cavity = append_sphere(&mut out, &b, true)?;
                out.solids.push(SolidRegion {
                    outer_shell: shell,
                    cavity_shells: vec![cavity],
                });
            }
            BooleanOp::Subtraction => {}
        }
    } else {
        let intersection = intersections.curves.first().ok_or_else(|| {
            GeometryError::UnresolvedIntersection(
                "transverse spheres have no intersection branch".into(),
            )
        })?;
        let circle = supports.curves[intersection.curve as usize].clone();
        out = transverse_caps(&out.id, &a, &b, operation, accuracy, delta, circle)?;
    }
    finish_sphere_sphere_result(out, &a, &b, operation, intersections.contacts, coincident)
}

fn sphere_operands<'a>(
    a: &'a BrepEnvelope,
    b: &'a BrepEnvelope,
) -> Result<(SphereInput<'a>, SphereInput<'a>), GeometryError> {
    a.validate()?;
    b.validate()?;
    let family = |brep: &BrepEnvelope| {
        match brep.geometry.surfaces.first() {
            Some(SurfaceGeometry::Sphere { .. }) => "sphere (canonical full sphere required)",
            Some(SurfaceGeometry::Cylinder { .. }) => "cylinder",
            Some(SurfaceGeometry::Cone { .. }) => "cone",
            Some(SurfaceGeometry::Torus { .. }) => "torus",
            Some(SurfaceGeometry::Plane { .. }) => "planar/compound solid",
            None => "empty solid",
        }
        .to_string()
    };
    let coverage = |error| match error {
        GeometryError::CoverageGap { .. } => GeometryError::CoverageGap {
            families: [family(a), family(b)],
        },
        other => other,
    };
    let a = full_sphere(a).map_err(coverage)?;
    let b = full_sphere(b).map_err(coverage)?;
    Ok((a, b))
}

fn sphere_supports(a: &SphereInput<'_>, b: &SphereInput<'_>) -> GeometryStore {
    let mut supports = GeometryStore::new();
    supports.surfaces = vec![
        SurfaceGeometry::Sphere {
            frame: a.frame,
            radius: a.radius,
        },
        SurfaceGeometry::Sphere {
            frame: b.frame,
            radius: b.radius,
        },
    ];
    supports
}

fn transverse_caps(
    id: &str,
    a: &SphereInput<'_>,
    b: &SphereInput<'_>,
    operation: BooleanOp,
    accuracy: Accuracy,
    delta: Point3,
    circle: CurveGeometry,
) -> Result<BrepEnvelope, GeometryError> {
    let (circle_frame, circle_radius) = match circle {
        CurveGeometry::Circle { frame, radius } => (frame, radius),
        _ => return Err(gap()),
    };
    let axis = unit(delta)?;
    let frame_a = Frame3 {
        origin: a.frame.origin,
        z: axis,
        x: circle_frame.x,
        y: circle_frame.y,
    };
    let frame_b = Frame3 {
        origin: b.frame.origin,
        ..frame_a
    };
    let latitudes = [(a, frame_a), (b, frame_b)].map(|(input, frame)| {
        let local = frame.local(circle_frame.origin);
        (local[2] / input.radius).asin()
    });
    if circle_radius <= 4.0 * accuracy.geometric || latitudes.iter().any(|v| !v.is_finite()) {
        return Err(GeometryError::UnresolvedIntersection(
            "intersection cap is below geometric resolution".into(),
        ));
    }
    let north_a = operation == BooleanOp::Intersection;
    let north_b = operation == BooleanOp::Union;
    check_retained_caps(a, b, latitudes, north_a, north_b, accuracy)?;
    let mut builder = Builder::new(id.to_owned(), accuracy)?;
    let vertex = builder.vertex(circle.elementary_point(0.0)?);
    let edge = builder.edge(circle, Interval::new(0.0, std::f64::consts::TAU)?, false);
    patch(
        &mut builder,
        a,
        &SpherePatch {
            frame: frame_a,
            latitude: latitudes[0],
            north: north_a,
            shared_edge: edge,
            shared_vertex: vertex,
        },
        false,
    )?;
    patch(
        &mut builder,
        b,
        &SpherePatch {
            frame: frame_b,
            latitude: latitudes[1],
            north: north_b,
            shared_edge: edge,
            shared_vertex: vertex,
        },
        operation == BooleanOp::Subtraction,
    )?;
    builder.finish_solid()
}

fn check_retained_caps(
    a: &SphereInput<'_>,
    b: &SphereInput<'_>,
    latitudes: [f64; 2],
    north_a: bool,
    north_b: bool,
    accuracy: Accuracy,
) -> Result<(), GeometryError> {
    for (input, latitude, north) in [(a, latitudes[0], north_a), (b, latitudes[1], north_b)] {
        let cap_height = input.radius
            * (1.0
                - if north {
                    latitude.sin()
                } else {
                    -latitude.sin()
                });
        if cap_height <= 4.0 * accuracy.geometric {
            return Err(GeometryError::UnresolvedIntersection(
                "retained spherical cap is below geometric resolution".into(),
            ));
        }
    }
    Ok(())
}

fn finish_sphere_sphere_result(
    mut out: BrepEnvelope,
    a: &SphereInput<'_>,
    b: &SphereInput<'_>,
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
    let report = BooleanReport {
        operation,
        quality: GeometryQuality::Analytic,
        contacts,
        coincident,
        face_mappings: [a, b]
            .map(|input| {
                let source = source(input);
                let result_faces = out
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
                    .collect();
                FaceMapping {
                    source,
                    result_faces,
                }
            })
            .into(),
    };
    Ok(BooleanResult { brep: out, report })
}
