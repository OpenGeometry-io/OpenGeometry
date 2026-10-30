use crate::exceptions::{enforce, Violation};
use crate::frozen_lists::INLINE_TESTS;
use crate::source_tree::{SourceFile, SourceTree};
use crate::{enforce_on_kernel, kernel_sources};

const TEST_MODULE: &str = "tests";

fn is_test_location(file: &SourceFile) -> bool {
    file.module.iter().any(|segment| segment == TEST_MODULE)
}

fn inline_test_violations(tree: &SourceTree) -> Vec<Violation> {
    let mut violations = Vec::new();
    for file in tree.files.iter().filter(|file| !is_test_location(file)) {
        for root in &file.scan.test_roots {
            let detail = format!(
                "test code `{}` sits inline; tests live in a sibling tests.rs or tests/",
                root.name
            );
            let key = [file.path.as_str(), root.name.as_str()];
            violations.push(Violation::new(&file.path, root.line, &key, detail));
        }
    }
    violations
}

fn ungated_test_module_violations(tree: &SourceTree) -> Vec<Violation> {
    let mut violations = Vec::new();
    for file in tree.files.iter().filter(|file| !file.test) {
        for declaration in &file.scan.mods {
            if declaration.name != TEST_MODULE || declaration.test {
                continue;
            }
            let detail = "`mod tests` is declared without `#[cfg(test)]`";
            violations.push(Violation::new(
                &file.path,
                declaration.line,
                &[&file.path],
                detail,
            ));
        }
    }
    violations
}

#[test]
fn tests_live_in_a_sibling_tests_rs_or_tests_directory() {
    enforce_on_kernel(
        "tests live in a sibling tests.rs or tests/",
        inline_test_violations,
        Some(&INLINE_TESTS),
    );
}

#[test]
fn every_tests_module_is_declared_under_cfg_test() {
    let violations = ungated_test_module_violations(&kernel_sources());
    enforce("mod tests only under #[cfg(test)]", &violations, None);
}
