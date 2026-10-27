use super::*;
use crate::ast::{DeclKind, ExprKind, StmtKind};

/// Parse a single expression that must make up the whole input.
fn parse(src: &str) -> Result<Expr, ParserError> {
    let mut parser = Parser::new(Lexer::new(src));
    let expr = parser.parse_expr()?;
    parser.expect(TokenKind::Eof)?;
    Ok(expr)
}

fn ast(src: &str) -> String {
    parse(src).unwrap().to_string()
}

fn parse_program(src: &str) -> Result<Ast, ParserError> {
    Parser::new(Lexer::new(src)).parse()
}

fn text(src: &str, span: Span) -> &str {
    &src[span.start..span.end]
}

#[test]
fn precedence_and_associativity() {
    assert_eq!(ast("1 + 2 * 3"), "(1 + (2 * 3))");
    assert_eq!(ast("1 * 2 + 3"), "((1 * 2) + 3)");
    assert_eq!(ast("1 - 2 - 3"), "((1 - 2) - 3)");
    assert_eq!(ast("8 / 4 / 2"), "((8 / 4) / 2)");
    assert_eq!(ast("1 + 7 % 3"), "(1 + (7 % 3))");
    assert_eq!(ast("7 % 3 * 2"), "((7 % 3) * 2)");
    assert_eq!(ast("7 * 3 % 2"), "((7 * 3) % 2)");
    assert_eq!(ast("(1 - 2) * 3"), "((1 - 2) * 3)");
    assert_eq!(ast("((7))"), "7");
}

#[test]
fn unary_minus() {
    assert_eq!(ast("-1"), "(- 1)");
    assert_eq!(ast("-2 * 3"), "((- 2) * 3)");
    assert_eq!(ast("2 * -3"), "(2 * (- 3))");
    assert_eq!(ast("1 - -2"), "(1 - (- 2))");
    assert_eq!(ast("--x"), "(- (- x))");
    assert_eq!(ast("-(1 + 2)"), "(- (1 + 2))");
    assert_eq!(ast("-{ 1 }"), "(- BLOCK: { 1 })");
}

#[test]
fn unary_minus_spans() {
    let src = "1 + -x";
    let expr = parse(src).unwrap();
    let ExprKind::Binary(_, _, rhs) = &expr.kind else {
        panic!("expected a binary expression, got {expr}");
    };
    assert_eq!(text(src, rhs.span), "-x");
    let src = "-(1 + 2)";
    assert_eq!(text(src, parse(src).unwrap().span), "-(1 + 2)");
}

#[test]
fn unary_minus_needs_an_operand() {
    for src in ["-", "1 + -", "- )"] {
        assert!(
            matches!(
                parse(src),
                Err(ParserError::UnexpectedToken {
                    expected: Expected::Expression,
                    ..
                })
            ),
            "input: {src:?}"
        );
    }
}

#[test]
fn variables() {
    assert_eq!(ast("x"), "x");
    assert_eq!(ast("x + y * 2"), "(x + (y * 2))");
}

#[test]
fn calls() {
    assert_eq!(ast("f()"), "f()");
    assert_eq!(ast("f(1)"), "f(1)");
    assert_eq!(ast("f(1, 2, 3)"), "f(1, 2, 3)");
    assert_eq!(ast("f (1)"), "f(1)");
    assert_eq!(ast("f(g(1), h())"), "f(g(1), h())");
    assert_eq!(ast("f({ 1 }, 2 + 3)"), "f(BLOCK: { 1 }, (2 + 3))");
}

#[test]
fn calls_bind_tighter_than_operators() {
    assert_eq!(ast("1 + f(2) * 3"), "(1 + (f(2) * 3))");
    assert_eq!(ast("f(1) - g(2)"), "(f(1) - g(2))");
    assert_eq!(ast("-f(1)"), "(- f(1))");
    assert_eq!(ast("-f(1)(2)"), "(- f(1)(2))");
}

#[test]
fn chained_calls() {
    assert_eq!(ast("f(1)(2)"), "f(1)(2)");
    assert_eq!(ast("f()()()"), "f()()()");

    // The outer call's callee is the inner call.
    let src = "f(1)(2)";
    let expr = parse(src).unwrap();
    let ExprKind::Call { callee, args } = &expr.kind else {
        panic!("expected a call, got {expr}");
    };
    assert!(matches!(callee.kind, ExprKind::Call { .. }));
    assert_eq!(text(src, callee.span), "f(1)");
    assert_eq!(text(src, args[0].span), "2");
}

#[test]
fn any_expression_can_be_called() {
    assert_eq!(ast("(f)(1)"), "f(1)");
    assert_eq!(ast("{ f }(1)"), "BLOCK: { f }(1)");
}

