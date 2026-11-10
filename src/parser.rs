use crate::ast::{Ast, BinOp, Decl, Expr, Ident, Stmt, UnOp};
use crate::diagnostic::Diagnostic;
use crate::lexer::{Lexer, Token, TokenKind};
use crate::span::Span;
use std::fmt;
use std::num::IntErrorKind;

/// What the parser was looking for when it hit an error.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Expected {
    Token(TokenKind),
    Expression,
}

/// How it is named in error messages, e.g. "expected an expression".
impl fmt::Display for Expected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expected::Token(kind) => write!(f, "{kind}"),
            Expected::Expression => write!(f, "an expression"),
        }
    }
}

#[derive(Debug)]
pub enum ParserError {
    /// The lexer found a character that cannot start a token.
    UnknownChar {
        ch: char,
        span: Span,
    },
    /// An integer literal that does not fit in an `i64`.
    InvalidInt {
        kind: IntErrorKind,
        span: Span,
    },
    UnexpectedToken {
        expected: Expected,
        actual: TokenKind,
        span: Span,
    },
    ChainedComparison {
        span: Span,
    },
    StringNotTerminated(Span),
    /// A `\` in a string followed by something that is not a valid escape.
    /// `escape` is that character; `None` for a `\` at the very end, which
    /// the lexer never produces. The span covers the backslash and the
    /// character.
    InvalidEscape {
        escape: Option<char>,
        span: Span,
    },
}

impl ParserError {
    fn span(&self) -> Span {
        match self {
            ParserError::UnknownChar { span, .. }
            | ParserError::InvalidInt { span, .. }
            | ParserError::UnexpectedToken { span, .. } => *span,
            ParserError::ChainedComparison { span } => *span,
            ParserError::StringNotTerminated(span) => *span,
            ParserError::InvalidEscape { span, .. } => *span,
        }
    }
}

/// The headline of the error; `diagnostic` adds the location and details.
impl fmt::Display for ParserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParserError::UnknownChar { ch, .. } => write!(f, "unknown character `{ch}`"),
            ParserError::InvalidInt {
                kind: IntErrorKind::PosOverflow,
                ..
            } => write!(f, "integer literal is too large"),
            ParserError::InvalidInt { .. } => write!(f, "invalid integer literal"),
            ParserError::UnexpectedToken {
                expected, actual, ..
            } => write!(f, "expected {expected}, found {actual}"),
            ParserError::ChainedComparison { .. } => {
                write!(f, "comparison operators cannot be chained")
            }
            ParserError::StringNotTerminated(_) => write!(f, "unterminated string literal"),
            ParserError::InvalidEscape {
                escape: Some(c), ..
            } => write!(f, "unknown escape `\\{}`", c.escape_debug()),
            ParserError::InvalidEscape { escape: None, .. } => {
                write!(f, "incomplete escape sequence")
            }
        }
    }
}

impl ParserError {
    /// The error as a report for the user: the `Display` headline plus where
    /// it happened and what to say there.
    pub fn diagnostic(&self) -> Diagnostic {
        let label = match self {
            ParserError::UnknownChar { .. } => "not part of the language".to_owned(),
            ParserError::InvalidInt {
                kind: IntErrorKind::PosOverflow,
                ..
            } => "does not fit in an `i64`".to_owned(),
            ParserError::InvalidInt { .. } => "numbers can only contain digits".to_owned(),
            ParserError::UnexpectedToken { expected, .. } => {
                format!("expected {expected}")
            }
            ParserError::ChainedComparison { .. } => "cannot follow another comparison".to_owned(),
            ParserError::StringNotTerminated(_) => "missing the terminating `\"`".to_owned(),
            ParserError::InvalidEscape { .. } => "not a valid escape".to_owned(),
        };
        let diagnostic = Diagnostic::new(self.to_string(), self.span(), label);
        match self {
            ParserError::InvalidInt {
                kind: IntErrorKind::PosOverflow,
                ..
            } => diagnostic.with_note(format!("the largest integer is {}", i64::MAX)),
            ParserError::ChainedComparison { .. } => {
                diagnostic.with_note("use `&&` to combine comparisons, e.g. `a < b && b < c`")
            }
            ParserError::InvalidEscape { .. } => diagnostic.with_note(
                "valid escapes are `\\n` `\\r` `\\t` `\\\\` `\\\"` `\\0`; \
                 write `\\\\` for a backslash",
            ),
            _ => diagnostic,
        }
    }
}

