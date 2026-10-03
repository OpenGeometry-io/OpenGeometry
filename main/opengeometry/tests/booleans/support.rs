use opengeometry::world_graph::Primitive;

pub(super) fn cuboid(width: f64) -> Primitive {
    Primitive::Cuboid {
        width,
        height: 2.0,
        depth: 2.0,
    }
}
