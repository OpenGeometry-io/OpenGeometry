//! B-rep validity checking and basic healing (debt item D5).
//!
//! `BrepBuilder` validates *topology* at construction, but a kernel also needs
//! a *geometric* validity check — is the shell closed, are edges manifold, are
//! face orientations consistent, is any geometry non-finite or degenerate — to
//! gate bad input and to detect when an operation produced garbage. And it
//! needs basic *healing* to recover near-valid geometry. The checker and the
//! healer are deliberately separate (PRD R4): healing must never quietly mask a
//! real defect, so the validity signal stays honest for upstream gating.

use std::collections::HashMap;

use openmaths::Vector3;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use super::Brep;

/// Wasm entry point: check a serialized B-rep's geometric validity, returning
/// the [`ValidityReport`] as JSON.
#[wasm_bindgen(js_name = checkBrepValidity)]
pub fn check_brep_validity_wasm(brep_json: String) -> Result<String, JsValue> {
    let brep: Brep = serde_json::from_str(&brep_json)
        .map_err(|e| JsValue::from_str(&format!("Invalid BRep JSON: {}", e)))?;
    let report = check_validity(&brep);
    serde_json::to_string(&report)
        .map_err(|e| JsValue::from_str(&format!("Failed to serialize report: {}", e)))
}

