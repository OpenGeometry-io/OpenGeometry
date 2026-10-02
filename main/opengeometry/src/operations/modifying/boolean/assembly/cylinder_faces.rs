use super::append::append_analytic_input;
use super::provenance::reverse_face;
use crate::brep::{
    boundary, uv_line, BrepEnvelope, Builder, CurveGeometry, FaceProvenance, Frame3, GeometryError,
    Orientation, PcurveGeometry, SurfaceGeometry,
};
use crate::math::{scale, Interval};
use crate::primitives;
use Orientation::{Forward as F, Reverse as R};

pub(crate) struct CylinderBand {
    pub(crate) frame: Frame3,
    pub(crate) radius: f64,
    pub(crate) z0: f64,
    pub(crate) z1: f64,
    pub(crate) low: (u32, u32),
    pub(crate) high: (u32, u32),
}

pub(crate) fn circular_boundary(
    builder: &mut Builder,
    frame: Frame3,
    radius: f64,
    z: f64,
) -> Result<(u32, u32), GeometryError> {
    let mut circle_frame = frame;
    circle_frame.origin = frame.point([0.0, 0.0, z]);
    let vertex = builder.vertex(frame.point([radius, 0.0, z]));
    let edge = builder.edge(
        CurveGeometry::Circle {
            frame: circle_frame,
            radius,
        },
        Interval::new(0.0, std::f64::consts::TAU)?,
        false,
    );
    Ok((vertex, edge))
}

pub(crate) fn cylinder_band(
    builder: &mut Builder,
    band: &CylinderBand,
    provenance: FaceProvenance,
    reversed: bool,
) -> Result<(), GeometryError> {
    let CylinderBand {
        frame,
        radius,
        z0,
        z1,
        low,
        high,
    } = *band;
    let seam = builder.edge(
        CurveGeometry::Line {
            origin: frame.point([radius, 0.0, z0]),
            direction: frame.z,
        },
        Interval::new(0.0, z1 - z0)?,
        true,
    );
    let face = builder.brep.topology.faces.len() as u32;
    let tau = std::f64::consts::TAU;
    builder.face(
        &format!("{face}:cylinder-band"),
        SurfaceGeometry::Cylinder { frame, radius },
        [[0.0, tau], [z0, z1]],
        vec![
            boundary(low.1, low.0, low.0, F, uv_line([0.0, z0], [1.0, 0.0])),
            boundary(seam, low.0, high.0, F, uv_line([tau, z0], [0.0, 1.0])),
            boundary(high.1, high.0, high.0, R, uv_line([0.0, z1], [1.0, 0.0])),
            boundary(seam, high.0, low.0, R, uv_line([0.0, z0], [0.0, 1.0])),
        ],
    )?;
    builder.brep.topology.faces[face as usize].provenance = provenance;
    if reversed {
        reverse_face(&mut builder.brep, face);
    }
    Ok(())
}

pub(crate) fn cylinder_cap(
    builder: &mut Builder,
    frame: Frame3,
    radius: f64,
    z: f64,
    boundary_edge: (u32, u32),
    top: bool,
    provenance: FaceProvenance,
) -> Result<(), GeometryError> {
    let mut surface_frame = frame;
    surface_frame.origin = frame.point([0.0, 0.0, z]);
    if !top {
        surface_frame.y = scale(surface_frame.y, -1.0);
        surface_frame.z = scale(surface_frame.z, -1.0);
    }
    let face = builder.brep.topology.faces.len() as u32;
    builder.face(
        &format!("{face}:cylinder-cap"),
        SurfaceGeometry::Plane {
            frame: surface_frame,
        },
        [[-radius, radius]; 2],
        vec![boundary(
            boundary_edge.1,
            boundary_edge.0,
            boundary_edge.0,
            if top { F } else { R },
            PcurveGeometry::Conic2 {
                origin: [0.0; 2],
                axis_a: [radius, 0.0],
                axis_b: [0.0, if top { radius } else { -radius }],
            },
        )],
    )?;
    builder.brep.topology.faces[face as usize].provenance = provenance;
    Ok(())
}

pub(crate) fn append_cylinder_segment(
    out: &mut BrepEnvelope,
    frame: Frame3,
    radius: f64,
    height: f64,
    provenances: [FaceProvenance; 3],
    index: usize,
) -> Result<(), GeometryError> {
    if height <= 4.0 * out.accuracy.geometric {
        return Err(GeometryError::UnresolvedIntersection(
            "cylinder boolean leaves a sub-tolerance axial segment".into(),
        ));
    }
    let part = primitives::cylinder(
        format!("{}:segment-{index}", out.id),
        frame,
        radius,
        height,
        out.accuracy,
    )?;
    let face_offset = out.topology.faces.len();
    append_analytic_input(out, &part)?;
    for (face, provenance) in out.topology.faces[face_offset..]
        .iter_mut()
        .zip(provenances)
    {
        face.provenance = provenance;
    }
    Ok(())
}

pub(crate) fn append_annular_cylinder(
    out: &mut BrepEnvelope,
    frame: Frame3,
    inner_radius: f64,
    outer_radius: f64,
    height: f64,
    provenances: [FaceProvenance; 4],
) -> Result<(), GeometryError> {
    let part = primitives::annular_cylinder(
        format!("{}:annular", out.id),
        frame,
        inner_radius,
        outer_radius,
        height,
        out.accuracy,
    )?;
    let face_offset = out.topology.faces.len();
    append_analytic_input(out, &part)?;
    for (face, provenance) in out.topology.faces[face_offset..]
        .iter_mut()
        .zip(provenances)
    {
        face.provenance = provenance;
    }
    Ok(())
}
