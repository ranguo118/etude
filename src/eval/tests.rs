use super::*;
use crate::lexer::Lexer;
use crate::parser::Parser;

// Values borrow from the AST, so the helpers keep the AST alive and
// return only owned results.

fn error(src: &str) -> EvalError {
    let ast = Parser::new(Lexer::new(src)).parse().unwrap();
    eval_with_output(&ast, &mut io::sink()).unwrap_err()
}

fn text(src: &str, span: Span) -> &str {
    &src[span.start..span.end]
}

#[test]
fn main_must_exist() {
    assert_eq!(error("def f() = 1;"), EvalError::MissingMain);
    assert_eq!(error(""), EvalError::MissingMain);
}

/// Run `src` with a lower call limit, so tests stay small and fast.
fn eval_with_max_depth(src: &str, max_depth: usize) -> Result<String, EvalError> {
    let ast = Parser::new(Lexer::new(src)).parse().unwrap();
    TreeEval::with_max_depth(&mut io::sink(), max_depth)
        .eval(&ast)
        .map(|v| v.to_string())
}

#[test]
fn infinite_recursion_hits_the_limit() {
    let src = "def f(n) = f(n + 1); def main() = f(0);";
    let EvalError::RecursionLimit { limit, span } = error(src) else {
        panic!("expected RecursionLimit");
    };
    assert_eq!(limit, MAX_CALL_DEPTH);
    // The call that went over is the recursive one, not the one in `main`.
    assert_eq!(span.start, src.find("f(n + 1)").unwrap());
    assert_eq!(text(src, span), "f(n + 1)");
}

#[test]
fn recursion_up_to_the_limit_is_fine() {
    let count = "def f(n) = if n == 0 { 0 } else { 1 + f(n - 1) };";
    // `f(n)` makes n + 1 calls.
    let src = format!("{count} def main() = f({});", MAX_CALL_DEPTH - 1);
    assert_eq!(printed(&src), (MAX_CALL_DEPTH - 1).to_string());
    let src = format!("{count} def main() = f({});", MAX_CALL_DEPTH);
    assert!(matches!(error(&src), EvalError::RecursionLimit { .. }));
}

#[test]
fn depth_counts_calls_in_progress_not_calls_made() {
    // Each `f(9)` returns before the next starts, so the depth goes back
    // down and 30 calls in total fit under a limit of 10.
    let src = "def f(n) = if n == 0 { 0 } else { 1 + f(n - 1) };
               def main() = f(9) + f(9) + f(9);";
    assert_eq!(eval_with_max_depth(src, 10).unwrap(), "27");
    assert!(matches!(
        eval_with_max_depth(
            "def f(n) = if n == 0 { 0 } else { f(n - 1) }; def main() = f(10);",
            10
        ),
        Err(EvalError::RecursionLimit { limit: 10, .. })
    ));
}

#[test]
fn mutual_and_lambda_recursion_count_too() {
    let src = "def ping(n) = pong(n + 1); def pong(n) = ping(n + 1); def main() = ping(0);";
    assert!(matches!(
        eval_with_max_depth(src, 50),
        Err(EvalError::RecursionLimit { limit: 50, .. })
    ));
    // Recursion through a function passed as an argument.
    let src = "def apply(g, x) = g(g, x); def main() = apply(fun(self, n) => self(self, n), 0);";
    assert!(matches!(
        eval_with_max_depth(src, 50),
        Err(EvalError::RecursionLimit { limit: 50, .. })
    ));
}

#[test]
fn deeply_nested_bodies_do_not_overflow_the_stack() {
    // Each call nests 30 blocks and `if`s, many native frames per call.
    // Without stack growth this overflows long before the limit.
    let mut body = String::from("f(n + 1)");
    for i in 0..30 {
        body = format!("{{ let x{i} = 1; if true {{ 1 + ({body}) }} else {{ 0 }} }}");
    }
    let src = format!("def f(n) = {body}; def main() = f(0);");
    assert!(matches!(
        eval_with_max_depth(&src, 1_000),
        Err(EvalError::RecursionLimit { limit: 1_000, .. })
    ));
}

