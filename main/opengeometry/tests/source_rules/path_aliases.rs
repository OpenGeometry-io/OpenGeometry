use crate::lexer::Token;
use crate::module_paths::{absolute_path, module_table, ModuleTable};
use crate::source_tree::{SourceFile, SourceTree};
use std::collections::BTreeMap;

const EXPANSION_LIMIT: usize = 32;
const EXPORT_HOPS: usize = 16;
const CRATE: &str = "crate";
pub(crate) const STD_CRATES: [&str; 2] = ["std", "core"];
const KEYWORDS: [&str; 32] = [
    "as", "async", "await", "break", "const", "continue", "dyn", "else", "enum", "extern", "fn",
    "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
    "return", "static", "struct", "trait", "type", "unsafe", "use", "where", "while",
];
const PARAMETER_LIST_HEADS: [&str; 2] = ["impl", "for"];
const BLOCK_BOUNDARIES: [&str; 2] = ["{", "}"];
const GROUP_OPENERS: [&str; 2] = ["(", "["];
const GROUP_CLOSERS: [&str; 2] = [")", "]"];

pub(crate) type Target = fn(&[String]) -> Option<&'static str>;
type NameTable = BTreeMap<Vec<String>, BTreeMap<String, &'static str>>;

fn is_arrow(code: &[Token], index: usize) -> bool {
    index > 0 && (code[index - 1].is("-") || code[index - 1].is("="))
}

fn is_one_of(token: &Token, words: &[&str]) -> bool {
    words.iter().any(|word| token.is(word))
}

fn closes_path_arguments(code: &[Token], close: usize) -> bool {
    let (mut angles, mut groups) = (0usize, 0usize);
    for index in (0..=close).rev() {
        let token = &code[index];
        let ends_statement = token.is(";") || is_one_of(token, &GROUP_OPENERS);
        if is_one_of(token, &BLOCK_BOUNDARIES) || (groups == 0 && ends_statement) {
            return false;
        }
        if is_one_of(token, &GROUP_CLOSERS) {
            groups += 1;
        } else if is_one_of(token, &GROUP_OPENERS) {
            groups -= 1;
        } else if groups == 0 && token.is(">") && !is_arrow(code, index) {
            angles += 1;
        } else if groups == 0 && token.is("<") {
            angles -= 1;
            if angles == 0 {
                let head = index.checked_sub(1).map(|before| &code[before]);
                return !head.is_some_and(|head| is_one_of(head, &PARAMETER_LIST_HEADS));
            }
        }
    }
    false
}

fn qualifies(code: &[Token], index: usize) -> bool {
    let token = &code[index];
    if token.is(">") {
        return !is_arrow(code, index) && closes_path_arguments(code, index);
    }
    token.is_ident() && !KEYWORDS.contains(&token.text.as_str())
}

pub(crate) fn opens_path(code: &[Token], index: usize) -> bool {
    code[index].is("::") && !(index > 0 && qualifies(code, index - 1))
}

pub(crate) fn path_at(code: &[Token], index: usize) -> Option<Vec<String>> {
    let leading = opens_path(code, index);
    let first = index + usize::from(leading);
    let starts = leading || !(index > 0 && code[index - 1].is("::"));
    if !starts || !code.get(first).is_some_and(Token::is_ident) {
        return None;
    }
    let mut segments = vec![code[first].text.clone()];
    let mut next = first + 1;
    while next + 1 < code.len() && code[next].is("::") && code[next + 1].is_ident() {
        segments.push(code[next + 1].text.clone());
        next += 2;
    }
    Some(segments)
}

fn type_alias_spans(code: &[Token]) -> Vec<(String, usize, usize)> {
    let mut spans = Vec::new();
    for index in 0..code.len().saturating_sub(1) {
        if !code[index].is("type") || !code[index + 1].is_ident() {
            continue;
        }
        let Some(end) = (index..code.len()).find(|&at| code[at].is(";")) else {
            continue;
        };
        if let Some(equals) = (index..end).find(|&at| code[at].is("=")) {
            spans.push((code[index + 1].text.clone(), equals + 1, end));
        }
    }
    spans
}

pub(crate) struct Exports {
    modules: ModuleTable,
    names: NameTable,
}

