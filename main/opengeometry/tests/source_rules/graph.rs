use crate::module_paths::{absolute_path, module_table, ModuleTable};
use crate::path_aliases::path_at;
use crate::source_tree::{SourceFile, SourceTree};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct Edge {
    pub(crate) from: usize,
    pub(crate) to: usize,
    pub(crate) from_module: Vec<String>,
    pub(crate) defining_module: Vec<String>,
    pub(crate) line: usize,
    pub(crate) path: String,
    pub(crate) test: bool,
}

type Bindings = BTreeMap<Vec<String>, BTreeMap<String, Vec<String>>>;

const RE_EXPORT_HOPS: usize = 16;
const PATH_HEADS: [&str; 3] = ["crate", "self", "super"];

struct Reference {
    segments: Vec<String>,
    scope: Vec<String>,
    line: usize,
    test: bool,
}

struct Resolver {
    modules: ModuleTable,
    bindings: Vec<Bindings>,
}

impl Resolver {
    fn new(tree: &SourceTree) -> Resolver {
        let modules = module_table(tree);
        let bindings = tree
            .files
            .iter()
            .map(|file| file_bindings(file, &modules))
            .collect();
        Resolver { modules, bindings }
    }

    fn locate(&self, absolute: &[String]) -> Option<(usize, usize)> {
        (0..=absolute.len()).rev().find_map(|length| {
            self.modules
                .get(&absolute[..length])
                .map(|&file| (file, length))
        })
    }

    fn follow(&self, mut absolute: Vec<String>) -> Option<(usize, Vec<String>)> {
        for _ in 0..RE_EXPORT_HOPS {
            let (file, length) = self.locate(&absolute)?;
            let rebound = (length < absolute.len())
                .then(|| self.bindings[file].get(&absolute[..length]))
                .flatten()
                .and_then(|names| names.get(&absolute[length]));
            match rebound {
                Some(target) => absolute = [target.as_slice(), &absolute[length + 1..]].concat(),
                None => return Some((file, absolute[..length].to_vec())),
            }
        }
        None
    }

    fn edge(&self, from: usize, reference: Reference) -> Option<Edge> {
        let local = self.bindings[from].get(&reference.scope);
        let absolute = absolute_path(&reference.segments, &reference.scope, local, &self.modules)?;
        let (named, depth) = self.locate(&absolute)?;
        let (defining, defining_module) = self.follow(absolute)?;
        let to = if depth == 0 { defining } else { named };
        (to != from).then(|| Edge {
            from,
            to,
            from_module: reference.scope,
            defining_module,
            line: reference.line,
            path: reference.segments.join("::"),
            test: reference.test,
        })
    }
}

pub(crate) fn import_edges(tree: &SourceTree) -> Vec<Edge> {
    let resolver = Resolver::new(tree);
    let mut edges = Vec::new();
    for (index, file) in tree.files.iter().enumerate() {
        let references = use_references(file)
            .into_iter()
            .chain(code_references(file, &resolver.modules));
        edges.extend(references.filter_map(|reference| resolver.edge(index, reference)));
    }
    edges
}

fn use_references(file: &SourceFile) -> Vec<Reference> {
    let mut references = Vec::new();
    for item in &file.scan.uses {
        for path in &item.paths {
            references.push(Reference {
                segments: path.segments.clone(),
                scope: [file.module.clone(), item.scope.clone()].concat(),
                line: item.line,
                test: file.test || item.test,
            });
        }
    }
    references
}

fn code_references(file: &SourceFile, modules: &ModuleTable) -> Vec<Reference> {
    let mut references = Vec::new();
    for index in 0..file.code.len() {
        if file.scan.contexts[index].use_item.is_some() {
            continue;
        }
        let Some(segments) = path_at(&file.code, index).filter(|path| path.len() > 1) else {
            continue;
        };
        let scope = [file.module.as_slice(), file.scan.scope_at(index)].concat();
        let child = [scope.as_slice(), &segments[..1]].concat();
        if !PATH_HEADS.contains(&segments[0].as_str()) && !modules.contains_key(&child) {
            continue;
        }
        references.push(Reference {
            segments,
            scope,
            line: file.code[index].line,
            test: file.is_test_at(index),
        });
    }
    references
}

fn file_bindings(file: &SourceFile, modules: &ModuleTable) -> Bindings {
    let mut bindings = Bindings::new();
    for _ in 0..2 {
        for item in &file.scan.uses {
            let scope = [file.module.clone(), item.scope.clone()].concat();
            for path in &item.paths {
                let Some(binding) = &path.binding else {
                    continue;
                };
                let local = bindings.get(&scope);
                if let Some(absolute) = absolute_path(&path.segments, &scope, local, modules) {
                    bindings
                        .entry(scope.clone())
                        .or_default()
                        .insert(binding.clone(), absolute);
                }
            }
        }
    }
    bindings
}

pub(crate) fn top_module(file: &SourceFile) -> Option<&str> {
    file.module.first().map(String::as_str)
}

pub(crate) fn cycles(adjacency: &BTreeMap<String, BTreeSet<String>>) -> Vec<BTreeSet<String>> {
    let mut found: Vec<BTreeSet<String>> = Vec::new();
    for module in adjacency.keys() {
        let cycle: BTreeSet<String> = reachable(adjacency, module)
            .into_iter()
            .filter(|other| reachable(adjacency, other).contains(module))
            .collect();
        if cycle.len() >= 2 && !found.contains(&cycle) {
            found.push(cycle);
        }
    }
    found
}

fn reachable(adjacency: &BTreeMap<String, BTreeSet<String>>, from: &str) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    let mut stack = vec![from.to_string()];
    while let Some(node) = stack.pop() {
        for next in adjacency.get(&node).into_iter().flatten() {
            if seen.insert(next.clone()) {
                stack.push(next.clone());
            }
        }
    }
    seen
}
