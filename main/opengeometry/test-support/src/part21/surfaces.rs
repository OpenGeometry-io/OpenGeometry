use super::document::Document;
use super::lexer::{fields, references};

fn bernstein(t: f64) -> [f64; 4] {
    let s = 1.0 - t;
    [s * s * s, 3.0 * s * s * t, 3.0 * s * t * t, t * t * t]
}

fn span(knots: &[f64], multiplicities: &[f64], value: f64) -> Result<(usize, f64), String> {
    let last = knots.len().checked_sub(1).ok_or("empty knot vector")?;
    let piecewise = last > 0
        && knots.len() == multiplicities.len()
        && multiplicities
            .iter()
            .enumerate()
            .all(|(i, &m)| m == if i == 0 || i == last { 4.0 } else { 3.0 });
    if !piecewise {
        return Err("B-spline surface is not piecewise cubic Bezier".into());
    }
    let index = knots[1..last]
        .iter()
        .take_while(|&&knot| knot <= value)
        .count();
    let (lo, hi) = (knots[index], knots[index + 1]);
    Ok((index, (value - lo) / (hi - lo)))
}

impl Document {
    pub(super) fn spline_surface_point(
        &self,
        expression: &str,
        uv: [f64; 2],
    ) -> Result<[f64; 3], String> {
        let args = fields(expression)?;
        if args[1] != "3" || args[2] != "3" {
            return Err("B-spline surface is not bicubic".into());
        }
        let rows = fields(&args[3])?
            .iter()
            .map(|row| references(row))
            .collect::<Vec<_>>();
        let (i, s) = span(&self.tuple(&args[10])?, &self.tuple(&args[8])?, uv[0])?;
        let (j, t) = span(&self.tuple(&args[11])?, &self.tuple(&args[9])?, uv[1])?;
        let (wu, wv) = (bernstein(s), bernstein(t));
        let mut point = [0.0; 3];
        for (a, weight_u) in wu.iter().enumerate() {
            for (b, weight_v) in wv.iter().enumerate() {
                let id = *rows
                    .get(3 * i + a)
                    .and_then(|row| row.get(3 * j + b))
                    .ok_or("B-spline surface control net is too small")?;
                let control = self.tuple(&fields(self.entity(id)?)?[1])?;
                if control.len() != 3 {
                    return Err("B-spline surface control is not three-dimensional".into());
                }
                for axis in 0..3 {
                    point[axis] += weight_u * weight_v * control[axis];
                }
            }
        }
        Ok(point)
    }
}
