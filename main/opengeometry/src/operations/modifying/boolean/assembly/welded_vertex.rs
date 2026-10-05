use crate::brep::{Accuracy, Builder};
use crate::math::{norm, sub, Point3};

pub(crate) fn add_welded_vertex(
    builder: &mut Builder,
    vertices: &mut Vec<(Point3, u32)>,
    point: Point3,
    accuracy: Accuracy,
) -> u32 {
    if let Some((_, id)) = vertices
        .iter()
        .find(|(existing, _)| norm(sub(*existing, point)) <= accuracy.geometric / 4.0)
    {
        return *id;
    }
    let id = builder.vertex(point);
    vertices.push((point, id));
    id
}
