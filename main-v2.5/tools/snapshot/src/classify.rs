use crate::digest::bits_list;
use crate::kernel::{classify_point, BrepEnvelope, PointClassification};
use crate::record::Record;

const STEPS: usize = 7;
const PADDING: f64 = 0.125;

fn symbol(classification: PointClassification) -> char {
    match classification {
        PointClassification::Inside => 'I',
        PointClassification::Outside => 'O',
        PointClassification::Boundary => 'B',
        PointClassification::Unknown => 'U',
    }
}

fn grid_axes(lo: [f64; 3], hi: [f64; 3]) -> [[f64; STEPS]; 3] {
    let span = (0..3)
        .map(|axis| hi[axis] - lo[axis])
        .fold(0.0_f64, f64::max);
    let pad = span * PADDING;
    std::array::from_fn(|axis| {
        let start = lo[axis] - pad;
        let width = hi[axis] - lo[axis] + 2.0 * pad;
        std::array::from_fn(|step| start + width * step as f64 / (STEPS - 1) as f64)
    })
}

pub(crate) fn classify_grid(record: &mut Record, label: &str, brep: &BrepEnvelope) {
    let title = format!("{label}.classify");
    let bounds = match brep.bounds() {
        Ok(Some(bounds)) => bounds,
        Ok(None) => {
            record.section(&title);
            record.line("bounds: none");
            return;
        }
        Err(error) => {
            record.error(&title, error);
            return;
        }
    };
    let lo = std::array::from_fn(|axis| bounds.axes[axis].lo);
    let hi = std::array::from_fn(|axis| bounds.axes[axis].hi);
    let axes = grid_axes(lo, hi);
    record.section(&title);
    for (name, values) in ["x", "y", "z"].iter().zip(&axes) {
        record.field(name, bits_list(values));
    }
    let mut errors = Vec::new();
    for (i, x) in axes[0].iter().enumerate() {
        for (j, y) in axes[1].iter().enumerate() {
            let mut row = String::with_capacity(STEPS);
            for (k, z) in axes[2].iter().enumerate() {
                match classify_point(brep, [*x, *y, *z]) {
                    Ok(classification) => row.push(symbol(classification)),
                    Err(error) => {
                        row.push('E');
                        errors.push(format!("{i} {j} {k}: {error:?}"));
                    }
                }
            }
            record.line(&format!("{i} {j} {row}"));
        }
    }
    for error in errors {
        record.line(&error);
    }
}
