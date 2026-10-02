use super::arcs::{
    add_cylinder_arc_face, selected_cylinder_arc, shifted_cylinder_arc, two_arc_plane_bounds,
    two_arc_plane_uses, ArcFaceBoundary, CylinderArc,
};
use crate::brep::{
    unit, Accuracy, BrepEnvelope, Builder, CurveGeometry, FaceRole, Frame3, GeometryError,
    SurfaceGeometry,
};
use crate::math::{cross, norm, scale, Interval, Point3};
use crate::operations::modifying::boolean::assembly::{
    cylinder_source, face_provenance, finish_cylinder_result,
};
use crate::operations::modifying::boolean::operands::CylinderInput;
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};

pub(super) struct EmbeddedPlacement {
    pub(super) alignment: f64,
    pub(super) radial: Point3,
    pub(super) b_span: [f64; 2],
}

struct EmbeddedCut {
    axis: Point3,
    levels: [f64; 4],
    a_outer: CylinderArc,
    a_inner: CylinderArc,
    b_boundary: CylinderArc,
}

struct EmbeddedLayout {
    vertices: [[u32; 2]; 4],
    vertical_edges: [[u32; 2]; 3],
    a_outer_edges: [u32; 4],
    a_inner_edges: [u32; 4],
    b_edges: [u32; 2],
}

pub(super) fn embedded_transverse_cylinder_boolean(
    a: &CylinderInput<'_>,
    b: &CylinderInput<'_>,
    placement: &EmbeddedPlacement,
    operation: BooleanOp,
    id: String,
    accuracy: Accuracy,
) -> Result<BooleanResult, GeometryError> {
    let EmbeddedPlacement {
        alignment,
        radial,
        b_span,
    } = *placement;
    if !matches!(operation, BooleanOp::Union | BooleanOp::Subtraction) {
        return Err(GeometryError::InvalidGeometry(
            "embedded transverse cylinder builder requires union or subtraction".into(),
        ));
    }
    let axis = a.frame.z;
    let distance = norm(radial);
    let toward_b = unit(radial)?;
    let lateral = unit(cross(axis, toward_b))?;
    let along =
        (distance * distance + a.radius * a.radius - b.radius * b.radius) / (2.0 * distance);
    let half_squared = (a.radius - along) * (a.radius + along);
    let clearance = 4.0 * accuracy.geometric;
    if half_squared <= clearance * clearance {
        return Err(GeometryError::UnresolvedIntersection(
            "embedded transverse cylinder chord is below geometric resolution".into(),
        ));
    }
    let half = half_squared.sqrt();
    let chord_center: Point3 = std::array::from_fn(|i| a.frame.origin[i] + toward_b[i] * along);
    let points = [
        std::array::from_fn(|i| chord_center[i] + lateral[i] * half),
        std::array::from_fn(|i| chord_center[i] - lateral[i] * half),
    ];
    let [a_outer, a_inner, b_boundary] = embedded_boundary_arcs(a, b, radial, points, operation)?;
    let levels = [0.0, b_span[0], b_span[1], a.height];
    if levels.windows(2).any(|span| span[1] - span[0] <= clearance) {
        return Err(GeometryError::UnresolvedIntersection(
            "embedded transverse cylinder leaves a sub-tolerance axial segment".into(),
        ));
    }
    let cut = EmbeddedCut {
        axis,
        levels,
        a_outer,
        a_inner,
        b_boundary,
    };

    let mut builder = Builder::new(id, accuracy)?;
    let layout = add_embedded_edges(&mut builder, points, &cut)?;
    add_embedded_lateral_faces(&mut builder, &cut, &layout)?;
    add_embedded_planar_faces(&mut builder, a, &cut, &layout, operation, accuracy)?;

    let mut out = builder.finish_solid()?;
    set_embedded_provenance(&mut out, a, b, alignment, operation);
    finish_cylinder_result(out, a, b, operation, false)
}

fn embedded_boundary_arcs(
    a: &CylinderInput<'_>,
    b: &CylinderInput<'_>,
    radial: Point3,
    points: [Point3; 2],
    operation: BooleanOp,
) -> Result<[CylinderArc; 3], GeometryError> {
    let mut b_base_frame = a.frame;
    b_base_frame.origin = std::array::from_fn(|i| a.frame.origin[i] + radial[i]);
    let a_outer = selected_cylinder_arc(
        a.frame,
        a.radius,
        points,
        b_base_frame.origin,
        b.radius,
        false,
        false,
    )?;
    let a_inner = selected_cylinder_arc(
        a.frame,
        a.radius,
        points,
        b_base_frame.origin,
        b.radius,
        true,
        false,
    )?;
    let b_boundary = selected_cylinder_arc(
        b_base_frame,
        b.radius,
        points,
        a.frame.origin,
        a.radius,
        operation == BooleanOp::Subtraction,
        operation == BooleanOp::Subtraction,
    )?;
    Ok([a_outer, a_inner, b_boundary])
}

