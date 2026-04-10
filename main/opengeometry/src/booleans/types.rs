use openmaths::Vector3;
use serde::{Deserialize, Serialize};

use crate::brep::Brep;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BooleanOperation {
    Union,
    Intersection,
    Subtraction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BooleanOperandKind {
    ClosedSolid,
    PlanarFace,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BooleanOptions {
    pub tolerance: Option<f64>,
    pub merge_coplanar_faces: bool,
}

impl Default for BooleanOptions {
    fn default() -> Self {
        Self {
            tolerance: None,
            merge_coplanar_faces: true,
        }
    }
}

impl BooleanOptions {
    /// Computes the working tolerance from the operand bounds when the caller
    /// does not supply one explicitly.
    pub fn resolve_tolerance(&self, lhs: &Brep, rhs: &Brep) -> f64 {
        self.resolve_tolerance_many(&[lhs, rhs])
    }

    /// Computes the working tolerance from the combined bounds of multiple
    /// operands when the caller does not supply one explicitly.
    pub fn resolve_tolerance_many(&self, operands: &[&Brep]) -> f64 {
        self.tolerance.unwrap_or_else(|| {
            let mut min = Vector3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
            let mut max = Vector3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);

            for brep in operands {
                for vertex in &brep.vertices {
                    min.x = min.x.min(vertex.position.x);
                    min.y = min.y.min(vertex.position.y);
                    min.z = min.z.min(vertex.position.z);
                    max.x = max.x.max(vertex.position.x);
                    max.y = max.y.max(vertex.position.y);
                    max.z = max.z.max(vertex.position.z);
                }
            }

            if !min.x.is_finite() || !max.x.is_finite() {
                return 1.0e-9;
            }

            let dx = max.x - min.x;
            let dy = max.y - min.y;
            let dz = max.z - min.z;
            let diagonal = (dx * dx + dy * dy + dz * dz).sqrt();
            (diagonal * 1.0e-8).max(1.0e-9)
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BooleanReport {
    pub operation: BooleanOperation,
    pub operand_kind: BooleanOperandKind,
    pub input_face_count: usize,
    pub input_triangle_count: usize,
    pub output_face_count: usize,
    pub output_shell_count: usize,
    pub empty: bool,
}

#[derive(Clone)]
pub struct BooleanOutput {
    pub brep: Brep,
    pub report: BooleanReport,
}
