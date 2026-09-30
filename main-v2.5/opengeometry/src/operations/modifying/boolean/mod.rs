mod assembly;
mod batch;
mod dispatch;
mod handlers;
mod operands;
mod shell;
#[cfg(test)]
mod tests;
mod types;

pub use batch::{subtract_planar_cutters, subtract_planar_cutters_with_handlers};
pub use dispatch::{boolean_brep, boolean_brep_outcome_with_handlers, boolean_brep_with_handlers};
pub use handlers::{boolean_boxes, boolean_spheres};
pub use shell::shell_brep;
pub use types::{BooleanOp, BooleanReport, BooleanResult};
