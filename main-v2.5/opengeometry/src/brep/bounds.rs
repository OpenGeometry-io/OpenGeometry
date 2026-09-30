use crate::math::{Interval, Point3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PatchBounds {
    pub axes: [Interval; 3],
}
impl PatchBounds {
    pub fn contains(&self, point: Point3) -> bool {
        (0..3).all(|i| self.axes[i].contains(point[i]))
    }

    pub(crate) fn overlaps(&self, other: PatchBounds, tolerance: f64) -> bool {
        (0..3).all(|axis| {
            self.axes[axis].lo <= other.axes[axis].hi + tolerance
                && self.axes[axis].hi + tolerance >= other.axes[axis].lo
        })
    }
}
