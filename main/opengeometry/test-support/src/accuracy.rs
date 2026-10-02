use opengeometry::brep::Accuracy;

pub fn accuracy(exchange: f64) -> Accuracy {
    Accuracy {
        geometric: 1e-8,
        intersection: 1e-9,
        tessellation: 0.01,
        exchange,
    }
}
