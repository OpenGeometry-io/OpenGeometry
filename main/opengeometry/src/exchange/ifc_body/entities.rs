use crate::brep::{Frame3, Orientation};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub(crate) enum IfcValue {
    Ref { id: u32 },
    List { values: Vec<IfcValue> },
    Real { value: f64 },
    Integer { value: i64 },
    Text { value: String },
    Enum { value: String },
    Bool { value: bool },
    Null,
    Derived,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IfcEntity {
    id: u32,
    entity_type: String,
    attributes: Vec<IfcValue>,
}

#[derive(Default)]
pub(super) struct Entities(pub(super) Vec<IfcEntity>);

impl Entities {
    pub(super) fn add(&mut self, entity_type: &str, attributes: Vec<IfcValue>) -> u32 {
        let id = self.0.len() as u32 + 1;
        self.0.push(IfcEntity {
            id,
            entity_type: entity_type.into(),
            attributes,
        });
        id
    }

    pub(super) fn point(&mut self, p: &[f64]) -> u32 {
        self.add("IfcCartesianPoint", vec![reals(p)])
    }

    pub(super) fn direction(&mut self, p: &[f64]) -> u32 {
        self.add("IfcDirection", vec![reals(p)])
    }

    pub(super) fn placement(&mut self, frame: Frame3) -> u32 {
        let origin = self.point(&frame.origin);
        let z = self.direction(&frame.z);
        let x = self.direction(&frame.x);
        self.add(
            "IfcAxis2Placement3D",
            vec![reference(origin), reference(z), reference(x)],
        )
    }
}

pub(super) fn real(value: f64) -> IfcValue {
    IfcValue::Real { value }
}

pub(super) fn reals(values: &[f64]) -> IfcValue {
    IfcValue::List {
        values: values.iter().map(|value| real(*value)).collect(),
    }
}

pub(super) fn enumeration(value: &str) -> IfcValue {
    IfcValue::Enum {
        value: value.into(),
    }
}

pub(super) fn reference(id: u32) -> IfcValue {
    IfcValue::Ref { id }
}

pub(super) fn references(ids: impl IntoIterator<Item = u32>) -> IfcValue {
    IfcValue::List {
        values: ids.into_iter().map(reference).collect(),
    }
}

pub(super) fn boolean(sense: Orientation) -> IfcValue {
    IfcValue::Bool {
        value: sense == Orientation::Forward,
    }
}
