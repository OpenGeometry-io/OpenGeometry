use crate::lexer::Token;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UsePath {
    pub(crate) segments: Vec<String>,
    pub(crate) binding: Option<String>,
}

impl UsePath {
    pub(crate) fn text(&self) -> String {
        self.segments.join("::")
    }

    pub(crate) fn is_super_glob(&self) -> bool {
        let segments = match self.segments.split_first() {
            Some((head, rest)) if head == "self" => rest,
            _ => &self.segments,
        };
        segments.last().is_some_and(|last| last == "*")
            && segments.len() > 1
            && segments[..segments.len() - 1]
                .iter()
                .all(|segment| segment == "super")
    }
}

pub(crate) fn expand_use_tree(tokens: &[&Token]) -> Vec<UsePath> {
    let mut paths = Vec::new();
    let mut at = 0;
    expand_subtree(tokens, &mut at, &[], &mut paths);
    paths
}

fn expand_subtree(tokens: &[&Token], at: &mut usize, prefix: &[String], out: &mut Vec<UsePath>) {
    let mut segments = prefix.to_vec();
    while let Some(token) = tokens.get(*at) {
        *at += 1;
        if token.is("::") {
            continue;
        }
        if token.is("{") {
            expand_group(tokens, at, &segments, out);
            return;
        }
        if token.is("*") {
            segments.push("*".into());
            out.push(UsePath {
                segments,
                binding: None,
            });
            return;
        }
        segments.push(token.text.clone());
        if tokens.get(*at).is_some_and(|next| next.is("::")) {
            continue;
        }
        let alias = aliased_name(tokens, at);
        if segments.len() > 1 && segments.last().is_some_and(|last| last == "self") {
            segments.pop();
        }
        let binding = alias.or_else(|| segments.last().cloned());
        out.push(UsePath { segments, binding });
        return;
    }
}

fn expand_group(tokens: &[&Token], at: &mut usize, prefix: &[String], out: &mut Vec<UsePath>) {
    while let Some(token) = tokens.get(*at) {
        if token.is("}") {
            *at += 1;
            return;
        }
        if token.is(",") {
            *at += 1;
            continue;
        }
        expand_subtree(tokens, at, prefix, out);
    }
}

fn aliased_name(tokens: &[&Token], at: &mut usize) -> Option<String> {
    if !tokens.get(*at).is_some_and(|token| token.is("as")) {
        return None;
    }
    let alias = tokens.get(*at + 1).map(|token| token.text.clone());
    *at += 2;
    alias
}
