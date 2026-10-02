#![cfg(not(target_arch = "wasm32"))]

mod determinism_rules;
mod exceptions;
mod external_crate_rules;
mod frozen_lists;
mod function_rules;
mod graph;
mod import_rules;
mod item_rules;
mod layer_rules;
mod lexer;
mod lexer_cases;
mod module_paths;
mod module_rules;
mod path_aliases;
mod scanner;
mod source_tree;
mod test_rules;
mod text_rules;
mod type_rules;
mod use_tree;

use exceptions::{enforce, ExceptionList, ListSpec, Violation};
use source_tree::SourceTree;
use std::path::{Path, PathBuf};

const RULES_ROOT: &str = "tests/source_rules";
const ROOTS_OUTSIDE_KERNEL_SRC: [&str; 4] = [
    "tests",
    "test-support/src",
    "examples",
    "../tools/parity-oracle/src",
];

fn kernel_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn load_tree(root: &Path) -> SourceTree {
    SourceTree::load(root)
        .unwrap_or_else(|error| panic!("cannot read {}: {}", error.path.display(), error.reason))
}

fn kernel_sources() -> SourceTree {
    load_tree(&kernel_dir().join("src"))
}

fn rule_sources() -> SourceTree {
    load_tree(&kernel_dir().join(RULES_ROOT))
}

fn rule_violations(check: fn(&SourceTree) -> Vec<Violation>) -> Vec<Violation> {
    check(&rule_sources())
        .into_iter()
        .map(|violation| violation.under(RULES_ROOT))
        .collect()
}

fn violations_outside_kernel_src(check: fn(&SourceTree) -> Vec<Violation>) -> Vec<Violation> {
    let mut violations = Vec::new();
    for root in ROOTS_OUTSIDE_KERNEL_SRC {
        let mut tree = load_tree(&kernel_dir().join(root));
        tree.files
            .retain(|file| !Path::new(root).join(&file.path).starts_with(RULES_ROOT));
        violations.extend(
            check(&tree)
                .into_iter()
                .map(|violation| violation.under(root)),
        );
    }
    violations
}

fn kernel_manifest() -> String {
    let path = kernel_dir().join("Cargo.toml");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
}

fn rule_list(spec: &ListSpec) -> ExceptionList {
    ExceptionList::load(&kernel_dir().join(RULES_ROOT).join(spec.file), spec)
}

fn enforce_on_kernel(
    rule: &str,
    check: fn(&SourceTree) -> Vec<Violation>,
    spec: Option<&ListSpec>,
) {
    let list = spec.map(rule_list);
    enforce(rule, &check(&kernel_sources()), list.as_ref());
}
