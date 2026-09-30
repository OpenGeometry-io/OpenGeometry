use super::error::GeometryError;
use super::topology::BrepEnvelope;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BodyType {
    Wire,
    Sheet,
    Solid,
}

impl BrepEnvelope {
    pub fn body_type(&self) -> Result<BodyType, GeometryError> {
        self.validate()?;
        if !self.topology.faces.is_empty() && !self.topology.wires.is_empty() {
            return Err(GeometryError::InvalidTopology(
                "BRep mixes faces and wires".into(),
            ));
        }
        if !self.solids.is_empty() {
            Ok(BodyType::Solid)
        } else if !self.topology.faces.is_empty() {
            Ok(BodyType::Sheet)
        } else if !self.topology.wires.is_empty() {
            Ok(BodyType::Wire)
        } else {
            Err(GeometryError::InvalidTopology("BRep has no body".into()))
        }
    }
}