#[test]
fn recursion_limit_diagnostic() {
    let diag = EvalError::RecursionLimit {
        limit: 10,
        span: Span { start: 0, end: 1 },
    }
    .diagnostic();
    assert_eq!(diag.message, "recursion limit reached");
    assert_eq!(
        diag.label.unwrap().1,
        "this call would exceed 10 nested calls"
    );
    assert_eq!(diag.notes.len(), 1);
}

#[test]
fn main_must_not_take_parameters() {
    let src = "def main(a, bc) = a;";
    let EvalError::MainHasParams(span) = error(src) else {
        panic!("expected MainHasParams");
    };
    assert_eq!(text(src, span), "a, bc");
}

#[test]
fn division_by_zero_points_at_divisor() {
    let src = "def main() = 1 / (2 - 2);";
    let EvalError::DivisionByZero(span) = error(src) else {
        panic!("expected DivisionByZero");
    };
    assert_eq!(text(src, span), "(2 - 2)");
}

#[test]
fn negation_overflow_points_at_whole_negation() {
    let src = "def main() = -(-9223372036854775807 - 1);";
    let EvalError::IntegerOverflow(span) = error(src) else {
        panic!("expected IntegerOverflow");
    };
    assert_eq!(text(src, span), "-(-9223372036854775807 - 1)");
}

#[test]
fn type_mismatch_reports_actual_type() {
    // Calling an integer: the callee is underlined.
    let src = "def main() = { let x = 1; x(2) };";
    assert!(matches!(
        error(src),
        EvalError::TypeMismatch { expected: Type::Fun, actual: Type::Int, span }
            if text(src, span) == "x"
    ));
    // Using a function as a number.
    let src = "def f() = 1; def main() = f + 1;";
    assert!(matches!(
        error(src),
        EvalError::TypeMismatch { expected: Type::Int, actual: Type::Fun, span }
            if text(src, span) == "f"
    ));
    let src = "def f() = 1; def main() = -f;";
    assert!(matches!(
        error(src),
        EvalError::TypeMismatch { expected: Type::Int, actual: Type::Fun, span }
            if text(src, span) == "f"
    ));
}

#[test]
fn diagnostics() {
    let src = "def main() = x;";
    let diag = error(src).diagnostic();
    assert_eq!(diag.message, "cannot find `x` in this scope");

    let src = "def f(a) = a; def main() = f(1, 2);";
    let diag = error(src).diagnostic();
    assert_eq!(diag.message, "wrong number of arguments");
    let (span, label) = diag.label.unwrap();
    assert_eq!(text(src, span), "f(1, 2)");
    assert_eq!(label, "expected 1 argument, found 2");

    let src = "def f(a, b) = a; def main() = f(1);";
    let (_, label) = error(src).diagnostic().label.unwrap();
    assert_eq!(label, "expected 2 arguments, found 1");

    let diag = EvalError::MissingMain.diagnostic();
    assert_eq!(diag.label, None);
    assert_eq!(diag.notes.len(), 1);
}

fn value(src: &str) -> i64 {
    let ast = Parser::new(Lexer::new(src)).parse().unwrap();
    match eval(&ast) {
        Ok(Value::Int(n)) => n,
        other => panic!("expected an integer, got {other:?}"),
    }
}

#[test]
fn let_shadows_in_the_same_block() {
    assert_eq!(value("def main() = { let x = 1; let x = x + 10; x };"), 11);
}

#[test]
fn inner_block_bindings_end_with_the_block() {
    let src = "def main() = { let x = 1; let y = { let x = 100; x }; x + y };";
    assert_eq!(value(src), 101);
}

#[test]
fn locals_shadow_globals() {
    assert_eq!(value("def x() = 1; def main() = { let x = 5; x };"), 5);
    // Parameters too.
    assert_eq!(value("def x() = 1; def f(x) = x; def main() = f(7);"), 7);
}

#[test]
fn functions_do_not_see_the_callers_locals() {
    let src = "def f() = y; def main() = { let y = 1; f() };";
    assert!(matches!(error(src), EvalError::UnboundIdent { name, .. } if name == "y"));
}

#[test]
fn arguments_are_evaluated_in_the_callers_scope() {
    let src = "def f(a) = a * 2; def main() = { let a = 21; f(a) };";
    assert_eq!(value(src), 42);
}

