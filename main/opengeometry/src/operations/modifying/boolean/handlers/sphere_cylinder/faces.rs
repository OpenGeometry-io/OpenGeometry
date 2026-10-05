use crate::brep::{
    boundary, uv_line, Builder, CurveGeometry, FaceRole, Frame3, GeometryError, Orientation,
    SurfaceGeometry,
};
use crate::math::{scale, Interval};
use crate::operations::modifying::boolean::assembly::{provenance, reverse_face};
use crate::operations::modifying::boolean::operands::SphereInput;
use Orientation::{Forward as F, Reverse as R};

pub(super) struct SphereBand {
    pub(super) frame: Frame3,
    pub(super) latitude0: f64,
    pub(super) latitude1: f64,
    pub(super) low: (u32, u32),
    pub(super) high: (u32, u32),
}

pub(super) fn sphere_band(
    builder: &mut Builder,
    input: &SphereInput<'_>,
    band: &SphereBand,
    reversed: bool,
) -> Result<(), GeometryError> {
    let SphereBand {
        frame,
        latitude0,
        latitude1,
        low,
        high,
    } = *band;
    let seam = builder.edge(
        CurveGeometry::Circle {
            frame: Frame3 {
                x: frame.x,
                y: frame.z,
                z: scale(frame.y, -1.0),
                ..frame
            },
            radius: input.radius,
        },
        Interval::new(latitude0, latitude1)?,
        true,
    );
    let face = builder.brep.topology.faces.len() as u32;
    let tau = std::f64::consts::TAU;
    builder.face(
        &format!("{face}:sphere-band"),
        SurfaceGeometry::Sphere {
            frame,
            radius: input.radius,
        },
        [[0.0, tau], [latitude0, latitude1]],
        vec![
            boundary(
                low.1,
                low.0,
                low.0,
                F,
                uv_line([0.0, latitude0], [1.0, 0.0]),
            ),
            boundary(seam, low.0, high.0, F, uv_line([tau, 0.0], [0.0, 1.0])),
            boundary(
                high.1,
                high.0,
                high.0,
                R,
                uv_line([0.0, latitude1], [1.0, 0.0]),
            ),
            boundary(seam, high.0, low.0, R, uv_line([0.0, 0.0], [0.0, 1.0])),
        ],
    )?;
    builder.brep.topology.faces[face as usize].provenance = provenance(
        input,
        if reversed {
            FaceRole::Cut
        } else {
            FaceRole::Split
        },
        reversed,
    );
    if reversed {
        reverse_face(&mut builder.brep, face);
    }
    Ok(())
}
