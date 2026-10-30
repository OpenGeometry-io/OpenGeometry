use crate::exceptions::{enforce, Violation};
use crate::lexer::Token;
use crate::path_aliases::opens_path;
use crate::source_tree::{SourceFile, SourceTree};
use crate::{kernel_manifest, kernel_sources};

const MATH: &str = "math";
const MATH_EXTERNAL_CRATES: [&str; 1] = ["serde"];
const DEPENDENCY_TABLE_SUFFIX: &str = "dependencies]";

fn manifest_crates(manifest: &str) -> Vec<String> {
    let mut crates = Vec::new();
    let mut in_dependencies = false;
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            in_dependencies = line.ends_with(DEPENDENCY_TABLE_SUFFIX);
            continue;
        }
        let Some((name, _)) = line.split_once('=') else {
            continue;
        };
        let name = name.trim().split('.').next().unwrap_or_default();
        if in_dependencies && !name.is_empty() {
            crates.push(name.replace('-', "_"));
        }
    }
    crates
}

fn names_crate_root(code: &[Token], index: usize) -> bool {
    let Some(before) = index.checked_sub(1).map(|at| &code[at]) else {
        return true;
    };
    if before.is(".") {
        return false;
    }
    !before.is("::") || opens_path(code, index - 1)
}

fn file_violations(file: &SourceFile, forbidden: &[String]) -> Vec<Violation> {
    let mut violations = Vec::new();
    for (index, token) in file.code.iter().enumerate() {
        let external = forbidden.iter().any(|name| token.is(name));
        if !external || file.is_test_at(index) || !names_crate_root(&file.code, index) {
            continue;
        }
        let detail = format!(
            "math reaches the external crate `{}`; it may use only std and serde",
            token.text
        );
        violations.push(Violation::new(
            &file.path,
            token.line,
            &[&file.path],
            detail,
        ));
    }
    violations
}

fn math_external_crate_violations(tree: &SourceTree, manifest: &str) -> Vec<Violation> {
    let forbidden: Vec<String> = manifest_crates(manifest)
        .into_iter()
        .filter(|name| !MATH_EXTERNAL_CRATES.contains(&name.as_str()))
        .collect();
    tree.files
        .iter()
        .filter(|file| !file.test && file.module.first().is_some_and(|top| top == MATH))
        .flat_map(|file| file_violations(file, &forbidden))
        .collect()
}

#[test]
fn math_imports_only_std_and_serde_among_external_crates() {
    let violations = math_external_crate_violations(&kernel_sources(), &kernel_manifest());
    enforce(
        "math imports only std and serde among external crates",
        &violations,
        None,
    );
}

#[test]
fn math_external_crate_rule_sees_rooted_paths_after_comparisons_and_generic_lists() {
    let tree = SourceTree::from_sources(&[
        ("math/mod.rs", "mod bound;\nmod compare;\nmod generic;\n"),
        (
            "math/bound.rs",
            "fn apply<F>(visit: F)\nwhere\n    F: for<'a> ::earcutr::Visit<'a>,\n{\n}\n",
        ),
        (
            "math/compare.rs",
            "fn near(count: usize) -> bool {\n    count > ::earcutr::LIMIT\n}\n",
        ),
        (
            "math/generic.rs",
            "impl<T> ::earcutr::Flatten for Points<T> {}\n",
        ),
    ]);
    let manifest = "[dependencies]\nearcutr = \"=0.3.0\"\n";
    let violations = math_external_crate_violations(&tree, manifest);
    let locations: Vec<&str> = violations
        .iter()
        .map(|violation| violation.location.as_str())
        .collect();
    assert_eq!(
        locations,
        [
            "src/math/bound.rs:3",
            "src/math/compare.rs:2",
            "src/math/generic.rs:1"
        ]
    );
}
