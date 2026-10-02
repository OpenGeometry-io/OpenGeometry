use crate::write::write_json;
use opengeometry::analytic::{
    tessellation::{tessellate, Tessellation},
    topology::BrepEnvelope,
};
use serde_json::{json, Value};
use std::{cmp::Ordering, error::Error, fs, path::Path};
use traced_analytic::analytic as traced;

pub(super) fn write_canonical_tessellations(output: &Path) -> Result<(), Box<dyn Error>> {
    let directory = output.join("tessellation");
    fs::create_dir_all(&directory)?;
    for name in [
        "box-union",
        "box-intersection",
        "box-cut",
        "box-cavity",
        "sphere-cut",
    ] {
        let source = fs::read_to_string(output.join(format!("{name}.brep.json")))?;
        write_canonical(&directory, name, &BrepEnvelope::from_json(&source)?)?;
    }
    for (folder, prefix) in [("boolean-matrix", "matrix"), ("batch-matrix", "batch")] {
        for (name, body) in result_breps(&output.join(folder))? {
            write_canonical(&directory, &format!("{prefix}-{name}"), &body)?;
        }
    }
    Ok(())
}

fn write_canonical(
    directory: &Path,
    name: &str,
    body: &BrepEnvelope,
) -> Result<(), Box<dyn Error>> {
    let main_value = match tessellate(body, 0.01, 2_000_000) {
        Ok(mesh) => canonical_tessellation(&mesh),
        Err(error) => json!({"error": serde_json::to_value(error)?}),
    };
    let traced_body = traced::BrepEnvelope::from_json(&body.to_json()?)?;
    let traced_value = match traced::tessellation::tessellate(&traced_body, 0.01, 2_000_000) {
        Ok(mesh) => canonical_tessellation(&Tessellation {
            positions: mesh.positions,
            normals: mesh.normals,
            indices: mesh.indices,
            triangle_face_ids: mesh.triangle_face_ids,
            outline_positions: mesh.outline_positions,
            outline_edge_ids: mesh.outline_edge_ids,
            revision: mesh.revision,
            achieved_deflection: mesh.achieved_deflection,
        }),
        Err(error) => json!({"error": serde_json::to_value(error)?}),
    };
    if main_value != traced_value {
        return Err(format!("traced canonical tessellation differs from main for {name}").into());
    }
    write_json(directory.join(format!("{name}.json")), &main_value)
}

fn result_breps(directory: &Path) -> Result<Vec<(String, BrepEnvelope)>, Box<dyn Error>> {
    let mut rows = Vec::new();
    for entry in fs::read_dir(directory)? {
        let file_name = entry?.file_name().to_string_lossy().into_owned();
        if file_name.contains(".step.") || file_name.contains(".fallback.") {
            continue;
        }
        let Some(name) = file_name.strip_suffix(".json") else {
            continue;
        };
        let row: Value = serde_json::from_slice(&fs::read(directory.join(&file_name))?)?;
        if let Some(brep) = row["result"].get("brep") {
            rows.push((name.to_string(), serde_json::from_value(brep.clone())?));
        }
    }
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(rows)
}

fn canonical_tessellation(mesh: &Tessellation) -> Value {
    let vertices = canonical_vertices(mesh);
    json!({
        "vertices": vertices,
        "vertexCount": mesh.positions.len() / 3,
        "triangles": canonical_triangles(mesh, &vertices),
        "outlines": canonical_outlines(mesh),
        "revision": mesh.revision,
        "achievedDeflection": mesh.achieved_deflection,
    })
}

fn canonical_vertices(mesh: &Tessellation) -> Vec<[f64; 6]> {
    let mut vertices = (0..mesh.positions.len() / 3)
        .map(|index| vertex(mesh, index))
        .collect::<Vec<_>>();
    vertices.sort_by(|a, b| lexicographic(a, b));
    vertices.dedup_by(|a, b| lexicographic(a, b).is_eq());
    vertices
}

fn canonical_triangles(mesh: &Tessellation, vertices: &[[f64; 6]]) -> Vec<[usize; 4]> {
    let mut triangles = mesh
        .indices
        .chunks_exact(3)
        .zip(&mesh.triangle_face_ids)
        .map(|(corners, &face_id)| {
            let mut corners = corners.iter().map(|&index| {
                let tuple = vertex(mesh, index as usize);
                vertices
                    .binary_search_by(|probe| lexicographic(probe, &tuple))
                    .unwrap()
            });
            let mut ring = [0; 3].map(|_| corners.next().unwrap());
            let least = (0..3).min_by_key(|&at| ring[at]).unwrap();
            ring.rotate_left(least);
            [face_id as usize, ring[0], ring[1], ring[2]]
        })
        .collect::<Vec<_>>();
    triangles.sort();
    triangles
}

fn canonical_outlines(mesh: &Tessellation) -> Vec<Value> {
    let mut outlines = mesh
        .outline_positions
        .chunks_exact(6)
        .zip(&mesh.outline_edge_ids)
        .map(|(segment, &edge_id)| {
            let start = [segment[0], segment[1], segment[2]].map(unsigned_zero);
            let end = [segment[3], segment[4], segment[5]].map(unsigned_zero);
            if lexicographic(&end, &start).is_lt() {
                (edge_id, end, start)
            } else {
                (edge_id, start, end)
            }
        })
        .collect::<Vec<_>>();
    outlines.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| lexicographic(&a.1, &b.1))
            .then_with(|| lexicographic(&a.2, &b.2))
    });
    outlines
        .into_iter()
        .map(|(edge_id, start, end)| json!([edge_id, start, end]))
        .collect()
}

fn vertex(mesh: &Tessellation, index: usize) -> [f64; 6] {
    let at = index * 3;
    [
        mesh.positions[at],
        mesh.positions[at + 1],
        mesh.positions[at + 2],
        mesh.normals[at] as f64,
        mesh.normals[at + 1] as f64,
        mesh.normals[at + 2] as f64,
    ]
    .map(unsigned_zero)
}

fn unsigned_zero(value: f64) -> f64 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

fn lexicographic(a: &[f64], b: &[f64]) -> Ordering {
    a.iter()
        .zip(b)
        .map(|(x, y)| x.total_cmp(y))
        .find(|order| order.is_ne())
        .unwrap_or(Ordering::Equal)
}
