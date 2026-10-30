use crate::exceptions::{enforce, Violation};
use crate::frozen_lists::PORTED_LONG_FUNCTIONS;
use crate::source_tree::SourceTree;
use crate::{kernel_sources, rule_list, rule_violations, violations_outside_kernel_src};

const MAX_FN_LINES: usize = 80;

fn long_function_violations(tree: &SourceTree, tests_count: bool) -> Vec<Violation> {
    let mut violations = Vec::new();
    for file in tree.files.iter().filter(|file| tests_count || !file.test) {
        let counted = file
            .scan
            .fns
            .iter()
            .filter(|function| tests_count || !function.test);
        for function in counted {
            let span = function.span();
            if span <= MAX_FN_LINES {
                continue;
            }
            let detail = format!(
                "fn `{}` spans {span} lines, above {MAX_FN_LINES}",
                function.name
            );
            let key = [file.path.as_str(), function.name.as_str()];
            violations
                .push(Violation::new(&file.path, function.start, &key, detail).with_measure(span));
        }
    }
    violations
}

#[test]
fn non_test_functions_are_at_most_80_lines() {
    let violations = long_function_violations(&kernel_sources(), false);
    let list = rule_list(&PORTED_LONG_FUNCTIONS);
    enforce(
        "every non-test fn at most 80 lines",
        &violations,
        Some(&list),
    );
}

#[test]
fn non_test_functions_outside_the_kernel_src_are_at_most_80_lines() {
    let violations = violations_outside_kernel_src(|tree| long_function_violations(tree, false));
    enforce(
        "every non-test fn outside the kernel src at most 80 lines",
        &violations,
        None,
    );
}

#[test]
fn source_rules_functions_are_at_most_80_lines() {
    let violations = rule_violations(|tree| long_function_violations(tree, true));
    enforce("every source_rules fn at most 80 lines", &violations, None);
}

#[test]
fn long_function_rule_counts_items_with_a_cfg_test_generic_parameter() {
    let body = "    step();\n".repeat(MAX_FN_LINES);
    let text = format!(
        "fn long<#[cfg(test)] T>() {{\n{body}}}\n\nimpl<#[cfg(test)] T> Holder<T> {{\n    fn method() {{\n{body}    }}\n}}\n"
    );
    let tree = SourceTree::from_sources(&[("math/mod.rs", &text)]);
    let names: Vec<String> = long_function_violations(&tree, false)
        .into_iter()
        .map(|violation| violation.key[1].clone())
        .collect();
    assert_eq!(names, ["long", "Holder::method"]);
}
