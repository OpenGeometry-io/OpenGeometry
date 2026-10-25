use super::error::{ErrorCode, ErrorContext, GraphError};
use super::export::up_conversion;
use super::graph::WorldGraph;
use crate::brep::{BodyType, Similarity3};
use crate::exchange::ifc_body::{export_ifc_body, AnalyticExchangeBodyV2};

impl WorldGraph {
    pub fn export_ifc_body(&self, og_id: &str) -> Result<AnalyticExchangeBodyV2, GraphError> {
        self.try_export_ifc_body(og_id)
            .map_err(|error| error.in_context(ErrorContext::Export))
    }

    fn try_export_ifc_body(&self, og_id: &str) -> Result<AnalyticExchangeBodyV2, GraphError> {
        if self.shape(og_id)?.brep.body_type()? != BodyType::Solid {
            return Err(GraphError::code(
                ErrorCode::InvalidOperand,
                "only a Solid exports an IFC body",
            ));
        }
        let (_, _, _, world) = self
            .placed_bodies(vec![og_id.into()], Similarity3::IDENTITY)?
            .pop()
            .ok_or_else(|| GraphError::code(ErrorCode::InvalidTopology, "body has no shape"))?;
        Ok(export_ifc_body(&world, up_conversion("Z").frame)?)
    }
}