#[test]
fn functions_are_values() {
    let src = "def inc(x) = x + 1; def twice(f, x) = f(f(x)); def main() = twice(inc, 5);";
    assert_eq!(value(src), 7);
}

/// There are no lambdas yet, so build a closure that captured `x = 5`
/// by hand and check that calling it sees `x`.
#[test]
fn calls_start_from_the_captured_environment() {
    let body_src = "def main() = x + 1;";
    let ast = Parser::new(Lexer::new(body_src)).parse().unwrap();
    let DeclKind::FunDecl { body, .. } = &ast.decls[0].kind;

    let x = Ident {
        name: "x".into(),
        span: Span { start: 0, end: 0 },
    };
    let params = vec![];
    let captured = Env::empty().bind(&x, Value::Int(5));
    let closure = Value::Fun(Rc::new(Closure {
        params: &params,
        body,
        env: captured,
    }));

    let mut sink = io::sink();
    let mut interp = TreeEval::new(&mut sink);
    interp.globals.insert("g", closure);
    let call = Parser::new(Lexer::new("def main() = g();"))
        .parse()
        .unwrap();
    let DeclKind::FunDecl { body: call, .. } = &call.decls[0].kind;
    let result = interp.eval_expr(call, &Env::empty()).unwrap();
    assert!(matches!(result, Value::Int(6)), "got {result:?}");
}

#[test]
fn lambda_captures_its_environment() {
    let src = "def main() = { let a = 10; let f = fun(x) => x + a; f(1) };";
    assert_eq!(value(src), 11);
}

#[test]
fn captured_parameter_outlives_the_call() {
    // `z` belongs to `make`'s call, which has returned by the time
    // `inc` runs.
    let src = "def make(z) = fun(x, y) => x + y + z;
               def main() = { let inc = make(1); inc(10000, 2) };";
    assert_eq!(value(src), 10003);
}

#[test]
fn capture_is_not_affected_by_later_shadowing() {
    let src = "def main() = { let x = 1; let f = fun() => x; let x = 2; f() };";
    assert_eq!(value(src), 1);
}

#[test]
fn lambda_does_not_see_later_bindings() {
    let src = "def main() = { let f = fun() => y; let y = 2; f() };";
    assert!(matches!(error(src), EvalError::UnboundIdent { name, .. } if name == "y"));
}

#[test]
fn lambda_sees_where_it_was_written_not_where_it_is_called() {
    // `apply` has its own `z`, but the lambda captured `main`'s.
    let src = "def apply(f) = { let z = 100; f() };
               def main() = { let z = 1; apply(fun() => z) };";
    assert_eq!(value(src), 1);
}

#[test]
fn currying() {
    assert_eq!(
        value("def add(a) = fun(b) => a + b; def main() = add(3)(4);"),
        7
    );
    let src = "def main() = { let a = 1; let f = fun(b) => fun(c) => a + b + c; f(10)(100) };";
    assert_eq!(value(src), 111);
}

#[test]
fn lambdas_as_arguments() {
    let src = "def twice(f, x) = f(f(x)); def main() = twice(fun(n) => n * 3, 2);";
    assert_eq!(value(src), 18);
}

#[test]
fn calling_a_lambda_directly() {
    assert_eq!(value("def main() = (fun(x) => x + 1)(41);"), 42);
    assert_eq!(value("def main() = (fun() => 7)();"), 7);
}

#[test]
fn lambda_argument_count_is_checked() {
    let src = "def main() = (fun(x) => x)(1, 2);";
    let EvalError::ArgumentCountMismatch {
        expected: 1,
        actual: 2,
        span,
    } = error(src)
    else {
        panic!("expected ArgumentCountMismatch");
    };
    assert_eq!(text(src, span), "(fun(x) => x)(1, 2)");
}

#[test]
fn a_lambda_is_a_value() {
    let src = "def main() = fun(x) => x;";
    let ast = Parser::new(Lexer::new(src)).parse().unwrap();
    let result = eval(&ast).unwrap();
    assert_eq!(result.ty(), Type::Fun);
}

/// Run `src` and return its result as printed by `Display`.
fn printed(src: &str) -> String {
    let ast = Parser::new(Lexer::new(src)).parse().unwrap();
    eval(&ast).unwrap().to_string()
}

