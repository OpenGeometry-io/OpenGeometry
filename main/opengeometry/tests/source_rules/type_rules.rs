use crate::enforce_on_kernel;
use crate::exceptions::Violation;
use crate::frozen_lists::{JSON_VALUES_IN_SIGNATURES, SERDE_JSON_OUTSIDE_EDGES, STRING_ERRORS};
use crate::lexer::Token;
use crate::path_aliases::{Exports, PathFinder, STD_CRATES};
use crate::source_tree::{SourceFile, SourceTree};
use std::collections::BTreeSet;

const JSON_EDGE_FILES: [&str; 4] = [
    "brep/topology/envelope.rs",
    "world_graph/queries.rs",
    "tessellation/snapshot_store.rs",
    "tessellation/cache.rs",
];
const BINDINGS: &str = "bindings/";
const STRING_CRATES: [&str; 2] = ["std", "alloc"];
const ERROR_TYPE_NAMES: [&str; 2] = ["Err", "Error"];
const BINDING_OPENERS: [&str; 2] = ["<", ","];
const BOXED: &str = "Box";
const STR: &str = "str";

const DECLARATIONS: [&str; 5] = ["fn", "struct", "enum", "union", "type"];

fn item_label(file: &SourceFile, index: usize) -> String {
    let context = file.scan.contexts[index];
    let name = file.scan.item_name(index);
    if let Some(item) = context.use_item {
        file.scan.uses[item].label.clone()
    } else if let Some(label) = file.scan.impl_labels.get(&context.item) {
        label.clone()
    } else if name.is_empty() {
        "file".to_string()
    } else {
        name.to_string()
    }
}

fn once_per_item(file: &SourceFile, index: usize, seen: &mut BTreeSet<String>) -> Option<String> {
    let item = item_label(file, index);
    seen.insert(item.clone()).then_some(item)
}

fn closing_index(code: &[Token], open: usize) -> usize {
    let mut depth = 0usize;
    for (index, token) in code.iter().enumerate().skip(open) {
        let arrow = index > 0 && (code[index - 1].is("-") || code[index - 1].is("="));
        if token.is("<") || token.is("(") || token.is("[") || token.is("{") {
            depth += 1;
        } else if (token.is(">") && !arrow) || token.is(")") || token.is("]") || token.is("}") {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return index;
            }
        }
    }
    code.len() - 1
}

fn last_type_argument(code: &[Token], open: usize) -> Vec<&str> {
    let mut close = closing_index(code, open);
    if code[close - 1].is(",") {
        close -= 1;
    }
    let mut depth = 0usize;
    let mut start = open + 1;
    for index in open + 1..close {
        let token = &code[index];
        let arrow = code[index - 1].is("-") || code[index - 1].is("=");
        if token.is("<") || token.is("(") || token.is("[") {
            depth += 1;
        } else if (token.is(">") && !arrow) || token.is(")") || token.is("]") {
            depth = depth.saturating_sub(1);
        } else if token.is(",") && depth == 0 {
            start = index + 1;
        }
    }
    code[start..close]
        .iter()
        .map(|token| token.text.as_str())
        .collect()
}

fn string_type(path: &[String]) -> Option<&'static str> {
    let qualified = matches!(path, [root, module, name]
        if STRING_CRATES.contains(&root.as_str()) && module == "string" && name == "String");
    (qualified || path == ["String"]).then_some("String")
}

fn result_type(path: &[String]) -> Option<&'static str> {
    let qualified = matches!(path, [root, module, name]
        if STD_CRATES.contains(&root.as_str()) && module == "result" && name == "Result");
    (qualified || path == ["Result"]).then_some("Result")
}

fn names_string(finder: &PathFinder, error: &[&str], scope: &[String]) -> bool {
    let error = error.strip_prefix(&["::"]).unwrap_or(error);
    let segments: Vec<String> = error
        .iter()
        .step_by(2)
        .map(|segment| (*segment).to_string())
        .collect();
    let is_path = error
        .iter()
        .skip(1)
        .step_by(2)
        .all(|separator| *separator == "::");
    is_path && !segments.is_empty() && finder.path_target(&segments, scope).is_some()
}

