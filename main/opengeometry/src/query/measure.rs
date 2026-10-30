use crate::brep::{BodyType, BrepEnvelope, GeometryError};

#[derive(Clone, Debug, PartialEq)]
pub struct Validity {
    pub valid: bool,
    pub closed: bool,
    pub body_type: Option<BodyType>,
    pub error: Option<GeometryError>,
}

pub fn validity(body: &BrepEnvelope) -> Validity {
    match body.body_type() {
        Ok(body_type) => Validity {
            valid: true,
            closed: closed(body, body_type),
            body_type: Some(body_type),
            error: None,
        },
        Err(error) => Validity {
            valid: false,
            closed: false,
            body_type: None,
            error: Some(error),
        },
    }
}

fn closed(body: &BrepEnvelope, body_type: BodyType) -> bool {
    let topology = &body.topology;
    match body_type {
        BodyType::Wire => topology.wires.iter().all(|wire| wire.is_closed),
        BodyType::Sheet | BodyType::Solid => {
            !topology.shells.is_empty() && topology.shells.iter().all(|shell| shell.is_closed)
        }
    }
}
