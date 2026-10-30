use super::arcs::{add_cylinder_arc_face, selected_cylinder_arc, ArcFaceBoundary, CylinderArc};
use super::contained::contained_cylinders;
use crate::brep::{
    padded_uv_bounds, plane_boundary, unit, Accuracy, BrepEnvelope, Builder, CurveGeometry,
    FaceRole, Frame3, GeometryError, Orientation, SurfaceGeometry, Use,
};
use crate::math::{cross, dot, norm, scale, sub, Interval, Point3};
use crate::operations::modifying::boolean::assembly::{
    cylinder_face_mappings, cylinder_source, face_provenance, finish_cylinder_result,
};
use crate::operations::modifying::boolean::handlers::disjoint::separate_boolean;
use crate::operations::modifying::boolean::operands::CylinderInput;
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};
use crate::primitives;

pub(super) struct ParallelSlice {
    pub(super) alignment: f64,
    pub(super) radial: Point3,
    pub(super) a_span: [f64; 2],
    pub(super) b_span: [f64; 2],
    pub(super) span: [f64; 2],
}

struct BaseCrossing {
    b_frame: Frame3,
    points: [Point3; 2],
}

struct LateralTopology {
    vertices: [[u32; 2]; 2],
    a_edges: [u32; 2],
    b_edges: [u32; 2],
}

pub(super) fn sliced_parallel_cylinder_boolean(
    a: &CylinderInput<'_>,
    b: &CylinderInput<'_>,
    slice: &ParallelSlice,
    operation: BooleanOp,
    id: String,
    accuracy: Accuracy,
    roundoff: f64,
) -> Result<BooleanResult, GeometryError> {
    let ParallelSlice {
        alignment,
        radial,
        a_span,
        b_span,
        span,
    } = *slice;
    let mut a_frame = a.frame;
    a_frame.origin = a.frame.point([0.0, 0.0, span[0]]);
    let mut b_frame = a_frame;
    b_frame.origin = std::array::from_fn(|axis| a_frame.origin[axis] + radial[axis]);
    let height = span[1] - span[0];
    let a_slice = CylinderInput {
        brep: a.brep,
        frame: a_frame,
        radius: a.radius,
        height,
    };
    let b_slice = CylinderInput {
        brep: b.brep,
        frame: b_frame,
        radius: b.radius,
        height,
    };
    let mut result = parallel_cylinder_boolean(&a_slice, &b_slice, 1.0, operation, id, accuracy)?;
    result.report.coincident = false;
    if operation == BooleanOp::Intersection && result.brep.topology.faces.len() == 4 {
        let a_caps = [cylinder_source(a, 1), cylinder_source(a, 2)];
        let b_caps = if alignment > 0.0 {
            [cylinder_source(b, 1), cylinder_source(b, 2)]
        } else {
            [cylinder_source(b, 2), cylinder_source(b, 1)]
        };
        for (face, boundary, end) in [(2, span[0], 0), (3, span[1], 1)] {
            let mut sources = Vec::with_capacity(2);
            if (boundary - a_span[end]).abs() <= roundoff {
                sources.push(a_caps[end].clone());
            }
            if (boundary - b_span[end]).abs() <= roundoff {
                sources.push(b_caps[end].clone());
            }
            if sources.len() > 1 {
                result.report.coincident = true;
            }
            if sources.is_empty() {
                return Err(GeometryError::InvalidTopology(
                    "parallel cylinder slice cap has no source face".into(),
                ));
            }
            result.brep.topology.faces[face].provenance =
                face_provenance(sources, FaceRole::Split, false);
        }
        result.report.face_mappings = cylinder_face_mappings(&result.brep, [a, b]);
        result.brep.validate()?;
    } else if operation == BooleanOp::Subtraction {
        result.report.coincident =
            (a_span[0] - b_span[0]).abs() <= roundoff || (a_span[1] - b_span[1]).abs() <= roundoff;
    }
    Ok(result)
}

