#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TokenKind {
    Ident,
    Lifetime,
    Literal,
    Punct,
    LineComment,
    BlockComment,
}

#[derive(Clone, Debug)]
pub(crate) struct Token {
    pub(crate) kind: TokenKind,
    pub(crate) text: String,
    pub(crate) line: usize,
}

impl Token {
    pub(crate) fn is_comment(&self) -> bool {
        matches!(self.kind, TokenKind::LineComment | TokenKind::BlockComment)
    }

    pub(crate) fn is(&self, text: &str) -> bool {
        matches!(self.kind, TokenKind::Ident | TokenKind::Punct) && self.text == text
    }

    pub(crate) fn is_ident(&self) -> bool {
        self.kind == TokenKind::Ident
    }
}

#[derive(Debug)]
pub(crate) struct LexError {
    pub(crate) line: usize,
    pub(crate) reason: &'static str,
}

pub(crate) fn tokenize(source: &str) -> Result<Vec<Token>, LexError> {
    let mut lexer = Lexer {
        chars: source.chars().collect(),
        at: 0,
        line: 1,
        tokens: Vec::new(),
    };
    while let Some(current) = lexer.peek(0) {
        lexer.next_token(current)?;
    }
    Ok(lexer.tokens)
}

fn is_ident_start(c: char) -> bool {
    c == '_' || c.is_alphabetic()
}

fn is_ident_continue(c: char) -> bool {
    c == '_' || c.is_alphanumeric()
}

struct Lexer {
    chars: Vec<char>,
    at: usize,
    line: usize,
    tokens: Vec<Token>,
}

impl Lexer {
    fn peek(&self, offset: usize) -> Option<char> {
        self.chars.get(self.at + offset).copied()
    }

    fn next_token(&mut self, current: char) -> Result<(), LexError> {
        let next = self.peek(1);
        if current == '\n' {
            self.line += 1;
            self.at += 1;
        } else if current.is_whitespace() {
            self.at += 1;
        } else if current == '/' && next == Some('/') {
            self.line_comment();
        } else if current == '/' && next == Some('*') {
            self.block_comment()?;
        } else if let Some(prefix) = self.raw_string_prefix() {
            self.raw_string(prefix)?;
        } else if current == '"' || (matches!(current, 'b' | 'c') && next == Some('"')) {
            self.quoted_string()?;
        } else if current == '\'' || (current == 'b' && next == Some('\'')) {
            self.char_or_lifetime()?;
        } else if current == 'r' && next == Some('#') && self.peek(2).is_some_and(is_ident_start) {
            self.raw_identifier();
        } else if is_ident_start(current) {
            let start = self.at;
            self.consume_while(is_ident_continue);
            self.push(TokenKind::Ident, start, self.line);
        } else if current.is_ascii_digit() {
            self.number();
        } else {
            self.punct(current);
        }
        Ok(())
    }

    fn push(&mut self, kind: TokenKind, start: usize, line: usize) {
        let text = self.chars[start..self.at].iter().collect();
        self.tokens.push(Token { kind, text, line });
    }

    fn consume_while(&mut self, accept: fn(char) -> bool) {
        while self.peek(0).is_some_and(accept) {
            self.at += 1;
        }
    }

    fn advance_counting_lines(&mut self) {
        if self.peek(0) == Some('\n') {
            self.line += 1;
        }
        self.at += 1;
    }

    fn line_comment(&mut self) {
        let start = self.at;
        while self.peek(0).is_some_and(|c| c != '\n') {
            self.at += 1;
        }
        self.push(TokenKind::LineComment, start, self.line);
    }

    fn block_comment(&mut self) -> Result<(), LexError> {
        let (start, line) = (self.at, self.line);
        let mut depth = 0usize;
        loop {
            match (self.peek(0), self.peek(1)) {
                (Some('/'), Some('*')) => {
                    depth += 1;
                    self.at += 2;
                }
                (Some('*'), Some('/')) => {
                    depth -= 1;
                    self.at += 2;
                    if depth == 0 {
                        break;
                    }
                }
                (Some(_), _) => self.advance_counting_lines(),
                (None, _) => {
                    return Err(LexError {
                        line,
                        reason: "unterminated block comment",
                    })
                }
            }
        }
        self.push(TokenKind::BlockComment, start, line);
        Ok(())
    }