impl Exports {
    pub(crate) fn build(tree: &SourceTree, target: Target) -> Exports {
        let mut exports = Exports {
            modules: module_table(tree),
            names: NameTable::new(),
        };
        for _ in 0..EXPORT_HOPS {
            let mut names = NameTable::new();
            for file in &tree.files {
                let finder = PathFinder::new(file, target, &exports);
                for (scope, name, found) in finder.named_targets() {
                    names.entry(scope).or_default().insert(name, found);
                }
            }
            if names == exports.names {
                break;
            }
            exports.names = names;
        }
        exports
    }

    fn target(&self, path: &[String], scope: &[String]) -> Option<&'static str> {
        let absolute = absolute_path(path, scope, None, &self.modules)?;
        (0..absolute.len()).find_map(|split| {
            let names = self.names.get(&absolute[..split])?;
            names.get(&absolute[split]).copied()
        })
    }
}

pub(crate) struct PathFinder<'a> {
    file: &'a SourceFile,
    bound: BTreeMap<String, Vec<Vec<String>>>,
    type_aliases: BTreeMap<String, &'static str>,
    target: Target,
    exports: &'a Exports,
}

impl<'a> PathFinder<'a> {
    pub(crate) fn new(
        file: &'a SourceFile,
        target: Target,
        exports: &'a Exports,
    ) -> PathFinder<'a> {
        let mut bound: BTreeMap<String, Vec<Vec<String>>> = BTreeMap::new();
        for item in &file.scan.uses {
            let scope = [file.module.as_slice(), &item.scope].concat();
            for path in &item.paths {
                let Some(binding) = &path.binding else {
                    continue;
                };
                let resolved = absolute_path(&path.segments, &scope, None, &exports.modules)
                    .map_or_else(
                        || path.segments.clone(),
                        |absolute| [vec![CRATE.to_string()], absolute].concat(),
                    );
                bound.entry(binding.clone()).or_default().push(resolved);
            }
        }
        let mut finder = PathFinder {
            file,
            bound,
            type_aliases: BTreeMap::new(),
            target,
            exports,
        };
        finder.follow_type_aliases();
        finder
    }

    fn follow_type_aliases(&mut self) {
        let declared = type_alias_spans(&self.file.code);
        loop {
            let known = self.type_aliases.len();
            for (name, start, end) in &declared {
                if self.type_aliases.contains_key(name) {
                    continue;
                }
                if let Some(found) = (*start..*end).find_map(|index| self.target_at(index)) {
                    self.type_aliases.insert(name.clone(), found);
                }
            }
            if self.type_aliases.len() == known {
                return;
            }
        }
    }

    fn named_targets(&self) -> Vec<(Vec<String>, String, &'static str)> {
        let mut named = Vec::new();
        for item in &self.file.scan.uses {
            let scope = [self.file.module.as_slice(), &item.scope].concat();
            for path in &item.paths {
                let found = self.path_target(&path.segments, &scope);
                if let (Some(binding), Some(found)) = (&path.binding, found) {
                    named.push((scope.clone(), binding.clone(), found));
                }
            }
        }
        let code = &self.file.code;
        for index in 1..code.len() {
            let found = self.type_aliases.get(&code[index].text);
            if let Some(&found) = found.filter(|_| code[index - 1].is("type")) {
                named.push((self.scope_at(index), code[index].text.clone(), found));
            }
        }
        named
    }

    fn expansions(&self, segments: &[String]) -> Vec<Vec<String>> {
        let mut found = vec![segments.to_vec()];
        let mut next = 0;
        while next < found.len() && found.len() < EXPANSION_LIMIT {
            let path = found[next].clone();
            next += 1;
            for bound in self.bound.get(&path[0]).into_iter().flatten() {
                let expanded = [bound.as_slice(), &path[1..]].concat();
                if !found.contains(&expanded) {
                    found.push(expanded);
                }
            }
        }
        found
    }

    pub(crate) fn scope_at(&self, index: usize) -> Vec<String> {
        [self.file.module.as_slice(), self.file.scan.scope_at(index)].concat()
    }

    pub(crate) fn path_target(
        &self,
        segments: &[String],
        scope: &[String],
    ) -> Option<&'static str> {
        if let Some(found) = segments
            .first()
            .and_then(|head| self.type_aliases.get(head))
        {
            return Some(found);
        }
        self.expansions(segments).iter().find_map(|expanded| {
            (self.target)(expanded).or_else(|| self.exports.target(expanded, scope))
        })
    }

    pub(crate) fn target_at(&self, index: usize) -> Option<&'static str> {
        let path = path_at(&self.file.code, index)?;
        self.path_target(&path, &self.scope_at(index))
    }
}