#[test]
fn booleans_are_values() {
    assert_eq!(printed("def main() = true;"), "true");
    assert_eq!(printed("def main() = false;"), "false");
    let ast = Parser::new(Lexer::new("def main() = true;"))
        .parse()
        .unwrap();
    assert_eq!(eval(&ast).unwrap().ty(), Type::Bool);
}

#[test]
fn booleans_flow_through_bindings_calls_and_closures() {
    assert_eq!(printed("def main() = { let b = false; b };"), "false");
    assert_eq!(printed("def id(x) = x; def main() = id(true);"), "true");
    let src = "def always(b) = fun() => b; def main() = always(false)();";
    assert_eq!(printed(src), "false");
}

#[test]
fn booleans_are_not_numbers() {
    // As the left or right operand of arithmetic.
    let src = "def main() = true + 1;";
    assert!(matches!(
        error(src),
        EvalError::TypeMismatch { expected: Type::Int, actual: Type::Bool, span }
            if text(src, span) == "true"
    ));
    let src = "def main() = 2 * false;";
    assert!(matches!(
        error(src),
        EvalError::TypeMismatch { expected: Type::Int, actual: Type::Bool, span }
            if text(src, span) == "false"
    ));
    // Negation.
    let src = "def main() = -true;";
    assert!(matches!(
        error(src),
        EvalError::TypeMismatch { expected: Type::Int, actual: Type::Bool, span }
            if text(src, span) == "true"
    ));
}

#[test]
fn booleans_cannot_be_called() {
    let src = "def main() = true(1);";
    assert!(matches!(
        error(src),
        EvalError::TypeMismatch { expected: Type::Fun, actual: Type::Bool, span }
            if text(src, span) == "true"
    ));
}

#[test]
fn bool_type_in_diagnostics() {
    let src = "def main() = true + 1;";
    let (_, label) = error(src).diagnostic().label.unwrap();
    assert_eq!(label, "expected `int`, found `bool`");
}

#[test]
fn logical_operators() {
    for (src, expected) in [
        ("true && true", "true"),
        ("true && false", "false"),
        ("false && true", "false"),
        ("false || false", "false"),
        ("false || true", "true"),
        ("true || false", "true"),
    ] {
        assert_eq!(printed(&format!("def main() = {src};")), expected, "{src}");
    }
}

#[test]
fn logical_operators_short_circuit() {
    // The right side would divide by zero; it must not be evaluated.
    assert_eq!(printed("def main() = false && (1 / 0 == 0);"), "false");
    assert_eq!(printed("def main() = true || (1 / 0 == 0);"), "true");
    // When the left side does not decide, the right side runs.
    assert!(matches!(
        error("def main() = true && (1 / 0 == 0);"),
        EvalError::DivisionByZero(_)
    ));
}

#[test]
fn logical_operators_need_booleans() {
    let src = "def main() = true && 1;";
    assert!(matches!(
        error(src),
        EvalError::TypeMismatch { expected: Type::Bool, actual: Type::Int, span }
            if text(src, span) == "1"
    ));
    let src = "def main() = 0 || true;";
    assert!(matches!(
        error(src),
        EvalError::TypeMismatch { expected: Type::Bool, actual: Type::Int, span }
            if text(src, span) == "0"
    ));
    let (_, label) = error("def main() = true && 1;").diagnostic().label.unwrap();
    assert_eq!(label, "expected `bool`, found `int`");
}

#[test]
fn equality_on_integers_and_booleans() {
    for (src, expected) in [
        ("1 == 1", "true"),
        ("1 == 2", "false"),
        ("1 != 2", "true"),
        ("1 != 1", "false"),
        ("true == true", "true"),
        ("true == false", "false"),
        ("true != false", "true"),
        ("false != false", "false"),
        // The results of comparisons are booleans too.
        ("(1 < 2) == (3 < 4)", "true"),
    ] {
        assert_eq!(printed(&format!("def main() = {src};")), expected, "{src}");
    }
}

