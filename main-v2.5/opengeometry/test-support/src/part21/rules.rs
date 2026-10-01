use super::document::Document;
use super::lexer::{fields, keywords, reference, references};
use std::collections::{BTreeMap, BTreeSet};

fn allowed(keyword: &str) -> bool {
    matches!(
        keyword,
        "ADVANCED_BREP_SHAPE_REPRESENTATION"
            | "ADVANCED_FACE"
            | "APPLICATION_CONTEXT"
            | "APPLICATION_PROTOCOL_DEFINITION"
            | "AXIS2_PLACEMENT_2D"
            | "AXIS2_PLACEMENT_3D"
            | "BREP_WITH_VOIDS"
            | "B_SPLINE_CURVE_WITH_KNOTS"
            | "CARTESIAN_POINT"
            | "CIRCLE"
            | "CLOSED_SHELL"
            | "CONICAL_SURFACE"
            | "CYLINDRICAL_SURFACE"
            | "DEFINITIONAL_REPRESENTATION"
            | "DIRECTION"
            | "EDGE_CURVE"
            | "EDGE_LOOP"
            | "ELLIPSE"
            | "FACE_BOUND"
            | "FACE_OUTER_BOUND"
            | "GEOMETRIC_REPRESENTATION_CONTEXT"
            | "GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT"
            | "GLOBAL_UNIT_ASSIGNED_CONTEXT"
            | "LENGTH_MEASURE"
            | "LENGTH_UNIT"
            | "LINE"
            | "MANIFOLD_SOLID_BREP"
            | "NAMED_UNIT"
            | "ORIENTED_CLOSED_SHELL"
            | "ORIENTED_EDGE"
            | "PCURVE"
            | "PLANE"
            | "PLANE_ANGLE_UNIT"
            | "PRODUCT"
            | "PRODUCT_CONTEXT"
            | "PRODUCT_DEFINITION"
            | "PRODUCT_DEFINITION_CONTEXT"
            | "PRODUCT_DEFINITION_FORMATION_WITH_SPECIFIED_SOURCE"
            | "PRODUCT_DEFINITION_SHAPE"
            | "REPRESENTATION_CONTEXT"
            | "SEAM_CURVE"
            | "SHAPE_DEFINITION_REPRESENTATION"
            | "SI_UNIT"
            | "SOLID_ANGLE_UNIT"
            | "SPHERICAL_SURFACE"
            | "SURFACE_CURVE"
            | "TOROIDAL_SURFACE"
            | "UNCERTAINTY_MEASURE_WITH_UNIT"
            | "VECTOR"
            | "VERTEX_LOOP"
            | "VERTEX_POINT"
    )
}

impl Document {
    pub fn parse(text: &str) -> Result<Self, String> {
        if !text.starts_with("ISO-10303-21;\nHEADER;\n")
            || !text.ends_with("ENDSEC;\nEND-ISO-10303-21;\n")
        {
            return Err("invalid Part-21 envelope".into());
        }
        let mut entities = BTreeMap::new();
        for line in text.lines() {
            if !line.starts_with('#') {
                continue;
            }
            let (id, expression) = line[1..].split_once('=').ok_or("entity lacks assignment")?;
            let id: usize = id.parse().map_err(|_| "invalid entity id")?;
            let expression = expression
                .strip_suffix(';')
                .ok_or("entity lacks terminator")?;
            if entities.insert(id, expression.to_string()).is_some() {
                return Err("duplicate entity id".into());
            }
            for keyword in keywords(expression) {
                if !allowed(&keyword) {
                    return Err(format!("unsupported entity keyword {keyword}"));
                }
            }
        }
        if entities.is_empty() {
            return Err("empty Part-21 data section".into());
        }
        for (expected, id) in entities.keys().enumerate() {
            if *id != expected + 1 {
                return Err(format!("nonsequential entity id #{id}"));
            }
        }
        for (id, expression) in &entities {
            for target in references(expression) {
                if !entities.contains_key(&target) {
                    return Err(format!("#{id} has unresolved reference #{target}"));
                }
            }
        }
        let document = Self { entities };
        document.check_contexts()?;
        document.check_solids()?;
        document.validate_loops_and_shells()?;
        document.validate_curve_endpoints()?;
        document.validate_pcurves()?;
        Ok(document)
    }

    fn oriented_edge(&self, id: usize) -> Result<(usize, usize, usize, bool), String> {
        let expression = self.entity(id)?;
        if !expression.starts_with("ORIENTED_EDGE(") {
            return Err(format!("#{id} is not ORIENTED_EDGE"));
        }
        let args = fields(expression)?;
        let edge = reference(&args[3])?;
        let edge_expression = self.entity(edge)?;
        if !edge_expression.starts_with("EDGE_CURVE(") {
            return Err(format!("#{edge} is not EDGE_CURVE"));
        }
        let edge_args = fields(edge_expression)?;
        let (mut from, mut to) = (reference(&edge_args[1])?, reference(&edge_args[2])?);
        let forward = args[4] == ".T.";
        if !forward {
            std::mem::swap(&mut from, &mut to);
        }
        Ok((edge, from, to, forward))
    }

