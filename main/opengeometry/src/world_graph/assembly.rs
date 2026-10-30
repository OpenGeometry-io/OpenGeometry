use super::candidate::IdKind;
use super::change_log::ChangeSet;
use super::error::{ErrorCode, GraphError};
use super::graph::WorldGraph;
use super::node::NodeKind;
use super::primitive::{check_no_plane, CreateOptions};
use crate::brep::Similarity3;

impl WorldGraph {
    pub fn create_system_assembly(
        &mut self,
        options: CreateOptions,
    ) -> Result<(String, ChangeSet), GraphError> {
        if options.body_type.is_some() {
            return Err(GraphError::code(
                ErrorCode::InvalidParameter,
                "assemblies have no body type",
            ));
        }
        check_no_plane(options.plane)?;
        let mut created = None;
        let changes = self.mutate(|draft| {
            if let Some(parent) = &options.parent {
                if !draft.nodes.contains_key(parent) {
                    return Err(GraphError::code(ErrorCode::UnknownNode, parent));
                }
            }
            let id = draft.new_og_id(options.og_id.as_deref(), IdKind::Assembly)?;
            let handle = draft.new_handle()?;
            draft.add_node(
                &id,
                handle,
                &options.parent,
                Similarity3::IDENTITY,
                None,
                NodeKind::SystemAssembly,
            );
            created = Some(id);
            Ok(())
        })?;
        Ok((
            created.ok_or_else(|| {
                GraphError::code(ErrorCode::InvalidTopology, "created node is missing")
            })?,
            changes,
        ))
    }
}
