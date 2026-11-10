use std::fmt;
use std::str::CharIndices;

use crate::span::Span;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TokenKind {
    // Literals and names
    Int,
    Str {
        terminated: bool,
    },
    Ident,

    // Keywords
    Let,
    Def,
    Fun,
    If,
    Else,
    True,
    False,

    // Arithmetic operators
    Plus,
    Minus,
    Star,
    Slash,
    Percent,

    // Comparison operators
    EqEq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,

    // Logical operators
    Bang,
    AndAnd,
    OrOr,

    // Punctuation
    Eq,
    FatArrow,
    Comma,
    Semicolon,

    // Brackets
    LParen,
    RParen,
    LBrace,
    RBrace,

    // Special
    /// A character that cannot start any token. The lexer never fails;
    /// the parser reports these when it reaches them.
    Unknown,
    /// End of input. `Lexer::next_token` keeps returning it once the input
    /// runs out.
    Eof,
}

/// How the token is named in error messages, e.g. "expected `;`, found
/// end of input". Use `{:?}` for the variant name when debugging.
impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            // Literals and names
            TokenKind::Int => "an integer",
            TokenKind::Str { .. } => "a string",
            TokenKind::Ident => "an identifier",
            // Keywords
            TokenKind::Let => "`let`",
            TokenKind::Def => "`def`",
            TokenKind::Fun => "`fun`",
            TokenKind::If => "`if`",
            TokenKind::Else => "`else`",
            TokenKind::True => "`true`",
            TokenKind::False => "`false`",
            // Arithmetic operators
            TokenKind::Plus => "`+`",
            TokenKind::Minus => "`-`",
            TokenKind::Star => "`*`",
            TokenKind::Slash => "`/`",
            TokenKind::Percent => "`%`",
            // Comparison operators
            TokenKind::EqEq => "`==`",
            TokenKind::Ne => "`!=`",
            TokenKind::Lt => "`<`",
            TokenKind::Le => "`<=`",
            TokenKind::Gt => "`>`",
            TokenKind::Ge => "`>=`",
            // Logical operators
            TokenKind::Bang => "`!`",
            TokenKind::AndAnd => "`&&`",
            TokenKind::OrOr => "`||`",
            // Punctuation
            TokenKind::Eq => "`=`",
            TokenKind::FatArrow => "`=>`",
            TokenKind::Comma => "`,`",
            TokenKind::Semicolon => "`;`",
            // Brackets
            TokenKind::LParen => "`(`",
            TokenKind::RParen => "`)`",
            TokenKind::LBrace => "`{`",
            TokenKind::RBrace => "`}`",
            // Special
            TokenKind::Unknown => "an unknown character",
            TokenKind::Eof => "end of input",
        };
        f.write_str(text)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Token {
    pub(crate) kind: TokenKind,
    pub(crate) span: Span,
}

impl Token {
    fn new(kind: TokenKind, start: usize, end: usize) -> Self {
        Token {
            kind,
            span: Span { start, end },
        }
    }
}

/// Hands out tokens one at a time. Like rustc's lexer cursor, it holds a plain
/// `CharIndices` and peeks by cloning it, which is cheap (two pointers) and
/// keeps `as_str()` and `offset()` available.
pub struct Lexer<'src> {
    src: &'src str,
    input: CharIndices<'src>,
}

impl<'src> Lexer<'src> {
    pub fn new(src: &'src str) -> Self {
        Lexer {
            src,
            input: src.char_indices(),
        }
    }

