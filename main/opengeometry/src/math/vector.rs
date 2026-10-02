pub type Point3 = [f64; 3];

pub(crate) fn add(a: Point3, b: Point3) -> Point3 {
    std::array::from_fn(|i| a[i] + b[i])
}
pub(crate) fn sub(a: Point3, b: Point3) -> Point3 {
    std::array::from_fn(|i| a[i] - b[i])
}
pub(crate) fn scale(a: Point3, s: f64) -> Point3 {
    a.map(|v| v * s)
}
pub(crate) fn dot(a: Point3, b: Point3) -> f64 {
    a[0].mul_add(b[0], a[1].mul_add(b[1], a[2] * b[2]))
}
pub(crate) fn cross(a: Point3, b: Point3) -> Point3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
pub(crate) fn norm(a: Point3) -> f64 {
    a[0].hypot(a[1]).hypot(a[2])
}
