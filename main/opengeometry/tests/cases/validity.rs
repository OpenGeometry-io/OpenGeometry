use opengeometry::brep::BrepEnvelope;

#[test]
fn main_brep_json_round_trips_byte_identically() {
    macro_rules! check {
        ($name:literal) => {{
            let source = include_str!(concat!("../fixtures/cases/", $name, ".brep.json"));
            let body = BrepEnvelope::from_json(source).expect($name);
            assert_eq!(body.to_json().expect($name), source, "{}", $name);
        }};
    }
    check!("cuboid");
    check!("cylinder");
    check!("sphere");
    check!("cone");
    check!("frustum");
    check!("torus");
    check!("annular-cylinder");
    check!("cylinder-with-hole");
    check!("circle");
    check!("linear-extrusion");
    check!("arc-edged-extrusion");
    check!("arc-edged-extrusion-with-holes");
    check!("box-union");
    check!("box-intersection");
    check!("box-cut");
    check!("box-cavity");
    check!("sphere-cut");
}
