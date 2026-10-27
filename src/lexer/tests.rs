use super::*;
use TokenKind::*;

/// Every token up to, but not including, `Eof`.
fn lex(src: &str) -> Vec<Token> {
    let mut lexer = Lexer::new(src);
    let mut tokens = Vec::new();
    loop {
        match lexer.next_token() {
            Token { kind: Eof, .. } => return tokens,
            tok => tokens.push(tok),
        }
    }
}

fn kinds(src: &str) -> Vec<TokenKind> {
    lex(src).into_iter().map(|t| t.kind).collect()
}

/// Each token's kind together with the source text its span covers.
fn tokens(src: &str) -> Vec<(TokenKind, &str)> {
    lex(src)
        .into_iter()
        .map(|t| (t.kind, &src[t.span.start..t.span.end]))
        .collect()
}

fn spans(src: &str) -> Vec<(usize, usize)> {
    lex(src)
        .into_iter()
        .map(|t| (t.span.start, t.span.end))
        .collect()
}

#[test]
fn single_tokens() {
    let cases = [
        ("+", Plus),
        ("-", Minus),
        ("*", Star),
        ("/", Slash),
        ("%", Percent),
        ("=", Eq),
        ("=>", FatArrow),
        ("==", EqEq),
        ("!=", Ne),
        ("<", Lt),
        ("<=", Le),
        (">", Gt),
        (">=", Ge),
        ("!", Bang),
        ("&&", AndAnd),
        ("||", OrOr),
        (",", Comma),
        (";", Semicolon),
        ("(", LParen),
        (")", RParen),
        ("{", LBrace),
        ("}", RBrace),
        ("let", Let),
        ("def", Def),
        ("fun", Fun),
        ("if", If),
        ("else", Else),
        ("true", True),
        ("false", False),
        ("42", Int),
        ("x", Ident),
    ];
    for (src, expected) in cases {
        assert_eq!(tokens(src), vec![(expected, src)], "input: {src:?}");
    }
}

#[test]
fn empty_and_whitespace_only() {
    assert_eq!(kinds(""), vec![]);
    assert_eq!(kinds(" "), vec![]);
    assert_eq!(kinds(" \t\r\n  \n"), vec![]);
}

#[test]
fn whitespace_between_tokens() {
    assert_eq!(
        tokens("1\t+\n2\r\n"),
        vec![(Int, "1"), (Plus, "+"), (Int, "2")]
    );
    assert_eq!(tokens("  x  "), vec![(Ident, "x")]);
}

#[test]
fn adjacent_tokens() {
    assert_eq!(tokens("1+2"), vec![(Int, "1"), (Plus, "+"), (Int, "2")]);
    assert_eq!(
        tokens("x=(y);"),
        vec![
            (Ident, "x"),
            (Eq, "="),
            (LParen, "("),
            (Ident, "y"),
            (RParen, ")"),
            (Semicolon, ";")
        ]
    );
    assert_eq!(kinds("((1))"), vec![LParen, LParen, Int, RParen, RParen]);
    assert_eq!(tokens("-5"), vec![(Minus, "-"), (Int, "5")]);
    assert_eq!(kinds("2*-3/x"), vec![Int, Star, Minus, Int, Slash, Ident]);
    assert_eq!(kinds("n%2"), vec![Ident, Percent, Int]);
}

#[test]
fn let_statement() {
    assert_eq!(
        tokens("let x = 1 + 2;"),
        vec![
            (Let, "let"),
            (Ident, "x"),
            (Eq, "="),
            (Int, "1"),
            (Plus, "+"),
            (Int, "2"),
            (Semicolon, ";")
        ]
    );
}

