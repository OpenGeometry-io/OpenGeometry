use opengeometry::brep::BrepEnvelope;
use opengeometry::tessellation::measure;

pub struct VolumeEstimate {
    pub value: f64,
    pub error_bound: f64,
}

pub fn estimate(body: &BrepEnvelope, deflection: f64) -> VolumeEstimate {
    let measured = measure(body, deflection).unwrap();
    VolumeEstimate {
        value: measured.volume,
        error_bound: measured.error_bound,
    }
}
