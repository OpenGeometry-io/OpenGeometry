use crate::exceptions::{enforce, Violation};
use crate::frozen_lists::RESERVED_FN_NAMES;
use crate::path_aliases::STD_CRATES;
use crate::scanner::AttributeItem;
use crate::source_tree::{SourceFile, SourceTree};
use crate::{enforce_on_kernel, kernel_sources};
use std::collections::BTreeSet;

const RESERVED_NAMES: [&str; 4] = ["unwrap", "drop", "clone", "into"];
const PRELUDE_TRAITS_WITH_RESERVED_METHODS: [&str; 3] = ["Clone", "Drop", "Into"];
const LINT_ALLOWANCES: [&str; 2] = ["allow", "expect"];
const CRATE_ROOT: &str = "lib.rs";

fn token_pair_violations(tree: &SourceTree, pair: [&str; 2], detail: &str) -> Vec<Violation> {
    let mut violations = Vec::new();
    for file in &tree.files {
        for window in file.code.windows(2) {
            if window[0].is(pair[0]) && window[1].is(pair[1]) {
                violations.push(Violation::new(
                    &file.path,
                    window[0].line,
                    &[&file.path],
                    detail,
                ));
            }
        }
    }
    violations
}

fn include_macro_violations(tree: &SourceTree) -> Vec<Violation> {
    token_pair_violations(
        tree,
        ["include", "!"],
        "`include!` pulls a file into a module",
    )
}

fn attribute_violations(tree: &SourceTree, name: &str) -> Vec<Violation> {
    let mut violations = Vec::new();
    for file in &tree.files {
        for attribute in file
            .scan
            .attributes
            .iter()
            .filter(|attribute| attribute.name == name)
        {
            let detail = format!("`#[{name}]` attribute");
            violations.push(Violation::new(
                &file.path,
                attribute.line,
                &[&file.path],
                detail,
            ));
        }
    }
    violations
}

fn extern_crate_violations(tree: &SourceTree) -> Vec<Violation> {
    token_pair_violations(tree, ["extern", "crate"], "`extern crate` declaration")
}

fn super_glob_violations(tree: &SourceTree) -> Vec<Violation> {
    let mut violations = Vec::new();
    for file in tree.files.iter().filter(|file| !file.test) {
        for item in file.scan.uses.iter().filter(|item| !item.test) {
            for path in item.paths.iter().filter(|path| path.is_super_glob()) {
                let detail = format!("`use {};` outside a test module", path.text());
                violations.push(Violation::new(&file.path, item.line, &[&file.path], detail));
            }
        }
    }
    violations
}

fn is_std_trait(path: &[String], file: &SourceFile, crate_traits: &BTreeSet<&str>) -> bool {
    let from_std = |segments: &[String]| {
        segments.len() > 1
            && segments
                .first()
                .is_some_and(|root| STD_CRATES.contains(&root.as_str()))
    };
    let [name] = path else {
        return from_std(path);
    };
    let imported = file
        .scan
        .uses
        .iter()
        .flat_map(|item| &item.paths)
        .find(|use_path| use_path.binding.as_ref() == Some(name));
    match imported {
        Some(use_path) => from_std(&use_path.segments),
        None => {
            PRELUDE_TRAITS_WITH_RESERVED_METHODS.contains(&name.as_str())
                && !crate_traits.contains(name.as_str())
        }
    }
}

fn reserved_fn_name_violations(tree: &SourceTree) -> Vec<Violation> {
    let crate_traits: BTreeSet<&str> = tree
        .files
        .iter()
        .flat_map(|file| &file.scan.traits)
        .map(String::as_str)
        .collect();
    let mut violations = Vec::new();
    for file in &tree.files {
        for function in &file.scan.fns {
            let std_impl = function
                .impl_trait
                .as_ref()
                .is_some_and(|path| is_std_trait(path, file, &crate_traits));
            if std_impl || !RESERVED_NAMES.contains(&function.bare.as_str()) {
                continue;
            }
            let detail = format!("fn `{}` reuses a std method name", function.name);
            let key = [file.path.as_str(), function.name.as_str()];
            violations.push(Violation::new(&file.path, function.start, &key, detail));
        }
    }
    violations
}

