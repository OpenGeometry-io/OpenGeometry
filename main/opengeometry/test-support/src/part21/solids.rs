use super::document::Document;
use super::lexer::{fields, reference, references};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SolidEntry {
    pub voids: usize,
    pub with_voids: bool,
}

impl Document {
    pub fn solids(&self) -> Result<Vec<SolidEntry>, String> {
        let mut result = Vec::new();
        for expression in self.entities.values() {
            if expression.starts_with("MANIFOLD_SOLID_BREP(") {
                result.push(SolidEntry {
                    voids: 0,
                    with_voids: false,
                });
            } else if expression.starts_with("BREP_WITH_VOIDS(") {
                result.push(SolidEntry {
                    voids: references(&fields(expression)?[2]).len(),
                    with_voids: true,
                });
            }
        }
        Ok(result)
    }

    pub(super) fn check_solids(&self) -> Result<(), String> {
        let mut uses = BTreeMap::new();
        for (id, expression) in &self.entities {
            if expression.starts_with("ORIENTED_CLOSED_SHELL(") {
                let shell = reference(&fields(expression)?[2])?;
                if !self.entity(shell)?.starts_with("CLOSED_SHELL(") {
                    return Err(format!(
                        "ORIENTED_CLOSED_SHELL #{id} does not orient a CLOSED_SHELL"
                    ));
                }
                uses.entry(*id).or_insert(0);
            } else if expression.starts_with("BREP_WITH_VOIDS(") {
                let voids = references(&fields(expression)?[2]);
                if voids.is_empty() {
                    return Err(format!("BREP_WITH_VOIDS #{id} has no void"));
                }
                for void in voids {
                    if !self.entity(void)?.starts_with("ORIENTED_CLOSED_SHELL(") {
                        return Err(format!(
                            "void #{void} of #{id} is not an ORIENTED_CLOSED_SHELL"
                        ));
                    }
                    *uses.entry(void).or_insert(0) += 1;
                }
            }
        }
        match uses.into_iter().find(|(_, count)| *count != 1) {
            Some((shell, count)) => Err(format!(
                "ORIENTED_CLOSED_SHELL #{shell} is used by {count} solids"
            )),
            None => Ok(()),
        }
    }
}
