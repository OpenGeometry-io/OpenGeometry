use super::document::Document;
use opengeometry::brep::BrepEnvelope;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::TAU;

#[derive(Default)]
pub struct ExpectedValues {
    points3: BTreeSet<[u64; 3]>,
    directions3: BTreeSet<[u64; 3]>,
    points2: BTreeSet<[u64; 2]>,
    directions2: BTreeSet<[u64; 2]>,
    radii3: BTreeSet<u64>,
    radii2: BTreeSet<u64>,
}

pub struct PcurveExpectation {
    pub curve: String,
    pub surface: String,
    pub bits: Option<[u64; 4]>,
}

struct PcurveUse<'a> {
    pcurve: &'a Value,
    surface: &'a Value,
    lift: [f64; 2],
    scale: f64,
}

struct MappedPcurve {
    origin: [f64; 2],
    direction: [f64; 2],
    radii: Vec<f64>,
}

pub fn pcurve_expectations(brep: &BrepEnvelope, unit_scale: f64) -> Vec<PcurveExpectation> {
    let body = body_json(brep);
    let mut expectations = Vec::new();
    for edge in array(&body["topology"]["edges"]) {
        if edge["geometry"]["kind"] != "Curve" {
            continue;
        }
        let curve = kind(&body["geometry"]["curves"][index(&edge["geometry"]["curve"])]);
        for use_ in pcurve_uses(&body, edge, unit_scale) {
            expectations.push(PcurveExpectation {
                curve: curve.to_string(),
                surface: kind(use_.surface).to_string(),
                bits: mapped_pcurve(&use_).map(|mapped| {
                    let ([u, v], [du, dv]) = (bits(mapped.origin), bits(mapped.direction));
                    [u, v, du, dv]
                }),
            });
        }
    }
    expectations
}

impl ExpectedValues {
    pub fn add_body(&mut self, brep: &BrepEnvelope, unit_scale: f64) {
        let body = body_json(brep);
        let mut surfaces = BTreeSet::new();
        for face in array(&body["topology"]["faces"]) {
            let surface = index(&face["surface"]);
            if surfaces.insert(surface) {
                self.add_surface(&body["geometry"]["surfaces"][surface], unit_scale);
            }
        }
        for vertex in array(&body["topology"]["vertices"]) {
            let position = vector::<3>(&vertex["position"]).map(|v| v * unit_scale);
            self.points3.insert(bits(position));
        }
        let mut curves = BTreeSet::new();
        for edge in array(&body["topology"]["edges"]) {
            if edge["geometry"]["kind"] != "Curve" {
                continue;
            }
            let curve = index(&edge["geometry"]["curve"]);
            if curves.insert(curve) {
                self.add_curve(&body["geometry"]["curves"][curve], unit_scale);
            }
            self.add_edge_pcurves(&body, edge, unit_scale);
        }
    }

    fn add_surface(&mut self, surface: &Value, scale: f64) {
        self.add_placement(&surface["frame"], scale);
        match kind(surface) {
            "Cylinder" | "Sphere" => {
                self.radii3
                    .insert(bits([number(&surface["radius"]) * scale])[0]);
            }
            "Torus" => {
                for key in ["major_radius", "minor_radius"] {
                    self.radii3.insert(bits([number(&surface[key]) * scale])[0]);
                }
            }
            _ => {}
        }
    }

    fn add_placement(&mut self, frame: &Value, scale: f64) {
        let origin = vector::<3>(&frame["origin"]).map(|v| v * scale);
        self.points3.insert(bits(origin));
        self.directions3.insert(bits(vector::<3>(&frame["z"])));
        self.directions3.insert(bits(vector::<3>(&frame["x"])));
    }