#[test]
fn keyword_only_matches_whole_word() {
    assert_eq!(tokens("letter"), vec![(Ident, "letter")]);
    assert_eq!(tokens("let1"), vec![(Ident, "let1")]);
    assert_eq!(tokens("le"), vec![(Ident, "le")]);
    assert_eq!(tokens("Let"), vec![(Ident, "Let")]);
    assert_eq!(kinds("let let"), vec![Let, Let]);
    assert_eq!(kinds("let(x)"), vec![Let, LParen, Ident, RParen]);
    assert_eq!(tokens("define"), vec![(Ident, "define")]);
    assert_eq!(tokens("def_"), vec![(Ident, "def_")]);
    assert_eq!(kinds("def f()"), vec![Def, Ident, LParen, RParen]);
}

#[test]
fn new_keywords_only_match_whole_words() {
    for word in [
        "trueish", "true1", "True", "falsey", "false_", "funny", "fun_", "Fun",
    ] {
        assert_eq!(tokens(word), vec![(Ident, word)], "input: {word:?}");
    }
    assert_eq!(kinds("true false"), vec![True, False]);
    assert_eq!(kinds("fun(x)"), vec![Fun, LParen, Ident, RParen]);
}

#[test]
fn arrow_is_one_token() {
    assert_eq!(tokens("=>"), vec![(FatArrow, "=>")]);
    assert_eq!(
        tokens("x => x"),
        vec![(Ident, "x"), (FatArrow, "=>"), (Ident, "x")]
    );
    // No spaces needed around it.
    assert_eq!(
        tokens("x=>x+1"),
        vec![
            (Ident, "x"),
            (FatArrow, "=>"),
            (Ident, "x"),
            (Plus, "+"),
            (Int, "1")
        ]
    );
}

#[test]
fn equals_followed_by_other_characters() {
    // `=` at the very end: peeking finds nothing.
    assert_eq!(tokens("x ="), vec![(Ident, "x"), (Eq, "=")]);
    // A space splits two-character tokens.
    assert_eq!(tokens("= >"), vec![(Eq, "="), (Gt, ">")]);
    assert_eq!(tokens("= ="), vec![(Eq, "="), (Eq, "=")]);
    // Longest match from the left.
    assert_eq!(tokens("=>="), vec![(FatArrow, "=>"), (Eq, "=")]);
    assert_eq!(tokens("==>"), vec![(EqEq, "=="), (Gt, ">")]);
    assert_eq!(tokens("=>>"), vec![(FatArrow, "=>"), (Gt, ">")]);
    assert_eq!(tokens("==="), vec![(EqEq, "=="), (Eq, "=")]);
    assert_eq!(
        tokens("let f = x => x;"),
        vec![
            (Let, "let"),
            (Ident, "f"),
            (Eq, "="),
            (Ident, "x"),
            (FatArrow, "=>"),
            (Ident, "x"),
            (Semicolon, ";"),
        ]
    );
}

#[test]
fn two_character_operators_take_both_characters() {
    for (src, kind) in [
        ("a==b", EqEq),
        ("a!=b", Ne),
        ("a<=b", Le),
        ("a>=b", Ge),
        ("a&&b", AndAnd),
        ("a||b", OrOr),
    ] {
        let op = &src[1..3];
        assert_eq!(
            tokens(src),
            vec![(Ident, "a"), (kind, op), (Ident, "b")],
            "input: {src:?}"
        );
    }
}

#[test]
fn one_character_operators() {
    assert_eq!(tokens("a<b"), vec![(Ident, "a"), (Lt, "<"), (Ident, "b")]);
    assert_eq!(tokens("a>b"), vec![(Ident, "a"), (Gt, ">"), (Ident, "b")]);
    assert_eq!(tokens("!a"), vec![(Bang, "!"), (Ident, "a")]);
    assert_eq!(tokens("!!a"), vec![(Bang, "!"), (Bang, "!"), (Ident, "a")]);
    // `<` followed by something other than `=`.
    assert_eq!(tokens("<-1"), vec![(Lt, "<"), (Minus, "-"), (Int, "1")]);
}