pub(super) fn parallel_cylinder_boolean(
    a: &CylinderInput<'_>,
    b: &CylinderInput<'_>,
    alignment: f64,
    operation: BooleanOp,
    id: String,
    accuracy: Accuracy,
) -> Result<BooleanResult, GeometryError> {
    let mut b_frame = a.frame;
    b_frame.origin = if alignment > 0.0 {
        b.frame.origin
    } else {
        b.frame.point([0.0, 0.0, b.height])
    };
    let delta = sub(b_frame.origin, a.frame.origin);
    let axial = dot(delta, a.frame.z);
    let radial = sub(delta, scale(a.frame.z, axial));
    let distance = norm(radial);
    let sum = a.radius + b.radius;
    let difference = (a.radius - b.radius).abs();
    let clearance = 4.0 * accuracy.geometric;
    let relation_roundoff = 128.0
        * f64::EPSILON
        * distance.max(sum).max(a.height).max(
            a.frame
                .origin
                .into_iter()
                .chain(b_frame.origin)
                .fold(1.0_f64, |largest, value| largest.max(value.abs())),
        );
    if (distance - sum).abs() <= clearance {
        if (distance - sum).abs() <= relation_roundoff {
            let toward_b = unit(radial)?;
            let bottom: Point3 =
                std::array::from_fn(|axis| a.frame.origin[axis] + toward_b[axis] * a.radius);
            let top: Point3 = std::array::from_fn(|axis| bottom[axis] + a.frame.z[axis] * a.height);
            return separate_boolean(a.brep, b.brep, operation, id, vec![bottom, top]);
        }
        return Err(GeometryError::UnresolvedIntersection(
            "parallel cylinders are nearly externally tangent".into(),
        ));
    }
    if (distance - difference).abs() <= clearance {
        return Err(GeometryError::UnresolvedIntersection(
            "parallel cylinders are internally tangent or nearly tangent".into(),
        ));
    }
    if distance <= difference {
        return nested_parallel_cylinder_boolean(a, b, b_frame, operation, id, accuracy);
    }
    if distance >= sum {
        return separate_boolean(a.brep, b.brep, operation, id, Vec::new());
    }

    let points = parallel_chord_points(a, b, radial, distance, clearance)?;
    let crossing = BaseCrossing { b_frame, points };
    overlapping_parallel_cylinder_boolean(a, b, &crossing, alignment, operation, id, accuracy)
}

fn nested_parallel_cylinder_boolean(
    a: &CylinderInput<'_>,
    b: &CylinderInput<'_>,
    b_frame: Frame3,
    operation: BooleanOp,
    id: String,
    accuracy: Accuracy,
) -> Result<BooleanResult, GeometryError> {
    let b_inside_a = b.radius < a.radius;
    if operation != BooleanOp::Subtraction || !b_inside_a {
        return contained_cylinders(a, b, b_inside_a, operation, id, accuracy);
    }
    let mut out = primitives::cylinder_with_circular_hole(
        id, a.frame, b_frame, b.radius, a.radius, a.height, accuracy,
    )?;
    out.topology.faces[0].provenance =
        face_provenance(vec![cylinder_source(a, 0)], FaceRole::Preserved, false);
    out.topology.faces[1].provenance =
        face_provenance(vec![cylinder_source(b, 0)], FaceRole::Cut, true);
    for (face, source) in [(2, cylinder_source(a, 1)), (3, cylinder_source(a, 2))] {
        out.topology.faces[face].provenance = face_provenance(vec![source], FaceRole::Split, false);
    }
    finish_cylinder_result(out, a, b, operation, true)
}

fn parallel_chord_points(
    a: &CylinderInput<'_>,
    b: &CylinderInput<'_>,
    radial: Point3,
    distance: f64,
    clearance: f64,
) -> Result<[Point3; 2], GeometryError> {
    let toward_b = unit(radial)?;
    let lateral = unit(cross(a.frame.z, toward_b))?;
    let along =
        (distance * distance + a.radius * a.radius - b.radius * b.radius) / (2.0 * distance);
    let half_squared = (a.radius - along) * (a.radius + along);
    if half_squared <= clearance * clearance {
        return Err(GeometryError::UnresolvedIntersection(
            "parallel cylinder intersection chord is below geometric resolution".into(),
        ));
    }
    let half = half_squared.sqrt();
    let chord_center: Point3 =
        std::array::from_fn(|axis| a.frame.origin[axis] + toward_b[axis] * along);
    let points = [
        std::array::from_fn(|axis| chord_center[axis] + lateral[axis] * half),
        std::array::from_fn(|axis| chord_center[axis] - lateral[axis] * half),
    ];
    Ok(points)
}

