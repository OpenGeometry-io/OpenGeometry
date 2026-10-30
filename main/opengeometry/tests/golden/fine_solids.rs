use crate::fixtures::{at, fine};
use crate::kernel::{primitives, BrepEnvelope, GeometryError, Point3};

pub(crate) fn sphere_at(
    name: &str,
    center: Point3,
    radius: f64,
) -> Result<BrepEnvelope, GeometryError> {
    primitives::sphere(name.into(), at(center), radius, fine())
}

pub(crate) fn cuboid_at(
    name: &str,
    origin: Point3,
    size: Point3,
) -> Result<BrepEnvelope, GeometryError> {
    primitives::cuboid(name.into(), at(origin), size, fine())
}