#[test]
fn equality_needs_matching_types() {
    let src = "def main() = 1 == true;";
    assert!(matches!(
        error(src),
        EvalError::TypeMismatch { expected: Type::Int, actual: Type::Bool, span }
            if text(src, span) == "true"
    ));
    let src = "def main() = false != 0;";
    assert!(matches!(
        error(src),
        EvalError::TypeMismatch { expected: Type::Bool, actual: Type::Int, span }
            if text(src, span) == "0"
    ));
}

#[test]
fn functions_cannot_be_compared() {
    // Whichever side the function is on, that side is reported.
    for (src, operand) in [
        ("def f() = 1; def main() = f == f;", "f"),
        ("def f() = 1; def main() = f != 1;", "f"),
        ("def f() = 1; def main() = 1 == f;", "f"),
        ("def main() = 1 == fun() => 1;", "fun() => 1"),
    ] {
        let err = error(src);
        assert!(
            matches!(&err, EvalError::NotComparable { ty: Type::Fun, span } if text(src, *span) == operand),
            "input: {src:?}, got: {err:?}"
        );
    }
    // `f == 1`: the left side is reported even though the right is an integer.
    let src = "def f() = 1; def main() = f == 1;";
    let EvalError::NotComparable { span, .. } = error(src) else {
        panic!("expected NotComparable");
    };
    assert_eq!(span.start, src.find("f ==").unwrap());
}

#[test]
fn not_comparable_diagnostic() {
    let src = "def f() = 1; def main() = f == f;";
    let diag = error(src).diagnostic();
    assert_eq!(diag.message, "values of type `fun` cannot be compared");
    assert_eq!(diag.label.unwrap().1, "this is a function");
}

#[test]
fn if_else_picks_a_branch() {
    assert_eq!(printed("def main() = if true { 1 } else { 2 };"), "1");
    assert_eq!(printed("def main() = if false { 1 } else { 2 };"), "2");
    assert_eq!(
        printed("def main() = if 1 < 2 && 2 < 3 { 1 } else { 2 };"),
        "1"
    );
    // The branches are blocks with their own scope.
    let src = "def main() = { let x = 1; if x == 1 { let x = 10; x } else { x } };";
    assert_eq!(printed(src), "10");
}

#[test]
fn else_if_chains() {
    let src = "def sign(n) = if n < 0 { -1 } else if n == 0 { 0 } else { 1 };
               def main() = sign(N);";
    for (n, expected) in [("-5", "-1"), ("0", "0"), ("7", "1")] {
        assert_eq!(printed(&src.replace('N', n)), expected, "n = {n}");
    }
}

#[test]
fn only_the_chosen_branch_is_evaluated() {
    // The other branch would divide by zero.
    assert_eq!(printed("def main() = if true { 1 } else { 1 / 0 };"), "1");
    assert_eq!(printed("def main() = if false { 1 / 0 } else { 2 };"), "2");
    assert_eq!(printed("def main() = if false { 1 / 0 };"), "()");
}

#[test]
fn if_without_else_is_unit() {
    assert_eq!(printed("def main() = if true { 1 };"), "()");
    assert_eq!(printed("def main() = if false { 1 };"), "()");
    let ast = Parser::new(Lexer::new("def main() = if true { 1 };"))
        .parse()
        .unwrap();
    assert_eq!(eval(&ast).unwrap().ty(), Type::Unit);
    // The branch still runs: an error inside it is reported.
    assert!(matches!(
        error("def main() = if true { 1 / 0 };"),
        EvalError::DivisionByZero(_)
    ));
}

#[test]
fn if_condition_must_be_a_boolean() {
    let src = "def main() = if 1 { 2 } else { 3 };";
    assert!(matches!(
        error(src),
        EvalError::TypeMismatch { expected: Type::Bool, actual: Type::Int, span }
            if text(src, span) == "1"
    ));
    let src = "def f() = 1; def main() = if f { 2 } else { 3 };";
    assert!(matches!(
        error(src),
        EvalError::TypeMismatch { expected: Type::Bool, actual: Type::Fun, span }
            if text(src, span) == "f"
    ));
}

