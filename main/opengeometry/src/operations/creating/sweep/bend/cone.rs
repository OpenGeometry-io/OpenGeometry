use super::frame::BendFrame;
use crate::brep::{Frame3, SurfaceGeometry};
use crate::math::{add, scale};
use crate::operations::OperationError;

pub(crate) struct Ruling {
    pub(crate) frame: Frame3,
    pub(crate) surface: SurfaceGeometry,
    pub(crate) turn: f64,
}

pub(crate) fn cone_ruling(
    frame: BendFrame,
    from: [f64; 2],
    to: [f64; 2],
) -> Result<Ruling, OperationError> {
    let [radial, axial] = [to[0] - from[0], to[1] - from[1]];
    let apex = from[1] - from[0] * axial / radial;
    let turn = if from[1] > apex { 1.0 } else { -1.0 };
    let cone = Frame3::from_axis(
        add(frame.centre, scale(frame.axis, apex)),
        scale(frame.axis, turn),
        frame.radial,
    )?;
    Ok(Ruling {
        frame: cone,
        surface: SurfaceGeometry::Cone {
            frame: cone,
            semi_angle: radial.abs().atan2(axial.abs()),
        },
        turn,
    })
}
