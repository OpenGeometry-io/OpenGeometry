use crate::exceptions::{enforce, Violation};
use crate::source_tree::SourceTree;
use crate::{kernel_sources, rule_violations};

const MAX_FILE_LINES: usize = 600;

fn comment_violations(tree: &SourceTree) -> Vec<Violation> {
    let mut violations = Vec::new();
    for file in &tree.files {
        for comment in &file.comments {
            let preview: String = comment.text.chars().take(40).collect();
            let detail = format!("comment `{preview}`");
            violations.push(Violation::new(
                &file.path,
                comment.line,
                &[&file.path],
                detail,
            ));
        }
    }
    violations
}

fn oversized_file_violations(tree: &SourceTree) -> Vec<Violation> {
    tree.files
        .iter()
        .filter(|file| file.line_count > MAX_FILE_LINES)
        .map(|file| {
            let detail = format!("{} lines, above {MAX_FILE_LINES}", file.line_count);
            Violation::new(&file.path, file.line_count, &[&file.path], detail)
        })
        .collect()
}

#[test]
fn source_files_contain_no_comments() {
    enforce(
        "zero comments",
        &comment_violations(&kernel_sources()),
        None,
    );
}

#[test]
fn source_files_are_at_most_600_lines() {
    let violations = oversized_file_violations(&kernel_sources());
    enforce("every file at most 600 lines", &violations, None);
}

#[test]
fn source_rules_files_contain_no_comments() {
    let violations = rule_violations(comment_violations);
    enforce("zero comments in source_rules", &violations, None);
}

#[test]
fn source_rules_files_are_at_most_600_lines() {
    let violations = rule_violations(oversized_file_violations);
    enforce(
        "every source_rules file at most 600 lines",
        &violations,
        None,
    );
}