fn unboxed<'a>(error: &'a [&'a str]) -> &'a [&'a str] {
    match error.iter().position(|token| *token == "<") {
        Some(open) if open > 0 && error[open - 1] == BOXED && error.last() == Some(&">") => {
            &error[open + 1..error.len() - 1]
        }
        _ => error,
    }
}

fn referent<'a>(error: &'a [&'a str]) -> &'a [&'a str] {
    let Some(rest) = error.strip_prefix(&["&"]) else {
        return error;
    };
    match rest {
        [lifetime, after @ ..] if lifetime.starts_with('\'') => after,
        _ => rest,
    }
}

fn text_error_kind(finder: &PathFinder, error: &[&str], scope: &[String]) -> Option<&'static str> {
    let text = referent(unboxed(error));
    if text == [STR] {
        return Some(STR);
    }
    names_string(finder, text, scope).then_some("String")
}

fn names_error_type(name: &str) -> bool {
    name.starts_with(|first: char| first == '_' || first.is_alphabetic())
}

fn result_error<'a>(
    code: &'a [Token],
    result_finder: &PathFinder,
    index: usize,
) -> Option<Vec<&'a str>> {
    let generic = code[index + 1].is("<");
    if !generic || !(code[index].is("Result") || result_finder.target_at(index).is_some()) {
        return None;
    }
    Some(last_type_argument(code, index + 1))
}

fn error_type_names(tree: &SourceTree, results: &Exports) -> BTreeSet<String> {
    let mut names: BTreeSet<String> = ERROR_TYPE_NAMES
        .iter()
        .map(|name| name.to_string())
        .collect();
    for file in &tree.files {
        let result_finder = PathFinder::new(file, result_type, results);
        for index in 0..file.code.len().saturating_sub(1) {
            let error = result_error(&file.code, &result_finder, index).unwrap_or_default();
            if let Some(name) = error.last().filter(|name| names_error_type(name)) {
                names.insert((*name).to_string());
            }
        }
    }
    names
}

fn binding_value(code: &[Token], start: usize) -> Vec<&str> {
    let mut depth = 0usize;
    let mut end = start;
    while let Some(token) = code.get(end) {
        let arrow = code[end - 1].is("-") || code[end - 1].is("=");
        if token.is("<") || token.is("(") || token.is("[") {
            depth += 1;
        } else if (token.is(">") && !arrow) || token.is(")") || token.is("]") {
            if depth == 0 {
                break;
            }
            depth -= 1;
        } else if token.is(",") && depth == 0 {
            break;
        }
        end += 1;
    }
    code[start..end]
        .iter()
        .map(|token| token.text.as_str())
        .collect()
}

fn error_binding_value<'a>(
    code: &'a [Token],
    index: usize,
    names: &BTreeSet<String>,
) -> Option<(&'a str, Vec<&'a str>)> {
    let opened = index
        .checked_sub(1)
        .is_some_and(|before| BINDING_OPENERS.iter().any(|opener| code[before].is(opener)));
    let assigned = code.get(index + 1).is_some_and(|next| next.is("="))
        && !code
            .get(index + 2)
            .is_some_and(|value| value.is("=") || value.is(">"));
    if !opened || !assigned || !names.contains(&code[index].text) {
        return None;
    }
    Some((code[index].text.as_str(), binding_value(code, index + 2)))
}

fn error_type_value<'a>(
    code: &'a [Token],
    index: usize,
    names: &BTreeSet<String>,
) -> Option<(&'a str, Vec<&'a str>)> {
    let name = code.get(index + 1)?;
    if !code[index].is("type") || !names.contains(&name.text) {
        return None;
    }
    let end = (index..code.len()).find(|&at| code[at].is(";"))?;
    let equals = (index..end).find(|&at| code[at].is("="))?;
    let value = code[equals + 1..end]
        .iter()
        .map(|token| token.text.as_str())
        .collect();
    Some((name.text.as_str(), value))
}

