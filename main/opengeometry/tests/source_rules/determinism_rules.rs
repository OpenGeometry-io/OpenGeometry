use crate::exceptions::{enforce, Violation};
use crate::kernel_sources;
use crate::path_aliases::{path_at, Exports, PathFinder, STD_CRATES};
use crate::source_tree::{SourceFile, SourceTree};

const GLOBAL_STATE_ITEMS: [&str; 6] = [
    "OnceLock",
    "OnceCell",
    "LazyLock",
    "LazyCell",
    "lazy_static",
    "thread_local",
];

fn nondeterministic_path(path: &[String]) -> Option<&'static str> {
    if let Some(name) = GLOBAL_STATE_ITEMS
        .iter()
        .copied()
        .find(|name| path.iter().any(|segment| segment == name))
    {
        return Some(name);
    }
    match path {
        [root, time, ..] if STD_CRATES.contains(&root.as_str()) && time == "time" => {
            Some("std::time")
        }
        [rand, _, ..] if rand == "rand" => Some("rand"),
        _ => None,
    }
}

fn static_mut_lines(file: &SourceFile) -> Vec<usize> {
    file.code
        .windows(2)
        .filter(|pair| pair[0].is("static") && pair[1].is("mut"))
        .map(|pair| pair[0].line)
        .collect()
}

fn file_violations(file: &SourceFile, exports: &Exports) -> Vec<Violation> {
    let finder = PathFinder::new(file, nondeterministic_path, exports);
    let mut violations = Vec::new();
    for line in static_mut_lines(file) {
        let detail = "`static mut` makes results depend on hidden state";
        violations.push(Violation::new(&file.path, line, &[&file.path], detail));
    }
    for index in 0..file.code.len() {
        if file.scan.contexts[index].use_item.is_some() {
            continue;
        }
        if let Some(name) = finder.target_at(index) {
            let path = path_at(&file.code, index).unwrap_or_default().join("::");
            let detail =
                format!("`{path}` reaches `{name}`, which makes results depend on hidden state");
            let line = file.code[index].line;
            violations.push(Violation::new(&file.path, line, &[&file.path], detail));
        }
    }
    for item in &file.scan.uses {
        let scope = [file.module.as_slice(), &item.scope].concat();
        for path in &item.paths {
            if let Some(name) = finder.path_target(&path.segments, &scope) {
                let detail = format!("`use {}` brings in `{name}`", path.text());
                violations.push(Violation::new(&file.path, item.line, &[&file.path], detail));
            }
        }
    }
    violations
}

#[test]
fn source_has_no_global_state_clock_or_randomness() {
    let tree = kernel_sources();
    let exports = Exports::build(&tree, nondeterministic_path);
    let violations: Vec<Violation> = tree
        .files
        .iter()
        .flat_map(|file| file_violations(file, &exports))
        .collect();
    enforce(
        "no thread_local!, static mut, OnceLock, OnceCell, LazyLock, LazyCell, lazy_static, std::time or rand",
        &violations,
        None,
    );
}

#[test]
fn determinism_rule_sees_a_rooted_clock_path_after_a_comparison() {
    let tree = SourceTree::from_sources(&[(
        "math/mod.rs",
        "fn late(stamp: u64) -> bool {\n    stamp > ::std::time::UNIX_EPOCH\n}\n",
    )]);
    let exports = Exports::build(&tree, nondeterministic_path);
    let violations = file_violations(&tree.files[0], &exports);
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].location, "src/math/mod.rs:2");
}