#[test]
fn call_spans() {
    let src = "1 + f(2, x * 3)";
    let expr = parse(src).unwrap();
    let ExprKind::Binary(_, _, call) = &expr.kind else {
        panic!("expected a binary expression, got {expr}");
    };
    assert_eq!(text(src, call.span), "f(2, x * 3)");
    let ExprKind::Call { callee, args } = &call.kind else {
        panic!("expected a call, got {call}");
    };
    assert_eq!(text(src, callee.span), "f");
    let arg_texts: Vec<_> = args.iter().map(|a| text(src, a.span)).collect();
    assert_eq!(arg_texts, ["2", "x * 3"]);

    let src = "(f)(1)";
    assert_eq!(text(src, parse(src).unwrap().span), "(f)(1)");
}

#[test]
fn trailing_commas() {
    assert_eq!(ast("f(1,)"), "f(1)");
    assert_eq!(ast("f(1, 2,)"), "f(1, 2)");
    assert_eq!(ast("f(\n    1,\n    2,\n)"), "f(1, 2)");
    assert_eq!(ast("f(1,)(2,)"), "f(1)(2)");
    assert_eq!(ast("fun(x,) => x"), "FUN (x) => x");
    assert_eq!(ast("fun(x, y,) => x"), "FUN (x, y) => x");
    assert_eq!(
        parse_program("def f(a, b,) = a;").unwrap().to_string(),
        "AST:\nDEF f (a, b) = a\n"
    );
    // The comma does not count as an argument or end up in the call's span.
    let src = "f(1,)";
    let expr = parse(src).unwrap();
    let ExprKind::Call { args, .. } = &expr.kind else {
        panic!("expected a call, got {expr}");
    };
    assert_eq!(args.len(), 1);
    assert_eq!(text(src, expr.span), "f(1,)");
}

#[test]
fn call_errors() {
    // A trailing comma is fine, but an empty argument is not.
    for (src, expected, actual) in [
        ("f(,)", Expected::Expression, TokenKind::Comma),
        ("f(1,,2)", Expected::Expression, TokenKind::Comma),
        ("f(1,", Expected::Expression, TokenKind::Eof),
        ("f(", Expected::Expression, TokenKind::Eof),
        ("f(1 2)", Expected::Token(TokenKind::RParen), TokenKind::Int),
        ("f(1", Expected::Token(TokenKind::RParen), TokenKind::Eof),
        ("f(1)(", Expected::Expression, TokenKind::Eof),
    ] {
        let err = parse(src).unwrap_err();
        assert!(
            matches!(
                err,
                ParserError::UnexpectedToken { expected: e, actual: a, .. }
                    if e == expected && a == actual
            ),
            "input: {src:?}, got: {err:?}"
        );
    }
}

#[test]
fn calls_in_programs() {
    let src = "def add(a, b) = a + b; def main() = add(1, add(2, 3));";
    assert_eq!(
        parse_program(src).unwrap().to_string(),
        "AST:\nDEF add (a, b) = (a + b)\nDEF main () = add(1, add(2, 3))\n"
    );
}

#[test]
fn not() {
    assert_eq!(ast("!a"), "(! a)");
    assert_eq!(ast("!!a"), "(! (! a))");
    assert_eq!(ast("!-a"), "(! (- a))");
    // Prefix operators bind tighter than any binary operator.
    assert_eq!(ast("!a && b"), "((! a) && b)");
    assert_eq!(ast("-a < b"), "((- a) < b)");
    assert_eq!(ast("!f(x)"), "(! f(x))");
}

#[test]
fn operator_precedence() {
    // Lowest to highest: `||`, `&&`, comparisons, `+ -`, `* /`.
    assert_eq!(ast("a || b && c"), "(a || (b && c))");
    assert_eq!(ast("a && b || c"), "((a && b) || c)");
    assert_eq!(ast("a == b && c"), "((a == b) && c)");
    assert_eq!(ast("a && b != c"), "(a && (b != c))");
    assert_eq!(ast("1 + 2 < 3 * 4"), "((1 + 2) < (3 * 4))");
    assert_eq!(ast("a < b || c >= d && e"), "((a < b) || ((c >= d) && e))");
}

#[test]
fn comparison_operators() {
    for op in ["==", "!=", "<", "<=", ">", ">="] {
        let src = format!("a {op} b");
        assert_eq!(ast(&src), format!("(a {op} b)"), "input: {src:?}");
    }
}

#[test]
fn logical_operators_are_left_associative() {
    assert_eq!(ast("a || b || c"), "((a || b) || c)");
    assert_eq!(ast("a && b && c"), "((a && b) && c)");
}

#[test]
fn comparisons_cannot_be_chained() {
    // The error points at the second comparison operator.
    for (src, at) in [
        ("a < b < c", 6),
        ("a == b != c", 7),
        ("a < b == c", 6),
        ("a < b + 1 >= c", 10),
        ("x && a <= b > c", 12),
    ] {
        let err = parse(src).unwrap_err();
        assert!(
            matches!(err, ParserError::ChainedComparison { span } if span.start == at),
            "input: {src:?}, got: {err:?}"
        );
    }
}

#[test]
fn separate_comparisons_are_fine() {
    assert_eq!(ast("a < b && b < c"), "((a < b) && (b < c))");
    // Parentheses make the grouping explicit.
    assert_eq!(ast("(a < b) == c"), "((a < b) == c)");
    assert_eq!(ast("a == (b < c)"), "(a == (b < c))");
}