fn add_embedded_edges(
    builder: &mut Builder,
    points: [Point3; 2],
    cut: &EmbeddedCut,
) -> Result<EmbeddedLayout, GeometryError> {
    let vertices: [[u32; 2]; 4] = std::array::from_fn(|level| {
        points.map(|point| {
            builder.vertex(std::array::from_fn(|i| {
                point[i] + cut.axis[i] * cut.levels[level]
            }))
        })
    });
    let vertical_ranges = [
        Interval::new(0.0, cut.levels[1] - cut.levels[0])?,
        Interval::new(0.0, cut.levels[2] - cut.levels[1])?,
        Interval::new(0.0, cut.levels[3] - cut.levels[2])?,
    ];
    let vertical_edges: [[u32; 2]; 3] = std::array::from_fn(|segment| {
        [0, 1].map(|point| {
            builder.edge(
                CurveGeometry::Line {
                    origin: std::array::from_fn(|i| {
                        points[point][i] + cut.axis[i] * cut.levels[segment]
                    }),
                    direction: cut.axis,
                },
                vertical_ranges[segment],
                false,
            )
        })
    });
    let mut arc_edge = |arc: CylinderArc, level: usize, seam: bool| {
        let shifted = shifted_cylinder_arc(arc, cut.axis, cut.levels[level]);
        builder.edge(
            CurveGeometry::Circle {
                frame: shifted.frame,
                radius: shifted.radius,
            },
            shifted.range,
            seam,
        )
    };
    let a_outer_edges = [
        arc_edge(cut.a_outer, 0, false),
        arc_edge(cut.a_outer, 1, true),
        arc_edge(cut.a_outer, 2, true),
        arc_edge(cut.a_outer, 3, false),
    ];
    let a_inner_edges = [
        arc_edge(cut.a_inner, 0, false),
        arc_edge(cut.a_inner, 1, false),
        arc_edge(cut.a_inner, 2, false),
        arc_edge(cut.a_inner, 3, false),
    ];
    let b_edges = [
        arc_edge(cut.b_boundary, 1, false),
        arc_edge(cut.b_boundary, 2, false),
    ];
    Ok(EmbeddedLayout {
        vertices,
        vertical_edges,
        a_outer_edges,
        a_inner_edges,
        b_edges,
    })
}

fn add_embedded_lateral_faces(
    builder: &mut Builder,
    cut: &EmbeddedCut,
    layout: &EmbeddedLayout,
) -> Result<(), GeometryError> {
    for segment in 0..3 {
        let height = cut.levels[segment + 1] - cut.levels[segment];
        add_cylinder_arc_face(
            builder,
            &format!("a:outer:{segment}"),
            shifted_cylinder_arc(cut.a_outer, cut.axis, cut.levels[segment]),
            &ArcFaceBoundary {
                bottom_edge: layout.a_outer_edges[segment],
                top_edge: layout.a_outer_edges[segment + 1],
                vertical_edges: layout.vertical_edges[segment],
                vertices: [layout.vertices[segment], layout.vertices[segment + 1]],
            },
            height,
        )?;
        let (arc, edges, key) = if segment == 1 {
            (cut.b_boundary, layout.b_edges, "b:boundary")
        } else {
            (
                cut.a_inner,
                [
                    layout.a_inner_edges[segment],
                    layout.a_inner_edges[segment + 1],
                ],
                if segment == 0 {
                    "a:inner:lower"
                } else {
                    "a:inner:upper"
                },
            )
        };
        add_cylinder_arc_face(
            builder,
            key,
            shifted_cylinder_arc(arc, cut.axis, cut.levels[segment]),
            &ArcFaceBoundary {
                bottom_edge: edges[0],
                top_edge: edges[1],
                vertical_edges: layout.vertical_edges[segment],
                vertices: [layout.vertices[segment], layout.vertices[segment + 1]],
            },
            height,
        )?;
    }
    Ok(())
}

