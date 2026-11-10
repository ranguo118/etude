use super::*;
use crate::lexer::Lexer;
use crate::parser::Parser;

/// The errors `check` reports for `src`, which must parse.
fn errors(src: &str) -> Vec<SemError> {
    let ast = Parser::new(Lexer::new(src)).parse().unwrap();
    check(&ast).err().unwrap_or_default()
}

fn text(src: &str, span: Span) -> &str {
    &src[span.start..span.end]
}

/// The byte offset of the `n`th (from 0) occurrence of `needle` in `src`.
fn nth(src: &str, needle: &str, n: usize) -> usize {
    src.match_indices(needle).nth(n).unwrap().0
}

#[test]
fn valid_programs_pass() {
    for src in [
        "def main() = 1;",
        // Definitions see each other in any order.
        "def main() = f(1); def f(x) = g(x); def g(x) = x;",
        "def main() = println(int_to_str(1));",
        "def main() = { let x = 1; let x = x + 1; x };",
        "def main() = fun(x) => fun(y) => x + y;",
        "def f(a, b) = a; def main() = f(1, 2);",
        // A local may shadow a builtin or a global.
        "def f() = 1; def main() = { let println = 2; let f = 3; println + f };",
    ] {
        assert_eq!(errors(src), [], "{src}");
    }
}

#[test]
fn unbound_variables_are_found_without_running() {
    // `y` is in a branch that never runs.
    let src = "def main() = if false { y } else { 1 };";
    let [SemError::UnboundIdent { name, span }] = &errors(src)[..] else {
        panic!("{:?}", errors(src));
    };
    assert_eq!(name, "y");
    assert_eq!(text(src, *span), "y");
}

#[test]
fn scopes_end_with_their_block_or_function() {
    let src = "def main() = { { let x = 1; x }; x };";
    let errs = errors(src);
    assert!(
        matches!(&errs[..], [SemError::UnboundIdent { span, .. }] if span.start == nth(src, "x", 2)),
        "{errs:?}"
    );
    // Parameters are visible only in their own function: no dynamic scope.
    let src = "def f() = n; def main() = { let n = 1; f() };";
    assert!(matches!(&errors(src)[..], [SemError::UnboundIdent { name, .. }] if name == "n"));
    let src = "def main() = { let f = fun(a) => a; a };";
    assert!(matches!(&errors(src)[..], [SemError::UnboundIdent { name, .. }] if name == "a"));
}

#[test]
fn a_closure_sees_only_what_was_bound_before_it() {
    let src = "def main() = { let f = fun() => y; let y = 1; f() };";
    assert!(matches!(&errors(src)[..], [SemError::UnboundIdent { name, .. }] if name == "y"));
    // A `let` is not in scope in its own initializer.
    let src = "def main() = { let g = fun(n) => g(n); 1 };";
    assert!(matches!(&errors(src)[..], [SemError::UnboundIdent { name, .. }] if name == "g"));
    let src = "def main() = { let x = x; 1 };";
    assert!(matches!(&errors(src)[..], [SemError::UnboundIdent { name, .. }] if name == "x"));
}

#[test]
fn duplicate_definitions() {
    let src = "def f() = 1; def main() = f(); def f() = 2;";
    let [SemError::DuplicateDef { name, span, first }] = &errors(src)[..] else {
        panic!("{:?}", errors(src));
    };
    assert_eq!(name, "f");
    assert_eq!(span.start, nth(src, "f()", 2));
    assert_eq!(first.start, nth(src, "f()", 0));
    // Each extra definition is reported against the first.
    let src = "def f() = 1; def f() = 2; def f() = 3; def main() = 1;";
    let errs = errors(src);
    assert_eq!(errs.len(), 2, "{errs:?}");
    assert!(errs.iter().all(
        |e| matches!(e, SemError::DuplicateDef { first, .. } if first.start == nth(src, "f()", 0))
    ));
}

#[test]
fn definitions_cannot_reuse_builtin_names() {
    let src = "def main() = 1; def println(s) = s;";
    let [SemError::RedefinesBuiltin { name, span }] = &errors(src)[..] else {
        panic!("{:?}", errors(src));
    };
    assert_eq!(name, "println");
    assert_eq!(text(src, *span), "println");
}

#[test]
fn duplicate_parameters() {
    let src = "def f(a, b, a) = a; def main() = 1;";
    let [SemError::DuplicateParam { name, span, first }] = &errors(src)[..] else {
        panic!("{:?}", errors(src));
    };
    assert_eq!(name, "a");
    assert_eq!(span.start, nth(src, "a", 1));
    assert_eq!(first.start, nth(src, "a", 0));
    let src = "def main() = fun(x, x) => x;";
    assert!(matches!(&errors(src)[..], [SemError::DuplicateParam { name, .. }] if name == "x"));
    // The same name in different lists is fine.
    assert_eq!(errors("def main() = fun(x) => fun(x) => x;"), []);
}

#[test]
fn every_error_is_reported() {
    let src = "def f(a, a) = b; def f() = 1; def main() = c;";
    let errs = errors(src);
    assert!(
        matches!(
            &errs[..],
            [
                SemError::DuplicateParam { .. },
                SemError::DuplicateDef { .. },
                SemError::UnboundIdent { name: b, .. },
                SemError::UnboundIdent { name: c, .. },
            ] if b == "b" && c == "c"
        ),
        "{errs:?}"
    );
}

#[test]
fn error_display_is_the_headline() {
    let errs = errors("def main() = x;");
    assert_eq!(errs[0].to_string(), "cannot find `x` in this scope");
}
