use super::{
    finite,
    interval::Interval,
    predicates::{discriminant_estimate, Sign},
    MathError,
};

#[derive(Clone, Debug, PartialEq)]
pub enum QuadraticRoots {
    Empty,
    One(f64),
    Two([f64; 2]),
    IdenticallyZero,
}

pub fn quadratic(a: f64, b: f64, c: f64) -> Result<QuadraticRoots, MathError> {
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

pub fn polynomial_bounds(coefficients: &[f64], domain: Interval) -> Result<Interval, MathError> {
    Interval::new(domain.lo, domain.hi)?;
    let mut result = Interval::point(0.0)?;
    for &coefficient in coefficients.iter().rev() {
        result = result.mul(domain)?.add(Interval::point(coefficient)?)?;
    }
    Ok(result)
}

#[derive(Clone, Debug)]
pub struct RootCandidates {
    pub intervals: Vec<Interval>,
    pub visited: usize,
}

// Every root is enclosed; a candidate interval need not actually contain a root.
pub fn isolate_candidates(
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
    Ok(RootCandidates { intervals, visited })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_quadratic_preserves_small_root_and_contacts() {
        let QuadraticRoots::Two(roots) = quadratic(1.0, -1e8, 1.0).unwrap() else {
            panic!("two roots expected")
        };
        assert!((roots[0] - 1e-8).abs() < 1e-22);
        assert!((roots[1] - 1e8).abs() < 1e-7);
        assert_eq!(quadratic(1.0, 2.0, 1.0), Ok(QuadraticRoots::One(-1.0)));
        assert_eq!(quadratic(1.0, 0.0, 1.0), Ok(QuadraticRoots::Empty));
        assert_eq!(
            quadratic(0.0, 0.0, 0.0),
            Ok(QuadraticRoots::IdenticallyZero)
        );
    }

    #[test]
    fn subdivision_preserves_simple_and_even_multiplicity_roots() {
        let roots = isolate_candidates(
            &[0.0, 0.0, -1.0, 0.0, 1.0],
            Interval::new(-2.0, 2.0).unwrap(),
            1e-4,
            100_000,
        )
        .unwrap();
        for expected in [-1.0, 0.0, 1.0] {
            assert!(roots.intervals.iter().any(|i| i.contains(expected)));
        }
        assert_eq!(
            isolate_candidates(
                &[1.0, 0.0, 1.0],
                Interval::new(-2.0, 2.0).unwrap(),
                1e-4,
                100
            )
            .unwrap()
            .intervals
            .len(),
            0
        );
        assert!(matches!(
            isolate_candidates(
                &[-1.0, 0.0, 1.0],
                Interval::new(-2.0, 2.0).unwrap(),
                1e-8,
                1
            ),
            Err(MathError::IterationLimit)
        ));
    }
}