fn add_embedded_planar_faces(
    builder: &mut Builder,
    a: &CylinderInput<'_>,
    cut: &EmbeddedCut,
    layout: &EmbeddedLayout,
    operation: BooleanOp,
    accuracy: Accuracy,
) -> Result<(), GeometryError> {
    let mut bottom_frame = a.frame;
    bottom_frame.y = scale(bottom_frame.y, -1.0);
    bottom_frame.z = scale(bottom_frame.z, -1.0);
    let bottom_arcs = [
        shifted_cylinder_arc(cut.a_outer, cut.axis, cut.levels[0]),
        shifted_cylinder_arc(cut.a_inner, cut.axis, cut.levels[0]),
    ];
    add_two_arc_plane_face(
        builder,
        "a:lower-cap",
        bottom_frame,
        bottom_arcs,
        [layout.a_outer_edges[0], layout.a_inner_edges[0]],
        layout.vertices[0],
        accuracy.geometric,
    )?;

    for (level, key) in [(1, "b:lower-interface"), (2, "b:upper-interface")] {
        let outward_positive = (operation == BooleanOp::Union && level == 2)
            || (operation == BooleanOp::Subtraction && level == 1);
        let mut frame = a.frame;
        frame.origin = a.frame.point([0.0, 0.0, cut.levels[level]]);
        if !outward_positive {
            frame.y = scale(frame.y, -1.0);
            frame.z = scale(frame.z, -1.0);
        }
        let arcs = [
            shifted_cylinder_arc(cut.a_inner, cut.axis, cut.levels[level]),
            shifted_cylinder_arc(cut.b_boundary, cut.axis, cut.levels[level]),
        ];
        let edges = if level == 1 {
            [layout.a_inner_edges[1], layout.b_edges[0]]
        } else {
            [layout.a_inner_edges[2], layout.b_edges[1]]
        };
        add_two_arc_plane_face(
            builder,
            key,
            frame,
            arcs,
            edges,
            layout.vertices[level],
            accuracy.geometric,
        )?;
    }

    let mut top_frame = a.frame;
    top_frame.origin = a.frame.point([0.0, 0.0, a.height]);
    let top_arcs = [
        shifted_cylinder_arc(cut.a_outer, cut.axis, cut.levels[3]),
        shifted_cylinder_arc(cut.a_inner, cut.axis, cut.levels[3]),
    ];
    add_two_arc_plane_face(
        builder,
        "a:upper-cap",
        top_frame,
        top_arcs,
        [layout.a_outer_edges[3], layout.a_inner_edges[3]],
        layout.vertices[3],
        accuracy.geometric,
    )
}

fn add_two_arc_plane_face(
    builder: &mut Builder,
    key: &str,
    frame: Frame3,
    arcs: [CylinderArc; 2],
    edges: [u32; 2],
    vertices: [u32; 2],
    tolerance: f64,
) -> Result<(), GeometryError> {
    let uses = two_arc_plane_uses(
        builder,
        frame,
        (arcs[0], edges[0]),
        (arcs[1], edges[1]),
        vertices,
    )?;
    builder.face(
        key,
        SurfaceGeometry::Plane { frame },
        two_arc_plane_bounds(frame, arcs, tolerance),
        uses,
    )
}

fn set_embedded_provenance(
    out: &mut BrepEnvelope,
    a: &CylinderInput<'_>,
    b: &CylinderInput<'_>,
    alignment: f64,
    operation: BooleanOp,
) {
    let a_lateral = cylinder_source(a, 0);
    let b_lateral = cylinder_source(b, 0);
    for face in [0, 1, 2, 4, 5] {
        out.topology.faces[face].provenance =
            face_provenance(vec![a_lateral.clone()], FaceRole::Split, false);
    }
    out.topology.faces[3].provenance = face_provenance(
        vec![b_lateral],
        if operation == BooleanOp::Subtraction {
            FaceRole::Cut
        } else {
            FaceRole::Split
        },
        operation == BooleanOp::Subtraction,
    );
    let a_caps = [cylinder_source(a, 1), cylinder_source(a, 2)];
    out.topology.faces[6].provenance =
        face_provenance(vec![a_caps[0].clone()], FaceRole::Preserved, false);
    out.topology.faces[9].provenance =
        face_provenance(vec![a_caps[1].clone()], FaceRole::Preserved, false);
    let b_caps = if alignment > 0.0 {
        [cylinder_source(b, 1), cylinder_source(b, 2)]
    } else {
        [cylinder_source(b, 2), cylinder_source(b, 1)]
    };
    for (face, source) in [(7, b_caps[0].clone()), (8, b_caps[1].clone())] {
        out.topology.faces[face].provenance = face_provenance(
            vec![source],
            if operation == BooleanOp::Subtraction {
                FaceRole::Cut
            } else {
                FaceRole::Split
            },
            operation == BooleanOp::Subtraction,
        );
    }
}