/// How operators of the same precedence group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Assoc {
    /// `a - b - c` is `(a - b) - c`.
    Left,
    /// `a < b < c` is an error.
    None,
}

fn unescape(text: &str, offset: usize) -> Result<String, ParserError> {
    let mut chars = text.char_indices();
    let mut res = String::new();
    while let Some((i, c)) = chars.next() {
        let next_c = if c == '\\' {
            let peek = chars.next();
            match peek {
                Some((j, c)) => match c {
                    'n' => Ok('\n'),
                    'r' => Ok('\r'),
                    't' => Ok('\t'),
                    '\\' => Ok('\\'),
                    '"' => Ok('"'),
                    '0' => Ok('\0'),
                    _ => Err(ParserError::InvalidEscape {
                        escape: Some(c),
                        span: Span {
                            start: offset + i,
                            end: offset + j + c.len_utf8(),
                        },
                    }),
                },
                // Not produced by the lexer, which always treats `\"` as an
                // escaped quote, so a terminated string never ends in `\`.
                None => Err(ParserError::InvalidEscape {
                    escape: None,
                    span: Span {
                        start: offset + i,
                        end: offset + i + c.len_utf8(),
                    },
                }),
            }
        } else {
            Ok(c)
        };
        res.push(next_c?);
    }
    Ok(res)
}

/// A recursive-descent parser in the style of rustc's: it always holds the
/// current token and moves forward with `bump`. Lookahead is just reading
/// `self.token`, which is `Copy`, so nothing stays borrowed.
pub struct Parser<'src> {
    lexer: Lexer<'src>,
    /// The token being looked at. `Eof` once the input runs out.
    token: Token,
    /// Where the previous token ended. Errors at the end of input point
    /// here, right after the last real token, not past trailing whitespace.
    prev_end: usize,
}

impl<'src> Parser<'src> {
    pub fn new(mut lexer: Lexer<'src>) -> Self {
        let token = lexer.next_token();
        Parser {
            lexer,
            token,
            prev_end: 0,
        }
    }

