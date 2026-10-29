use crate::brep::{
    unit, uv_line, Builder, CurveGeometry, Frame3, GeometryError, PcurveGeometry, Use,
};
use crate::math::{add, cross, dot, norm, scale, sub, Interval, Point3};
use crate::operations::modifying::boolean::operands::{BoxInput, CylinderInput};
use crate::operations::modifying::boolean::types::BooleanOp;

pub(super) struct Chart {
    pub(super) frame: Frame3,
    pub(super) radius: f64,
    pub(super) height: f64,
    pub(super) a: f64,
    pub(super) c: f64,
    pub(super) e: f64,
    pub(super) flipped: bool,
    pub(super) box_face: usize,
}

pub(super) struct HalfSpaceUses {
    pub(super) side: Vec<Use>,
    pub(super) side_bounds: [[f64; 2]; 2],
    pub(super) bottom: Option<Vec<Use>>,
    pub(super) top: Vec<Use>,
    pub(super) top_partial: bool,
    pub(super) cut: Vec<Use>,
    pub(super) cut_frame: Frame3,
    pub(super) cut_bounds: [[f64; 2]; 2],
}

impl Chart {
    pub(super) fn point(&self, theta: f64, w: f64) -> Point3 {
        self.frame
            .point([self.radius * theta.cos(), self.radius * theta.sin(), w])
    }

    pub(super) fn circle(&self, w: f64) -> CurveGeometry {
        CurveGeometry::Circle {
            frame: Frame3 {
                origin: self.frame.point([0.0, 0.0, w]),
                ..self.frame
            },
            radius: self.radius,
        }
    }

    pub(super) fn cap_bottom(&self) -> Frame3 {
        Frame3 {
            origin: self.frame.origin,
            x: self.frame.x,
            y: scale(self.frame.y, -1.0),
            z: scale(self.frame.z, -1.0),
        }
    }

    pub(super) fn cap_top(&self) -> Frame3 {
        Frame3 {
            origin: self.frame.point([0.0, 0.0, self.height]),
            ..self.frame
        }
    }
}

pub(super) fn gap() -> GeometryError {
    GeometryError::CoverageGap {
        families: ["cylinder".into(), "half-space cuboid".into()],
    }
}

pub(super) fn segment(
    builder: &mut Builder,
    a: Point3,
    b: Point3,
) -> Result<(u32, Point3), GeometryError> {
    let direction = unit(sub(b, a))?;
    let edge = builder.edge(
        CurveGeometry::Line {
            origin: a,
            direction,
        },
        Interval::new(0.0, norm(sub(b, a)))?,
        false,
    );
    Ok((edge, direction))
}

pub(super) fn line_pcurve(frame: Frame3, origin: Point3, direction: Point3) -> PcurveGeometry {
    let local = frame.local(origin);
    uv_line(
        [local[0], local[1]],
        [dot(direction, frame.x), dot(direction, frame.y)],
    )
}

pub(super) fn conic(a: f64, b: f64) -> PcurveGeometry {
    PcurveGeometry::Conic2 {
        origin: [0.0; 2],
        axis_a: [a, 0.0],
        axis_b: [0.0, b],
    }
}

pub(super) fn half_space_chart(
    box_: &BoxInput<'_>,
    cylinder: &CylinderInput<'_>,
    [axis, high]: [usize; 2],
    operation: BooleanOp,
) -> Result<Chart, GeometryError> {
    let box_axes = [box_.frame.x, box_.frame.y, box_.frame.z];
    let mut plane_point = [0.0; 3];
    plane_point[axis] = box_.size[axis] * high as f64;
    let plane_point = box_.frame.point(plane_point);
    let box_outward = scale(box_axes[axis], if high == 1 { 1.0 } else { -1.0 });
    let normal = if operation == BooleanOp::Subtraction {
        box_outward
    } else {
        scale(box_outward, -1.0)
    };
    let flipped = dot(normal, cylinder.frame.z) < 0.0;
    let (origin, z) = if flipped {
        (
            add(
                cylinder.frame.origin,
                scale(cylinder.frame.z, cylinder.height),
            ),
            scale(cylinder.frame.z, -1.0),
        )
    } else {
        (cylinder.frame.origin, cylinder.frame.z)
    };
    let a = norm(cross(z, normal));
    if a <= 1.0e-12 {
        return Err(gap());
    }
    let y = unit(cross(normal, z))?;
    let frame = Frame3 {
        origin,
        x: cross(y, z),
        y,
        z,
    };
    frame.validate()?;
    Ok(Chart {
        frame,
        radius: cylinder.radius,
        height: cylinder.height,
        a,
        c: dot(normal, z),
        e: dot(normal, sub(plane_point, origin)),
        flipped,
        box_face: [2, 4, 0][axis] + high,
    })
}
