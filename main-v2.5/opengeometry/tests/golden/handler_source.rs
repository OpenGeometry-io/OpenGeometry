use crate::record::Failure;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const BOOLEAN_SOURCE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/operations/modifying/boolean"
);

const BOOLEAN_HANDLER_FLOOR: usize = 29;

const HANDLER_LITERAL: &str = "Handler {";

const HANDLER_ID_IMPL: &str = "impl HandlerId {";

const NAME_METHOD: &str = "fn as_str(";

const NAME_MATCH: &str = "match self {";

const HANDLER_ID_PATH: &str = "HandlerId::";

const TABLE_ENTRY_NAME: &str = ".name";

const DECLARATION_KEYWORDS: [&str; 6] = ["struct", "enum", "union", "impl", "trait", "for"];

const PATTERN_KEYWORDS: [&str; 2] = ["in", "if"];

const RECORDING_CALLS: [(&str, usize); 3] = [
    ("record_handler(", 1),
    ("record_optional(", 1),
    ("handlers.push(", 0),
];

type HandlerNames = BTreeMap<String, String>;

struct Source {
    path: PathBuf,
    text: String,
}

impl Source {
    fn failure(&self, rule: &str, offset: usize, found: &str) -> Failure {
        let offset = offset + found.len() - found.trim_start().len();
        let line = self.text[..offset].matches('\n').count() + 1;
        format!(
            "handler scan: {rule}; {}:{line} has `{}`",
            self.path.display(),
            found.trim()
        )
        .into()
    }
}

fn rust_sources(directory: &Path, sources: &mut Vec<PathBuf>) -> Result<(), Failure> {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            rust_sources(&path, sources)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            sources.push(path);
        }
    }
    Ok(())
}

fn string_literal(value: &str) -> Option<&str> {
    let value = value.trim();
    let value = value
        .strip_prefix('{')
        .and_then(|block| block.strip_suffix('}'))
        .map_or(value, str::trim);
    let quoted = value.strip_prefix('"')?.strip_suffix('"')?;
    (!quoted.contains('"')).then_some(quoted)
}