/// Wasm entry point: heal a serialized B-rep, returning JSON
/// `{ "brep": <healed brep>, "report": <heal report> }`.
#[wasm_bindgen(js_name = healBrep)]
pub fn heal_brep_wasm(brep_json: String, tolerance: f64) -> Result<String, JsValue> {
    let brep: Brep = serde_json::from_str(&brep_json)
        .map_err(|e| JsValue::from_str(&format!("Invalid BRep JSON: {}", e)))?;
    let (healed, report) = heal(&brep, tolerance);
    serde_json::to_string(&serde_json::json!({ "brep": healed, "report": report }))
        .map_err(|e| JsValue::from_str(&format!("Failed to serialize healed brep: {}", e)))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ValidityReport {
    /// Construction-time topology invariants hold (ids, links, manifold edges).
    pub topology_ok: bool,
    /// Every edge is shared by exactly two half-edges (a closed 2-manifold).
    pub closed_shell: bool,
    /// No edge is used by more than two half-edges.
    pub manifold_edges: bool,
    /// Each shared edge is traversed in opposite directions by its two
    /// half-edges (consistent face orientation).
    pub consistent_orientation: bool,
    /// All vertex coordinates are finite.
    pub finite_geometry: bool,
    /// No face collapses to ~zero area.
    pub no_degenerate_faces: bool,
    pub issues: Vec<String>,
}

impl ValidityReport {
    /// Whether the B-rep is a valid solid by every checked criterion.
    pub fn is_valid(&self) -> bool {
        self.topology_ok
            && self.closed_shell
            && self.manifold_edges
            && self.consistent_orientation
            && self.finite_geometry
            && self.no_degenerate_faces
    }
}

const AREA_EPSILON: f64 = 1.0e-12;

/// Runs the full geometric validity check. Never mutates the input.
pub fn check_validity(brep: &Brep) -> ValidityReport {
    let mut report = ValidityReport {
        topology_ok: brep.validate_topology().is_ok(),
        closed_shell: true,
        manifold_edges: true,
        consistent_orientation: true,
        finite_geometry: true,
        no_degenerate_faces: true,
        issues: Vec::new(),
    };

    if !report.topology_ok {
        report.issues.push("topology invariants violated".into());
    }

    // Half-edge incidence per edge.
    let mut incidence: HashMap<u32, Vec<usize>> = HashMap::new();
    for (idx, he) in brep.halfedges.iter().enumerate() {
        incidence.entry(he.edge).or_default().push(idx);
    }

    for (edge_id, halfedges) in &incidence {
        match halfedges.len() {
            1 => {
                report.closed_shell = false;
                report
                    .issues
                    .push(format!("edge {} is a boundary (open shell)", edge_id));
            }
            2 => {
                let a = &brep.halfedges[halfedges[0]];
                let b = &brep.halfedges[halfedges[1]];
                if !(a.from == b.to && a.to == b.from) {
                    report.consistent_orientation = false;
                    report
                        .issues
                        .push(format!("edge {} has inconsistent orientation", edge_id));
                }
            }
            n => {
                report.manifold_edges = false;
                report.issues.push(format!(
                    "edge {} is non-manifold ({} half-edges)",
                    edge_id, n
                ));
            }
        }
    }

    for vertex in &brep.vertices {
        let p = vertex.position;
        if !(p.x.is_finite() && p.y.is_finite() && p.z.is_finite()) {
            report.finite_geometry = false;
            report
                .issues
                .push(format!("vertex {} has non-finite coordinates", vertex.id));
        }
    }

    for face in &brep.faces {
        let verts = brep.get_vertices_by_face_id(face.id);
        if verts.len() >= 3 && polygon_area(&verts) <= AREA_EPSILON {
            report.no_degenerate_faces = false;
            report
                .issues
                .push(format!("face {} is degenerate (~zero area)", face.id));
        }
    }

    report
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HealReport {
    pub welded_vertices: usize,
    pub recomputed_normals: bool,
    pub actions: Vec<String>,
}

/// Heals common defects: welds near-coincident vertices (a frequent artifact of
/// quantized/generated input) and recomputes face normals. Returns the healed
/// B-rep and a report of what changed. Structural repairs beyond welding
/// (shell sewing, loop-orientation reversal) are intentionally out of scope so
/// the healer stays conservative and the validity signal honest.
pub fn heal(brep: &Brep, tolerance: f64) -> (Brep, HealReport) {
    let mut report = HealReport::default();
    let mut healed = brep.clone();

    let remap = weld_map(&healed.vertices, tolerance);
    let welds = remap
        .iter()
        .enumerate()
        .filter(|(i, canonical)| **canonical != *i as u32)
        .count();

    if welds > 0 {
        // Rewrite half-edge endpoints and vertex outgoing references through the
        // canonical map, then compact the vertex list.
        for he in &mut healed.halfedges {
            he.from = remap[he.from as usize];
            he.to = remap[he.to as usize];
        }

        let mut compact: Vec<u32> = vec![u32::MAX; healed.vertices.len()];
        let mut new_vertices = Vec::new();
        for (old_id, canonical) in remap.iter().enumerate() {
            if *canonical == old_id as u32 {
                compact[old_id] = new_vertices.len() as u32;
                let mut v = healed.vertices[old_id].clone();
                v.id = compact[old_id];
                new_vertices.push(v);
            }
        }
        for he in &mut healed.halfedges {
            he.from = compact[remap[he.from as usize] as usize];
            he.to = compact[remap[he.to as usize] as usize];
        }
        for v in &mut new_vertices {
            if let Some(outgoing) = v.outgoing_halfedge {
                let _ = outgoing; // half-edge ids unchanged; endpoints already remapped
            }
        }
        healed.vertices = new_vertices;
        report.welded_vertices = welds;
        report
            .actions
            .push(format!("welded {} coincident vertices", welds));
    }

    if !healed.faces.is_empty() {
        healed.recompute_face_normals();
        report.recomputed_normals = true;
        report.actions.push("recomputed face normals".into());
    }

    (healed, report)
}

/// Builds a vertex remap where each entry points to the lowest-index vertex
/// within tolerance of it. The threshold for a pair is the *looser* of the
/// global tolerance and either vertex's own per-vertex tolerance (D2) — so an
/// imprecise vertex (e.g. from a boolean intersection) welds at its own scale.
fn weld_map(vertices: &[super::Vertex], tolerance: f64) -> Vec<u32> {
    let mut remap: Vec<u32> = (0..vertices.len() as u32).collect();
    for i in 0..vertices.len() {
        if remap[i] != i as u32 {
            continue;
        }
        for j in (i + 1)..vertices.len() {
            if remap[j] != j as u32 {
                continue;
            }
            let a = vertices[i].position;
            let b = vertices[j].position;
            let dx = a.x - b.x;
            let dy = a.y - b.y;
            let dz = a.z - b.z;
            let pair_tol = tolerance
                .max(vertices[i].tolerance.unwrap_or(0.0))
                .max(vertices[j].tolerance.unwrap_or(0.0));
            if dx * dx + dy * dy + dz * dz <= pair_tol * pair_tol {
                remap[j] = i as u32;
            }
        }
    }
    remap
}

fn polygon_area(points: &[Vector3]) -> f64 {
    // Newell's area via the magnitude of the summed cross products.
    let mut nx = 0.0;
    let mut ny = 0.0;
    let mut nz = 0.0;
    let n = points.len();
    for i in 0..n {
        let a = points[i];
        let b = points[(i + 1) % n];
        nx += a.y * b.z - a.z * b.y;
        ny += a.z * b.x - a.x * b.z;
        nz += a.x * b.y - a.y * b.x;
    }
    0.5 * (nx * nx + ny * ny + nz * nz).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brep::BrepBuilder;
    use crate::primitives::cylinder::OGCylinder;
    use uuid::Uuid;

    fn closed_tetra() -> Brep {
        let mut b = BrepBuilder::new(Uuid::new_v4());
        b.add_vertices(&[
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.5, 0.866, 0.0),
            Vector3::new(0.5, 0.288, 0.816),
        ]);
        b.add_face(&[0, 2, 1], &[]).unwrap();
        b.add_face(&[0, 1, 3], &[]).unwrap();
        b.add_face(&[1, 2, 3], &[]).unwrap();
        b.add_face(&[2, 0, 3], &[]).unwrap();
        b.build().unwrap()
    }

    #[test]
    fn closed_solid_is_valid() {
        let report = check_validity(&closed_tetra());
        assert!(report.is_valid(), "tetra valid: {:?}", report.issues);
        assert!(report.closed_shell);
        assert!(report.manifold_edges);
        assert!(report.consistent_orientation);
    }

    #[test]
    fn cylinder_primitive_is_valid() {
        let mut cyl = OGCylinder::new("v-cyl".into());
        cyl.set_config(
            Vector3::new(0.0, 0.0, 0.0),
            1.0,
            2.0,
            2.0 * std::f64::consts::PI,
            24,
        )
        .unwrap();
        let report = check_validity(cyl.brep());
        assert!(report.closed_shell, "cylinder closed: {:?}", report.issues);
        assert!(report.manifold_edges);
    }

    #[test]
    fn open_shell_is_flagged_invalid() {
        // A single triangle face: its three edges are boundaries (one half-edge).
        let mut b = BrepBuilder::new(Uuid::new_v4());
        b.add_vertices(&[
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
        ]);
        b.add_face(&[0, 1, 2], &[]).unwrap();
        let brep = b.build().unwrap();

        let report = check_validity(&brep);
        assert!(!report.closed_shell);
        assert!(!report.is_valid());
        assert!(report.issues.iter().any(|i| i.contains("open shell")));
    }

    #[test]
    fn per_vertex_tolerance_widens_the_weld() {
        // Two vertices 5e-4 apart: too far for a 1e-6 global tolerance, but one
        // vertex declares a 1e-3 per-vertex tolerance, so they weld (D2).
        let mut b = BrepBuilder::new(Uuid::new_v4());
        b.add_vertices(&[
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
        ]);
        b.add_face(&[0, 1, 2], &[]).unwrap();
        let mut brep = b.build().unwrap();
        let mut imprecise = super::super::Vertex::new(3, Vector3::new(5.0e-4, 0.0, 0.0));
        imprecise.tolerance = Some(1.0e-3);
        brep.vertices.push(imprecise);

        // Global tolerance alone (1e-6) would NOT weld them.
        let (_, tight) = heal(&brep, 1.0e-6);
        // ...but the per-vertex 1e-3 tolerance does.
        assert_eq!(tight.welded_vertices, 1, "per-vertex tolerance honored");
    }

    #[test]
    fn healing_welds_coincident_vertices() {
        // Two vertices a hair apart get welded; the rest is untouched.
        let mut b = BrepBuilder::new(Uuid::new_v4());
        b.add_vertices(&[
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
        ]);
        b.add_face(&[0, 1, 2], &[]).unwrap();
        let mut brep = b.build().unwrap();
        // Inject a near-duplicate of vertex 0.
        brep.vertices
            .push(super::super::Vertex::new(3, Vector3::new(1.0e-7, 0.0, 0.0)));

        let before = brep.vertices.len();
        let (_healed, report) = heal(&brep, 1.0e-6);
        assert_eq!(report.welded_vertices, 1);
        assert!(report.recomputed_normals);
        assert!(before > _healed.vertices.len());
    }
}