    fn add_curve(&mut self, curve: &Value, scale: f64) {
        match kind(curve) {
            "Line" => {
                let origin = vector::<3>(&curve["origin"]).map(|v| v * scale);
                let velocity = vector::<3>(&curve["direction"]).map(|v| v * scale);
                let length = fold_length(&velocity);
                self.points3.insert(bits(origin));
                self.directions3.insert(bits(velocity.map(|v| v / length)));
            }
            "Circle" => {
                self.add_placement(&curve["frame"], scale);
                self.radii3
                    .insert(bits([number(&curve["radius"]) * scale])[0]);
            }
            "Ellipse" => {
                self.add_placement(&curve["frame"], scale);
                for key in ["major_radius", "minor_radius"] {
                    self.radii3.insert(bits([number(&curve[key]) * scale])[0]);
                }
            }
            _ => {}
        }
    }

    fn add_edge_pcurves(&mut self, body: &Value, edge: &Value, scale: f64) {
        for use_ in pcurve_uses(body, edge, scale) {
            if let Some(mapped) = mapped_pcurve(&use_) {
                self.points2.insert(bits(mapped.origin));
                self.directions2.insert(bits(mapped.direction));
                for radius in mapped.radii {
                    self.radii2.insert(bits([radius])[0]);
                }
            }
        }
    }
}

fn body_json(brep: &BrepEnvelope) -> Value {
    let json = brep
        .to_json()
        .unwrap_or_else(|error| panic!("BRep does not serialise: {error}"));
    serde_json::from_str(&json).unwrap_or_else(|error| panic!("BRep JSON does not parse: {error}"))
}

fn pcurve_uses<'a>(body: &'a Value, edge: &'a Value, scale: f64) -> Vec<PcurveUse<'a>> {
    let halfedges = &body["topology"]["halfedges"];
    let uses = [&edge["halfedge"], &edge["twin_halfedge"]]
        .into_iter()
        .filter(|use_| !use_.is_null())
        .map(|use_| &halfedges[index(use_)])
        .collect::<Vec<_>>();
    let pcurve =
        |use_: &Value| &body["geometry"]["pcurves"][index(&use_["geometry_use"]["pcurve"])];
    if uses
        .iter()
        .any(|use_| kind(pcurve(use_)) == "ProjectedCurve")
    {
        return Vec::new();
    }
    uses.into_iter()
        .map(|use_| {
            let face = &body["topology"]["faces"][index(&use_["face"])];
            PcurveUse {
                pcurve: pcurve(use_),
                surface: &body["geometry"]["surfaces"][index(&face["surface"])],
                lift: vector::<2>(&use_["geometry_use"]["periodic_lift"]),
                scale,
            }
        })
        .collect()
}

fn mapped_pcurve(use_: &PcurveUse<'_>) -> Option<MappedPcurve> {
    let s = use_.scale;
    let metric = match kind(use_.surface) {
        "Plane" => [s, s],
        "Cylinder" => [1.0, s],
        "Cone" => [1.0, s / number(&use_.surface["semi_angle"]).cos()],
        _ => [1.0, 1.0],
    };
    let period = match kind(use_.surface) {
        "Plane" => [0.0, 0.0],
        "Torus" => [TAU; 2],
        _ => [TAU, 0.0],
    };
    let lift = use_.lift;
    let pcurve = use_.pcurve;
    let origin = || -> [f64; 2] {
        let origin = vector::<2>(&pcurve["origin"]);
        std::array::from_fn(|i| (origin[i] + lift[i] * period[i]) * metric[i])
    };
    match kind(pcurve) {
        "Line2" => {
            let direction = vector::<2>(&pcurve["direction"]);
            let velocity: [f64; 2] = std::array::from_fn(|i| direction[i] * metric[i]);
            let length = fold_length(&velocity);
            Some(MappedPcurve {
                origin: origin(),
                direction: velocity.map(|v| v / length),
                radii: Vec::new(),
            })
        }
        "Conic2" => {
            let axis_a = vector::<2>(&pcurve["axis_a"]);
            let axis_b = vector::<2>(&pcurve["axis_b"]);
            let a = [axis_a[0] * metric[0], axis_a[1] * metric[1], 0.0];
            let b = [axis_b[0] * metric[0], axis_b[1] * metric[1], 0.0];
            let (ra, rb) = (fold_length(&a), fold_length(&b));
            let axis = if ra >= rb { a } else { b };
            Some(MappedPcurve {
                origin: origin(),
                direction: [axis[0], axis[1]],
                radii: vec![ra.max(rb), ra.min(rb)],
            })
        }
        _ => None,
    }
}

