use opengeometry::brep::{Accuracy as NextAccuracy, Frame3 as NextFrame};
use opengeometry::operations::modifying::boolean as next_boolean;
use opengeometry::primitives as next_primitives;
use opengeometry::tessellation::tessellate as next_tessellate;
use std::{error::Error, time::Instant};

fn next() -> Result<(f64, usize), Box<dyn Error>> {
    let accuracy = NextAccuracy {
        geometric: 1e-8,
        intersection: 1e-9,
        tessellation: 0.01,
        exchange: 1e-6,
    };
    let frame = NextFrame {
        x: [1.0, 0.0, 0.0],
        y: [0.0, 0.0, -1.0],
        z: [0.0, 1.0, 0.0],
        ..NextFrame::IDENTITY
    };
    let host = next_primitives::linear_extrusion(
        "wall".into(),
        frame,
        vec![[-30.0, -0.1], [30.0, -0.1], [30.0, 0.1], [-30.0, 0.1]],
        vec![],
        3.0,
        accuracy,
    )?;
    let mut cutters = Vec::new();
    for index in 0..50 {
        cutters.push(next_primitives::cuboid(
            format!("cut-{index}"),
            NextFrame {
                origin: [-27.0 + index as f64 * 1.1 - 0.25, 0.0, 0.2],
                ..frame
            },
            [0.5, 0.4, 2.0],
            accuracy,
        )?);
    }
    let result = next_boolean::subtract_planar_cutters(&host, &cutters, "result".into())?;
    result.brep.validate()?;
    let start = Instant::now();
    let mut triangles = 0;
    for _ in 0..100 {
        let mesh = next_tessellate(&result.brep, 0.01, 2_000_000)?;
        triangles = mesh.indices.len() / 3;
    }
    Ok((start.elapsed().as_secs_f64() * 10.0, triangles))
}

fn main() -> Result<(), Box<dyn Error>> {
    let (next_ms, triangles) = next()?;
    println!("{{\"nextMs\":{next_ms},\"triangles\":{triangles}}}");
    Ok(())
}