    pub(crate) fn src(&self) -> &'src str {
        self.src
    }

    /// Byte offset of the next character.
    fn pos(&self) -> usize {
        self.input.offset()
    }

    /// The next character, without consuming it.
    fn peek(&self) -> Option<char> {
        self.input.clone().next().map(|(_, c)| c)
    }

    /// The character after the next one, without consuming either.
    fn peek_second(&self) -> Option<char> {
        let mut chars = self.input.clone();
        chars.next();
        chars.next().map(|(_, c)| c)
    }

    fn bump(&mut self) -> Option<char> {
        self.input.next().map(|(_, c)| c)
    }

    fn eat_while(&mut self, pred: impl Fn(char) -> bool) {
        while self.peek().is_some_and(&pred) {
            self.bump();
        }
    }

    /// Skip whitespace and `//` comments, in any order. A loop rather than
    /// recursion, so any number of comment lines takes no extra stack.
    fn skip_trivia(&mut self) {
        loop {
            self.eat_while(|c| c.is_ascii_whitespace());
            if self.peek() == Some('/') && self.peek_second() == Some('/') {
                self.eat_while(|c| c != '\n'); // the newline itself is whitespace
            } else {
                break;
            }
        }
    }

    /// The next token, or `Eof` (with an empty span at the end of the
    /// input) once there are none left. Keeps returning `Eof` after that.
    /// Never fails: a character that cannot start a token comes back as
    /// `Unknown`. Integers are not range-checked here; the parser does that.
    /// Whitespace and `//` comments between tokens are skipped.
    pub(crate) fn next_token(&mut self) -> Token {
        self.skip_trivia();

        let start = self.pos();
        let Some(c) = self.bump() else {
            return Token::new(TokenKind::Eof, start, start);
        };

        let kind = match c {
            '+' => TokenKind::Plus,
            '-' => TokenKind::Minus,
            '*' => TokenKind::Star,
            '/' => TokenKind::Slash,
            '%' => TokenKind::Percent,
            '=' if let Some('=') = self.peek() => {
                self.bump();
                TokenKind::EqEq
            }
            '=' if let Some('>') = self.peek() => {
                self.bump();
                TokenKind::FatArrow
            }
            '=' => TokenKind::Eq,
            '!' if let Some('=') = self.peek() => {
                self.bump();
                TokenKind::Ne
            }
            '!' => TokenKind::Bang,
            '<' if let Some('=') = self.peek() => {
                self.bump();
                TokenKind::Le
            }
            '<' => TokenKind::Lt,
            '>' if let Some('=') = self.peek() => {
                self.bump();
                TokenKind::Ge
            }
            '>' => TokenKind::Gt,
            '|' if let Some('|') = self.peek() => {
                self.bump();
                TokenKind::OrOr
            }
            '&' if let Some('&') = self.peek() => {
                self.bump();
                TokenKind::AndAnd
            }
            ',' => TokenKind::Comma,
            ';' => TokenKind::Semicolon,
            '(' => TokenKind::LParen,
            ')' => TokenKind::RParen,
            '{' => TokenKind::LBrace,
            '}' => TokenKind::RBrace,
            '"' => {
                let terminated = loop {
                    match self.peek() {
                        None | Some('\n') => break false, // leave the newline for skip_trivia
                        Some('"') => {
                            self.bump();
                            break true;
                        }
                        Some('\\') => {
                            self.bump();
                            // Skip whatever is escaped, even `"` or `\`; the parser
                            // decides whether it is a valid escape. Never a newline,
                            // so an unterminated string still stops at the line end.
                            if self.peek().is_some_and(|c| c != '\n') {
                                self.bump();
                            }
                        }
                        Some(_) => {
                            self.bump();
                        }
                    }
                };
                TokenKind::Str { terminated }
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                self.eat_while(|c| c.is_ascii_alphanumeric() || c == '_');
                match &self.src[start..self.pos()] {
                    "let" => TokenKind::Let,
                    "def" => TokenKind::Def,
                    "true" => TokenKind::True,
                    "false" => TokenKind::False,
                    "fun" => TokenKind::Fun,
                    "if" => TokenKind::If,
                    "else" => TokenKind::Else,
                    _ => TokenKind::Ident,
                }
            }
            c if c.is_ascii_digit() => {
                self.eat_while(|c| c.is_ascii_alphanumeric() || c == '_');
                TokenKind::Int
            }
            _ => TokenKind::Unknown,
        };

        Token::new(kind, start, self.pos())
    }
}

#[cfg(test)]
mod tests;