fn string_error_at(
    code: &[Token],
    finder: &PathFinder,
    result_finder: &PathFinder,
    names: &BTreeSet<String>,
    index: usize,
) -> Option<String> {
    let scope = finder.scope_at(index);
    if let Some((name, value)) = error_type_value(code, index, names) {
        let kind = text_error_kind(finder, &value, &scope)?;
        return Some(format!("`type {name} = {kind}`"));
    }
    if let Some((name, value)) = error_binding_value(code, index, names) {
        let kind = text_error_kind(finder, &value, &scope)?;
        return Some(format!("`{name} = {kind}` binding"));
    }
    let error = result_error(code, result_finder, index)?;
    let kind = text_error_kind(finder, &error, &scope)?;
    Some(format!("`Result<…, {kind}>`"))
}

fn string_error_violations(tree: &SourceTree) -> Vec<Violation> {
    let (strings, results) = (
        Exports::build(tree, string_type),
        Exports::build(tree, result_type),
    );
    let names = error_type_names(tree, &results);
    let mut violations = Vec::new();
    for file in &tree.files {
        let finder = PathFinder::new(file, string_type, &strings);
        let result_finder = PathFinder::new(file, result_type, &results);
        let mut seen = BTreeSet::new();
        for index in 0..file.code.len().saturating_sub(1) {
            let Some(form) = string_error_at(&file.code, &finder, &result_finder, &names, index)
            else {
                continue;
            };
            if let Some(item) = once_per_item(file, index, &mut seen) {
                let detail = format!("{form} in `{item}`");
                let line = file.code[index].line;
                violations.push(Violation::new(
                    &file.path,
                    line,
                    &[&file.path, &item],
                    detail,
                ));
            }
        }
    }
    violations
}

fn is_json_edge(path: &str) -> bool {
    path.starts_with(BINDINGS) || JSON_EDGE_FILES.contains(&path)
}

fn serde_json_item(path: &[String]) -> Option<&'static str> {
    path.first()
        .is_some_and(|head| head == "serde_json")
        .then_some("serde_json")
}

fn only_through_other_files(_: &[String]) -> Option<&'static str> {
    None
}

fn serde_json_violations(tree: &SourceTree) -> Vec<Violation> {
    let exports = Exports::build(tree, serde_json_item);
    let mut violations = Vec::new();
    for file in tree.files.iter().filter(|file| !is_json_edge(&file.path)) {
        let reexported = PathFinder::new(file, only_through_other_files, &exports);
        let mut seen = BTreeSet::new();
        for (index, token) in file.code.iter().enumerate() {
            let named = token.is("serde_json") || reexported.target_at(index).is_some();
            if !named || file.is_test_at(index) {
                continue;
            }
            if let Some(item) = once_per_item(file, index, &mut seen) {
                let detail = format!("serde_json in `{item}` outside the JSON edges");
                violations.push(Violation::new(
                    &file.path,
                    token.line,
                    &[&file.path, &item],
                    detail,
                ));
            }
        }
    }
    violations
}

fn json_value(path: &[String]) -> Option<&'static str> {
    let crate_path = path.first().is_some_and(|head| head == "serde_json");
    (crate_path && path.iter().any(|segment| segment == "Value")).then_some("serde_json::Value")
}

fn declaration_end(code: &[Token], start: usize) -> usize {
    let function = code[start].is("fn");
    let mut index = start;
    while index < code.len() {
        let token = &code[index];
        if token.is(";") {
            return index;
        }
        if token.is("{") {
            return if function {
                index
            } else {
                closing_index(code, index)
            };
        }
        if token.is("(") && !function {
            return closing_index(code, index);
        }
        if token.is("(") || token.is("<") || token.is("[") {
            index = closing_index(code, index);
        }
        index += 1;
    }
    code.len()
}

fn json_value_violations(tree: &SourceTree) -> Vec<Violation> {
    let exports = Exports::build(tree, json_value);
    let mut violations = Vec::new();
    for file in tree
        .files
        .iter()
        .filter(|file| !file.path.starts_with(BINDINGS))
    {
        let finder = PathFinder::new(file, json_value, &exports);
        for (index, token) in file.code.iter().enumerate() {
            let declares = DECLARATIONS.iter().any(|keyword| token.is(keyword));
            let named = file.code.get(index + 1).is_some_and(Token::is_ident);
            if !declares || !named || file.is_test_at(index) {
                continue;
            }
            let end = declaration_end(&file.code, index);
            if (index..end).any(|at| finder.target_at(at).is_some()) {
                let item = item_label(file, index);
                let detail = format!("serde_json::Value in the fields or signature of `{item}`");
                violations.push(Violation::new(
                    &file.path,
                    token.line,
                    &[&file.path, &item],
                    detail,
                ));
            }
        }
    }
    violations
}

