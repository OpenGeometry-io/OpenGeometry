use super::{finite, MathError};

#[derive(Clone, Debug)]
pub struct LinearSolution<const N: usize> {
    pub value: [f64; N],
    pub residual: f64,
    pub minimum_scaled_pivot: f64,
}

pub fn solve<const N: usize>(
    matrix: [[f64; N]; N],
    rhs: [f64; N],
) -> Result<LinearSolution<N>, MathError> {
    if N == 0 || N > 4 {
        return Err(MathError::InvalidDimension);
    }
    let mut a = matrix;
    let mut b = rhs;
    let mut minimum_scaled_pivot = f64::INFINITY;
    for i in 0..N {
        let mut scale: f64 = 0.0;
        for v in a[i] {
            scale = scale.max(finite(v)?.abs());
        }
        finite(b[i])?;
        if scale == 0.0 {
            return Err(MathError::SingularSystem);
        }
        for v in &mut a[i] {
            *v = finite(*v / scale)?;
        }
        b[i] = finite(b[i] / scale)?;
    }
    for column in 0..N {
        let mut pivot = column;
        for row in column + 1..N {
            if a[row][column].abs() > a[pivot][column].abs() {
                pivot = row;
            }
        }
        let size = a[pivot][column].abs();
        if size <= 64.0 * f64::EPSILON {
            return Err(MathError::SingularSystem);
        }
        minimum_scaled_pivot = minimum_scaled_pivot.min(size);
        a.swap(column, pivot);
        b.swap(column, pivot);
        for row in column + 1..N {
            let factor = a[row][column] / a[column][column];
            a[row][column] = 0.0;
            for j in column + 1..N {
                a[row][j] = finite((-factor).mul_add(a[column][j], a[row][j]))?;
            }
            b[row] = finite((-factor).mul_add(b[column], b[row]))?;
        }
    }
    let mut value = [0.0; N];
    for i in (0..N).rev() {
        let mut r = b[i];
        for (j, v) in value.iter().enumerate().skip(i + 1) {
            r = finite((-a[i][j]).mul_add(*v, r))?;
        }
        value[i] = finite(r / a[i][i])?;
    }
    let mut residual: f64 = 0.0;
    for i in 0..N {
        let mut r = -rhs[i];
        for (j, v) in value.iter().enumerate() {
            r = finite(matrix[i][j].mul_add(*v, r))?;
        }
        residual = residual.max(r.abs());
    }
    Ok(LinearSolution {
        value,
        residual,
        minimum_scaled_pivot,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pivoting_and_row_scaling_recover_known_solution() {
        let matrix = [
            [0.0, 2.0, 1.0, 0.0],
            [1e-12, 0.0, 0.0, 2e-12],
            [1.0, 1.0, 1.0, 1.0],
            [2.0, 0.0, 3.0, 1.0],
        ];
        let expected = [1.0, -2.0, 3.0, 0.5];
        let rhs = matrix.map(|row| row.into_iter().zip(expected).map(|(a, x)| a * x).sum());
        let solution = solve(matrix, rhs).unwrap();
        for (a, b) in solution.value.into_iter().zip(expected) {
            assert!((a - b).abs() < 1e-12);
        }
        assert!(solution.residual < 1e-12);
    }

    #[test]
    fn singular_and_nonfinite_systems_return_errors() {
        assert!(matches!(
            solve([[1.0, 2.0], [2.0, 4.0]], [1.0, 2.0]),
            Err(MathError::SingularSystem)
        ));
        assert!(matches!(
            solve([[f64::NAN]], [1.0]),
            Err(MathError::NonFinite)
        ));
    }
}
