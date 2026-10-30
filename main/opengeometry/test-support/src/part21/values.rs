use super::document::Document;
use super::lexer::{fields, keyword, keywords, reference, references};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq)]
pub struct EdgeSupport {
    pub edge: usize,
    pub curve: usize,
    pub kind: String,
    pub radii: Vec<f64>,
    pub with_pcurves: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CurveRadii {
    pub curve: usize,
    pub kind: String,
    pub dimension: usize,
    pub radii: Vec<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductCounts {
    pub product: usize,
    pub solids: usize,
    pub faces: usize,
    pub edges: usize,
    pub voids: usize,
    pub pcurveless_edges: usize,
    pub fitted_curves: usize,
}

struct ProductLinks {
    products: Vec<usize>,
    parents: BTreeMap<usize, usize>,
    shapes: Vec<(usize, usize)>,
}

impl Document {
    pub fn kind_counts(&self) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        for expression in self.entities.values() {
            for kind in keywords(expression).into_iter().collect::<BTreeSet<_>>() {
                *counts.entry(kind).or_insert(0) += 1;
            }
        }
        counts
    }

    pub fn points3(&self) -> Result<BTreeMap<usize, [f64; 3]>, String> {
        self.typed_tuples("CARTESIAN_POINT(")
    }

    pub fn points2(&self) -> Result<BTreeMap<usize, [f64; 2]>, String> {
        self.typed_tuples("CARTESIAN_POINT(")
    }

    pub fn directions3(&self) -> Result<BTreeMap<usize, [f64; 3]>, String> {
        self.typed_tuples("DIRECTION(")
    }

    pub fn directions2(&self) -> Result<BTreeMap<usize, [f64; 2]>, String> {
        self.typed_tuples("DIRECTION(")
    }

    fn typed_tuples<const N: usize>(
        &self,
        prefix: &str,
    ) -> Result<BTreeMap<usize, [f64; N]>, String> {
        let tuples = self.tuples(prefix, N)?;
        Ok(tuples
            .into_iter()
            .map(|(id, v)| (id, std::array::from_fn(|axis| v[axis])))
            .collect())
    }

    fn tuples(&self, prefix: &str, dimension: usize) -> Result<BTreeMap<usize, Vec<f64>>, String> {
        let mut result = BTreeMap::new();
        for (id, expression) in &self.entities {
            if !expression.starts_with(prefix) {
                continue;
            }
            let values = self.tuple(&fields(expression)?[1])?;
            if values.len() == dimension {
                result.insert(*id, values);
            }
        }
        Ok(result)
    }

    pub fn spline_control_points(&self) -> Result<BTreeSet<usize>, String> {
        let mut result = BTreeSet::new();
        for expression in self.entities.values() {
            if expression.starts_with("B_SPLINE_CURVE_WITH_KNOTS(") {
                result.extend(references(&fields(expression)?[2]));
            }
        }
        Ok(result)
    }

    pub fn edge_supports(&self) -> Result<Vec<EdgeSupport>, String> {
        self.entities
            .iter()
            .filter(|(_, expression)| expression.starts_with("EDGE_CURVE("))
            .map(|(id, expression)| self.edge_support(*id, expression))
            .collect()
    }

    fn edge_support(&self, edge: usize, expression: &str) -> Result<EdgeSupport, String> {
        let mut curve = reference(&fields(expression)?[3])?;
        let support = self.entity(curve)?;
        let with_pcurves =
            support.starts_with("SURFACE_CURVE(") || support.starts_with("SEAM_CURVE(");
        if with_pcurves {
            curve = reference(&fields(support)?[1])?;
        }
        let curve_expression = self.entity(curve)?;
        Ok(EdgeSupport {
            edge,
            curve,
            kind: keyword(curve_expression).into(),
            radii: conic_radii(curve_expression)?,
            with_pcurves,
        })
    }

    pub fn curve_radii(&self) -> Result<Vec<CurveRadii>, String> {
        let mut result = Vec::new();
        for (id, expression) in &self.entities {
            let radii = conic_radii(expression)?;
            if radii.is_empty() {
                continue;
            }
            let placement = self.entity(reference(&fields(expression)?[1])?)?;
            let dimension = match keyword(placement) {
                "AXIS2_PLACEMENT_2D" => 2,
                "AXIS2_PLACEMENT_3D" => 3,
                _ => return Err(format!("#{id} has no conic placement")),
            };
            result.push(CurveRadii {
                curve: *id,
                kind: keyword(expression).into(),
                dimension,
                radii,
            });
        }
        Ok(result)
    }

    pub fn product_counts(&self) -> Result<Vec<ProductCounts>, String> {
        let links = self.product_links()?;
        let mut result = Vec::new();
        for (shape, representation) in &links.shapes {
            let mut product = *shape;
            for _ in 0..3 {
                product = *links
                    .parents
                    .get(&product)
                    .ok_or_else(|| format!("shape #{shape} does not reach a PRODUCT"))?;
            }
            result.push(self.representation_counts(product, *representation)?);
        }
        result.sort_by_key(|counts| counts.product);
        if !result
            .iter()
            .map(|counts| counts.product)
            .eq(links.products.iter().copied())
        {
            return Err("every PRODUCT needs exactly one shape representation".into());
        }
        Ok(result)
    }

    fn product_links(&self) -> Result<ProductLinks, String> {
        let mut links = ProductLinks {
            products: Vec::new(),
            parents: BTreeMap::new(),
            shapes: Vec::new(),
        };
        for (id, expression) in &self.entities {
            let parent_field = match keyword(expression) {
                "PRODUCT" => {
                    links.products.push(*id);
                    continue;
                }
                "SHAPE_DEFINITION_REPRESENTATION" => {
                    let args = fields(expression)?;
                    links
                        .shapes
                        .push((reference(&args[0])?, reference(&args[1])?));
                    continue;
                }
                "PRODUCT_DEFINITION_FORMATION_WITH_SPECIFIED_SOURCE"
                | "PRODUCT_DEFINITION"
                | "PRODUCT_DEFINITION_SHAPE" => 2,
                _ => continue,
            };
            let parent = reference(&fields(expression)?[parent_field])?;
            links.parents.insert(*id, parent);
        }
        Ok(links)
    }

    fn representation_counts(
        &self,
        product: usize,
        representation: usize,
    ) -> Result<ProductCounts, String> {
        let mut counts = ProductCounts {
            product,
            solids: 0,
            faces: 0,
            edges: 0,
            voids: 0,
            pcurveless_edges: 0,
            fitted_curves: 0,
        };
        let mut seen = BTreeSet::new();
        for solid in references(&fields(self.entity(representation)?)?[1]) {
            self.add_solid(&mut counts, solid, &mut seen)?;
        }
        Ok(counts)
    }

    fn add_solid(
        &self,
        counts: &mut ProductCounts,
        solid: usize,
        seen: &mut BTreeSet<usize>,
    ) -> Result<(), String> {
        let expression = self.entity(solid)?;
        let args = fields(expression)?;
        counts.solids += 1;
        let mut shells = vec![reference(&args[1])?];
        if expression.starts_with("BREP_WITH_VOIDS(") {
            for void in references(&args[2]) {
                counts.voids += 1;
                shells.push(reference(&fields(self.entity(void)?)?[2])?);
            }
        }
        for shell in shells {
            for face in references(&fields(self.entity(shell)?)?[1]) {
                counts.faces += 1;
                for (edge, _) in self.face_edges(face)? {
                    if !seen.insert(edge) {
                        continue;
                    }
                    counts.edges += 1;
                    let support = self.edge_support(edge, self.entity(edge)?)?;
                    counts.pcurveless_edges += usize::from(!support.with_pcurves);
                    if support.kind == "B_SPLINE_CURVE_WITH_KNOTS" && seen.insert(support.curve) {
                        counts.fitted_curves += 1;
                    }
                }
            }
        }
        Ok(())
    }
}

fn conic_radii(expression: &str) -> Result<Vec<f64>, String> {
    let count = match keyword(expression) {
        "CIRCLE" => 1,
        "ELLIPSE" => 2,
        _ => return Ok(Vec::new()),
    };
    fields(expression)?[2..2 + count]
        .iter()
        .map(|value| value.parse().map_err(|_| "invalid conic radius".into()))
        .collect()
}
