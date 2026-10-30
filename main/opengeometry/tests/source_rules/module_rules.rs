use crate::exceptions::{enforce, Violation};
use crate::source_tree::SourceTree;
use crate::{enforce_on_kernel, kernel_sources};

const CATCH_ALL_NAMES: [&str; 5] = ["utils", "helpers", "common", "misc", "booleans"];
const HANDLERS: &str = "operations/modifying/boolean/handlers";
const OPERANDS: [&str; 6] = [
    "box",
    "conic",
    "cylinder",
    "planar_extrusion",
    "sphere",
    "torus",
];
const RELATIONS: [&str; 2] = ["coincident", "disjoint"];
const FAMILIES: [&str; 5] = [
    "containment",
    "generic",
    "planar",
    "rectilinear",
    "vertical_arc_extrusion",
];

fn missing_mod_rs_violations(tree: &SourceTree) -> Vec<Violation> {
    tree.directories
        .iter()
        .filter(|directory| !tree.has_file(&format!("{directory}/mod.rs")))
        .map(|directory| {
            Violation::new(
                directory,
                0,
                &[directory],
                "directory module without mod.rs",
            )
        })
        .collect()
}

fn sibling_file_violations(tree: &SourceTree) -> Vec<Violation> {
    tree.directories
        .iter()
        .filter(|directory| tree.has_file(&format!("{directory}.rs")))
        .map(|directory| {
            let file = format!("{directory}.rs");
            let detail = format!("`{file}` sits beside the directory `{directory}/`");
            Violation::new(&file, 1, &[&file], detail)
        })
        .collect()
}

fn catch_all_name_violations(tree: &SourceTree) -> Vec<Violation> {
    let mut violations = Vec::new();
    for file in &tree.files {
        let declared = file.module.last().map(|name| (name.as_str(), 1));
        let inline = file
            .scan
            .mods
            .iter()
            .filter(|declaration| declaration.inline)
            .map(|declaration| (declaration.name.as_str(), declaration.line));
        for (name, line) in declared.into_iter().chain(inline) {
            if CATCH_ALL_NAMES.contains(&name) {
                let detail = format!("module `{name}` has a catch-all name");
                violations.push(Violation::new(&file.path, line, &[&file.path], detail));
            }
        }
    }
    violations
}

fn is_operand_pair(stem: &str) -> bool {
    OPERANDS.iter().any(|first| {
        stem.strip_prefix(first)
            .and_then(|rest| rest.strip_prefix('_'))
            .is_some_and(|second| OPERANDS.contains(&second))
    })
}

fn is_same_operand_pair(stem: &str) -> bool {
    OPERANDS.iter().any(|operand| {
        let plural = if operand.ends_with('x') { "es" } else { "s" };
        stem == format!("{operand}{plural}")
    })
}

fn is_handler_file_name(stem: &str) -> bool {
    stem == "mod"
        || is_operand_pair(stem)
        || is_same_operand_pair(stem)
        || RELATIONS.contains(&stem)
}

fn is_family_name(name: &str) -> bool {
    is_operand_pair(name) || FAMILIES.contains(&name)
}

fn handler_name_violations(tree: &SourceTree) -> Vec<Violation> {
    let prefix = format!("{HANDLERS}/");
    let mut violations: Vec<Violation> = tree
        .files
        .iter()
        .filter(|file| file.directory() == HANDLERS && !is_handler_file_name(file.stem()))
        .map(|file| {
            let detail = "handler file is neither `<operand>_<operand>.rs` nor a family directory";
            Violation::new(&file.path, 1, &[&file.path], detail)
        })
        .collect();
    for directory in &tree.directories {
        let Some(name) = directory.strip_prefix(&prefix) else {
            continue;
        };
        if !name.contains('/') && !is_family_name(name) {
            let detail = format!("handler family `{name}/` is not in the operand vocabulary");
            violations.push(Violation::new(directory, 0, &[directory], detail));
        }
    }
    violations
}

#[test]
fn directory_modules_use_mod_rs() {
    enforce(
        "directory modules use mod.rs",
        &missing_mod_rs_violations(&kernel_sources()),
        None,
    );
}

#[test]
fn no_module_file_sits_beside_its_directory() {
    let violations = sibling_file_violations(&kernel_sources());
    enforce("no x.rs beside x/", &violations, None);
}

#[test]
fn no_module_has_a_catch_all_name() {
    enforce_on_kernel(
        "no module named utils, helpers, common, misc or booleans",
        catch_all_name_violations,
        None,
    );
}

#[test]
fn boolean_handler_files_are_operand_pairs_or_family_directories() {
    enforce_on_kernel(
        "handler files are <operand>_<operand>.rs or <family>/",
        handler_name_violations,
        None,
    );
}
