use super::error::{finite, MathError};
use super::interval::Interval;
use super::predicates::{discriminant_estimate, Sign};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum QuadraticRoots {
    Empty,
    One(f64),
    Two([f64; 2]),
    IdenticallyZero,
}

pub(crate) fn quadratic(a: f64, b: f64, c: f64) -> Result<QuadraticRoots, MathError> {
    for value in [a, b, c] {
        finite(value)?;
    }
    if a == 0.0 {
        return if b != 0.0 {
            Ok(QuadraticRoots::One(finite(-c / b)?))
        } else if c == 0.0 {
            Ok(QuadraticRoots::IdenticallyZero)
        } else {
            Ok(QuadraticRoots::Empty)
        };
    }
    let maximum = a.abs().max(b.abs()).max(c.abs());
    let exponent = maximum.to_bits() & 0x7ff0000000000000;
    let scale = if exponent == 0 {
        f64::MIN_POSITIVE
    } else {
        f64::from_bits(exponent)
    };
    let [aa, bb, cc] = [a / scale, b / scale, c / scale];
    for (original, normalized) in [a, b, c].into_iter().zip([aa, bb, cc]) {
        if original != 0.0 && !normalized.is_normal() {
            return Err(MathError::ArithmeticRange);
        }
    }
    let (relation, d) = discriminant_estimate(aa, bb, cc)?;
    match relation {
        Sign::Negative => Ok(QuadraticRoots::Empty),
        Sign::Zero => Ok(QuadraticRoots::One(finite((-0.5 * b) / a)?)),
        Sign::Positive => {
            if d <= 0.0 || !d.is_finite() {
                return Err(MathError::ArithmeticRange);
            }
            let q = -0.5 * (bb + d.sqrt().copysign(bb));
            let mut roots = [finite(q / aa)?, finite(cc / q)?];
            if roots[0] > roots[1] {
                roots.swap(0, 1);
            }
            Ok(QuadraticRoots::Two(roots))
        }
    }
}

fn polynomial_bounds(coefficients: &[f64], domain: Interval) -> Result<Interval, MathError> {
    Interval::new(domain.lo, domain.hi)?;
    let mut result = Interval::point(0.0)?;
    for &coefficient in coefficients.iter().rev() {
        result = result
            .mul_interval(domain)?
            .add_interval(Interval::point(coefficient)?)?;
    }
    Ok(result)
}

#[derive(Clone, Debug)]
pub struct RootCandidates {
    pub(crate) intervals: Vec<Interval>,
}

pub(crate) fn isolate_candidates(
    coefficients: &[f64],
    domain: Interval,
    width: f64,
    max_visits: usize,
) -> Result<RootCandidates, MathError> {
    Interval::new(domain.lo, domain.hi)?;
    if !width.is_finite() || width <= 0.0 || coefficients.is_empty() {
        return Err(MathError::InvalidInterval);
    }
    for &c in coefficients {
        finite(c)?;
    }
    if coefficients.iter().all(|&c| c == 0.0) {
        return Err(MathError::SingularSystem);
    }
    let mut pending = vec![domain];
    let mut intervals: Vec<Interval> = Vec::new();
    let mut visited = 0;
    while let Some(current) = pending.pop() {
        visited += 1;
        if visited > max_visits {
            return Err(MathError::IterationLimit);
        }
        if !polynomial_bounds(coefficients, current)?.contains(0.0) {
            continue;
        }
        if current.width() <= width {
            if let Some(previous) = intervals.last_mut() {
                if previous.hi >= current.lo {
                    *previous = previous.hull(current);
                    continue;
                }
            }
            intervals.push(current);
        } else {
            let middle = current.midpoint();
            if middle <= current.lo || middle >= current.hi {
                return Err(MathError::ArithmeticRange);
            }
            pending.push(Interval::new(middle, current.hi)?);
            pending.push(Interval::new(current.lo, middle)?);
        }
    }
    Ok(RootCandidates { intervals })
}
