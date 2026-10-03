use crate::brep::bounds::PatchBounds;
use crate::math::{dot, norm, sub, Point3};

pub(crate) fn point_segment_distance(point: Point3, a: Point3, b: Point3) -> f64 {
    let chord = sub(b, a);
    let length_squared = dot(chord, chord);
    if length_squared == 0.0 {
        return norm(sub(point, a));
    }
    let parameter = (dot(sub(point, a), chord) / length_squared).clamp(0.0, 1.0);
    norm(sub(
        point,
        [
            a[0] + parameter * chord[0],
            a[1] + parameter * chord[1],
            a[2] + parameter * chord[2],
        ],
    ))
}

pub(crate) fn bounds_chord_deviation(bounds: PatchBounds, a: Point3, b: Point3) -> f64 {
    let mut maximum: f64 = 0.0;
    for mask in 0..8 {
        let corner = std::array::from_fn(|axis| {
            if mask & (1 << axis) == 0 {
                bounds.axes[axis].lo
            } else {
                bounds.axes[axis].hi
            }
        });
        maximum = maximum.max(point_segment_distance(corner, a, b));
    }
    maximum
}