    pub(super) fn face_edges(&self, face_id: usize) -> Result<Vec<(usize, bool)>, String> {
        let face = self.entity(face_id)?;
        if !face.starts_with("ADVANCED_FACE(") {
            return Err(format!("#{face_id} is not ADVANCED_FACE"));
        }
        let face_args = fields(face)?;
        let mut edges = Vec::new();
        for bound in references(&face_args[1]) {
            let bound_args = fields(self.entity(bound)?)?;
            let loop_id = reference(&bound_args[1])?;
            let loop_expression = self.entity(loop_id)?;
            if loop_expression.starts_with("VERTEX_LOOP(") {
                continue;
            }
            if !loop_expression.starts_with("EDGE_LOOP(") {
                return Err(format!("#{loop_id} is not an edge loop"));
            }
            let loop_args = fields(loop_expression)?;
            let oriented = references(&loop_args[1]);
            if oriented.is_empty() {
                return Err(format!("#{loop_id} is empty"));
            }
            let mut first = None;
            let mut prior = None;
            for use_id in oriented {
                let (edge, from, to, forward) = self.oriented_edge(use_id)?;
                if let Some(previous) = prior {
                    if previous != from {
                        return Err(format!("#{loop_id} does not close at #{use_id}"));
                    }
                } else {
                    first = Some(from);
                }
                prior = Some(to);
                edges.push((edge, forward));
            }
            if prior != first {
                return Err(format!("#{loop_id} has distinct endpoints"));
            }
        }
        Ok(edges)
    }

    fn validate_loops_and_shells(&self) -> Result<(), String> {
        let mut encountered = BTreeSet::new();
        for (id, expression) in &self.entities {
            if !expression.starts_with("CLOSED_SHELL(") {
                continue;
            }
            let args = fields(expression)?;
            let mut uses: BTreeMap<usize, Vec<bool>> = BTreeMap::new();
            for face in references(&args[1]) {
                for (edge, forward) in self.face_edges(face)? {
                    uses.entry(edge).or_default().push(forward);
                    encountered.insert(edge);
                }
            }
            for (edge, senses) in uses {
                if senses.len() != 2 || senses[0] == senses[1] {
                    return Err(format!("shell #{id} does not pair edge #{edge} oppositely"));
                }
            }
        }
        for (id, expression) in &self.entities {
            if expression.starts_with("EDGE_CURVE(") && !encountered.contains(id) {
                return Err(format!("edge #{id} is not in a closed shell"));
            }
        }
        Ok(())
    }

    fn uncertainty(&self) -> Result<f64, String> {
        let expression = self
            .entities
            .values()
            .find(|expression| expression.starts_with("UNCERTAINTY_MEASURE_WITH_UNIT("))
            .ok_or("missing uncertainty")?;
        let args = fields(expression)?;
        let values = self.tuple(&args[0])?;
        values
            .first()
            .copied()
            .ok_or_else(|| "empty uncertainty".into())
    }

    fn validate_curve_endpoints(&self) -> Result<(), String> {
        let tolerance = self.uncertainty()?;
        if !tolerance.is_finite() || tolerance <= 0.0 {
            return Err("invalid uncertainty".into());
        }
        for (id, expression) in &self.entities {
            if !expression.starts_with("EDGE_CURVE(") {
                continue;
            }
            let args = fields(expression)?;
            let mut geometry = reference(&args[3])?;
            let support = self.entity(geometry)?;
            if support.starts_with("SURFACE_CURVE(") || support.starts_with("SEAM_CURVE(") {
                geometry = reference(&fields(support)?[1])?;
            }
            for vertex in [reference(&args[1])?, reference(&args[2])?] {
                let point = self.vertex(vertex)?;
                let distance = self.curve_distance(geometry, point)?;
                if !distance.is_finite() || distance > tolerance {
                    return Err(format!("edge #{id} vertex #{vertex} misses curve #{geometry}: {distance} > {tolerance}"));
                }
            }
        }
        Ok(())
    }

    fn validate_pcurves(&self) -> Result<(), String> {
        let tolerance = self.uncertainty()?;
        for (id, expression) in &self.entities {
            if !expression.starts_with("SURFACE_CURVE(") && !expression.starts_with("SEAM_CURVE(") {
                continue;
            }
            let args = fields(expression)?;
            let curve = reference(&args[1])?;
            let curve_is_spline = self
                .entity(curve)?
                .starts_with("B_SPLINE_CURVE_WITH_KNOTS(");
            for pcurve in references(&args[2]) {
                let pcurve_args = fields(self.entity(pcurve)?)?;
                let surface = reference(&pcurve_args[1])?;
                let definition = reference(&pcurve_args[2])?;
                let representation = fields(self.entity(definition)?)?;
                let uv_curve = *references(&representation[1])
                    .first()
                    .ok_or("empty pcurve representation")?;
                for sample in 0..=16 {
                    let parameter = sample as f64 / 16.0;
                    let uv = self.pcurve_point(uv_curve, parameter)?;
                    let point = self.surface_point(surface, uv)?;
                    let distance = if curve_is_spline {
                        let on_curve = self.spline_point(curve, parameter)?;
                        if on_curve.len() != 3 {
                            return Err("3D spline is not three-dimensional".into());
                        }
                        point
                            .iter()
                            .zip(on_curve)
                            .map(|(a, b)| (a - b).powi(2))
                            .sum::<f64>()
                            .sqrt()
                    } else {
                        self.curve_distance(curve, point)?
                    };
                    if !distance.is_finite() || distance > tolerance {
                        return Err(format!("surface curve #{id} pcurve #{pcurve} sample {sample} misses 3D curve: {distance} > {tolerance}"));
                    }
                }
            }
        }
        Ok(())
    }
}