#[test]
fn comparison_operators_longest_match() {
    assert_eq!(tokens("<=="), vec![(Le, "<="), (Eq, "=")]);
    assert_eq!(tokens("!=="), vec![(Ne, "!="), (Eq, "=")]);
    assert_eq!(tokens("<=>"), vec![(Le, "<="), (Gt, ">")]);
    assert_eq!(tokens(">=="), vec![(Ge, ">="), (Eq, "=")]);
    assert_eq!(tokens("! ="), vec![(Bang, "!"), (Eq, "=")]);
    assert_eq!(tokens("< ="), vec![(Lt, "<"), (Eq, "=")]);
}

#[test]
fn single_bar_and_ampersand_are_unknown() {
    assert_eq!(tokens("|"), vec![(Unknown, "|")]);
    assert_eq!(tokens("&"), vec![(Unknown, "&")]);
    assert_eq!(tokens("| |"), vec![(Unknown, "|"), (Unknown, "|")]);
    assert_eq!(tokens("|||"), vec![(OrOr, "||"), (Unknown, "|")]);
    assert_eq!(tokens("&&&"), vec![(AndAnd, "&&"), (Unknown, "&")]);
    assert_eq!(tokens("&|"), vec![(Unknown, "&"), (Unknown, "|")]);
}

#[test]
fn if_and_else_are_keywords() {
    assert_eq!(
        kinds("if a { 1 } else { 2 }"),
        vec![If, Ident, LBrace, Int, RBrace, Else, LBrace, Int, RBrace]
    );
    assert_eq!(kinds("else if"), vec![Else, If]);
    for word in ["iff", "if_", "If", "elsewhere", "else1", "Else"] {
        assert_eq!(tokens(word), vec![(Ident, word)], "input: {word:?}");
    }
}

#[test]
fn condition_shaped_input() {
    assert_eq!(
        tokens("!(a<=1)||b!=c&&d"),
        vec![
            (Bang, "!"),
            (LParen, "("),
            (Ident, "a"),
            (Le, "<="),
            (Int, "1"),
            (RParen, ")"),
            (OrOr, "||"),
            (Ident, "b"),
            (Ne, "!="),
            (Ident, "c"),
            (AndAnd, "&&"),
            (Ident, "d"),
        ]
    );
}

#[test]
fn lambda_shaped_input() {
    assert_eq!(
        kinds("fun(a, b) => { a }"),
        vec![
            Fun, LParen, Ident, Comma, Ident, RParen, FatArrow, LBrace, Ident, RBrace
        ]
    );
    assert_eq!(
        spans("fun(a) => a"),
        vec![(0, 3), (3, 4), (4, 5), (5, 6), (7, 9), (10, 11)]
    );
}

#[test]
fn identifiers() {
    assert_eq!(tokens("x1"), vec![(Ident, "x1")]);
    assert_eq!(tokens("abc123def"), vec![(Ident, "abc123def")]);
    assert_eq!(tokens("foo bar"), vec![(Ident, "foo"), (Ident, "bar")]);
}

#[test]
fn identifiers_are_ascii_only() {
    assert_eq!(tokens("é"), vec![(Unknown, "é")]);
    assert_eq!(tokens("日本"), vec![(Unknown, "日"), (Unknown, "本")]);
    assert_eq!(
        tokens("aéb"),
        vec![(Ident, "a"), (Unknown, "é"), (Ident, "b")]
    );
}

#[test]
fn underscores_in_identifiers() {
    assert_eq!(tokens("a_b"), vec![(Ident, "a_b")]);
    assert_eq!(tokens("x_"), vec![(Ident, "x_")]);
    assert_eq!(tokens("a__1"), vec![(Ident, "a__1")]);
    // An identifier can start with `_`, or be just underscores.
    assert_eq!(tokens("_a"), vec![(Ident, "_a")]);
    assert_eq!(tokens("_1"), vec![(Ident, "_1")]);
    assert_eq!(tokens("_"), vec![(Ident, "_")]);
    assert_eq!(tokens("__"), vec![(Ident, "__")]);
    // A leading `_` does not make a keyword an identifier, and does not
    // stop one from being a different identifier.
    assert_eq!(tokens("_let"), vec![(Ident, "_let")]);
    assert_eq!(kinds("let _x"), vec![Let, Ident]);
}