pub fn check_values_belong(document: &Document, expected: &ExpectedValues) -> Result<(), String> {
    let controls = document.spline_control_points()?;
    let none = BTreeSet::new();
    check_members(
        "CARTESIAN_POINT",
        document.points3()?,
        &controls,
        &expected.points3,
    )?;
    check_members(
        "CARTESIAN_POINT",
        document.points2()?,
        &controls,
        &expected.points2,
    )?;
    check_members(
        "DIRECTION",
        document.directions3()?,
        &none,
        &expected.directions3,
    )?;
    check_members(
        "DIRECTION",
        document.directions2()?,
        &none,
        &expected.directions2,
    )?;
    for curve in document.curve_radii()? {
        let radii = if curve.dimension == 2 {
            &expected.radii2
        } else {
            &expected.radii3
        };
        check_radii(
            &format!("{} #{}", curve.kind, curve.curve),
            &curve.radii,
            radii,
        )?;
    }
    for (surface, radii) in document.surface_radii()? {
        check_radii(&format!("{surface} surface"), &radii, &expected.radii3)?;
    }
    Ok(())
}

fn check_members<const N: usize>(
    kind: &str,
    values: BTreeMap<usize, [f64; N]>,
    excluded: &BTreeSet<usize>,
    expected: &BTreeSet<[u64; N]>,
) -> Result<(), String> {
    match values
        .into_iter()
        .find(|(id, value)| !excluded.contains(id) && !expected.contains(&bits(*value)))
    {
        Some((id, value)) => Err(missing_value(&format!("{kind} #{id}"), &value)),
        None => Ok(()),
    }
}

fn check_radii(owner: &str, radii: &[f64], expected: &BTreeSet<u64>) -> Result<(), String> {
    match radii
        .iter()
        .find(|radius| !expected.contains(&bits([**radius])[0]))
    {
        Some(radius) => Err(missing_value(&format!("{owner} radius"), &[*radius])),
        None => Ok(()),
    }
}

fn missing_value(owner: &str, value: &[f64]) -> String {
    format!(
        "{owner} {value:?} is not a value of the placed BRep (B-spline controls and knots are excluded)"
    )
}

fn fold_length(values: &[f64]) -> f64 {
    values.iter().fold(0.0_f64, |n, v| n.hypot(*v))
}

pub fn bits<const N: usize>(values: [f64; N]) -> [u64; N] {
    values.map(|v| if v == 0.0 { 0.0_f64 } else { v }.to_bits())
}

fn kind(value: &Value) -> &str {
    value["kind"]
        .as_str()
        .unwrap_or_else(|| panic!("BRep JSON {value} has no kind"))
}

fn array(value: &Value) -> &[Value] {
    value
        .as_array()
        .unwrap_or_else(|| panic!("BRep JSON {value} is not an array"))
}

fn index(value: &Value) -> usize {
    value
        .as_u64()
        .and_then(|id| usize::try_from(id).ok())
        .unwrap_or_else(|| panic!("BRep JSON {value} is not an index"))
}

fn number(value: &Value) -> f64 {
    value
        .as_f64()
        .unwrap_or_else(|| panic!("BRep JSON {value} is not a number"))
}

fn vector<const N: usize>(value: &Value) -> [f64; N] {
    let values = array(value);
    if values.len() != N {
        panic!("BRep JSON {value} does not hold {N} numbers");
    }
    std::array::from_fn(|i| number(&values[i]))
}