#[test]
fn no_result_uses_string_as_its_error() {
    enforce_on_kernel(
        "no String, &str or Box<str> error type in a Result, an Err or Error type or binding, or an alias a Result names",
        string_error_violations,
        Some(&STRING_ERRORS),
    );
}

#[test]
fn serde_json_appears_only_at_json_edges_and_in_tests() {
    enforce_on_kernel(
        "serde_json only at the JSON edges",
        serde_json_violations,
        Some(&SERDE_JSON_OUTSIDE_EDGES),
    );
}

#[test]
fn json_values_stay_out_of_fields_and_signatures_outside_bindings() {
    enforce_on_kernel(
        "no serde_json::Value in fields or signatures outside bindings",
        json_value_violations,
        Some(&JSON_VALUES_IN_SIGNATURES),
    );
}

fn items_in(check: fn(&SourceTree) -> Vec<Violation>, source: &str) -> Vec<String> {
    check(&SourceTree::from_sources(&[("geom2d/mod.rs", source)]))
        .into_iter()
        .map(|violation| violation.key[1].clone())
        .collect()
}

const ERROR_BINDINGS: &str = "fn parse<T: FromStr<Err = String>>(text: &str) -> Option<T> {\n    None\n}\n\
    fn convert<T>(value: u8) -> Option<T>\nwhere\n    T: TryFrom<u8, Error = String>,\n{\n    None\n}\n\
    impl<T: FromStr<Err = String>> Parsed for Wrapper<T> {}\n\
    fn count<I: Iterator<Item = String>>(items: I) -> usize {\n    items.count()\n}\n";

#[test]
fn string_error_rule_catches_error_bindings_in_bounds_and_impls() {
    assert_eq!(
        items_in(string_error_violations, ERROR_BINDINGS),
        ["parse", "convert", "impl@Wrapper:Parsed"]
    );
}

const STR_ERRORS: &str = "fn parse(text: &str) -> Result<u8, &'static str> {\n    Err(text)\n}\n\
    fn build() -> Result<(), Box<str>> {\n    Ok(())\n}\n\
    struct Reader;\nimpl FromStr for Reader {\n    type Err = &'static str;\n}\n";

#[test]
fn string_error_rule_catches_str_and_boxed_str_errors() {
    assert_eq!(
        items_in(string_error_violations, STR_ERRORS),
        ["parse", "build", "Reader::Err"]
    );
}

const ALIASED_ERROR: &str =
    "trait Parse {\n    type Failure;\n    fn parse(&self) -> Result<u8, Self::Failure>;\n}\n\
    impl Parse for Text {\n    type Failure = String;\n}\n\
    impl Iterator for Text {\n    type Item = String;\n}\n";

#[test]
fn string_error_rule_catches_a_string_alias_used_as_an_error_type() {
    assert_eq!(
        items_in(string_error_violations, ALIASED_ERROR),
        ["Text::Failure"]
    );
}

const CONST_IN_FN: &str = "fn encode(data: &[u8]) -> usize {\n    if data.as_ptr() as *const u8 == PTR {\n\
    serde_json::to_string(&0);\n    }\n    const EMPTY: Option<serde_json::Value> = None;\n    0\n}\n";

#[test]
fn serde_json_rule_keys_statements_with_const_to_their_fn() {
    assert_eq!(items_in(serde_json_violations, CONST_IN_FN), ["encode"]);
}

const POINTER_FIELD: &str =
    "struct Probe {\n    #[cfg(test)]\n    origin: *const u8,\n    value: serde_json::Value,\n}\n";

#[test]
fn serde_json_rule_sees_a_field_after_a_cfg_test_pointer_field() {
    assert_eq!(items_in(serde_json_violations, POINTER_FIELD), ["Probe"]);
}
