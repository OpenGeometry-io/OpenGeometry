use super::{finite, MathError};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Interval {
    pub lo: f64,
    pub hi: f64,
}

pub fn next_up(x: f64) -> f64 {
    if x.is_nan() || x == f64::INFINITY {
        return x;
    }
    if x == 0.0 {
        return f64::from_bits(1);
    }
    f64::from_bits(if x > 0.0 {
        x.to_bits() + 1
    } else {
        x.to_bits() - 1
    })
}

pub fn next_down(x: f64) -> f64 {
    -next_up(-x)
}

impl Interval {
    pub fn new(lo: f64, hi: f64) -> Result<Self, MathError> {
        finite(lo)?;
        finite(hi)?;
        if lo > hi {
            return Err(MathError::InvalidInterval);
        }
        Ok(Self { lo, hi })
    }

    pub fn point(x: f64) -> Result<Self, MathError> {
        Self::new(x, x)
    }

    pub fn contains(self, value: f64) -> bool {
        self.lo <= value && value <= self.hi
    }

    pub fn overlaps(self, other: Self) -> bool {
        self.lo <= other.hi && other.lo <= self.hi
    }

    pub fn width(self) -> f64 {
        self.hi - self.lo
    }

    pub fn midpoint(self) -> f64 {
        self.lo * 0.5 + self.hi * 0.5
    }

    pub fn hull(self, other: Self) -> Self {
        Self {
            lo: self.lo.min(other.lo),
            hi: self.hi.max(other.hi),
        }
    }

    fn rounded(lo: f64, hi: f64) -> Result<Self, MathError> {
        Self::new(next_down(finite(lo)?), next_up(finite(hi)?))
    }

    pub fn add(self, other: Self) -> Result<Self, MathError> {
        Self::rounded(self.lo + other.lo, self.hi + other.hi)
    }

    pub fn sub(self, other: Self) -> Result<Self, MathError> {
        Self::rounded(self.lo - other.hi, self.hi - other.lo)
    }

    pub fn mul(self, other: Self) -> Result<Self, MathError> {
        let products = [
            self.lo * other.lo,
            self.lo * other.hi,
            self.hi * other.lo,
            self.hi * other.hi,
        ];
        for p in products {
            finite(p)?;
        }
        Self::rounded(
            products.into_iter().fold(f64::INFINITY, f64::min),
            products.into_iter().fold(f64::NEG_INFINITY, f64::max),
        )
    }

    pub fn div(self, other: Self) -> Result<Self, MathError> {
        if other.contains(0.0) {
            return Err(MathError::DivisionByZero);
        }
        self.mul(Self::rounded(1.0 / other.hi, 1.0 / other.lo)?)
    }

    pub fn square(self) -> Result<Self, MathError> {
        let lower = if self.contains(0.0) {
            0.0
        } else {
            (self.lo * self.lo).min(self.hi * self.hi)
        };
        let upper = (self.lo * self.lo).max(self.hi * self.hi);
        let mut result = Self::rounded(lower, upper)?;
        result.lo = result.lo.max(0.0);
        Ok(result)
    }

    pub fn sqrt(self) -> Result<Self, MathError> {
        if self.lo < 0.0 {
            return Err(MathError::InvalidInterval);
        }
        let mut result = Self::rounded(self.lo.sqrt(), self.hi.sqrt())?;
        result.lo = result.lo.max(0.0);
        Ok(result)
    }

    pub fn sin(self) -> Result<Self, MathError> {
        Self::new(self.lo, self.hi)?;
        let pi = Self::new(
            next_down(std::f64::consts::PI),
            next_up(std::f64::consts::PI),
        )?;
        let tau = pi.mul(Self::point(2.0)?)?;
        // A broad bound avoids unreliable argument reduction for large inputs.
        if self.width() >= tau.lo || self.lo.abs().max(self.hi.abs()) > 32.0 * pi.lo {
            return Self::new(-1.0, 1.0);
        }
        let mut result = sin_endpoint(self.lo, tau)?.hull(sin_endpoint(self.hi, tau)?);
        let first = (self.lo / std::f64::consts::PI - 0.5).floor() as i32 - 1;
        let last = (self.hi / std::f64::consts::PI - 0.5).ceil() as i32 + 1;
        for k in first..=last {
            let critical = pi.mul(Self::point(k as f64 + 0.5)?)?;
            if self.overlaps(critical) {
                result = result.hull(Self::point(if k % 2 == 0 { 1.0 } else { -1.0 })?);
            }
        }
        Self::new(result.lo.max(-1.0), result.hi.min(1.0))
    }

    pub fn cos(self) -> Result<Self, MathError> {
        let pi = Self::new(
            next_down(std::f64::consts::PI),
            next_up(std::f64::consts::PI),
        )?;
        self.add(pi.mul(Self::point(0.5)?)?)?.sin()
    }
}

fn sin_endpoint(x: f64, tau: Interval) -> Result<Interval, MathError> {
    let k = (x / std::f64::consts::TAU).round();
    let y = Interval::point(x)?.sub(tau.mul(Interval::point(k)?)?)?;
    let y2 = y.square()?;
    let mut term = y;
    let mut sum = term;
    for n in 1..=20 {
        let denominator = (2 * n * (2 * n + 1)) as f64;
        term = term.mul(y2)?.div(Interval::point(-denominator)?)?;
        sum = sum.add(term)?;
    }
    let next = term.mul(y2)?.div(Interval::point((42 * 43) as f64)?)?;
    let remainder = next.lo.abs().max(next.hi.abs());
    sum.add(Interval::new(-remainder, remainder)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic_encloses_cancellation_and_extrema() {
        let a = Interval::point(0.1).unwrap();
        let b = Interval::point(0.2).unwrap();
        assert!(a.add(b).unwrap().contains(0.3));
        assert!(a.sub(a).unwrap().contains(0.0));
        assert_eq!(
            a.div(Interval::new(-1.0, 1.0).unwrap()),
            Err(MathError::DivisionByZero)
        );
        assert!(Interval::new(-2.0, 3.0)
            .unwrap()
            .square()
            .unwrap()
            .contains(9.0));
        assert!(Interval::new(0.0, std::f64::consts::PI)
            .unwrap()
            .sin()
            .unwrap()
            .contains(1.0));
        assert!(Interval::new(-1000.0, 1000.0)
            .unwrap()
            .cos()
            .unwrap()
            .contains(-1.0));
    }

    #[test]
    fn trigonometric_bounds_cover_dense_independent_samples() {
        for i in -256..256 {
            let lo = i as f64 * 0.125;
            let hi = lo + 0.375;
            let interval = Interval::new(lo, hi).unwrap();
            let sin = interval.sin().unwrap();
            let cos = interval.cos().unwrap();
            for j in 0..=32 {
                let x = lo + (hi - lo) * j as f64 / 32.0;
                assert!(sin.contains(x.sin()), "{interval:?} {sin:?} {x}");
                assert!(cos.contains(x.cos()), "{interval:?} {cos:?} {x}");
            }
        }
    }

    #[test]
    fn floating_neighbors_cover_zero_and_negative_numbers() {
        assert!(next_up(0.0) > 0.0);
        assert!(next_down(0.0) < 0.0);
        assert!(next_up(-1.0) > -1.0);
        assert!(next_down(-1.0) < -1.0);
        assert!(Interval::point(2.0)
            .unwrap()
            .sqrt()
            .unwrap()
            .contains(2.0_f64.sqrt()));
    }
}
