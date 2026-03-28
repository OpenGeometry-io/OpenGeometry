use std::collections::{HashMap, HashSet};

use crate::brep::Brep;

use super::{
    TopologyChangeJournal, TopologyCreatedIds, TopologyDomainJournal, TopologyRemap,
    TopologyRemapEntry, TopologyRemapStatus,
};

#[derive(Clone)]
pub(super) struct TopologySnapshot {
    shell_ids: Vec<u32>,
    face_ids: Vec<u32>,
    loop_ids: Vec<u32>,
    edge_ids: Vec<u32>,
    vertex_ids: Vec<u32>,
}

impl TopologySnapshot {
    pub(super) fn from_brep(brep: &Brep) -> Self {
        Self {
            shell_ids: sorted_ids(brep.shells.iter().map(|shell| shell.id).collect()),
            face_ids: sorted_ids(brep.faces.iter().map(|face| face.id).collect()),
            loop_ids: sorted_ids(brep.loops.iter().map(|loop_ref| loop_ref.id).collect()),
            edge_ids: sorted_ids(brep.edges.iter().map(|edge| edge.id).collect()),
            vertex_ids: sorted_ids(brep.vertices.iter().map(|vertex| vertex.id).collect()),
        }
    }

    pub(super) fn face_ids(&self) -> &[u32] {
        &self.face_ids
    }

    pub(super) fn edge_ids(&self) -> &[u32] {
        &self.edge_ids
    }

    pub(super) fn vertex_ids(&self) -> &[u32] {
        &self.vertex_ids
    }
}

pub(super) fn topology_changed(before: &TopologySnapshot, after: &TopologySnapshot) -> bool {
    before.shell_ids != after.shell_ids
        || before.face_ids != after.face_ids
        || before.loop_ids != after.loop_ids
        || before.edge_ids != after.edge_ids
        || before.vertex_ids != after.vertex_ids
}

pub(super) fn build_topology_remap(
    before: &TopologySnapshot,
    after: &TopologySnapshot,
    journal: Option<&TopologyChangeJournal>,
) -> TopologyRemap {
    let after_face_ids: HashSet<u32> = after.face_ids.iter().copied().collect();
    let after_edge_ids: HashSet<u32> = after.edge_ids.iter().copied().collect();
    let after_vertex_ids: HashSet<u32> = after.vertex_ids.iter().copied().collect();

    let face_mapping = build_domain_mapping(
        &before.face_ids,
        &after_face_ids,
        journal.map(|entry| &entry.faces.mapping),
    );
    let edge_mapping = build_domain_mapping(
        &before.edge_ids,
        &after_edge_ids,
        journal.map(|entry| &entry.edges.mapping),
    );
    let vertex_mapping = build_domain_mapping(
        &before.vertex_ids,
        &after_vertex_ids,
        journal.map(|entry| &entry.vertices.mapping),
    );

    let created_ids = TopologyCreatedIds {
        faces: resolve_created_ids(
            &before.face_ids,
            &after.face_ids,
            journal.map(|entry| &entry.faces),
        ),
        edges: resolve_created_ids(
            &before.edge_ids,
            &after.edge_ids,
            journal.map(|entry| &entry.edges),
        ),
        vertices: resolve_created_ids(
            &before.vertex_ids,
            &after.vertex_ids,
            journal.map(|entry| &entry.vertices),
        ),
    };

    TopologyRemap {
        faces: build_domain_entries_from_mapping(&before.face_ids, &face_mapping),
        edges: build_domain_entries_from_mapping(&before.edge_ids, &edge_mapping),
        vertices: build_domain_entries_from_mapping(&before.vertex_ids, &vertex_mapping),
        created_ids,
    }
}

pub(super) fn build_domain_entries_from_mapping(
    old_ids: &[u32],
    mapping: &HashMap<u32, Vec<u32>>,
) -> Vec<TopologyRemapEntry> {
    let mut normalized_mapping: HashMap<u32, Vec<u32>> = HashMap::new();

    for old_id in old_ids {
        let mut new_ids = mapping.get(old_id).cloned().unwrap_or_default();
        new_ids.sort_unstable();
        new_ids.dedup();
        normalized_mapping.insert(*old_id, new_ids);
    }

    let mut new_id_usage_count: HashMap<u32, usize> = HashMap::new();
    for new_ids in normalized_mapping.values() {
        for new_id in new_ids {
            *new_id_usage_count.entry(*new_id).or_insert(0) += 1;
        }
    }

    let mut entries = Vec::with_capacity(old_ids.len());
    for old_id in old_ids {
        let new_ids = normalized_mapping.get(old_id).cloned().unwrap_or_default();
        let primary_id = new_ids.first().copied();
        let status = classify_status(&new_ids, &new_id_usage_count);

        entries.push(TopologyRemapEntry {
            old_id: *old_id,
            new_ids,
            primary_id,
            status,
        });
    }

    entries.sort_by_key(|entry| entry.old_id);
    entries
}

fn classify_status(
    new_ids: &[u32],
    new_id_usage_count: &HashMap<u32, usize>,
) -> TopologyRemapStatus {
    if new_ids.is_empty() {
        return TopologyRemapStatus::Deleted;
    }

    if new_ids.len() > 1 {
        return TopologyRemapStatus::Split;
    }

    let new_id = new_ids[0];
    if new_id_usage_count.get(&new_id).copied().unwrap_or(0) > 1 {
        return TopologyRemapStatus::Merged;
    }

    TopologyRemapStatus::Unchanged
}

fn build_domain_mapping(
    old_ids: &[u32],
    after_ids: &HashSet<u32>,
    journal_mapping: Option<&HashMap<u32, Vec<u32>>>,
) -> HashMap<u32, Vec<u32>> {
    let mut mapping = HashMap::new();

    for old_id in old_ids {
        if let Some(mapped) = journal_mapping.and_then(|entry| entry.get(old_id)) {
            let mut normalized = mapped.clone();
            normalized.sort_unstable();
            normalized.dedup();
            normalized.retain(|id| after_ids.contains(id));
            mapping.insert(*old_id, normalized);
            continue;
        }

        if after_ids.contains(old_id) {
            mapping.insert(*old_id, vec![*old_id]);
        } else {
            mapping.insert(*old_id, Vec::new());
        }
    }

    mapping
}

fn resolve_created_ids(
    before_ids: &[u32],
    after_ids: &[u32],
    domain_journal: Option<&TopologyDomainJournal>,
) -> Vec<u32> {
    let before_set: HashSet<u32> = before_ids.iter().copied().collect();
    let after_set: HashSet<u32> = after_ids.iter().copied().collect();

    if let Some(journal) = domain_journal {
        let mut created = journal.created_ids.clone();
        created.sort_unstable();
        created.dedup();
        created.retain(|id| after_set.contains(id));
        return created;
    }

    let mut inferred = after_ids
        .iter()
        .copied()
        .filter(|id| !before_set.contains(id))
        .collect::<Vec<_>>();
    inferred.sort_unstable();
    inferred.dedup();
    inferred
}

fn sorted_ids(mut ids: Vec<u32>) -> Vec<u32> {
    ids.sort_unstable();
    ids
}