#[test]
fn chained_comparison_diagnostic() {
    let src = "a < b < c";
    let diag = parse(src).unwrap_err().diagnostic();
    assert_eq!(diag.message, "comparison operators cannot be chained");
    let (span, label) = diag.label.unwrap();
    assert_eq!(text(src, span), "<");
    assert_eq!(label, "cannot follow another comparison");
    assert_eq!(
        diag.notes,
        ["use `&&` to combine comparisons, e.g. `a < b && b < c`"]
    );
}

#[test]
fn if_expressions() {
    assert_eq!(ast("if c { 1 }"), "(IF c BLOCK: { 1 })");
    assert_eq!(
        ast("if c { 1 } else { 2 }"),
        "(IF c BLOCK: { 1 } BLOCK: { 2 })"
    );
    assert_eq!(
        ast("if a < b && c { 1 } else { 2 }"),
        "(IF ((a < b) && c) BLOCK: { 1 } BLOCK: { 2 })"
    );
}

#[test]
fn else_if_chains() {
    assert_eq!(
        ast("if a { 1 } else if b { 2 } else { 3 }"),
        "(IF a BLOCK: { 1 } (IF b BLOCK: { 2 } BLOCK: { 3 }))"
    );
    assert_eq!(
        ast("if a { 1 } else if b { 2 }"),
        "(IF a BLOCK: { 1 } (IF b BLOCK: { 2 }))"
    );
}

#[test]
fn if_is_an_expression() {
    assert_eq!(
        ast("1 + if c { 2 } else { 3 }"),
        "(1 + (IF c BLOCK: { 2 } BLOCK: { 3 }))"
    );
    assert_eq!(
        ast("{ let x = if c { 1 } else { 2 }; x }"),
        "BLOCK: { LET: x = (IF c BLOCK: { 1 } BLOCK: { 2 }) ; x }"
    );
    assert_eq!(
        ast("f(if c { 1 } else { 2 })"),
        "f((IF c BLOCK: { 1 } BLOCK: { 2 }))"
    );
}

#[test]
fn if_spans() {
    for src in [
        "if c { 1 }",
        "if c { 1 } else { 2 }",
        "if a { 1 } else if b { 2 } else { 3 }",
    ] {
        assert_eq!(text(src, parse(src).unwrap().span), src);
    }
    let src = "if a { 1 } else if b { 2 }";
    let ExprKind::If {
        f_branch: else_branch,
        ..
    } = &parse(src).unwrap().kind
    else {
        panic!("expected an if");
    };
    assert_eq!(text(src, else_branch.as_ref().unwrap().span), "if b { 2 }");
}

#[test]
fn if_errors() {
    for (src, expected, actual) in [
        // Branches must be blocks.
        ("if c 1", Expected::Token(TokenKind::LBrace), TokenKind::Int),
        (
            "if c { 1 } else 2",
            Expected::Token(TokenKind::LBrace),
            TokenKind::Int,
        ),
        (
            "if c { 1 } else",
            Expected::Token(TokenKind::LBrace),
            TokenKind::Eof,
        ),
        // A missing condition: the block is read as the condition.
        (
            "if { 1 }",
            Expected::Token(TokenKind::LBrace),
            TokenKind::Eof,
        ),
        // `else` on its own.
        ("else { 1 }", Expected::Expression, TokenKind::Else),
    ] {
        let err = parse(src).unwrap_err();
        assert!(
            matches!(
                err,
                ParserError::UnexpectedToken { expected: e, actual: a, .. }
                    if e == expected && a == actual
            ),
            "input: {src:?}, got: {err:?}"
        );
    }
}

/// The text of a string literal after parsing, i.e. with escapes
/// replaced by the characters they stand for.
fn string_value(src: &str) -> String {
    match parse(src).unwrap().kind {
        ExprKind::Str(s) => s.to_string(),
        other => panic!("expected a string, got {other:?}"),
    }
}