fn overlapping_parallel_cylinder_boolean(
    a: &CylinderInput<'_>,
    b: &CylinderInput<'_>,
    crossing: &BaseCrossing,
    alignment: f64,
    operation: BooleanOp,
    id: String,
    accuracy: Accuracy,
) -> Result<BooleanResult, GeometryError> {
    let (a_inside, b_inside, reverse_b) = match operation {
        BooleanOp::Union => (false, false, false),
        BooleanOp::Intersection => (true, true, false),
        BooleanOp::Subtraction => (false, true, true),
    };
    let a_arc = selected_cylinder_arc(
        a.frame,
        a.radius,
        crossing.points,
        crossing.b_frame.origin,
        b.radius,
        a_inside,
        false,
    )?;
    let b_arc = selected_cylinder_arc(
        crossing.b_frame,
        b.radius,
        crossing.points,
        a.frame.origin,
        a.radius,
        b_inside,
        reverse_b,
    )?;

    let mut builder = Builder::new(id, accuracy)?;
    let LateralTopology {
        vertices,
        a_edges,
        b_edges,
    } = add_parallel_lateral_faces(&mut builder, a, crossing.points, a_arc, b_arc)?;
    add_parallel_caps(
        &mut builder,
        a,
        b,
        crossing,
        [(a_arc, a_edges), (b_arc, b_edges)],
        vertices,
        accuracy,
    )?;
    let mut out = builder.finish_solid()?;
    set_parallel_provenance(&mut out, a, b, alignment, operation, reverse_b);
    finish_cylinder_result(out, a, b, operation, true)
}

fn add_parallel_lateral_faces(
    builder: &mut Builder,
    a: &CylinderInput<'_>,
    points: [Point3; 2],
    a_arc: CylinderArc,
    b_arc: CylinderArc,
) -> Result<LateralTopology, GeometryError> {
    let bottom_vertices = [builder.vertex(points[0]), builder.vertex(points[1])];
    let top_points =
        points.map(|point| std::array::from_fn(|axis| point[axis] + a.frame.z[axis] * a.height));
    let top_vertices = [builder.vertex(top_points[0]), builder.vertex(top_points[1])];
    let vertices = [bottom_vertices, top_vertices];
    let vertical_range = Interval::new(0.0, a.height)?;
    let vertical_edges = [0, 1].map(|point| {
        builder.edge(
            CurveGeometry::Line {
                origin: points[point],
                direction: a.frame.z,
            },
            vertical_range,
            false,
        )
    });
    let add_arc_edges =
        |builder: &mut Builder, arc: CylinderArc| -> Result<[u32; 2], GeometryError> {
            let mut top_frame = arc.frame;
            top_frame.origin =
                std::array::from_fn(|axis| arc.frame.origin[axis] + a.frame.z[axis] * a.height);
            Ok([
                builder.edge(
                    CurveGeometry::Circle {
                        frame: arc.frame,
                        radius: arc.radius,
                    },
                    arc.range,
                    false,
                ),
                builder.edge(
                    CurveGeometry::Circle {
                        frame: top_frame,
                        radius: arc.radius,
                    },
                    arc.range,
                    false,
                ),
            ])
        };
    let a_edges = add_arc_edges(builder, a_arc)?;
    let b_edges = add_arc_edges(builder, b_arc)?;
    add_cylinder_arc_face(
        builder,
        "a:lateral",
        a_arc,
        &ArcFaceBoundary {
            bottom_edge: a_edges[0],
            top_edge: a_edges[1],
            vertical_edges,
            vertices,
        },
        a.height,
    )?;
    add_cylinder_arc_face(
        builder,
        "b:lateral",
        b_arc,
        &ArcFaceBoundary {
            bottom_edge: b_edges[0],
            top_edge: b_edges[1],
            vertical_edges,
            vertices,
        },
        a.height,
    )?;
    Ok(LateralTopology {
        vertices,
        a_edges,
        b_edges,
    })
}

