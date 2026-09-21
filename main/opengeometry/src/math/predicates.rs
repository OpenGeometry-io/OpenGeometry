use super::{finite, MathError};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Sign {
    Negative,
    Zero,
    Positive,
}

impl Sign {
    fn of(value: f64) -> Self {
        if value > 0.0 {
            Self::Positive
        } else if value < 0.0 {
            Self::Negative
        } else {
            Self::Zero
        }
    }
}

type Expansion = Vec<f64>;

fn two_sum(a: f64, b: f64) -> Result<(f64, f64), MathError> {
    let sum = finite(a + b)?;
    let bv = finite(sum - a)?;
    let av = finite(sum - bv)?;
    Ok((sum, finite((a - av) + (b - bv))?))
}

fn two_product(a: f64, b: f64) -> Result<(f64, f64), MathError> {
    let product = finite(a * b)?;
    // Below this bound the exact product's low bits may not fit in f64.
    if a != 0.0 && b != 0.0 && product.abs() < f64::MIN_POSITIVE * 18_014_398_509_481_984.0 {
        return Err(MathError::ArithmeticRange);
    }
    Ok((product, finite(a.mul_add(b, -product))?))
}

fn grow(expansion: &mut Expansion, value: f64) -> Result<(), MathError> {
    let mut q = value;
    let mut result = Vec::with_capacity(expansion.len() + 1);
    for &component in expansion.iter() {
        let (sum, error) = two_sum(q, component)?;
        if error != 0.0 {
            result.push(error);
        }
        q = sum;
    }
    if q != 0.0 || result.is_empty() {
        result.push(q);
    }
    *expansion = result;
    Ok(())
}

fn difference(a: f64, b: f64) -> Result<Expansion, MathError> {
    finite(a)?;
    finite(b)?;
    let (sum, error) = two_sum(a, -b)?;
    let mut expansion = Vec::with_capacity(2);
    if error != 0.0 {
        expansion.push(error);
    }
    expansion.push(sum);
    Ok(expansion)
}

fn add(a: &Expansion, b: &Expansion, sign: f64) -> Result<Expansion, MathError> {
    let mut result = a.clone();
    for component in b {
        grow(&mut result, component * sign)?;
    }
    Ok(result)
}

fn multiply(a: &Expansion, b: &Expansion) -> Result<Expansion, MathError> {
    let mut result = vec![0.0];
    for &x in a {
        for &y in b {
            let (product, error) = two_product(x, y)?;
            grow(&mut result, error)?;
            grow(&mut result, product)?;
        }
    }
    Ok(result)
}

fn determinant2(
    a: &Expansion,
    b: &Expansion,
    c: &Expansion,
    d: &Expansion,
) -> Result<Expansion, MathError> {
    add(&multiply(a, d)?, &multiply(b, c)?, -1.0)
}

fn sign(expansion: &Expansion) -> Sign {
    expansion
        .iter()
        .rev()
        .find(|&&v| v != 0.0)
        .map_or(Sign::Zero, |&v| Sign::of(v))
}

