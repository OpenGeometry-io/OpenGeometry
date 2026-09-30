use opengeometry::analytic::{
    booleans as source_boolean, primitives as source_primitives,
    tessellation::tessellate as source_tessellate, topology::Accuracy as SourceAccuracy,
    Frame3 as SourceFrame,
};
use std::{error::Error, time::Instant};

fn source() -> Result<(f64, usize), Box<dyn Error>> {
    let accuracy = SourceAccuracy {
        geometric: 1e-8,
        intersection: 1e-9,
        tessellation: 0.01,
        exchange: 1e-6,
    };
    let frame = SourceFrame {
        x: [1.0, 0.0, 0.0],
        y: [0.0, 0.0, -1.0],
        z: [0.0, 1.0, 0.0],
        ..SourceFrame::IDENTITY
    };
    let host = source_primitives::linear_extrusion(
        "wall".into(),
        frame,
        vec![[-30.0, -0.1], [30.0, -0.1], [30.0, 0.1], [-30.0, 0.1]],
        vec![],
        3.0,
        accuracy,
    )?;
    let mut cutters = Vec::new();
    for index in 0..50 {
        cutters.push(source_primitives::cuboid(
            format!("cut-{index}"),
            SourceFrame {
                origin: [-27.0 + index as f64 * 1.1 - 0.25, 0.0, 0.2],
                ..frame
            },
            [0.5, 0.4, 2.0],
            accuracy,
        )?);
    }
    let result = source_boolean::subtract_planar_cutters(&host, &cutters, "result".into())?;
    result.brep.validate()?;
    let start = Instant::now();
    let mut triangles = 0;
    for _ in 0..100 {
        let mesh = source_tessellate(&result.brep, 0.01, 2_000_000)?;
        triangles = mesh.indices.len() / 3;
    }
    Ok((start.elapsed().as_secs_f64() * 10.0, triangles))
}

fn main() -> Result<(), Box<dyn Error>> {
    let (source_ms, triangles) = source()?;
    println!(
        "{}",
        serde_json::json!({"sourceMs": source_ms, "triangles": triangles})
    );
    Ok(())
}
