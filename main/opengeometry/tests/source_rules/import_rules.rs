use crate::exceptions::{enforce, Violation};
use crate::kernel_sources;
use crate::lexer::Token;
use crate::source_tree::SourceTree;

fn function_local_use_violations(tree: &SourceTree) -> Vec<Violation> {
    let mut violations = Vec::new();
    for file in &tree.files {
        for item in file.scan.uses.iter().filter(|item| item.local) {
            let paths: Vec<String> = item.paths.iter().map(|path| path.text()).collect();
            let detail = format!("`use {}` inside a function or block", paths.join(", "));
            violations.push(Violation::new(&file.path, item.line, &[&file.path], detail));
        }
    }
    violations
}

fn is_restricted_visibility(code: &[Token], index: usize) -> bool {
    index >= 3 && code[index - 3].is("pub") && code[index - 2].is("(") && code[index - 1].is("in")
}

fn is_parent_qualifier(code: &[Token], index: usize) -> bool {
    let self_super =
        code[index].is("self") && code.get(index + 2).is_some_and(|next| next.is("super"));
    code[index].is("crate") || code[index].is("super") || self_super
}

fn inline_path_violations(tree: &SourceTree) -> Vec<Violation> {
    let mut violations = Vec::new();
    for file in &tree.files {
        for (index, pair) in file.code.windows(2).enumerate() {
            let qualifier = is_parent_qualifier(&file.code, index);
            if !qualifier || !pair[1].is("::") || file.scan.contexts[index].use_item.is_some() {
                continue;
            }
            let previous = index.checked_sub(1).map(|before| &file.code[before]);
            if previous.is_some_and(|before| before.is("::") || before.is("$"))
                || is_restricted_visibility(&file.code, index)
            {
                continue;
            }
            let head = if pair[0].is("self") {
                "self::super"
            } else {
                &pair[0].text
            };
            let detail = format!("inline `{head}::` path outside a use item");
            violations.push(Violation::new(
                &file.path,
                pair[0].line,
                &[&file.path],
                detail,
            ));
        }
    }
    violations
}

#[test]
fn no_use_is_function_local() {
    let violations = function_local_use_violations(&kernel_sources());
    enforce("no function-local use", &violations, None);
}

#[test]
fn crate_and_super_paths_appear_only_in_use_items() {
    let violations = inline_path_violations(&kernel_sources());
    enforce("no inline crate:: or super:: paths", &violations, None);
}
