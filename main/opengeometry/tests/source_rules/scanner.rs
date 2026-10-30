use crate::lexer::Token;
use crate::use_tree::{expand_use_tree, UsePath};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub(crate) struct UseItem {
    pub(crate) line: usize,
    pub(crate) label: String,
    pub(crate) paths: Vec<UsePath>,
    pub(crate) scope: Vec<String>,
    pub(crate) test: bool,
    pub(crate) local: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct FnItem {
    pub(crate) name: String,
    pub(crate) bare: String,
    pub(crate) impl_trait: Option<Vec<String>>,
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) test: bool,
}

impl FnItem {
    pub(crate) fn span(&self) -> usize {
        self.end + 1 - self.start
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ModItem {
    pub(crate) name: String,
    pub(crate) line: usize,
    pub(crate) test: bool,
    pub(crate) inline: bool,
    pub(crate) scope: Vec<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct AttributeItem {
    pub(crate) name: String,
    pub(crate) words: Vec<String>,
    pub(crate) line: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct TestRoot {
    pub(crate) name: String,
    pub(crate) line: usize,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Context {
    pub(crate) test: bool,
    pub(crate) use_item: Option<usize>,
    pub(crate) item: usize,
    pub(crate) scope: usize,
}

#[derive(Debug, Default)]
pub(crate) struct FileScan {
    pub(crate) uses: Vec<UseItem>,
    pub(crate) fns: Vec<FnItem>,
    pub(crate) mods: Vec<ModItem>,
    pub(crate) attributes: Vec<AttributeItem>,
    pub(crate) contexts: Vec<Context>,
    pub(crate) item_names: Vec<String>,
    pub(crate) scopes: Vec<Vec<String>>,
    pub(crate) traits: Vec<String>,
    pub(crate) test_roots: Vec<TestRoot>,
    pub(crate) impl_labels: BTreeMap<usize, String>,
}

impl FileScan {
    pub(crate) fn item_name(&self, index: usize) -> &str {
        &self.item_names[self.contexts[index].item]
    }

    pub(crate) fn scope_at(&self, index: usize) -> &[String] {
        &self.scopes[self.contexts[index].scope]
    }
}

pub(crate) fn scan_file(tokens: &[Token]) -> FileScan {
    let mut scanner = Scanner {
        tokens,
        at: 0,
        frames: vec![Frame {
            kind: FrameKind::Block,
            test: false,
            test_element: None,
            item: 0,
            scope: 0,
            depth: 0,
            generics: 0,
        }],
        pending: Pending::default(),
        seen: BTreeMap::new(),
        scan: FileScan {
            item_names: vec![String::new()],
            scopes: vec![Vec::new()],
            ..FileScan::default()
        },
    };
    while scanner.at < tokens.len() {
        scanner.step();
    }
    scanner.scan
}

type TraitPath = Option<Vec<String>>;

enum FrameKind {
    Module,
    Function(usize),
    Owner(TraitPath),
    Block,
}

struct Frame {
    kind: FrameKind,
    test: bool,
    test_element: Option<usize>,
    item: usize,
    scope: usize,
    depth: usize,
    generics: usize,
}

#[derive(Default)]
struct Pending {
    test: bool,
    test_case: bool,
    function: Option<(String, usize)>,
    module: Option<String>,
    owner: Option<TraitPath>,
    item: Option<usize>,
}

impl Pending {
    fn names_nothing(&self) -> bool {
        self.function.is_none()
            && self.module.is_none()
            && self.owner.is_none()
            && self.item.is_none()
    }
}

struct Scanner<'a> {
    tokens: &'a [Token],
    at: usize,
    frames: Vec<Frame>,
    pending: Pending,
    seen: BTreeMap<String, usize>,
    scan: FileScan,
}

const NAMED_ITEMS: [&str; 6] = ["struct", "enum", "union", "const", "static", "type"];
const NOT_NAMES: [&str; 5] = ["fn", "mut", "dyn", "unsafe", "const"];
const GENERIC_ITEMS: [&str; 6] = ["fn", "struct", "enum", "union", "trait", "type"];
const GENERIC_HEADS: [&str; 2] = ["impl", "for"];

impl Scanner<'_> {
    fn token(&self, offset: usize) -> Option<&Token> {
        self.tokens.get(self.at + offset)
    }

    fn top(&self) -> &Frame {
        &self.frames[self.frames.len() - 1]
    }

    fn in_function(&self) -> bool {
        self.frames
            .iter()
            .any(|frame| matches!(frame.kind, FrameKind::Function(_)))
    }

    fn opens_generic_parameters(&self) -> bool {
        let before = |offset: usize| self.at.checked_sub(offset).map(|at| &self.tokens[at]);
        let after_head =
            before(1).is_some_and(|head| GENERIC_HEADS.iter().any(|keyword| head.is(keyword)));
        let after_name = before(1).is_some_and(Token::is_ident)
            && before(2).is_some_and(|item| GENERIC_ITEMS.iter().any(|keyword| item.is(keyword)));
        after_head || after_name
    }

    fn in_test(&self) -> bool {
        self.pending.test || self.top().test || self.top().test_element.is_some()
    }

    fn scope(&self) -> Vec<String> {
        self.scan.scopes[self.top().scope].clone()
    }

    fn numbered(&mut self, path: String) -> String {
        let count = self.seen.entry(path.clone()).or_insert(0);
        *count += 1;
        if *count > 1 {
            format!("{path}#{count}")
        } else {
            path
        }
    }

    fn new_item(&mut self, name: &str, numbered: bool) -> usize {
        let parent = &self.scan.item_names[self.top().item];
        let path = if parent.is_empty() {
            name.to_string()
        } else {
            format!("{parent}::{name}")
        };
        let path = if numbered { self.numbered(path) } else { path };
        self.scan.item_names.push(path);
        self.scan.item_names.len() - 1
    }

    fn record_context(&mut self, use_item: Option<usize>) {
        let context = Context {
            test: self.in_test(),
            use_item,
            item: self.pending.item.unwrap_or(self.top().item),
            scope: self.top().scope,
        };
        self.scan.contexts.push(context);
    }

    fn step(&mut self) {
        let tokens = self.tokens;
        let token = &tokens[self.at];
        if token.is("#")
            && self
                .token(1)
                .is_some_and(|next| next.is("[") || next.is("!"))
        {
            self.attribute();
            return;
        }
        if token.is("use") {
            self.use_item();
            return;
        }
        self.note_keyword();
        self.record_context(None);
        self.structure();
        self.at += 1;
    }

    fn matching_close(&self, open: usize, left: &str, right: &str) -> usize {
        let mut depth = 0usize;
        for (index, token) in self.tokens.iter().enumerate().skip(open) {
            if token.is(left) {
                depth += 1;
            } else if token.is(right) {
                depth -= 1;
                if depth == 0 {
                    return index;
                }
            }
        }
        self.tokens.len() - 1
    }

    fn attribute(&mut self) {
        let inner = self.token(1).is_some_and(|next| next.is("!"));
        let open = self.at + 1 + usize::from(inner);
        let close = self.matching_close(open, "[", "]");
        let words: Vec<String> = self.tokens[open + 1..close]
            .iter()
            .map(|token| token.text.clone())
            .collect();
        let test_case =
            words == ["test"] || words.first().is_some_and(|w| w == "wasm_bindgen_test");
        let marks_test = test_case || words == ["cfg", "(", "test", ")"];
        if let Some(name) = words.first() {
            self.scan.attributes.push(AttributeItem {
                name: name.clone(),
                words: words.clone(),
                line: self.tokens[self.at].line,
            });
        }
        let depth = self.top().depth;
        if marks_test && !inner && depth > 0 {
            if let Some(frame) = self.frames.last_mut() {
                frame.test_element = Some(depth);
            }
        } else if marks_test && !inner {
            self.pending.test = true;
            self.pending.test_case |= test_case;
        }
        while self.at <= close {
            self.record_context(None);
            self.at += 1;
        }
    }

    fn use_item(&mut self) {
        let start = self.at;
        let mut end = start;
        let mut depth = 0usize;
        while end < self.tokens.len() {
            let token = &self.tokens[end];
            if token.is("{") {
                depth += 1;
            } else if token.is("}") {
                depth = depth.saturating_sub(1);
            } else if token.is(";") && depth == 0 {
                break;
            }
            end += 1;
        }
        let body: Vec<&Token> = self.tokens[start + 1..end.min(self.tokens.len())]
            .iter()
            .collect();
        let index = self.scan.uses.len();
        let paths = expand_use_tree(&body);
        let text: Vec<String> = paths.iter().map(UsePath::text).collect();
        let item = UseItem {
            line: self.tokens[start].line,
            label: self.numbered(format!("use@{}", text.join(","))),
            paths,
            scope: self.scope(),
            test: self.in_test(),
            local: self.frames[1..]
                .iter()
                .any(|frame| !matches!(frame.kind, FrameKind::Module)),
        };
        self.scan.uses.push(item);
        while self.at <= end && self.at < self.tokens.len() {
            self.record_context(Some(index));
            self.at += 1;
        }
        self.pending = Pending::default();
    }

    fn next_name(&self) -> Option<String> {
        self.token(1)
            .filter(|next| next.is_ident() && !NOT_NAMES.contains(&next.text.as_str()))
            .map(|next| next.text.clone())
    }

    fn note_keyword(&mut self) {
        let tokens = self.tokens;
        let token = &tokens[self.at];
        let word = token.text.as_str();
        if !token.is_ident() {
            return;
        }
        if word == "fn" {
            if let Some(bare) = self.next_name() {
                self.pending.item = Some(self.new_item(&bare, true));
                self.pending.function = Some((bare, token.line));
            }
        } else if word == "mod" {
            self.note_module();
        } else if word == "impl" && self.pending.function.is_none() {
            self.note_impl();
        } else if word == "trait" {
            if let Some(name) = self.next_name() {
                self.scan.traits.push(name.clone());
                self.pending.item = Some(self.new_item(&name, true));
                self.pending.owner = Some(None);
            }
        } else if NAMED_ITEMS.contains(&word) && self.pending.item.is_none() && !self.keys_to_fn() {
            let name = self.next_name().or_else(|| {
                self.token(1)
                    .filter(|next| next.is("mut"))
                    .and_then(|_| self.token(2))
                    .map(|name| name.text.clone())
            });
            if let Some(name) = name {
                self.pending.item = Some(self.new_item(&name, true));
            }
        }
    }

    fn keys_to_fn(&self) -> bool {
        let pointer = self.at > 0 && self.tokens[self.at - 1].is("*");
        self.tokens[self.at].is("const") && (pointer || self.in_function())
    }

    fn note_impl(&mut self) {
        let (owner, trait_path) = self.impl_owner();
        let item = self.new_item(&owner, false);
        let owner = &self.scan.item_names[item];
        let label = match &trait_path {
            Some(path) => format!("impl@{owner}:{}", path.join("::")),
            None => format!("impl@{owner}"),
        };
        let label = self.numbered(label);
        self.scan.impl_labels.insert(item, label);
        self.pending.item = Some(item);
        self.pending.owner = Some(trait_path);
    }

    fn note_module(&mut self) {
        let Some(name) = self.next_name() else {
            return;
        };
        if self.token(2).is_some_and(|after| after.is(";")) {
            let item = ModItem {
                name,
                line: self.tokens[self.at].line,
                test: self.in_test(),
                inline: false,
                scope: self.scope(),
            };
            self.scan.mods.push(item);
        } else {
            self.pending.item = Some(self.new_item(&name, true));
            self.pending.module = Some(name);
        }
    }

    fn impl_owner(&self) -> (String, TraitPath) {
        let mut depth = 0usize;
        let mut owner = String::new();
        let mut segments = Vec::new();
        let mut trait_path = None;
        for (index, token) in self.tokens.iter().enumerate().skip(self.at + 1) {
            let after_dash = self.tokens[index - 1].is("-");
            if depth == 0 && (token.is("{") || token.is("where") || token.is(";")) {
                break;
            }
            if token.is("<") || token.is("(") || token.is("[") {
                depth += 1;
            } else if (token.is(">") && !after_dash) || token.is(")") || token.is("]") {
                depth = depth.saturating_sub(1);
            } else if depth == 0 && token.is("for") {
                trait_path = Some(std::mem::take(&mut segments));
                owner.clear();
            } else if depth == 0 && token.is_ident() && !NOT_NAMES.contains(&token.text.as_str()) {
                owner = token.text.clone();
                segments.push(token.text.clone());
            }
        }
        (owner, trait_path)
    }

    fn structure(&mut self) {
        let tokens = self.tokens;
        let token = &tokens[self.at];
        let depth = self.top().depth;
        if token.is("{") {
            self.open_frame();
        } else if token.is("}") {
            self.close_frame();
        } else if token.is("(") || token.is("[") || self.opens_angle() {
            if let Some(frame) = self.frames.last_mut() {
                frame.depth += 1;
                frame.generics += usize::from(token.is("<"));
            }
        } else if token.is(")") || token.is("]") || self.closes_angle() {
            if let Some(frame) = self.frames.last_mut() {
                frame.depth = frame.depth.saturating_sub(1);
                frame.generics -= usize::from(token.is(">"));
                frame.test_element = frame.test_element.filter(|element| *element <= frame.depth);
            }
        } else if token.is(";") && depth == 0 {
            let pending = std::mem::take(&mut self.pending);
            if let Some((bare, start)) = pending.function {
                let test = pending.test || self.top().test;
                let index = self.declare_fn(bare, start, test, pending.item);
                self.scan.fns[index].end = token.line;
            }
        } else if token.is(",") {
            self.end_element(depth);
        }
    }

    fn opens_angle(&self) -> bool {
        self.tokens[self.at].is("<") && (self.top().generics > 0 || self.opens_generic_parameters())
    }

    fn closes_angle(&self) -> bool {
        let arrow =
            self.at > 0 && (self.tokens[self.at - 1].is("-") || self.tokens[self.at - 1].is("="));
        self.tokens[self.at].is(">") && self.top().generics > 0 && !arrow
    }

    fn end_element(&mut self, depth: usize) {
        if depth == 0 && self.pending.names_nothing() {
            self.pending = Pending::default();
        }
        if let Some(frame) = self.frames.last_mut() {
            frame.test_element = frame.test_element.filter(|element| *element != depth);
        }
    }

    fn declare_fn(&mut self, bare: String, start: usize, test: bool, item: Option<usize>) -> usize {
        let impl_trait = match &self.top().kind {
            FrameKind::Owner(trait_path) => trait_path.clone(),
            _ => None,
        };
        let name = item.map_or(bare.clone(), |item| self.scan.item_names[item].clone());
        self.scan.fns.push(FnItem {
            name,
            bare,
            impl_trait,
            start,
            end: start,
            test,
        });
        self.scan.fns.len() - 1
    }

    fn note_test_root(&mut self, item: usize, marked: bool) {
        if marked && !self.top().test {
            self.scan.test_roots.push(TestRoot {
                name: self.scan.item_names[item].clone(),
                line: self.tokens[self.at].line,
            });
        }
    }

    fn open_frame(&mut self) {
        let pending = std::mem::take(&mut self.pending);
        let test = pending.test || self.top().test || self.top().test_element.is_some();
        let item = pending.item.unwrap_or(self.top().item);
        let mut scope = self.top().scope;
        let kind = if let Some((bare, start)) = pending.function {
            self.note_test_root(item, pending.test_case);
            FrameKind::Function(self.declare_fn(bare, start, test, pending.item))
        } else if let Some(name) = pending.module {
            self.note_test_root(item, pending.test);
            self.scan.mods.push(ModItem {
                name: name.clone(),
                line: self.tokens[self.at].line,
                test,
                inline: true,
                scope: self.scope(),
            });
            let nested = [self.scope(), vec![name]].concat();
            self.scan.scopes.push(nested);
            scope = self.scan.scopes.len() - 1;
            FrameKind::Module
        } else if let Some(trait_path) = pending.owner {
            FrameKind::Owner(trait_path)
        } else {
            FrameKind::Block
        };
        self.frames.push(Frame {
            kind,
            test,
            test_element: None,
            item,
            scope,
            depth: 0,
            generics: 0,
        });
    }

    fn close_frame(&mut self) {
        if self.frames.len() > 1 {
            if let Some(frame) = self.frames.pop() {
                if let FrameKind::Function(index) = frame.kind {
                    self.scan.fns[index].end = self.tokens[self.at].line;
                }
            }
        }
        self.pending = Pending::default();
    }
}