#[test]
fn unit_values() {
    // Unit compares equal to itself.
    assert_eq!(
        printed("def main() = (if false { 1 }) == (if true { 2 });"),
        "true"
    );
    assert_eq!(
        printed("def main() = (if false { 1 }) != (if false { 2 });"),
        "false"
    );
    // But it is not a number or a boolean.
    let src = "def main() = (if false { 1 }) + 1;";
    assert!(matches!(
        error(src),
        EvalError::TypeMismatch {
            expected: Type::Int,
            actual: Type::Unit,
            ..
        }
    ));
    let src = "def main() = (if false { 1 }) == 1;";
    assert!(matches!(
        error(src),
        EvalError::TypeMismatch { expected: Type::Unit, actual: Type::Int, span }
            if text(src, span) == "1"
    ));
    let (_, label) = error(src).diagnostic().label.unwrap();
    assert_eq!(label, "expected `()`, found `int`");
}

#[test]
fn unit_literal() {
    assert_eq!(printed("def main() = ();"), "()");
    assert_eq!(printed("def main() = () == ();"), "true");
    // The literal and a missing `else` produce the same value.
    assert_eq!(printed("def main() = () == if false { 1 };"), "true");
    assert_eq!(
        printed("def id(x) = x; def main() = { let u = (); id(u) };"),
        "()"
    );
    let src = "def main() = () + 1;";
    assert!(matches!(
        error(src),
        EvalError::TypeMismatch { expected: Type::Int, actual: Type::Unit, span }
            if text(src, span) == "()"
    ));
    let src = "def main() = ()();";
    assert!(matches!(
        error(src),
        EvalError::TypeMismatch { expected: Type::Fun, actual: Type::Unit, span }
            if text(src, span) == "()"
    ));
}

#[test]
fn recursion_terminates_with_if() {
    let fact = "def fact(n) = if n == 0 { 1 } else { n * fact(n - 1) };";
    assert_eq!(
        printed(&format!("{fact} def main() = fact(10);")),
        "3628800"
    );
    assert_eq!(printed(&format!("{fact} def main() = fact(0);")), "1");
    let fib = "def fib(n) = if n < 2 { n } else { fib(n - 1) + fib(n - 2) };";
    assert_eq!(printed(&format!("{fib} def main() = fib(20);")), "6765");
}

#[test]
fn mutual_recursion() {
    let src = "def even(n) = if n == 0 { true } else { odd(n - 1) };
               def odd(n) = if n == 0 { false } else { even(n - 1) };
               def main() = even(10) && odd(7) && !even(3);";
    assert_eq!(printed(src), "true");
}

