use crate::tessellation::display::encode;

#[test]
fn nonfinite_f32_encoding_is_rejected() {
    let mut error = 0.0;
    assert!(encode(&[f64::MAX, 0.0, 0.0], [0.0; 3], &mut error).is_err());
}
