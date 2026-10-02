use crate::brep::bounds::PatchBounds;
use crate::brep::error::GeometryError;
use crate::brep::frame::{checked, Frame3};
use crate::math::{add, finite, norm, scale, Interval, Point3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum CurveGeometry {
    Line {
        origin: Point3,
        direction: Point3,
    },
    Circle {
        frame: Frame3,
        radius: f64,
    },
    Ellipse {
        frame: Frame3,
        major_radius: f64,
        minor_radius: f64,
    },
    Intersection {
        definition: u32,
    },
}

pub(crate) trait Curve {
    fn point_at(&self, t: f64) -> Result<Point3, GeometryError>;
    fn tangent_at(&self, t: f64) -> Result<Point3, GeometryError>;
    fn enclose(&self, range: Interval) -> Result<PatchBounds, GeometryError>;
}

impl CurveGeometry {
    pub(crate) fn validate(&self) -> Result<(), GeometryError> {
        match self {
            Self::Line { origin, direction } => {
                checked(*origin)?;
                checked(*direction)?;
                if (norm(*direction) - 1.0).abs() > 1e-12 {
                    return Err(GeometryError::InvalidGeometry(
                        "line direction must be unit length".into(),
                    ));
                }
            }
            Self::Circle { frame, radius } => {
                frame.validate()?;
                if !radius.is_finite() || *radius <= 0.0 {
                    return Err(GeometryError::InvalidGeometry(
                        "circle radius must be positive".into(),
                    ));
                }
            }
            Self::Ellipse {
                frame,
                major_radius: a,
                minor_radius: b,
            } => {
                frame.validate()?;
                if !a.is_finite() || !b.is_finite() || *b <= 0.0 || a < b {
                    return Err(GeometryError::InvalidGeometry(
                        "invalid ellipse radii".into(),
                    ));
                }
            }
            Self::Intersection { .. } => {}
        };
        Ok(())
    }

    pub(crate) fn elementary_point(&self, t: f64) -> Result<Point3, GeometryError> {
        self.validate()?;
        finite(t)?;
        checked(match self {
            Self::Line { origin, direction } => add(*origin, scale(*direction, t)),
            Self::Circle { frame, radius } => {
                frame.point([radius * t.cos(), radius * t.sin(), 0.0])
            }
            Self::Ellipse {
                frame,
                major_radius: a,
                minor_radius: b,
            } => frame.point([a * t.cos(), b * t.sin(), 0.0]),
            Self::Intersection { definition } => {
                return Err(GeometryError::MissingReference {
                    kind: "intersection".into(),
                    index: *definition,
                })
            }
        })
    }

    pub(crate) fn elementary_tangent(&self, t: f64) -> Result<Point3, GeometryError> {
        self.validate()?;
        finite(t)?;
        checked(match self {
            Self::Line { direction, .. } => *direction,
            Self::Circle { frame, radius } => {
                frame.vector([-radius * t.sin(), radius * t.cos(), 0.0])
            }
            Self::Ellipse {
                frame,
                major_radius: a,
                minor_radius: b,
            } => frame.vector([-a * t.sin(), b * t.cos(), 0.0]),
            Self::Intersection { definition } => {
                return Err(GeometryError::MissingReference {
                    kind: "intersection".into(),
                    index: *definition,
                })
            }
        })
    }

    pub(super) fn elementary_bounds(&self, range: Interval) -> Result<PatchBounds, GeometryError> {
        self.validate()?;
        Interval::new(range.lo, range.hi)?;
        let zero = Interval::point(0.0)?;
        match self {
            Self::Line { origin, direction } => {
                let mut axes = [zero; 3];
                for i in 0..3 {
                    axes[i] = Interval::point(origin[i])?
                        .add_interval(range.mul_interval(Interval::point(direction[i])?)?)?;
                }
                Ok(PatchBounds { axes })
            }
            Self::Circle { frame, radius } => frame.bounds([
                range.cos()?.mul_interval(Interval::point(*radius)?)?,
                range.sin()?.mul_interval(Interval::point(*radius)?)?,
                zero,
            ]),
            Self::Ellipse {
                frame,
                major_radius: a,
                minor_radius: b,
            } => frame.bounds([
                range.cos()?.mul_interval(Interval::point(*a)?)?,
                range.sin()?.mul_interval(Interval::point(*b)?)?,
                zero,
            ]),
            Self::Intersection { definition } => Err(GeometryError::MissingReference {
                kind: "intersection".into(),
                index: *definition,
            }),
        }
    }

    #[cfg(test)]
    pub(super) fn sample_elementary(
        &self,
        range: Interval,
        deflection: f64,
        max_segments: usize,
    ) -> Result<Vec<Point3>, GeometryError> {
        self.validate()?;
        Interval::new(range.lo, range.hi)?;
        if !deflection.is_finite() || deflection <= 0.0 {
            return Err(GeometryError::InvalidGeometry(
                "deflection must be finite and positive".into(),
            ));
        }
        let bound = match self {
            Self::Line { .. } => 0.0,
            Self::Circle { radius, .. } => *radius,
            Self::Ellipse { major_radius, .. } => *major_radius,
            Self::Intersection { definition } => {
                return Err(GeometryError::MissingReference {
                    kind: "intersection".into(),
                    index: *definition,
                })
            }
        };
        let minimum = if bound > 0.0 && range.width() >= std::f64::consts::TAU - 1e-12 {
            3.0
        } else {
            1.0
        };
        let count = (range.width().abs() * (bound / (8.0 * deflection)).sqrt())
            .ceil()
            .max(minimum);
        if !count.is_finite() || count > max_segments as f64 {
            return Err(GeometryError::LimitExceeded(
                "curve tessellation segments".into(),
            ));
        }
        let n = count as usize;
        let mut samples = Vec::with_capacity(n + 1);
        for i in 0..=n {
            samples.push(
                self.elementary_point(range.lo + (range.hi - range.lo) * i as f64 / n as f64)?,
            );
        }
        Ok(samples)
    }
}