fn add_parallel_caps(
    builder: &mut Builder,
    a: &CylinderInput<'_>,
    b: &CylinderInput<'_>,
    crossing: &BaseCrossing,
    arc_edges: [(CylinderArc, [u32; 2]); 2],
    vertices: [[u32; 2]; 2],
    accuracy: Accuracy,
) -> Result<(), GeometryError> {
    let mut lower_frame = a.frame;
    lower_frame.y = scale(lower_frame.y, -1.0);
    lower_frame.z = scale(lower_frame.z, -1.0);
    let mut upper_frame = a.frame;
    upper_frame.origin = a.frame.point([0.0, 0.0, a.height]);
    let planar_bounds = parallel_cap_bounds(
        a,
        b,
        crossing.b_frame,
        crossing.points,
        upper_frame,
        accuracy,
    );
    let [bottom_vertices, top_vertices] = vertices;
    let cap_uses =
        |builder: &Builder, frame: Frame3, top: bool| -> Result<Vec<Use>, GeometryError> {
            let vertex_set = if top { top_vertices } else { bottom_vertices };
            let edge_index = usize::from(top);
            let mut uses = Vec::with_capacity(2);
            for (arc, edges) in arc_edges {
                let sense = match (top, arc.reversed) {
                    (false, false) => Orientation::Reverse,
                    (false, true) => Orientation::Forward,
                    (true, false) => Orientation::Forward,
                    (true, true) => Orientation::Reverse,
                };
                let (from, to) = if sense == Orientation::Forward {
                    (vertex_set[arc.start], vertex_set[arc.end])
                } else {
                    (vertex_set[arc.end], vertex_set[arc.start])
                };
                uses.push(plane_boundary(
                    builder,
                    frame,
                    edges[edge_index],
                    from,
                    to,
                    sense,
                )?);
            }
            Ok(uses)
        };
    let lower_uses = cap_uses(builder, lower_frame, false)?;
    builder.face(
        "lower_cap",
        SurfaceGeometry::Plane { frame: lower_frame },
        planar_bounds,
        lower_uses,
    )?;
    let upper_uses = cap_uses(builder, upper_frame, true)?;
    builder.face(
        "upper_cap",
        SurfaceGeometry::Plane { frame: upper_frame },
        planar_bounds,
        upper_uses,
    )
}

fn parallel_cap_bounds(
    a: &CylinderInput<'_>,
    b: &CylinderInput<'_>,
    b_frame: Frame3,
    points: [Point3; 2],
    upper_frame: Frame3,
    accuracy: Accuracy,
) -> [[f64; 2]; 2] {
    padded_uv_bounds(
        [
            a.frame.origin,
            b_frame.origin,
            points[0],
            points[1],
            std::array::from_fn(|axis| a.frame.origin[axis] + a.frame.x[axis] * a.radius),
            std::array::from_fn(|axis| a.frame.origin[axis] - a.frame.x[axis] * a.radius),
            std::array::from_fn(|axis| a.frame.origin[axis] + a.frame.y[axis] * a.radius),
            std::array::from_fn(|axis| a.frame.origin[axis] - a.frame.y[axis] * a.radius),
            std::array::from_fn(|axis| b_frame.origin[axis] + a.frame.x[axis] * b.radius),
            std::array::from_fn(|axis| b_frame.origin[axis] - a.frame.x[axis] * b.radius),
            std::array::from_fn(|axis| b_frame.origin[axis] + a.frame.y[axis] * b.radius),
            std::array::from_fn(|axis| b_frame.origin[axis] - a.frame.y[axis] * b.radius),
        ]
        .into_iter()
        .map(|point| upper_frame.local(point)),
        accuracy.geometric,
    )
}

fn set_parallel_provenance(
    out: &mut BrepEnvelope,
    a: &CylinderInput<'_>,
    b: &CylinderInput<'_>,
    alignment: f64,
    operation: BooleanOp,
    reverse_b: bool,
) {
    out.topology.faces[0].provenance =
        face_provenance(vec![cylinder_source(a, 0)], FaceRole::Split, false);
    out.topology.faces[1].provenance = face_provenance(
        vec![cylinder_source(b, 0)],
        if reverse_b {
            FaceRole::Cut
        } else {
            FaceRole::Split
        },
        reverse_b,
    );
    let a_caps = [cylinder_source(a, 1), cylinder_source(a, 2)];
    let b_caps = if alignment > 0.0 {
        [cylinder_source(b, 1), cylinder_source(b, 2)]
    } else {
        [cylinder_source(b, 2), cylinder_source(b, 1)]
    };
    for (face, cap) in [(2, 0), (3, 1)] {
        let sources = if operation == BooleanOp::Subtraction {
            vec![a_caps[cap].clone()]
        } else {
            vec![a_caps[cap].clone(), b_caps[cap].clone()]
        };
        out.topology.faces[face].provenance = face_provenance(sources, FaceRole::Split, false);
    }
}