#[test]
fn number_followed_by_letters_is_one_token() {
    // The parser rejects these as invalid integer literals.
    assert_eq!(tokens("123abc"), vec![(Int, "123abc")]);
    assert_eq!(tokens("2let"), vec![(Int, "2let")]);
    assert_eq!(
        tokens("1x2 + 3"),
        vec![(Int, "1x2"), (Plus, "+"), (Int, "3")]
    );
    // `_` belongs to the number too, so `1_000` is one (invalid) literal,
    // not `1` followed by the identifier `_000`.
    assert_eq!(tokens("1_000"), vec![(Int, "1_000")]);
    assert_eq!(tokens("1_"), vec![(Int, "1_")]);
    assert_eq!(tokens("1__a2"), vec![(Int, "1__a2")]);
    assert_eq!(
        tokens("1_0 + _1"),
        vec![(Int, "1_0"), (Plus, "+"), (Ident, "_1")]
    );
}

#[test]
fn integers() {
    assert_eq!(tokens("0"), vec![(Int, "0")]);
    assert_eq!(tokens("007"), vec![(Int, "007")]);
    assert_eq!(kinds("1 2 3"), vec![Int, Int, Int]);
}

#[test]
fn integers_are_not_range_checked() {
    // Overflow is the parser's job; the lexer only finds the digits.
    let src = "1 + 99999999999999999999";
    assert_eq!(lex(src)[2], Token::new(Int, 4, 24));
}

#[test]
fn unknown_char() {
    assert_eq!(lex("1 $ 2")[1], Token::new(Unknown, 2, 3));
}

#[test]
fn lexing_continues_after_unknown_char() {
    assert_eq!(kinds("1 $ 2"), vec![Int, Unknown, Int]);
    assert_eq!(kinds("$$"), vec![Unknown, Unknown]);
}

#[test]
fn unknown_chars() {
    for c in ['$', '.', '|', '&', '٣', '€', 'é'] {
        let src = c.to_string();
        assert_eq!(tokens(&src), vec![(Unknown, src.as_str())], "input: {c:?}");
    }
}

#[test]
fn unknown_multibyte_char_span() {
    assert_eq!(lex("1€")[1], Token::new(Unknown, 1, 4));
}

#[test]
fn token_spans() {
    assert_eq!(spans("let x = 42"), vec![(0, 3), (4, 5), (6, 7), (8, 10)]);
    assert_eq!(spans("(1)"), vec![(0, 1), (1, 2), (2, 3)]);
    assert_eq!(spans("  foo  "), vec![(2, 5)]);
}

#[test]
fn spans_are_byte_offsets() {
    // 'é' is two bytes in UTF-8, each CJK character three.
    assert_eq!(spans("é + 1"), vec![(0, 2), (3, 4), (5, 6)]);
    assert_eq!(spans("日本 = 1"), vec![(0, 3), (3, 6), (7, 8), (9, 10)]);
}

#[test]
fn non_ascii_whitespace_is_not_skipped() {
    // U+00A0 NO-BREAK SPACE is two bytes.
    assert_eq!(
        tokens("1\u{a0}2"),
        vec![(Int, "1"), (Unknown, "\u{a0}"), (Int, "2")]
    );
}

#[test]
fn comments_are_skipped() {
    assert_eq!(kinds("// only a comment"), vec![]);
    assert_eq!(kinds("//"), vec![]);
    assert_eq!(kinds("// a\n// b\n\n   // c\n"), vec![]);
    assert_eq!(tokens("1 // one"), vec![(Int, "1")]);
    assert_eq!(
        tokens("1 // one\n+ 2"),
        vec![(Int, "1"), (Plus, "+"), (Int, "2")]
    );
    assert_eq!(
        tokens("// first\nlet x = 1; // trailing\n// last"),
        vec![
            (Let, "let"),
            (Ident, "x"),
            (Eq, "="),
            (Int, "1"),
            (Semicolon, ";")
        ]
    );
}