    fn text(&self, span: Span) -> &'src str {
        &self.lexer.src()[span.start..span.end]
    }

    fn unary_op(kind: TokenKind) -> Option<UnOp> {
        match kind {
            TokenKind::Minus => Some(UnOp::Neg),
            TokenKind::Bang => Some(UnOp::Not),
            _ => None,
        }
    }

    /// The binary operator a token stands for in infix position, with its
    /// precedence (higher binds tighter) and associativity.
    fn binary_op(kind: TokenKind) -> Option<(BinOp, u32, Assoc)> {
        match kind {
            // Logical
            TokenKind::OrOr => Some((BinOp::Or, 1, Assoc::Left)),
            TokenKind::AndAnd => Some((BinOp::And, 2, Assoc::Left)),

            // Comparison
            TokenKind::EqEq => Some((BinOp::Eq, 3, Assoc::None)),
            TokenKind::Ne => Some((BinOp::Ne, 3, Assoc::None)),
            TokenKind::Lt => Some((BinOp::Lt, 3, Assoc::None)),
            TokenKind::Le => Some((BinOp::Le, 3, Assoc::None)),
            TokenKind::Gt => Some((BinOp::Gt, 3, Assoc::None)),
            TokenKind::Ge => Some((BinOp::Ge, 3, Assoc::None)),

            // Arithmetic
            TokenKind::Plus => Some((BinOp::Add, 4, Assoc::Left)),
            TokenKind::Minus => Some((BinOp::Sub, 4, Assoc::Left)),
            TokenKind::Star => Some((BinOp::Mul, 5, Assoc::Left)),
            TokenKind::Slash => Some((BinOp::Div, 5, Assoc::Left)),
            TokenKind::Percent => Some((BinOp::Rem, 5, Assoc::Left)),

            _ => None,
        }
    }

    fn bump(&mut self) {
        self.prev_end = self.token.span.end;
        self.token = self.lexer.next_token();
    }

    fn peek_is(&self, kind: TokenKind) -> bool {
        self.token.kind == kind
    }

    /// Consume the current token if it is `kind`, or fail.
    fn expect(&mut self, kind: TokenKind) -> Result<Span, ParserError> {
        let span = self.token.span;
        if kind == self.token.kind {
            self.bump();
            Ok(span)
        } else {
            Err(self.unexpected(Expected::Token(kind)))
        }
    }

    /// An error saying the current token is not what we wanted. Bad input
    /// from the lexer (an unknown character or an unterminated string) is
    /// reported as such wherever it appears, since no grammar rule accepts
    /// it.
    fn unexpected(&self, expected: Expected) -> ParserError {
        let Token { kind, span } = self.token;
        match kind {
            TokenKind::Unknown => ParserError::UnknownChar {
                // An `Unknown` token is exactly one character.
                ch: self.text(span).chars().next().unwrap_or_default(),
                span,
            },
            TokenKind::Str { terminated: false } => ParserError::StringNotTerminated(span),
            TokenKind::Eof => ParserError::UnexpectedToken {
                expected,
                actual: kind,
                span: Span {
                    start: self.prev_end,
                    end: self.prev_end,
                },
            },
            actual => ParserError::UnexpectedToken {
                expected,
                actual,
                span,
            },
        }
    }

    fn parse_comma_list<T>(
        &mut self,
        close: TokenKind,
        mut parse_elem: impl FnMut(&mut Self) -> Result<T, ParserError>,
    ) -> Result<Vec<T>, ParserError> {
        let mut res = Vec::new();
        while !self.peek_is(close) {
            let elem = parse_elem(self)?;
            res.push(elem);
            if self.peek_is(TokenKind::Comma) {
                self.bump();
            } else {
                break;
            }
        }
        Ok(res)
    }

    fn parse_ident(&mut self) -> Result<Ident, ParserError> {
        let span = self.expect(TokenKind::Ident)?;
        Ok(Ident {
            name: self.text(span).to_owned(),
            span,
        })
    }

    fn parse_let(&mut self) -> Result<Stmt, ParserError> {
        let start = self.expect(TokenKind::Let)?.start;

        let id = self.parse_ident()?;

        self.expect(TokenKind::Eq)?;

        let init = self.parse_expr()?;
        let end = self.expect(TokenKind::Semicolon)?.end;
        Ok(Stmt::let_decl(id, init, start, end))
    }

    fn parse_block(&mut self) -> Result<Expr, ParserError> {
        let start = self.expect(TokenKind::LBrace)?.start;

        let mut stmts = Vec::<Stmt>::new();
        loop {
            if self.peek_is(TokenKind::Let) {
                stmts.push(self.parse_let()?);
            } else {
                let expr = self.parse_expr()?;
                if self.peek_is(TokenKind::Semicolon) {
                    let start = expr.span.start;
                    let end = self.token.span.end;
                    self.bump();
                    stmts.push(Stmt::expr_stmt(expr, start, end));
                } else {
                    let end = self.expect(TokenKind::RBrace)?.end;
                    break Ok(Expr::block(stmts, expr, start, end));
                }
            }
        }
    }

    fn parse_if(&mut self) -> Result<Expr, ParserError> {
        let start = self.expect(TokenKind::If)?.start;
        let cond = self.parse_expr()?;
        let t_branch = self.parse_block()?;
        let (f_branch, end) = if let TokenKind::Else = self.token.kind {
            self.bump();
            // `else if …` chains: the else branch is another `if`.
            let f_branch = if let TokenKind::If = self.token.kind {
                self.parse_if()?
            } else {
                self.parse_block()?
            };
            let end = f_branch.span.end;
            (Some(f_branch), end)
        } else {
            (None, t_branch.span.end)
        };

        Ok(Expr::if_expr(cond, t_branch, f_branch, start, end))
    }

    fn parse_atom(&mut self) -> Result<Expr, ParserError> {
        match self.token.kind {
            TokenKind::Int => {
                let span = self.token.span;
                let text = self.text(span);
                self.bump();
                text.parse::<i64>()
                    .map(|n| Expr::int(n, span.start, span.end))
                    .map_err(|e| ParserError::InvalidInt {
                        kind: *e.kind(),
                        span,
                    })
            }
            // An unterminated string falls through to `unexpected`, which
            // reports it as such.
            TokenKind::Str { terminated: true } => {
                let span = self.token.span;
                let text = self.text(span);
                self.bump();
                unescape(&text[1..text.len() - 1], span.start + 1).map(|s| Expr::str(s, span))
            }
            TokenKind::True => {
                let span = self.token.span;
                self.bump();
                Ok(Expr::bool(true, span.start, span.end))
            }
            TokenKind::False => {
                let span = self.token.span;
                self.bump();
                Ok(Expr::bool(false, span.start, span.end))
            }
            TokenKind::Ident => {
                let id = self.parse_ident()?;
                Ok(Expr::var(id.name, id.span.start, id.span.end))
            }
            TokenKind::If => self.parse_if(),
            TokenKind::Fun => {
                let start = self.token.span.start;
                self.bump();

                self.expect(TokenKind::LParen)?;
                let params = self.parse_comma_list(TokenKind::RParen, Self::parse_ident)?;
                self.expect(TokenKind::RParen)?;

                self.expect(TokenKind::FatArrow)?;

                let body = self.parse_expr()?;
                let end = body.span.end;
                Ok(Expr::lambda(params, body, start, end))
            }
            TokenKind::LParen => {
                let start = self.token.span.start;
                self.bump();
                if self.peek_is(TokenKind::RParen) {
                    // Unit literal
                    let end = self.token.span.end;
                    self.bump();
                    Ok(Expr::unit(start, end))
                } else {
                    let mut expr = self.parse_expr()?;
                    let end = self.expect(TokenKind::RParen)?.end;
                    // Widen span to include the parenthesis
                    expr.span = Span { start, end };
                    Ok(expr)
                }
            }
            TokenKind::LBrace => self.parse_block(),
            _ => Err(self.unexpected(Expected::Expression)),
        }
    }

    fn parse_call(&mut self) -> Result<Expr, ParserError> {
        let mut expr = self.parse_atom()?;

        while self.peek_is(TokenKind::LParen) {
            self.bump();
            let args = self.parse_comma_list(TokenKind::RParen, Self::parse_expr)?;
            let start = expr.span.start;
            let end = self.expect(TokenKind::RParen)?.end;
            expr = Expr::call(expr, args, start, end)
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expr, ParserError> {
        if let Some(op) = Self::unary_op(self.token.kind) {
            let start = self.token.span.start;
            self.bump();
            let expr = self.parse_unary()?;
            let end = expr.span.end;
            Ok(Expr::unary(op, expr, start, end))
        } else {
            self.parse_call()
        }
    }

    fn parse_binary(&mut self, min: u32) -> Result<Expr, ParserError> {
        let mut lhs = self.parse_unary()?;
        // Precedence of the previous operator in this loop, if it was
        // non-associative.
        let mut prev_non_assoc = None;
        while let Some((op, prec, assoc)) = Parser::binary_op(self.token.kind)
            && prec >= min
        {
            // A non-associative operator cannot follow another one of the
            // same precedence: `a < b < c` is an error, not `(a < b) < c`.
            if assoc == Assoc::None && prev_non_assoc == Some(prec) {
                return Err(ParserError::ChainedComparison {
                    span: self.token.span,
                });
            }
            self.bump();
            // `prec + 1`: an operator of the same precedence on the right ends
            // the operand, so it comes back to this loop. Left-associative
            // operators continue here; non-associative ones hit the check
            // above.
            let rhs = self.parse_binary(prec + 1)?;
            let start = lhs.span.start;
            let end = rhs.span.end;
            lhs = Expr::binary(op, lhs, rhs, start, end);
            prev_non_assoc = (assoc == Assoc::None).then_some(prec);
        }
        Ok(lhs)
    }

    fn parse_expr(&mut self) -> Result<Expr, ParserError> {
        self.parse_binary(0)
    }

    fn parse_function(&mut self) -> Result<Decl, ParserError> {
        let start = self.expect(TokenKind::Def)?.start;

        let id = self.parse_ident()?;

        self.expect(TokenKind::LParen)?;
        let params = self.parse_comma_list(TokenKind::RParen, Self::parse_ident)?;

        self.expect(TokenKind::RParen)?;
        self.expect(TokenKind::Eq)?;

        let body = self.parse_expr()?;
        let end = self.expect(TokenKind::Semicolon)?.end;
        Ok(Decl::function(id, params, body, start, end))
    }

    pub fn parse(&mut self) -> Result<Ast, ParserError> {
        let mut decls = Vec::<Decl>::new();
        while !self.peek_is(TokenKind::Eof) {
            decls.push(self.parse_function()?);
        }
        Ok(Ast { decls })
    }
}

#[cfg(test)]
mod tests;
