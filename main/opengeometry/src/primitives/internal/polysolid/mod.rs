mod checks;
mod outline;
mod path;
mod stroke;

#[cfg(test)]
mod tests;

use crate::brep::{Accuracy, BrepEnvelope, Frame3};
use outline::arc_polysolid;
pub use path::{PolysolidError, PolysolidJustify, PolysolidPath};
use stroke::stroked_polysolid;

pub fn polysolid(
    id: String,
    frame: Frame3,
    path: &PolysolidPath,
    justify: PolysolidJustify,
    sizes: [f64; 2],
    accuracy: Accuracy,
) -> Result<BrepEnvelope, PolysolidError> {
    if path.segments.iter().any(|segment| segment.arc.is_some()) {
        arc_polysolid(id, frame, path, justify, sizes, accuracy)
    } else {
        stroked_polysolid(id, frame, path, justify, sizes, accuracy)
    }
}
