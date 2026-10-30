use opengeometry::brep::{Accuracy, BrepEnvelope, Frame3};
use opengeometry::exchange::export_step;
use opengeometry::operations::modifying::boolean::{boolean_brep, BooleanOp};
use opengeometry::primitives::linear_extrusion;
use opengeometry::tessellation::tessellate;
use opengeometry_test_support::part21::Document;
use opengeometry_test_support::volume;

const GOING: f64 = 0.28;

fn profile(count: usize, riser: f64) -> Vec<[f64; 2]> {
    let drop = 0.25 * riser.hypot(GOING) / GOING;
    let end = count as f64 * GOING;
    let mut points = vec![
        [0.0, 0.0],
        [(drop - riser) * GOING / riser, 0.0],
        [end, riser + riser / GOING * end - drop],
    ];
    for step in (0..count).rev() {
        let height = (step + 1) as f64 * riser;
        points.push([(step + 1) as f64 * GOING, height]);
        points.push([step as f64 * GOING, height]);
    }
    points
}

fn area(points: &[[f64; 2]]) -> f64 {
    let twice: f64 = (0..points.len())
        .map(|index| {
            let [a, b] = [points[index], points[(index + 1) % points.len()]];
            a[0] * b[1] - b[0] * a[1]
        })
        .sum();
    twice.abs() * 0.5
}

fn frame(origin: [f64; 3]) -> Frame3 {
    Frame3 {
        origin,
        x: [1.0, 0.0, 0.0],
        y: [0.0, 1.0, 0.0],
        z: [0.0, 0.0, 1.0],
    }
}

fn block(origin: [f64; 3], [width, depth, height]: [f64; 3], id: &str) -> BrepEnvelope {
    let outline = vec![[0.0, 0.0], [width, 0.0], [width, depth], [0.0, depth]];
    linear_extrusion(
        id.into(),
        frame(origin),
        outline,
        vec![],
        height,
        Accuracy::STANDARD,
    )
    .unwrap()
}

fn reloaded(body: &BrepEnvelope) -> BrepEnvelope {
    BrepEnvelope::from_json(&body.to_json().unwrap()).unwrap()
}

fn cut(body: &BrepEnvelope, cutter: &BrepEnvelope, id: &str) -> BrepEnvelope {
    let result = boolean_brep(body, cutter, BooleanOp::Subtraction, id.into())
        .unwrap_or_else(|error| panic!("{id}: {error:?}"));
    reloaded(&result.brep)
}

fn assert_waist_cuts_to_the_clipped_prism(count: usize, riser: f64, [width, depth]: [f64; 2]) {
    let outline = profile(count, riser);
    let flight = linear_extrusion(
        "flight".into(),
        frame([0.0, 0.0, -0.5]),
        outline.clone(),
        vec![],
        1.0,
        Accuracy::STANDARD,
    )
    .unwrap();
    let near = cut(
        &reloaded(&flight),
        &block([-2.0, -1.0, 0.0], [width, depth, 2.0], "near"),
        "near",
    );
    let body = cut(
        &near,
        &block([-2.0, -1.0, -2.0], [width, depth, 1.7], "far"),
        "far",
    );
    let label = format!("{count} steps of {riser}");
    body.validate()
        .unwrap_or_else(|error| panic!("{label}: {error:?}"));
    for deflection in [0.01, 0.00075] {
        tessellate(&body, deflection, 2_000_000)
            .unwrap_or_else(|error| panic!("{label} at {deflection}: {error:?}"));
    }
    let expected = area(&outline) * 0.3;
    let measured = volume::estimate(&body, 0.00075).value;
    assert!(
        (measured - expected).abs() <= 1e-9 * expected,
        "{label}: volume {measured}, expected {expected}"
    );
    let (text, _) = export_step(&body, "metre").unwrap();
    assert_eq!(
        Document::parse(&text).unwrap().count("MANIFOLD_SOLID_BREP"),
        1,
        "{label}"
    );
}

#[test]
fn an_eight_and_a_seventeen_step_waist_cut_by_two_half_space_boxes_keep_the_clipped_prism() {
    for count in [8, 17] {
        assert_waist_cuts_to_the_clipped_prism(count, 3.0 / 17.0, [8.0, 5.0]);
    }
}