#[test]
fn strings_are_values() {
    assert_eq!(printed(r#"def main() = "hi";"#), r#""hi""#);
    assert_eq!(printed(r#"def main() = "";"#), r#""""#);
    let ast = Parser::new(Lexer::new(r#"def main() = "hi";"#))
        .parse()
        .unwrap();
    assert_eq!(eval(&ast).unwrap().ty(), Type::Str);
}

#[test]
fn string_results_are_displayed_escaped() {
    assert_eq!(printed(r#"def main() = "a\tb\nc";"#), r#""a\tb\nc""#);
    assert_eq!(printed(r#"def main() = "say \"hi\"";"#), r#""say \"hi\"""#);
    assert_eq!(printed(r#"def main() = "\\";"#), r#""\\""#);
}

#[test]
fn strings_flow_through_bindings_calls_and_closures() {
    assert_eq!(printed(r#"def main() = { let s = "x"; s };"#), r#""x""#);
    assert_eq!(printed(r#"def id(x) = x; def main() = id("y");"#), r#""y""#);
    let src = r#"def greet(name) = fun() => name; def main() = greet("z")();"#;
    assert_eq!(printed(src), r#""z""#);
}

#[test]
fn string_equality() {
    for (src, expected) in [
        (r#""a" == "a""#, "true"),
        (r#""a" == "b""#, "false"),
        (r#""a" != "b""#, "true"),
        (r#""" == """#, "true"),
        (r#""é" == "é""#, "true"),
        // Escapes are compared after they are replaced.
        (r#""\t" == "	""#, "true"),
        (r#"{ let s = "x"; s == s }"#, "true"),
    ] {
        assert_eq!(printed(&format!("def main() = {src};")), expected, "{src}");
    }
}

#[test]
fn strings_are_not_numbers() {
    // `+` and the orderings are for integers; `concat` joins strings.
    let src = r#"def main() = "a" + "b";"#;
    assert!(matches!(
        error(src),
        EvalError::TypeMismatch { expected: Type::Int, actual: Type::Str, span }
            if text(src, span) == r#""a""#
    ));
    let src = r#"def main() = "a" < "b";"#;
    assert!(matches!(
        error(src),
        EvalError::TypeMismatch {
            expected: Type::Int,
            actual: Type::Str,
            ..
        }
    ));
    let src = r#"def main() = -"a";"#;
    assert!(matches!(
        error(src),
        EvalError::TypeMismatch {
            expected: Type::Int,
            actual: Type::Str,
            ..
        }
    ));
}

#[test]
fn strings_compare_only_with_strings() {
    let src = r#"def main() = "x" == 1;"#;
    assert!(matches!(
        error(src),
        EvalError::TypeMismatch { expected: Type::Str, actual: Type::Int, span }
            if text(src, span) == "1"
    ));
    let (_, label) = error(src).diagnostic().label.unwrap();
    assert_eq!(label, "expected `string`, found `int`");
}

/// Run `src`, returning the printed result and what the program wrote.
fn run(src: &str) -> (String, String) {
    let ast = Parser::new(Lexer::new(src)).parse().unwrap();
    let mut out = Vec::new();
    let value = eval_with_output(&ast, &mut out).unwrap().to_string();
    (value, String::from_utf8(out).unwrap())
}

#[test]
fn println_writes_the_raw_string_and_a_newline() {
    let src = r#"def main() = { println("a\tb"); println("say \"hi\"") };"#;
    assert_eq!(run(src), ("()".into(), "a\tb\nsay \"hi\"\n".into()));
    assert_eq!(run(r#"def main() = println("");"#).1, "\n");
}

#[test]
fn output_follows_evaluation_order() {
    let src = r#"
        def say(s, n) = { println(s); n };
        def main() = say("left", 1) + say("right", 2);
    "#;
    assert_eq!(run(src), ("3".into(), "left\nright\n".into()));
    // Only the branch taken runs.
    let src = r#"def main() = if false { println("no") } else { println("yes") };"#;
    assert_eq!(run(src).1, "yes\n");
}

#[test]
fn int_to_str_formats_in_decimal() {
    assert_eq!(run("def main() = int_to_str(42);").0, r#""42""#);
    assert_eq!(run("def main() = int_to_str(-7);").0, r#""-7""#);
    let src = "def main() = int_to_str(-9223372036854775807 - 1);";
    assert_eq!(run(src).0, r#""-9223372036854775808""#);
    let src = "def main() = println(int_to_str(10 * 10));";
    assert_eq!(run(src).1, "100\n");
}

#[test]
fn builtins_are_values() {
    assert_eq!(run("def main() = println;").0, "BUILTIN println");
    let src = "def apply(f, x) = f(x); def main() = apply(int_to_str, 5);";
    assert_eq!(run(src).0, r#""5""#);
    let src = r#"def main() = { let p = println; p("via let") };"#;
    assert_eq!(run(src).1, "via let\n");
}

#[test]
fn builtins_check_argument_count() {
    let src = "def main() = println();";
    assert!(matches!(
        error(src),
        EvalError::ArgumentCountMismatch { expected: 1, actual: 0, span }
            if text(src, span) == "println()"
    ));
    let src = r#"def main() = int_to_str(1, 2);"#;
    assert!(matches!(
        error(src),
        EvalError::ArgumentCountMismatch {
            expected: 1,
            actual: 2,
            ..
        }
    ));
}

#[test]
fn builtins_check_argument_types() {
    let src = "def main() = println(1 + 2);";
    assert!(matches!(
        error(src),
        EvalError::TypeMismatch { expected: Type::Str, actual: Type::Int, span }
            if text(src, span) == "1 + 2"
    ));
    let src = r#"def main() = int_to_str("1");"#;
    assert!(matches!(
        error(src),
        EvalError::TypeMismatch { expected: Type::Int, actual: Type::Str, span }
            if text(src, span) == r#""1""#
    ));
}

#[test]
fn builtins_cannot_be_compared() {
    let src = "def main() = println == println;";
    assert!(matches!(
        error(src),
        EvalError::NotComparable { ty: Type::Fun, span }
            if span.start == src.find("println").unwrap()
    ));
    let src = "def main() = 1 == int_to_str;";
    assert!(matches!(
        error(src),
        EvalError::NotComparable { ty: Type::Fun, span }
            if text(src, span) == "int_to_str"
    ));
}

#[test]
fn top_level_def_cannot_redefine_a_builtin() {
    let src = "def main() = 1; def println(s) = s;";
    assert!(matches!(
        error(src),
        EvalError::RedefinesBuiltin { name, span }
            if name == "println" && span.start == src.find("println").unwrap()
                && text(src, span) == "println"
    ));
}

#[test]
fn locals_may_shadow_builtins() {
    let src = "def main() = { let println = fun(x) => x + 1; println(1) };";
    assert_eq!(run(src), ("2".into(), String::new()));
    let src = "def f(int_to_str) = int_to_str * 2; def main() = f(3);";
    assert_eq!(run(src).0, "6");
}

#[test]
fn output_errors_point_at_the_call() {
    struct Closed;
    impl Write for Closed {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("stream closed"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let src = r#"def main() = println("x");"#;
    let ast = Parser::new(Lexer::new(src)).parse().unwrap();
    let err = eval_with_output(&ast, &mut Closed).unwrap_err();
    assert!(matches!(
        &err,
        EvalError::Output { message, span }
            if message == "stream closed" && text(src, *span) == r#"println("x")"#
    ));
}

#[test]
fn builtin_names_are_unique() {
    let mut names: Vec<_> = Builtin::ALL.iter().map(|b| b.name()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), Builtin::ALL.len());
}

#[test]
fn remainder_has_the_sign_of_the_dividend() {
    assert_eq!(printed("def main() = 7 % 3;"), "1");
    assert_eq!(printed("def main() = -7 % 3;"), "-1");
    assert_eq!(printed("def main() = 7 % -3;"), "1");
    assert_eq!(printed("def main() = 6 % 3;"), "0");
    // `%` binds like `*` and `/`: (2 * 7) % 4, then + 1.
    assert_eq!(printed("def main() = 1 + 2 * 7 % 4;"), "3");
    // a == (a / b) * b + a % b
    assert_eq!(printed("def main() = -7 / 2 * 2 + -7 % 2;"), "-7");
}

#[test]
fn remainder_by_zero_and_overflow() {
    let src = "def main() = 5 % (1 - 1);";
    assert!(matches!(
        error(src),
        EvalError::DivisionByZero(span) if text(src, span) == "(1 - 1)"
    ));
    // The quotient i64::MIN / -1 does not fit, so neither does `%`.
    let src = "def main() = (-9223372036854775807 - 1) % -1;";
    assert!(matches!(
        error(src),
        EvalError::IntegerOverflow(span) if text(src, span) == "(-9223372036854775807 - 1) % -1"
    ));
}

#[test]
fn concat_joins_two_strings() {
    assert_eq!(
        run(r#"def main() = concat("foo", "bar");"#).0,
        r#""foobar""#
    );
    assert_eq!(run(r#"def main() = concat("", "");"#).0, r#""""#);
    let src = r#"def main() = concat("é", concat("\n", "日本"));"#;
    assert_eq!(run(src).0, r#""é\n日本""#);
    let src = r#"def main() = println(concat("n = ", int_to_str(3)));"#;
    assert_eq!(run(src).1, "n = 3\n");
    // The result is an ordinary string.
    let src = r#"def main() = concat("a", "b") == "ab";"#;
    assert_eq!(run(src).0, "true");
}

#[test]
fn concat_checks_both_arguments() {
    let src = r#"def main() = concat("a", 1);"#;
    assert!(matches!(
        error(src),
        EvalError::TypeMismatch { expected: Type::Str, actual: Type::Int, span }
            if text(src, span) == "1"
    ));
    let src = r#"def main() = concat("a");"#;
    assert!(matches!(
        error(src),
        EvalError::ArgumentCountMismatch {
            expected: 2,
            actual: 1,
            ..
        }
    ));
}

#[test]
fn print_writes_without_a_newline() {
    let src = r#"def main() = { print("a"); print("b"); println("c"); print("d") };"#;
    assert_eq!(run(src), ("()".into(), "abc\nd".into()));
}