#[test]
fn string_literals() {
    assert_eq!(string_value(r#""hello""#), "hello");
    assert_eq!(string_value(r#""""#), "");
    assert_eq!(string_value(r#""two words""#), "two words");
    assert_eq!(string_value(r#""é 日本 😀""#), "é 日本 😀");
    // Displayed with quotes and escapes, e.g. by `--ast`.
    assert_eq!(ast(r#""hi""#), r#""hi""#);
    assert_eq!(ast(r#""a\nb""#), r#""a\nb""#);
}

#[test]
fn string_escapes() {
    for (src, expected) in [
        (r#""\n""#, "\n"),
        (r#""\r""#, "\r"),
        (r#""\t""#, "\t"),
        (r#""\\""#, "\\"),
        (r#""\"""#, "\""),
        (r#""\0""#, "\0"),
        (r#""a\tb\nc""#, "a\tb\nc"),
        (r#""say \"hi\"""#, "say \"hi\""),
        // `\\` followed by `n` is a backslash and an `n`, not a newline.
        (r#""\\n""#, "\\n"),
    ] {
        assert_eq!(string_value(src), expected, "input: {src}");
    }
}

#[test]
fn string_spans() {
    let src = r#"f("hi", "a\nb")"#;
    let ExprKind::Call { args, .. } = &parse(src).unwrap().kind else {
        panic!("expected a call");
    };
    // Spans include the quotes and are in source bytes, before escapes
    // are replaced.
    assert_eq!(text(src, args[0].span), r#""hi""#);
    assert_eq!(text(src, args[1].span), r#""a\nb""#);
}

#[test]
fn strings_in_expressions() {
    assert_eq!(ast(r#""a" == "b""#), r#"("a" == "b")"#);
    assert_eq!(ast(r#"f("x", 1)"#), r#"f("x", 1)"#);
    assert_eq!(
        ast(r#"{ let s = "x"; s }"#),
        r#"BLOCK: { LET: s = "x" ; s }"#
    );
}

#[test]
fn unterminated_strings() {
    for (src, covered) in [
        (r#""abc"#, r#""abc"#),
        (r#"f("abc"#, r#""abc"#),
        // An escaped quote does not end the string.
        (r#""abc\""#, r#""abc\""#),
        // A string cannot span lines; the error stops at the line end.
        ("\"abc\n1", "\"abc"),
    ] {
        let err = parse(src).unwrap_err();
        let ParserError::StringNotTerminated(span) = err else {
            panic!("input: {src:?}, got: {err:?}");
        };
        assert_eq!(text(src, span), covered, "input: {src:?}");
    }
}

#[test]
fn invalid_escapes() {
    for (src, escape, covered) in [
        (r#""\q""#, 'q', r"\q"),
        (r#""ab\qcd""#, 'q', r"\q"),
        // No octal, Unicode or other escapes (yet).
        (r#""\1""#, '1', r"\1"),
        (r#""\u{41}""#, 'u', r"\u"),
        (r#""\x41""#, 'x', r"\x"),
        // Non-ASCII escaped characters: the span ends on a char boundary.
        (r#""é\é""#, 'é', r"\é"),
        (r#""\😀""#, '😀', r"\😀"),
    ] {
        let err = parse(src).unwrap_err();
        let ParserError::InvalidEscape {
            escape: Some(c),
            span,
        } = err
        else {
            panic!("input: {src:?}, got: {err:?}");
        };
        assert_eq!(c, escape, "input: {src:?}");
        assert_eq!(text(src, span), covered, "input: {src:?}");
    }
}

#[test]
fn invalid_escape_diagnostic() {
    let diag = parse(r#""\q""#).unwrap_err().diagnostic();
    assert_eq!(diag.message, r"unknown escape `\q`");
    assert_eq!(diag.label.unwrap().1, "not a valid escape");
    assert_eq!(
        diag.notes,
        [r#"valid escapes are `\n` `\r` `\t` `\\` `\"` `\0`; write `\\` for a backslash"#]
    );
    // Escaped characters are shown readably in the headline.
    let diag = parse("\"\\\t\"").unwrap_err().diagnostic();
    assert_eq!(diag.message, r"unknown escape `\\t`");
}

#[test]
fn unterminated_strings_anywhere() {
    // Wherever the parser rejects the token, the report is about the
    // string, not "expected X, found a string".
    for (src, covered) in [
        // Where `,` or `)` is expected.
        (r#"def main() = f(1 "abc"#, r#""abc"#),
        // Where `;` is expected.
        ("def main() = 1 \"abc\n", "\"abc"),
        // Where `}` is expected.
        ("def main() = { 1 \"x }", "\"x }"),
        // Where an identifier is expected.
        (r#"def "abc"#, r#""abc"#),
        ("def main() = { let \"x = 1; 2 };", "\"x = 1; 2 };"),
        (r#"def f(a, "b"#, r#""b"#),
        // Where `def` is expected.
        ("\"abc", "\"abc"),
    ] {
        let err = parse_program(src).unwrap_err();
        let ParserError::StringNotTerminated(span) = err else {
            panic!("input: {src:?}, got: {err:?}");
        };
        assert_eq!(text(src, span), covered, "input: {src:?}");
    }
    // A terminated string in the wrong place is still an ordinary
    // unexpected token.
    assert!(matches!(
        parse_program(r#"def "f"() = 1;"#),
        Err(ParserError::UnexpectedToken {
            expected: Expected::Token(TokenKind::Ident),
            actual: TokenKind::Str { terminated: true },
            ..
        })
    ));
}

#[test]
fn unterminated_string_diagnostic() {
    let diag = parse(r#""abc"#).unwrap_err().diagnostic();
    assert_eq!(diag.message, "unterminated string literal");
    assert_eq!(diag.label.unwrap().1, "missing the terminating `\"`");
}

#[test]
fn booleans() {
    assert_eq!(ast("true"), "true");
    assert_eq!(ast("false"), "false");
    assert!(matches!(parse("true").unwrap().kind, ExprKind::Bool(true)));
    assert!(matches!(
        parse("false").unwrap().kind,
        ExprKind::Bool(false)
    ));
}

#[test]
fn booleans_in_expressions() {
    assert_eq!(ast("f(true, false)"), "f(true, false)");
    assert_eq!(ast("{ let b = true; b }"), "BLOCK: { LET: b = true ; b }");
    assert_eq!(ast("fun() => false"), "FUN () => false");
    // Type errors are the evaluator's job; these parse.
    assert_eq!(ast("true + 1"), "(true + 1)");
    assert_eq!(ast("-false"), "(- false)");
}

#[test]
fn boolean_spans() {
    let src = "f(1, true)";
    let ExprKind::Call { args, .. } = &parse(src).unwrap().kind else {
        panic!("expected a call");
    };
    assert_eq!(text(src, args[1].span), "true");
}

#[test]
fn booleans_are_not_names() {
    for (src, actual) in [
        ("{ let true = 1; 1 }", TokenKind::True),
        ("fun(false) => 1", TokenKind::False),
    ] {
        assert!(
            matches!(
                parse(src),
                Err(ParserError::UnexpectedToken {
                    expected: Expected::Token(TokenKind::Ident),
                    actual: a,
                    ..
                }) if a == actual
            ),
            "input: {src:?}"
        );
    }
    assert!(matches!(
        parse_program("def true() = 1;"),
        Err(ParserError::UnexpectedToken {
            expected: Expected::Token(TokenKind::Ident),
            actual: TokenKind::True,
            ..
        })
    ));
}

#[test]
fn unit_literal() {
    assert_eq!(ast("()"), "()");
    assert_eq!(ast("( )"), "()");
    assert!(matches!(parse("()").unwrap().kind, ExprKind::Unit));
    // `(())` is unit in parentheses, not a nested unit type.
    assert!(matches!(parse("(())").unwrap().kind, ExprKind::Unit));
}

#[test]
fn unit_in_expressions() {
    assert_eq!(ast("f(())"), "f(())");
    assert_eq!(ast("f((), 1)"), "f((), 1)");
    assert_eq!(ast("{ let u = (); u }"), "BLOCK: { LET: u = () ; u }");
    assert_eq!(ast("fun() => ()"), "FUN () => ()");
    assert_eq!(ast("() == ()"), "(() == ())");
    // A call with no arguments is still a call, not a call on unit.
    assert_eq!(ast("f()"), "f()");
}

#[test]
fn unit_spans() {
    let src = "f(1, ( ))";
    let ExprKind::Call { args, .. } = &parse(src).unwrap().kind else {
        panic!("expected a call");
    };
    assert_eq!(text(src, args[1].span), "( )");
    // Parentheses around unit widen the span like any other expression.
    let src = "(())";
    assert_eq!(text(src, parse(src).unwrap().span), "(())");
}

#[test]
fn unclosed_unit() {
    assert!(matches!(
        parse("("),
        Err(ParserError::UnexpectedToken {
            expected: Expected::Expression,
            actual: TokenKind::Eof,
            ..
        })
    ));
}

#[test]
fn lambdas() {
    assert_eq!(ast("fun() => 1"), "FUN () => 1");
    assert_eq!(ast("fun(x) => x"), "FUN (x) => x");
    assert_eq!(ast("fun(a, b) => a * b"), "FUN (a, b) => (a * b)");
    assert_eq!(ast("fun (x) => x"), "FUN (x) => x");
    assert_eq!(
        ast("fun(x) => { let y = x; y }"),
        "FUN (x) => BLOCK: { LET: y = x ; y }"
    );
}

#[test]
fn lambda_body_extends_as_far_right_as_possible() {
    assert_eq!(ast("fun(x) => x + 1 * 2"), "FUN (x) => (x + (1 * 2))");
    // A lambda returning a lambda (currying).
    assert_eq!(
        ast("fun(a) => fun(b) => a + b"),
        "FUN (a) => FUN (b) => (a + b)"
    );
    // A call after the body belongs to the body, not to the lambda.
    let expr = parse("fun(f) => f(1)").unwrap();
    let ExprKind::Lambda { body, .. } = &expr.kind else {
        panic!("expected a lambda, got {expr}");
    };
    assert!(matches!(body.kind, ExprKind::Call { .. }), "got {body}");
}

#[test]
fn lambda_body_ends_at_comma_and_closing_paren() {
    assert_eq!(ast("g(fun(x) => x * 2, 3)"), "g(FUN (x) => (x * 2), 3)");
    // Parenthesized, a lambda can be called right away.
    let expr = parse("(fun(x) => x + 1)(41)").unwrap();
    let ExprKind::Call { callee, args } = &expr.kind else {
        panic!("expected a call, got {expr}");
    };
    assert!(
        matches!(callee.kind, ExprKind::Lambda { .. }),
        "got {callee}"
    );
    assert_eq!(args.len(), 1);
}

#[test]
fn lambda_spans() {
    let src = "g(fun(x, yy) => x * yy, 3)";
    let expr = parse(src).unwrap();
    let ExprKind::Call { args, .. } = &expr.kind else {
        panic!("expected a call, got {expr}");
    };
    assert_eq!(text(src, args[0].span), "fun(x, yy) => x * yy");
    let ExprKind::Lambda { params, body } = &args[0].kind else {
        panic!("expected a lambda, got {}", args[0]);
    };
    let names: Vec<_> = params.iter().map(|p| text(src, p.span)).collect();
    assert_eq!(names, ["x", "yy"]);
    assert_eq!(text(src, body.span), "x * yy");

    // Including the parentheses around it.
    let src = "(fun() => 1)";
    assert_eq!(text(src, parse(src).unwrap().span), "(fun() => 1)");
}

#[test]
fn lambda_errors() {
    for (src, expected, actual) in [
        (
            "fun x = 1",
            Expected::Token(TokenKind::LParen),
            TokenKind::Ident,
        ),
        (
            "fun(x) x",
            Expected::Token(TokenKind::FatArrow),
            TokenKind::Ident,
        ),
        // `=` is for `def`; a lambda's body follows `=>`.
        (
            "fun(x) = x",
            Expected::Token(TokenKind::FatArrow),
            TokenKind::Eq,
        ),
        (
            "fun x => 1",
            Expected::Token(TokenKind::LParen),
            TokenKind::Ident,
        ),
        (
            "fun(x,,y) => x",
            Expected::Token(TokenKind::Ident),
            TokenKind::Comma,
        ),
        (
            "fun(1) => x",
            Expected::Token(TokenKind::Ident),
            TokenKind::Int,
        ),
        ("fun(x) =>", Expected::Expression, TokenKind::Eof),
        (
            "fun(x = x",
            Expected::Token(TokenKind::RParen),
            TokenKind::Eq,
        ),
    ] {
        let err = parse(src).unwrap_err();
        assert!(
            matches!(
                err,
                ParserError::UnexpectedToken { expected: e, actual: a, .. }
                    if e == expected && a == actual
            ),
            "input: {src:?}, got: {err:?}"
        );
    }
}

#[test]
fn lambdas_in_programs() {
    let src = "def add(a) = fun(b) => a + b; def main() = add(1)(2);";
    assert_eq!(
        parse_program(src).unwrap().to_string(),
        "AST:\nDEF add (a) = FUN (b) => (a + b)\nDEF main () = add(1)(2)\n"
    );
}

#[test]
fn expected_expression() {
    assert!(matches!(
        parse(""),
        Err(ParserError::UnexpectedToken {
            expected: Expected::Expression,
            actual: TokenKind::Eof,
            span: Span { start: 0, end: 0 },
        })
    ));
    assert!(matches!(
        parse("1 +"),
        Err(ParserError::UnexpectedToken {
            expected: Expected::Expression,
            actual: TokenKind::Eof,
            span: Span { start: 3, end: 3 },
        })
    ));
    assert!(matches!(
        parse(")"),
        Err(ParserError::UnexpectedToken {
            expected: Expected::Expression,
            actual: TokenKind::RParen,
            span: Span { start: 0, end: 1 },
        })
    ));
}

#[test]
fn unclosed_paren() {
    assert!(matches!(
        parse("(1"),
        Err(ParserError::UnexpectedToken {
            expected: Expected::Token(TokenKind::RParen),
            actual: TokenKind::Eof,
            span: Span { start: 2, end: 2 },
        })
    ));
    assert!(matches!(
        parse("(1 2"),
        Err(ParserError::UnexpectedToken {
            expected: Expected::Token(TokenKind::RParen),
            actual: TokenKind::Int,
            ..
        })
    ));
}

#[test]
fn trailing_tokens() {
    assert!(matches!(
        parse("1 2"),
        Err(ParserError::UnexpectedToken {
            expected: Expected::Token(TokenKind::Eof),
            actual: TokenKind::Int,
            span: Span { start: 2, end: 3 },
        })
    ));
    assert!(matches!(
        parse("1)"),
        Err(ParserError::UnexpectedToken {
            expected: Expected::Token(TokenKind::Eof),
            actual: TokenKind::RParen,
            ..
        })
    ));
}

#[test]
fn unknown_char_is_reported_where_the_parser_reaches_it() {
    for (src, start) in [
        ("$ + 1", 0), // first token
        ("1 + $", 4), // atom position
        ("1 $", 2),   // operator position, reported by the final `expect(Eof)`
        ("(1 $", 3),  // inside parentheses
    ] {
        let err = parse(src).unwrap_err();
        assert!(
            matches!(err, ParserError::UnknownChar { ch: '$', span } if span.start == start),
            "input: {src:?}, got: {err:?}"
        );
        assert_eq!(text(src, err.span()), "$");
    }
}

#[test]
fn largest_int() {
    assert_eq!(ast("9223372036854775807"), "9223372036854775807");
}

#[test]
fn invalid_int() {
    assert!(matches!(
        parse("1 + 9223372036854775808"),
        Err(ParserError::InvalidInt {
            kind: IntErrorKind::PosOverflow,
            span: Span { start: 4, end: 23 },
        })
    ));
}

#[test]
fn number_followed_by_letters_is_invalid() {
    assert!(matches!(
        parse("123abc"),
        Err(ParserError::InvalidInt {
            kind: IntErrorKind::InvalidDigit,
            span: Span { start: 0, end: 6 },
        })
    ));
}

#[test]
fn underscore_in_number_is_invalid() {
    // No digit separators: the whole literal is reported, not just `1`.
    assert!(matches!(
        parse("1_000 + 2"),
        Err(ParserError::InvalidInt {
            kind: IntErrorKind::InvalidDigit,
            span: Span { start: 0, end: 5 },
        })
    ));
}

#[test]
fn names_can_start_with_underscore() {
    assert_eq!(ast("_x + _"), "(_x + _)");
    assert_eq!(ast("fun(_a, _) => _a"), "FUN (_a, _) => _a");
    assert_eq!(
        ast("{ let _unused = 1; 2 }"),
        "BLOCK: { LET: _unused = 1 ; 2 }"
    );
    assert_eq!(
        parse_program("def _helper(_n) = _n;").unwrap().to_string(),
        "AST:\nDEF _helper (_n) = _n\n"
    );
}

#[test]
fn expression_spans() {
    let src = "1 + x * 3";
    let expr = parse(src).unwrap();
    assert_eq!(text(src, expr.span), "1 + x * 3");
    let ExprKind::Binary(_, lhs, rhs) = &expr.kind else {
        panic!("expected a binary expression, got {expr}");
    };
    assert_eq!(text(src, lhs.span), "1");
    assert_eq!(text(src, rhs.span), "x * 3");
}

#[test]
fn blocks() {
    assert_eq!(ast("{ 1 }"), "BLOCK: { 1 }");
    assert_eq!(
        ast("{ let a = 1; let b = a + 2; a * b }"),
        "BLOCK: { LET: a = 1 ; LET: b = (a + 2) ; (a * b) }"
    );
    assert_eq!(ast("{ 1; 2 }"), "BLOCK: { EXPRSTMT: 1 ; 2 }");
    assert_eq!(
        ast("{ let x = { 1 }; x }"),
        "BLOCK: { LET: x = BLOCK: { 1 } ; x }"
    );
    assert_eq!(ast("1 + { 2 }"), "(1 + BLOCK: { 2 })");
}

#[test]
fn block_and_statement_spans() {
    let src = "{ let a = 1; a + 1; a }";
    let expr = parse(src).unwrap();
    assert_eq!(text(src, expr.span), src);
    let ExprKind::Block { stmts, tail } = &expr.kind else {
        panic!("expected a block, got {expr}");
    };
    assert_eq!(text(src, stmts[0].span), "let a = 1;");
    assert_eq!(text(src, stmts[1].span), "a + 1;");
    assert_eq!(text(src, tail.span), "a");
    let StmtKind::Let { id, .. } = &stmts[0].kind else {
        panic!("expected a let statement");
    };
    assert_eq!(text(src, id.span), "a");
}

#[test]
fn block_errors() {
    // The block must end in an expression.
    assert!(matches!(
        parse("{ let a = 1; }"),
        Err(ParserError::UnexpectedToken {
            expected: Expected::Expression,
            actual: TokenKind::RBrace,
            ..
        })
    ));
    assert!(matches!(
        parse("{ }"),
        Err(ParserError::UnexpectedToken {
            expected: Expected::Expression,
            actual: TokenKind::RBrace,
            ..
        })
    ));
    // Missing `;` between statements.
    assert!(matches!(
        parse("{ 1 2 }"),
        Err(ParserError::UnexpectedToken {
            expected: Expected::Token(TokenKind::RBrace),
            actual: TokenKind::Int,
            ..
        })
    ));
    assert!(matches!(
        parse("{ 1"),
        Err(ParserError::UnexpectedToken {
            expected: Expected::Token(TokenKind::RBrace),
            actual: TokenKind::Eof,
            ..
        })
    ));
    // `let` needs a name, `=` and `;`.
    assert!(matches!(
        parse("{ let = 1; 1 }"),
        Err(ParserError::UnexpectedToken {
            expected: Expected::Token(TokenKind::Ident),
            ..
        })
    ));
    assert!(matches!(
        parse("{ let a 1; 1 }"),
        Err(ParserError::UnexpectedToken {
            expected: Expected::Token(TokenKind::Eq),
            ..
        })
    ));
    assert!(matches!(
        parse("{ let a = 1 a }"),
        Err(ParserError::UnexpectedToken {
            expected: Expected::Token(TokenKind::Semicolon),
            ..
        })
    ));
}

#[test]
fn functions() {
    let src = "def main() = 1; def add(a, b) = a + b;";
    assert_eq!(
        parse_program(src).unwrap().to_string(),
        "AST:\nDEF main () = 1\nDEF add (a, b) = (a + b)\n"
    );
}

#[test]
fn function_parameters_and_spans() {
    let src = "def f(x, yy, zzz) = { x };";
    let ast = parse_program(src).unwrap();
    let DeclKind::FunDecl { id, params, body } = &ast.decls[0].kind;
    assert_eq!(id.name, "f");
    let names: Vec<_> = params.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["x", "yy", "zzz"]);
    for param in params {
        assert_eq!(text(src, param.span), param.name);
    }
    assert_eq!(text(src, body.span), "{ x }");
    assert_eq!(text(src, ast.decls[0].span), src);
}

#[test]
fn empty_program() {
    assert!(parse_program("").unwrap().decls.is_empty());
}

#[test]
fn program_errors() {
    // Only `def` is allowed at the top level for now.
    assert!(matches!(
        parse_program("1 + 2"),
        Err(ParserError::UnexpectedToken {
            expected: Expected::Token(TokenKind::Def),
            actual: TokenKind::Int,
            span: Span { start: 0, end: 1 },
        })
    ));
    assert!(matches!(
        parse_program("def f() = 1; let x = 2;"),
        Err(ParserError::UnexpectedToken {
            expected: Expected::Token(TokenKind::Def),
            actual: TokenKind::Let,
            ..
        })
    ));
    // A trailing comma is fine, but an empty parameter is not.
    assert!(matches!(
        parse_program("def f(a,,b) = 1"),
        Err(ParserError::UnexpectedToken {
            expected: Expected::Token(TokenKind::Ident),
            actual: TokenKind::Comma,
            span: Span { start: 8, end: 9 },
        })
    ));
    assert!(matches!(
        parse_program("def f(,) = 1"),
        Err(ParserError::UnexpectedToken {
            expected: Expected::Token(TokenKind::Ident),
            actual: TokenKind::Comma,
            ..
        })
    ));
    assert!(matches!(
        parse_program("def = 1"),
        Err(ParserError::UnexpectedToken {
            expected: Expected::Token(TokenKind::Ident),
            ..
        })
    ));
    assert!(matches!(
        parse_program("def f = 1"),
        Err(ParserError::UnexpectedToken {
            expected: Expected::Token(TokenKind::LParen),
            ..
        })
    ));
    assert!(matches!(
        parse_program("def f(a b) = 1"),
        Err(ParserError::UnexpectedToken {
            expected: Expected::Token(TokenKind::RParen),
            actual: TokenKind::Ident,
            ..
        })
    ));
    assert!(matches!(
        parse_program("def f() 1"),
        Err(ParserError::UnexpectedToken {
            expected: Expected::Token(TokenKind::Eq),
            ..
        })
    ));
}

#[test]
fn def_requires_semicolon() {
    for (src, actual) in [
        ("def f() = 1", TokenKind::Eof),
        ("def f() = 1 def g() = 2;", TokenKind::Def),
        ("def f() = { 1 }", TokenKind::Eof),
        ("def f() = 1 2;", TokenKind::Int),
    ] {
        assert!(
            matches!(
                parse_program(src),
                Err(ParserError::UnexpectedToken {
                    expected: Expected::Token(TokenKind::Semicolon),
                    actual: a,
                    ..
                }) if a == actual
            ),
            "input: {src:?}"
        );
    }
    assert_eq!(parse_program("def f() = { 1 };").unwrap().decls.len(), 1);
}

#[test]
fn every_error_has_a_span() {
    let cases = [
        ("1 + $", Span { start: 4, end: 5 }), // unknown char
        ("99999999999999999999", Span { start: 0, end: 20 }), // invalid int
        ("1 2", Span { start: 2, end: 3 }),   // unexpected token
        // At end of input, the error points right after the last token.
        ("1 +  ", Span { start: 3, end: 3 }), // eof, before trailing whitespace
        ("(1\n\n", Span { start: 2, end: 2 }), // eof, before trailing newlines
    ];
    for (src, expected) in cases {
        assert_eq!(parse(src).unwrap_err().span(), expected, "input: {src:?}");
    }
}

#[test]
fn error_display_is_the_headline() {
    let cases = [
        ("1 2", "expected end of input, found an integer"),
        ("1 +", "expected an expression, found end of input"),
        ("(1 2", "expected `)`, found an integer"),
        ("1 $", "unknown character `$`"),
        ("99999999999999999999", "integer literal is too large"),
        ("123abc", "invalid integer literal"),
    ];
    for (src, expected) in cases {
        assert_eq!(
            parse(src).unwrap_err().to_string(),
            expected,
            "input: {src:?}"
        );
    }
}

#[test]
fn diagnostics() {
    let src = "1 $ 2";
    let diag = parse(src).unwrap_err().diagnostic();
    assert_eq!(diag.message, "unknown character `$`");
    assert_eq!(
        diag.label,
        Some((Span { start: 2, end: 3 }, "not part of the language".into()))
    );

    let src = "{ 1";
    let diag = parse(src).unwrap_err().diagnostic();
    assert_eq!(diag.message, "expected `}`, found end of input");
    assert_eq!(
        diag.label,
        Some((Span { start: 3, end: 3 }, "expected `}`".into()))
    );

    let src = "99999999999999999999";
    let diag = parse(src).unwrap_err().diagnostic();
    assert_eq!(diag.notes, ["the largest integer is 9223372036854775807"]);
}