pub fn orient2d(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> Result<Sign, MathError> {
    for value in a.into_iter().chain(b).chain(c) {
        finite(value)?;
    }
    let acx = a[0] - c[0];
    let acy = a[1] - c[1];
    let bcx = b[0] - c[0];
    let bcy = b[1] - c[1];
    let left = acx * bcy;
    let right = acy * bcx;
    let det = left - right;
    let e = f64::EPSILON * 0.5;
    let bound = (3.0 + 16.0 * e) * e * (left.abs() + right.abs());
    if det.is_finite() && bound.is_normal() && det.abs() > bound {
        return Ok(Sign::of(det));
    }
    Ok(sign(&determinant2(
        &difference(a[0], c[0])?,
        &difference(a[1], c[1])?,
        &difference(b[0], c[0])?,
        &difference(b[1], c[1])?,
    )?))
}

pub fn orient3d(a: [f64; 3], b: [f64; 3], c: [f64; 3], d: [f64; 3]) -> Result<Sign, MathError> {
    for value in a.into_iter().chain(b).chain(c).chain(d) {
        finite(value)?;
    }
    let ad: [f64; 3] = std::array::from_fn(|i| a[i] - d[i]);
    let bd: [f64; 3] = std::array::from_fn(|i| b[i] - d[i]);
    let cd: [f64; 3] = std::array::from_fn(|i| c[i] - d[i]);
    let terms = [
        bd[0] * cd[1],
        cd[0] * bd[1],
        cd[0] * ad[1],
        ad[0] * cd[1],
        ad[0] * bd[1],
        bd[0] * ad[1],
    ];
    let det = ad[2] * (terms[0] - terms[1])
        + bd[2] * (terms[2] - terms[3])
        + cd[2] * (terms[4] - terms[5]);
    let permanent = (terms[0].abs() + terms[1].abs()) * ad[2].abs()
        + (terms[2].abs() + terms[3].abs()) * bd[2].abs()
        + (terms[4].abs() + terms[5].abs()) * cd[2].abs();
    let e = f64::EPSILON * 0.5;
    let bound = (7.0 + 56.0 * e) * e * permanent;
    if det.is_finite() && bound.is_normal() && det.abs() > bound {
        return Ok(Sign::of(det));
    }
    let mut vectors = Vec::with_capacity(3);
    for p in [a, b, c] {
        vectors.push([
            difference(p[0], d[0])?,
            difference(p[1], d[1])?,
            difference(p[2], d[2])?,
        ]);
    }
    let [a, b, c] = [&vectors[0], &vectors[1], &vectors[2]];
    let x = multiply(&a[0], &determinant2(&b[1], &b[2], &c[1], &c[2])?)?;
    let y = multiply(&a[1], &determinant2(&b[0], &b[2], &c[0], &c[2])?)?;
    let z = multiply(&a[2], &determinant2(&b[0], &b[1], &c[0], &c[1])?)?;
    Ok(sign(&add(&add(&x, &y, -1.0)?, &z, 1.0)?))
}

pub fn incircle2d(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> Result<Sign, MathError> {
    for value in a.into_iter().chain(b).chain(c).chain(d) {
        finite(value)?;
    }
    let ad = [a[0] - d[0], a[1] - d[1]];
    let bd = [b[0] - d[0], b[1] - d[1]];
    let cd = [c[0] - d[0], c[1] - d[1]];
    let lift = |p: [f64; 2]| p[0] * p[0] + p[1] * p[1];
    let [al, bl, cl] = [lift(ad), lift(bd), lift(cd)];
    let terms = [
        bd[0] * cd[1],
        cd[0] * bd[1],
        cd[0] * ad[1],
        ad[0] * cd[1],
        ad[0] * bd[1],
        bd[0] * ad[1],
    ];
    let det = al * (terms[0] - terms[1]) + bl * (terms[2] - terms[3]) + cl * (terms[4] - terms[5]);
    let permanent = (terms[0].abs() + terms[1].abs()) * al
        + (terms[2].abs() + terms[3].abs()) * bl
        + (terms[4].abs() + terms[5].abs()) * cl;
    let e = f64::EPSILON * 0.5;
    let bound = (10.0 + 96.0 * e) * e * permanent;
    if det.is_finite() && bound.is_normal() && det.abs() > bound {
        return Ok(Sign::of(det));
    }
    let mut rows = Vec::with_capacity(3);
    for p in [a, b, c] {
        let x = difference(p[0], d[0])?;
        let y = difference(p[1], d[1])?;
        let lift = add(&multiply(&x, &x)?, &multiply(&y, &y)?, 1.0)?;
        rows.push([x, y, lift]);
    }
    let [a, b, c] = [&rows[0], &rows[1], &rows[2]];
    let x = multiply(&a[2], &determinant2(&b[0], &b[1], &c[0], &c[1])?)?;
    let y = multiply(&b[2], &determinant2(&c[0], &c[1], &a[0], &a[1])?)?;
    let z = multiply(&c[2], &determinant2(&a[0], &a[1], &b[0], &b[1])?)?;
    Ok(sign(&add(&add(&x, &y, 1.0)?, &z, 1.0)?))
}

pub fn discriminant(a: f64, b: f64, c: f64) -> Result<Sign, MathError> {
    Ok(discriminant_estimate(a, b, c)?.0)
}

pub fn discriminant_estimate(a: f64, b: f64, c: f64) -> Result<(Sign, f64), MathError> {
    for value in [a, b, c] {
        finite(value)?;
    }
    let bb = multiply(&vec![b], &vec![b])?;
    let ac = multiply(&vec![a], &vec![c])?;
    let four_ac = multiply(&ac, &vec![4.0])?;
    let expansion = add(&bb, &four_ac, -1.0)?;
    Ok((sign(&expansion), finite(expansion.iter().copied().sum())?))
}

pub fn plane_sphere_relation(
    origin: [f64; 3],
    normal: [f64; 3],
    center: [f64; 3],
    radius: f64,
) -> Result<Sign, MathError> {
    finite(radius)?;
    let mut distance = vec![0.0];
    let mut length_squared = vec![0.0];
    for i in 0..3 {
        finite(normal[i])?;
        distance = add(
            &distance,
            &multiply(&difference(center[i], origin[i])?, &vec![normal[i]])?,
            1.0,
        )?;
        length_squared = add(
            &length_squared,
            &multiply(&vec![normal[i]], &vec![normal[i]])?,
            1.0,
        )?;
    }
    let radius_squared = multiply(&vec![radius], &vec![radius])?;
    Ok(sign(&add(
        &multiply(&distance, &distance)?,
        &multiply(&radius_squared, &length_squared)?,
        -1.0,
    )?))
}

pub fn sphere_sphere_relation(
    a: [f64; 3],
    ra: f64,
    b: [f64; 3],
    rb: f64,
) -> Result<[Sign; 2], MathError> {
    finite(ra)?;
    finite(rb)?;
    let mut distance_squared = vec![0.0];
    for i in 0..3 {
        let d = difference(a[i], b[i])?;
        distance_squared = add(&distance_squared, &multiply(&d, &d)?, 1.0)?;
    }
    let sum = add(&vec![ra], &vec![rb], 1.0)?;
    let difference = difference(ra, rb)?;
    Ok([
        sign(&add(&distance_squared, &multiply(&sum, &sum)?, -1.0)?),
        sign(&add(
            &distance_squared,
            &multiply(&difference, &difference)?,
            -1.0,
        )?),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_retains_exact_orientation() {
        let m = 134_217_728.0;
        assert_eq!((m + 1.0) * (m - 1.0) - m * m, 0.0);
        assert_eq!(
            orient2d([0.0, 0.0], [m + 1.0, m], [m, m - 1.0]),
            Ok(Sign::Negative)
        );
        assert_eq!(
            orient2d([0.0, 0.0], [m, m - 1.0], [m + 1.0, m]),
            Ok(Sign::Positive)
        );
    }

    #[test]
    fn tetrahedron_and_cocircular_signs() {
        let d = [0.0, 0.0, 0.0];
        assert_eq!(
            orient3d([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], d),
            Ok(Sign::Positive)
        );
        assert_eq!(
            incircle2d([1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -1.0]),
            Ok(Sign::Zero)
        );
        assert_eq!(
            incircle2d([1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, 0.0]),
            Ok(Sign::Positive)
        );
    }

    #[test]
    fn input_and_arithmetic_limits_are_explicit() {
        assert_eq!(
            orient2d([f64::NAN, 0.0], [1.0, 0.0], [0.0, 1.0]),
            Err(MathError::NonFinite)
        );
        assert_eq!(discriminant(1.0, 2.0, 1.0), Ok(Sign::Zero));
        assert_eq!(
            discriminant(1.0, 2.0, 1.0 + f64::EPSILON),
            Ok(Sign::Negative)
        );
        assert_eq!(
            discriminant(f64::MIN_POSITIVE, 0.0, f64::MIN_POSITIVE),
            Err(MathError::ArithmeticRange)
        );
    }

    #[test]
    fn exact_integer_grid_agrees_with_integer_determinants() {
        let mut seed = 7_u64;
        for _ in 0..2048 {
            let mut p = [[0_i64; 2]; 3];
            for row in &mut p {
                for x in row {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    *x = ((seed >> 32) % 100_000) as i64 - 50_000;
                }
            }
            let det = (p[0][0] - p[2][0]) as i128 * (p[1][1] - p[2][1]) as i128
                - (p[0][1] - p[2][1]) as i128 * (p[1][0] - p[2][0]) as i128;
            let expected = match det.cmp(&0) {
                std::cmp::Ordering::Less => Sign::Negative,
                std::cmp::Ordering::Equal => Sign::Zero,
                std::cmp::Ordering::Greater => Sign::Positive,
            };
            let f = p.map(|v| v.map(|x| x as f64));
            assert_eq!(orient2d(f[0], f[1], f[2]), Ok(expected));
        }
    }
}
