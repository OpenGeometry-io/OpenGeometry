use crate::lexer::{tokenize, Token, TokenKind};

fn tokens_of(source: &str) -> Vec<Token> {
    tokenize(source).unwrap_or_else(|error| panic!("line {}: {}", error.line, error.reason))
}

fn kinds_of(source: &str) -> Vec<(TokenKind, String)> {
    tokens_of(source)
        .into_iter()
        .map(|token| (token.kind, token.text))
        .collect()
}

#[test]
fn lexer_keeps_comment_markers_inside_strings_raw_strings_and_byte_strings() {
    let source = r####"let a = "// not /* a comment"; let b = r#"say "// hi" "#; let c = b"/*"; let d = br##"x"#"##;"####;
    let tokens = tokens_of(source);
    assert!(tokens.iter().all(|token| !token.is_comment()));
    let literals: Vec<&str> = tokens
        .iter()
        .filter(|token| token.kind == TokenKind::Literal)
        .map(|token| token.text.as_str())
        .collect();
    assert_eq!(
        literals,
        [
            "\"// not /* a comment\"",
            "r#\"say \"// hi\" \"#",
            "b\"/*\"",
            "br##\"x\"#\"##"
        ]
    );
}

#[test]
fn lexer_tells_char_literals_from_lifetimes() {
    let kinds = kinds_of(
        "fn f<'a>(x: &'a str) -> char { let q = '\\''; let s = 'x'; let b = b'{'; '\\u{7f}' }",
    );
    let literals: Vec<&str> = kinds
        .iter()
        .filter(|(kind, _)| *kind == TokenKind::Literal)
        .map(|(_, text)| text.as_str())
        .collect();
    let lifetimes = kinds
        .iter()
        .filter(|(kind, _)| *kind == TokenKind::Lifetime)
        .count();
    assert_eq!(literals, ["'\\''", "'x'", "b'{'", "'\\u{7f}'"]);
    assert_eq!(lifetimes, 2);
    assert_eq!(kinds.iter().filter(|(_, text)| text == "{").count(), 1);
}

#[test]
fn lexer_nests_block_comments_and_counts_their_lines() {
    let tokens = tokens_of("a /* outer /* inner */ still outer\n */ b // tail\nc");
    let comments: Vec<(TokenKind, usize)> = tokens
        .iter()
        .filter(|token| token.is_comment())
        .map(|token| (token.kind, token.line))
        .collect();
    assert_eq!(
        comments,
        [(TokenKind::BlockComment, 1), (TokenKind::LineComment, 2)]
    );
    let idents: Vec<(&str, usize)> = tokens
        .iter()
        .filter(|token| token.is_ident())
        .map(|token| (token.text.as_str(), token.line))
        .collect();
    assert_eq!(idents, [("a", 1), ("b", 2), ("c", 3)]);
}

#[test]
fn lexer_rejects_unterminated_literals_and_comments() {
    for source in ["\"open", "r#\"open\"", "/* open /* */", "b'x"] {
        assert!(tokenize(source).is_err(), "{source}");
    }
}

#[test]
fn lexer_reads_a_raw_identifier_as_its_plain_name() {
    let kinds = kinds_of("let r#type = r#OnceLock::new(); fn r#unwrap() {}");
    let idents: Vec<&str> = kinds
        .iter()
        .filter(|(kind, _)| *kind == TokenKind::Ident)
        .map(|(_, text)| text.as_str())
        .collect();
    assert_eq!(idents, ["let", "type", "OnceLock", "new", "fn", "unwrap"]);
}