#[test]
fn comment_ends_at_newline() {
    // `\r\n` endings: the `\r` is inside the comment, `\n` ends it.
    assert_eq!(tokens("1 // c\r\n2"), vec![(Int, "1"), (Int, "2")]);
    // Anything up to the newline is ignored, even `//` again or code.
    assert_eq!(kinds("// x // y + 1\nz"), vec![Ident]);
    assert_eq!(kinds("///"), vec![]);
    assert_eq!(kinds("//// banner ////\n1"), vec![Int]);
}

#[test]
fn comments_may_contain_anything() {
    assert_eq!(kinds("// é € 日本 $ \u{a0}\n1"), vec![Int]);
    // Spans after a non-ASCII comment are still byte offsets.
    assert_eq!(spans("// é\nfoo"), vec![(6, 9)]);
}

#[test]
fn slash_is_still_division() {
    assert_eq!(kinds("a / b"), vec![Ident, Slash, Ident]);
    assert_eq!(kinds("a/b"), vec![Ident, Slash, Ident]);
    assert_eq!(kinds("/"), vec![Slash]);
    assert_eq!(kinds("1 / // halve\n2"), vec![Int, Slash, Int]);
    // Two slashes always start a comment, even with no space before.
    assert_eq!(kinds("a//b"), vec![Ident]);
    assert_eq!(kinds("a/ /b"), vec![Ident, Slash, Slash, Ident]);
}

#[test]
fn spans_after_comments() {
    assert_eq!(spans("// x\nfoo"), vec![(5, 8)]);
    assert_eq!(spans("a // x\n  b"), vec![(0, 1), (9, 10)]);
}

#[test]
fn eof_after_trailing_comment() {
    // `Eof` is at the very end of the input, after the comment.
    let src = "1 // done";
    let mut lexer = Lexer::new(src);
    assert_eq!(lexer.next_token().kind, Int);
    assert_eq!(lexer.next_token(), Token::new(Eof, src.len(), src.len()));
}

const STR: TokenKind = Str { terminated: true };
const UNTERMINATED: TokenKind = Str { terminated: false };