fn is_identifier(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

fn preceding_word(before: &str) -> &str {
    before
        .trim_end()
        .rsplit(|character: char| !is_identifier(character))
        .next()
        .unwrap_or_default()
}

fn declares_type(before: &str) -> bool {
    let before = before.trim_end_matches(is_identifier).trim_end();
    DECLARATION_KEYWORDS.contains(&preceding_word(before)) || before.ends_with("->")
}

fn binds_pattern(after: &str) -> bool {
    let rest = after.trim_start_matches(|character: char| {
        character.is_whitespace() || character == ')' || character == ']'
    });
    let word = rest
        .split(|character: char| !is_identifier(character))
        .next();
    word.is_some_and(|word| PATTERN_KEYWORDS.contains(&word))
        || rest.starts_with(['|', ':'])
        || (rest.starts_with('=') && !rest.starts_with("=="))
}

fn top_level_fields(body: &str) -> (Vec<(usize, &str)>, usize) {
    let mut fields = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    for (index, byte) in body.bytes().enumerate() {
        match byte {
            b'{' | b'(' | b'[' => depth += 1,
            b'}' | b')' | b']' if depth == 0 => {
                fields.push((start, &body[start..index]));
                return (fields, index);
            }
            b'}' | b')' | b']' => depth -= 1,
            b',' if depth == 0 => {
                fields.push((start, &body[start..index]));
                start = index + 1;
            }
            _ => {}
        }
    }
    (fields, body.len())
}

fn name_value(field: &str) -> Option<&str> {
    let rest = field.trim_start().strip_prefix("name")?.trim_start();
    if rest.is_empty() {
        return Some(rest);
    }
    rest.strip_prefix(':')
        .filter(|value| !value.starts_with(':'))
}

fn arm_variants(pattern: &str) -> impl Iterator<Item = &str> {
    pattern
        .split('|')
        .filter_map(|alternative| alternative.trim().rsplit("::").next())
}

fn arm_value_len(value: &str) -> usize {
    let block = value.trim_start();
    if let Some(inner) = block.strip_prefix('{') {
        return value.len() - block.len() + top_level_fields(inner).1 + 2;
    }
    let (fields, end) = top_level_fields(value);
    fields.first().map_or(end, |(_, field)| field.len())
}

fn match_arms(body: &str) -> Vec<(usize, &str, &str)> {
    let mut arms = Vec::new();
    let mut start = 0;
    while let Some(arrow) = body[start..].find("=>") {
        let pattern = &body[start..start + arrow];
        let value = start + arrow + 2;
        let end = (value + arm_value_len(&body[value..])).min(body.len());
        arms.push((value, pattern, &body[value..end]));
        start = end;
    }
    arms
}

fn name_arms(source: &Source) -> Result<HandlerNames, Failure> {
    let text = &source.text;
    let Some(arms) = text
        .find(HANDLER_ID_IMPL)
        .and_then(|start| Some(start + text[start..].find(NAME_METHOD)?))
        .and_then(|start| Some(start + text[start..].find(NAME_MATCH)? + NAME_MATCH.len()))
    else {
        return Ok(HandlerNames::new());
    };
    let body = &text[arms..arms + top_level_fields(&text[arms..]).1];
    let mut names = HandlerNames::new();
    for (offset, pattern, value) in match_arms(body) {
        let Some(name) = string_literal(value) else {
            let rule = "every HandlerId::as_str arm must return a string literal";
            return Err(source.failure(rule, arms + offset, value));
        };
        for variant in arm_variants(pattern) {
            names.insert(variant.to_string(), name.to_string());
        }
    }
    Ok(names)
}

fn names_handler(value: &str, names: &HandlerNames) -> bool {
    value
        .trim_start()
        .strip_prefix(HANDLER_ID_PATH)
        .is_some_and(|path| {
            let end = path.find(|character| !is_identifier(character));
            names.contains_key(&path[..end.unwrap_or(path.len())])
        })
}

fn table_entry_name(value: &str) -> bool {
    value
        .trim()
        .strip_suffix(TABLE_ENTRY_NAME)
        .is_some_and(|entry| !entry.is_empty() && entry.chars().all(is_identifier))
}

fn table_references(source: &Source, names: &HandlerNames) -> Result<usize, Failure> {
    let mut references = 0;
    for (start, _) in source.text.match_indices(HANDLER_LITERAL) {
        if declares_type(&source.text[..start]) {
            continue;
        }
        let body = start + HANDLER_LITERAL.len();
        let (fields, end) = top_level_fields(&source.text[body..]);
        if binds_pattern(&source.text[(body + end + 1).min(source.text.len())..]) {
            continue;
        }
        for (offset, field) in fields {
            let Some(value) = name_value(field) else {
                continue;
            };
            if !names_handler(value, names) {
                let rule = "the name field of every *Handler literal must be a HandlerId variant";
                return Err(source.failure(rule, body + offset, field));
            }
            references += 1;
        }
    }
    Ok(references)
}

fn recording_references(source: &Source, names: &HandlerNames) -> Result<usize, Failure> {
    let mut references = 0;
    for (call, argument) in RECORDING_CALLS {
        for (start, _) in source.text.match_indices(call) {
            if preceding_word(&source.text[..start]) == "fn" {
                continue;
            }
            let arguments = start + call.len();
            let (fields, _) = top_level_fields(&source.text[arguments..]);
            let (offset, value) = fields.get(argument).copied().unwrap_or((0, ""));
            if !names_handler(value, names) && !table_entry_name(value) {
                let rule = "every record_handler, record_optional and handlers.push handler argument must be a HandlerId variant or a handler table entry's name";
                return Err(source.failure(rule, arguments + offset, value));
            }
            references += 1;
        }
    }
    Ok(references)
}

fn boolean_sources() -> Result<Vec<Source>, Failure> {
    let mut paths = Vec::new();
    rust_sources(Path::new(BOOLEAN_SOURCE), &mut paths)?;
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            Ok(Source {
                text: fs::read_to_string(&path)?,
                path,
            })
        })
        .collect()
}

pub(crate) fn boolean_handler_names() -> Result<BTreeSet<String>, Failure> {
    let sources = boolean_sources()?;
    let mut names = HandlerNames::new();
    for source in &sources {
        names.extend(name_arms(source)?);
    }
    let mut references = 0;
    for source in &sources {
        references += table_references(source, &names)?;
        references += recording_references(source, &names)?;
    }
    if names.is_empty() || references == 0 {
        return Err(format!("no boolean handler names found under {BOOLEAN_SOURCE}").into());
    }
    let names = names.into_values().collect::<BTreeSet<_>>();
    if names.len() < BOOLEAN_HANDLER_FLOOR {
        return Err(format!(
            "handler scan: at least {BOOLEAN_HANDLER_FLOOR} boolean handler names must be scanned; found {} under {BOOLEAN_SOURCE}",
            names.len()
        )
        .into());
    }
    Ok(names)
}