fn names_lint_allowance(attribute: &AttributeItem) -> bool {
    attribute
        .words
        .windows(2)
        .any(|pair| LINT_ALLOWANCES.contains(&pair[0].as_str()) && pair[1] == "(")
}

fn is_doc_text(attribute: &AttributeItem) -> bool {
    attribute
        .words
        .windows(2)
        .any(|pair| pair[0] == "doc" && pair[1] == "=")
}

fn attribute_matches<'a>(
    files: impl Iterator<Item = &'a SourceFile>,
    matches: fn(&AttributeItem) -> bool,
    detail: &str,
) -> Vec<Violation> {
    let mut violations = Vec::new();
    for file in files {
        for attribute in file.scan.attributes.iter().filter(|item| matches(item)) {
            let text = format!("`#[{}]` {detail}", attribute.words.join(""));
            violations.push(Violation::new(
                &file.path,
                attribute.line,
                &[&file.path],
                text,
            ));
        }
    }
    violations
}

#[test]
fn source_uses_no_include_macro() {
    enforce(
        "no include!",
        &include_macro_violations(&kernel_sources()),
        None,
    );
}

#[test]
fn source_uses_no_path_attribute() {
    let violations = attribute_violations(&kernel_sources(), "path");
    enforce("no #[path]", &violations, None);
}

#[test]
fn use_super_glob_appears_only_in_test_modules() {
    let violations = super_glob_violations(&kernel_sources());
    enforce("no use super::* outside test modules", &violations, None);
}

#[test]
fn no_test_is_ignored() {
    let violations = attribute_violations(&kernel_sources(), "ignore");
    enforce("no #[ignore]", &violations, None);
}

#[test]
fn no_fn_is_named_unwrap_drop_clone_or_into() {
    enforce_on_kernel(
        "no fn named unwrap, drop, clone or into",
        reserved_fn_name_violations,
        Some(&RESERVED_FN_NAMES),
    );
}

#[test]
fn no_lint_is_allowed_outside_lib_rs() {
    let tree = kernel_sources();
    let files = tree.files.iter().filter(|file| file.path != CRATE_ROOT);
    let violations = attribute_matches(files, names_lint_allowance, "outside lib.rs");
    enforce("no #[allow] outside lib.rs", &violations, None);
}

#[test]
fn no_doc_attribute_carries_text() {
    let tree = kernel_sources();
    let violations = attribute_matches(tree.files.iter(), is_doc_text, "is a comment");
    enforce("zero comments, doc attributes included", &violations, None);
}

#[test]
fn use_super_glob_rule_catches_a_self_super_glob() {
    let tree = SourceTree::from_sources(&[
        ("geom2d/mod.rs", "mod edge;\n"),
        ("geom2d/edge.rs", "use self::super::*;\n"),
    ]);
    let violations = super_glob_violations(&tree);
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].location, "src/geom2d/edge.rs:1");
}

#[test]
fn source_declares_no_extern_crate() {
    enforce_on_kernel("no extern crate", extern_crate_violations, None);
}

#[test]
fn extern_crate_rule_catches_plain_aliased_and_self_forms() {
    let tree = SourceTree::from_sources(&[(
        "lib.rs",
        "extern crate earcutr;\nextern crate earcutr as ear;\nextern crate self as kernel;\n",
    )]);
    let lines: Vec<String> = extern_crate_violations(&tree)
        .into_iter()
        .map(|violation| violation.location)
        .collect();
    assert_eq!(lines, ["src/lib.rs:1", "src/lib.rs:2", "src/lib.rs:3"]);
}
