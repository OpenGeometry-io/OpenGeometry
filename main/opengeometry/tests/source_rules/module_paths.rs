use crate::source_tree::SourceTree;
use std::collections::BTreeMap;

pub(crate) type ModuleTable = BTreeMap<Vec<String>, usize>;

pub(crate) fn module_table(tree: &SourceTree) -> ModuleTable {
    let mut modules = ModuleTable::new();
    for (index, file) in tree.files.iter().enumerate() {
        modules.insert(file.module.clone(), index);
        for declaration in file
            .scan
            .mods
            .iter()
            .filter(|declaration| declaration.inline)
        {
            let inline = [
                file.module.clone(),
                declaration.scope.clone(),
                vec![declaration.name.clone()],
            ]
            .concat();
            modules.insert(inline, index);
        }
    }
    modules
}

pub(crate) fn absolute_path(
    segments: &[String],
    scope: &[String],
    local: Option<&BTreeMap<String, Vec<String>>>,
    modules: &ModuleTable,
) -> Option<Vec<String>> {
    let (head, rest) = segments.split_first()?;
    match head.as_str() {
        "crate" => Some(rest.to_vec()),
        "self" if rest.first().is_some_and(|next| next == "super") => {
            absolute_path(rest, scope, local, modules)
        }
        "self" => Some([scope, rest].concat()),
        "super" => {
            let supers = segments
                .iter()
                .take_while(|segment| *segment == "super")
                .count();
            let base = scope.get(..scope.len().checked_sub(supers)?)?;
            Some([base, &segments[supers..]].concat())
        }
        _ => {
            let child = [scope, std::slice::from_ref(head)].concat();
            if modules.contains_key(&child) {
                Some([scope, segments].concat())
            } else {
                let base = local?.get(head)?;
                Some([base.as_slice(), rest].concat())
            }
        }
    }
}