    fn raw_string_prefix(&self) -> Option<usize> {
        let mut offset = 0;
        if matches!(self.peek(0), Some('b' | 'c')) {
            offset = 1;
        }
        if self.peek(offset) != Some('r') {
            return None;
        }
        let mut probe = offset + 1;
        while self.peek(probe) == Some('#') {
            probe += 1;
        }
        (self.peek(probe) == Some('"')).then_some(offset + 1)
    }

    fn raw_string(&mut self, prefix: usize) -> Result<(), LexError> {
        let (start, line) = (self.at, self.line);
        self.at += prefix;
        let mut hashes = 0;
        while self.peek(0) == Some('#') {
            hashes += 1;
            self.at += 1;
        }
        self.at += 1;
        loop {
            match self.peek(0) {
                Some('"') if (1..=hashes).all(|k| self.peek(k) == Some('#')) => {
                    self.at += 1 + hashes;
                    break;
                }
                Some(_) => self.advance_counting_lines(),
                None => {
                    return Err(LexError {
                        line,
                        reason: "unterminated raw string",
                    })
                }
            }
        }
        self.push(TokenKind::Literal, start, line);
        Ok(())
    }

    fn quoted_string(&mut self) -> Result<(), LexError> {
        let (start, line) = (self.at, self.line);
        if self.peek(0) != Some('"') {
            self.at += 1;
        }
        self.at += 1;
        loop {
            match self.peek(0) {
                Some('"') => {
                    self.at += 1;
                    break;
                }
                Some('\\') => {
                    self.at += 1;
                    self.advance_counting_lines();
                }
                Some(_) => self.advance_counting_lines(),
                None => {
                    return Err(LexError {
                        line,
                        reason: "unterminated string",
                    })
                }
            }
        }
        self.push(TokenKind::Literal, start, line);
        Ok(())
    }

    fn char_or_lifetime(&mut self) -> Result<(), LexError> {
        let start = self.at;
        let byte = self.peek(0) == Some('b');
        let quote = usize::from(byte);
        let escaped = self.peek(quote + 1) == Some('\\');
        let single = self.peek(quote + 2) == Some('\'');
        if !escaped && !single {
            if byte {
                return Err(LexError {
                    line: self.line,
                    reason: "malformed byte literal",
                });
            }
            self.at += 1;
            self.consume_while(is_ident_continue);
            self.push(TokenKind::Lifetime, start, self.line);
            return Ok(());
        }
        self.at += quote + 1;
        if escaped {
            self.at += 2;
        }
        while self.peek(0).is_some_and(|c| c != '\'' && c != '\n') {
            self.at += 1;
        }
        if self.peek(0) != Some('\'') {
            return Err(LexError {
                line: self.line,
                reason: "unterminated character literal",
            });
        }
        self.at += 1;
        self.push(TokenKind::Literal, start, self.line);
        Ok(())
    }

    fn raw_identifier(&mut self) {
        self.at += 2;
        let start = self.at;
        self.consume_while(is_ident_continue);
        self.push(TokenKind::Ident, start, self.line);
    }

    fn number(&mut self) {
        let start = self.at;
        self.consume_while(is_ident_continue);
        if self.peek(0) == Some('.') && self.peek(1).is_some_and(|c| c.is_ascii_digit()) {
            self.at += 1;
            self.consume_while(is_ident_continue);
        }
        let hex = self.chars[start..].starts_with(&['0', 'x']);
        let exponent = matches!(self.chars[self.at - 1], 'e' | 'E');
        if exponent && !hex && matches!(self.peek(0), Some('+' | '-')) {
            self.at += 1;
            self.consume_while(is_ident_continue);
        }
        self.push(TokenKind::Literal, start, self.line);
    }

    fn punct(&mut self, current: char) {
        let start = self.at;
        self.at += if current == ':' && self.peek(1) == Some(':') {
            2
        } else {
            1
        };
        self.push(TokenKind::Punct, start, self.line);
    }
}