#[test]
fn strings() {
    assert_eq!(tokens(r#""abc""#), vec![(STR, r#""abc""#)]);
    assert_eq!(tokens(r#""""#), vec![(STR, r#""""#)]);
    assert_eq!(tokens(r#"" ""#), vec![(STR, r#"" ""#)]);
    // The token is the raw source text; escapes are the parser's job.
    assert_eq!(tokens(r#""a\nb""#), vec![(STR, r#""a\nb""#)]);
}

#[test]
fn strings_between_tokens() {
    assert_eq!(
        tokens(r#""" + 1"#),
        vec![(STR, r#""""#), (Plus, "+"), (Int, "1")]
    );
    assert_eq!(
        tokens(r#"f("a","b")"#),
        vec![
            (Ident, "f"),
            (LParen, "("),
            (STR, r#""a""#),
            (Comma, ","),
            (STR, r#""b""#),
            (RParen, ")")
        ]
    );
    assert_eq!(kinds(r#""a""b""#), vec![STR, STR]);
    assert_eq!(kinds(r#"x"y"z"#), vec![Ident, STR, Ident]);
}

#[test]
fn escapes_do_not_end_the_string() {
    // An escaped quote is content, not the closing quote.
    assert_eq!(tokens(r#""a\"b""#), vec![(STR, r#""a\"b""#)]);
    assert_eq!(tokens(r#""\"""#), vec![(STR, r#""\"""#)]);
    // An escaped backslash does not escape the quote after it.
    assert_eq!(tokens(r#""\\" x"#), vec![(STR, r#""\\""#), (Ident, "x")]);
    assert_eq!(tokens(r#""\\\"""#), vec![(STR, r#""\\\"""#)]);
    // Unknown escapes are still skipped as a pair; the parser rejects them.
    assert_eq!(tokens(r#""\q" 1"#), vec![(STR, r#""\q""#), (Int, "1")]);
}

#[test]
fn unterminated_strings() {
    assert_eq!(tokens(r#"""#), vec![(UNTERMINATED, r#"""#)]);
    assert_eq!(tokens(r#""abc"#), vec![(UNTERMINATED, r#""abc"#)]);
    // The closing quote is escaped, so the string never ends.
    assert_eq!(tokens(r#""\""#), vec![(UNTERMINATED, r#""\""#)]);
    assert_eq!(tokens(r#""\"#), vec![(UNTERMINATED, r#""\"#)]);
}

#[test]
fn strings_stop_at_newline() {
    // No raw newlines in strings: a missing quote ends the string at the
    // line end instead of swallowing the rest of the file.
    assert_eq!(
        tokens("\"abc\nx = 1;"),
        vec![
            (UNTERMINATED, "\"abc"),
            (Ident, "x"),
            (Eq, "="),
            (Int, "1"),
            (Semicolon, ";")
        ]
    );
    assert_eq!(tokens("\"\n1"), vec![(UNTERMINATED, "\""), (Int, "1")]);
    assert_eq!(
        tokens("\"ab\r\n1"),
        vec![(UNTERMINATED, "\"ab\r"), (Int, "1")]
    );
    // A backslash cannot escape the newline either.
    assert_eq!(
        tokens("\"a\\\n1"),
        vec![(UNTERMINATED, "\"a\\"), (Int, "1")]
    );
}

#[test]
fn strings_and_comments() {
    // `//` inside a string is content, not a comment.
    assert_eq!(
        tokens(r#""http://x" 1"#),
        vec![(STR, r#""http://x""#), (Int, "1")]
    );
    // A quote inside a comment does not start a string.
    assert_eq!(kinds("// say \"hi\n1"), vec![Int]);
    assert_eq!(kinds("\"a\" // \"b\""), vec![STR]);
}

#[test]
fn string_spans_are_byte_offsets() {
    // `é` is two bytes and `€` three.
    assert_eq!(spans(r#""é€" 1"#), vec![(0, 7), (8, 9)]);
    assert_eq!(tokens(r#""日本""#), vec![(STR, r#""日本""#)]);
    assert_eq!(spans("\"é\n1"), vec![(0, 3), (4, 5)]);
}

#[test]
fn many_comment_lines() {
    // Skipped in a loop: no recursion per comment.
    let src = "// line\n".repeat(100_000) + "1";
    assert_eq!(kinds(&src), vec![Int]);
}

#[test]
fn next_token_returns_eof_at_end() {
    let mut lexer = Lexer::new("1  ");
    assert_eq!(lexer.next_token().kind, Int);
    for _ in 0..2 {
        assert_eq!(lexer.next_token(), Token::new(Eof, 3, 3));
    }
    assert_eq!(Lexer::new("").next_token(), Token::new(Eof, 0, 0));
}

#[test]
fn token_kind_display() {
    let cases = [
        (Int, "an integer"),
        (STR, "a string"),
        (UNTERMINATED, "a string"),
        (Ident, "an identifier"),
        (Let, "`let`"),
        (Def, "`def`"),
        (Fun, "`fun`"),
        (True, "`true`"),
        (False, "`false`"),
        (Eq, "`=`"),
        (FatArrow, "`=>`"),
        (If, "`if`"),
        (Else, "`else`"),
        (Bang, "`!`"),
        (EqEq, "`==`"),
        (Ne, "`!=`"),
        (Lt, "`<`"),
        (Le, "`<=`"),
        (Gt, "`>`"),
        (Ge, "`>=`"),
        (OrOr, "`||`"),
        (AndAnd, "`&&`"),
        (Plus, "`+`"),
        (Minus, "`-`"),
        (Star, "`*`"),
        (Slash, "`/`"),
        (Percent, "`%`"),
        (Comma, "`,`"),
        (Semicolon, "`;`"),
        (LParen, "`(`"),
        (RParen, "`)`"),
        (LBrace, "`{`"),
        (RBrace, "`}`"),
        (Unknown, "an unknown character"),
        (Eof, "end of input"),
    ];
    for (kind, expected) in cases {
        assert_eq!(kind.to_string(), expected);
    }
}

/// Exhaustively lex every string up to length 4 over a small alphabet and
/// check invariants that must hold for any input.
#[test]
fn invariants_on_all_short_inputs() {
    const ALPHABET: [char; 21] = [
        'a', 'l', 'e', 't', '_', '1', '+', ';', '=', '>', '<', '!', '|', '&', ' ', 'é', '$', '/',
        '\n', '"', '\\',
    ];

    let mut inputs = vec![String::new()];
    let mut frontier = vec![String::new()];
    for _ in 0..4 {
        frontier = frontier
            .iter()
            .flat_map(|s| ALPHABET.iter().map(move |c| format!("{s}{c}")))
            .collect();
        inputs.extend(frontier.iter().cloned());
    }

    for src in &inputs {
        let mut prev_end = 0;
        for tok in lex(src) {
            let span = tok.span;
            assert!(span.start < span.end, "empty span in {src:?}");
            assert!(span.start >= prev_end, "overlapping spans in {src:?}");
            assert!(span.end <= src.len(), "span out of bounds in {src:?}");
            prev_end = span.end;

            // Panics if the span is not on char boundaries.
            let text = &src[span.start..span.end];
            // No token spans lines, and comments never end up inside one
            // (though a string may contain `//`).
            assert!(!text.contains('\n'), "newline in token {text:?} in {src:?}");
            if !matches!(tok.kind, Str { .. }) {
                assert!(!text.contains("//"), "comment in token {text:?} in {src:?}");
            }
            assert!(!text.trim().is_empty(), "whitespace-only token in {src:?}");

            match tok.kind {
                Int => {
                    assert!(text.starts_with(|c: char| c.is_ascii_digit()));
                    assert!(
                        text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'),
                        "{text:?}"
                    );
                }
                Ident => {
                    assert!(text.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_'));
                    assert!(text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'));
                    let keywords = ["let", "def", "fun", "true", "false", "if", "else"];
                    assert!(!keywords.contains(&text), "{text:?}");
                }
                Let => assert_eq!(text, "let"),
                Def => assert_eq!(text, "def"),
                FatArrow => assert_eq!(text, "=>"),
                EqEq => assert_eq!(text, "=="),
                Ne => assert_eq!(text, "!="),
                Le => assert_eq!(text, "<="),
                Ge => assert_eq!(text, ">="),
                Slash => assert_eq!(text, "/"),
                Str { terminated: true } => {
                    assert!(text.len() >= 2 && text.starts_with('"') && text.ends_with('"'));
                    // The closing quote is not escaped: an even number of
                    // backslashes come right before it.
                    let body = &text[1..text.len() - 1];
                    let trailing = body.len() - body.trim_end_matches('\\').len();
                    assert!(trailing % 2 == 0, "escaped closing quote in {text:?}");
                }
                Str { terminated: false } => {
                    assert!(text.starts_with('"'), "{text:?}");
                    // It stopped at the end of the line or of the input.
                    let next = src[span.end..].chars().next();
                    assert!(matches!(next, None | Some('\n')), "{text:?} in {src:?}");
                }
                OrOr => assert_eq!(text, "||"),
                AndAnd => assert_eq!(text, "&&"),
                _ => assert_eq!(text.chars().count(), 1, "{text:?}"),
            }
        }
    }
}