fn covering(count: usize) -> [f64; 2] {
    [(count as f64 * GOING + 3.0).max(8.0), 5.5]
}

#[test]
fn waists_of_two_to_twenty_four_steps_cut_cleanly() {
    for count in 2..=24 {
        assert_waist_cuts_to_the_clipped_prism(count, 3.0 / 17.0, covering(count));
    }
}

#[test]
fn waists_of_three_to_twenty_four_steps_over_a_rise_of_1_4118_cut_cleanly() {
    for count in 3..=24 {
        assert_waist_cuts_to_the_clipped_prism(count, 1.4118 / count as f64, covering(count));
    }
}

const FLOORKIT_SECTION_CLIPPER: &str = include_str!("fixtures/floorkit-section-clipper.json");

#[test]
fn an_eight_step_waist_clipped_by_floorkits_section_half_space_tessellates() {
    let outline = vec![
        [0.0, 0.0],
        [0.0013254343282881683, 0.0],
        [2.24, 1.4109293481124237],
        [2.24, 1.411764705882353],
        [1.9600000000000002, 1.411764705882353],
        [1.9600000000000002, 1.2352941176470589],
        [1.6800000000000002, 1.2352941176470589],
        [1.6800000000000002, 1.0588235294117647],
        [1.4000000000000001, 1.0588235294117647],
        [1.4000000000000001, 0.8823529411764707],
        [1.12, 0.8823529411764707],
        [1.12, 0.7058823529411765],
        [0.8400000000000001, 0.7058823529411765],
        [0.8400000000000001, 0.5294117647058824],
        [0.56, 0.5294117647058824],
        [0.56, 0.35294117647058826],
        [0.28, 0.35294117647058826],
        [0.28, 0.17647058823529413],
        [0.0, 0.17647058823529413],
    ];
    let flight = linear_extrusion(
        "stair-f0-s0".into(),
        frame([0.0, 0.0, -0.5]),
        outline.clone(),
        vec![],
        1.0,
        Accuracy::STANDARD,
    )
    .unwrap();
    let clipper = BrepEnvelope::from_json(FLOORKIT_SECTION_CLIPPER).unwrap();
    let body = boolean_brep(&flight, &clipper, BooleanOp::Subtraction, "near".into())
        .unwrap()
        .brep;
    body.validate().unwrap();
    let perimeter: f64 = (0..outline.len())
        .map(|index| {
            let [a, b] = [outline[index], outline[(index + 1) % outline.len()]];
            (b[0] - a[0]).hypot(b[1] - a[1])
        })
        .sum();
    let surface = 2.0 * area(&outline) + 0.5 * perimeter;
    for deflection in [0.01, 0.00075] {
        let mesh = tessellate(&body, deflection, 2_000_000)
            .unwrap_or_else(|error| panic!("at {deflection}: {error:?}"));
        let point = |id: u32| {
            let index = id as usize * 3;
            [
                mesh.positions[index],
                mesh.positions[index + 1],
                mesh.positions[index + 2],
            ]
        };
        let meshed: f64 = mesh
            .indices
            .chunks_exact(3)
            .map(|triangle| {
                let [a, b, c] = [point(triangle[0]), point(triangle[1]), point(triangle[2])];
                let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                let n = [
                    u[1] * v[2] - u[2] * v[1],
                    u[2] * v[0] - u[0] * v[2],
                    u[0] * v[1] - u[1] * v[0],
                ];
                0.5 * (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt()
            })
            .sum();
        assert!(
            (meshed - surface).abs() <= 1e-9 * surface,
            "at {deflection}: mesh area {meshed}, expected {surface}"
        );
    }
    let expected = area(&outline) * 0.5;
    let measured = volume::estimate(&body, 0.00075).value;
    assert!(
        (measured - expected).abs() <= 1e-9 * expected,
        "volume {measured}, expected {expected}"
    );
    let (text, _) = export_step(&body, "metre").unwrap();
    assert_eq!(
        Document::parse(&text).unwrap().count("MANIFOLD_SOLID_BREP"),
        1
    );
}
