//! A tree-walking interpreter: it evaluates the AST directly, checking
//! types as it goes.
//!
//! Values borrow from the AST (the `'ast` lifetime) instead of copying
//! function bodies out of it, so the AST must outlive every value. Local
//! variables live in an immutable linked list (`Env`) that closures share
//! by capturing it; top-level functions and builtins live in a table
//! (`TreeEval::globals`) and are looked up by name.

use crate::{
    ast::{Ast, BinOp, Decl, DeclKind, Expr, ExprKind, Ident, Stmt, StmtKind, UnOp},
    builtin::Builtin,
    diagnostic::Diagnostic,
    span::Span,
    types::Type,
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    fmt,
    io::{self, Write},
    rc::Rc,
};

/// How many calls may be in progress at once. Deeper recursion is reported
/// as an error, the same in every build, so that a runaway program stops
/// with a clear message.
pub const MAX_CALL_DEPTH: usize = 10_000;

/// When less than this much stack is left, `eval_expr` continues on a new
/// segment of `STACK_SEGMENT` bytes. The evaluator recurses once per nested
/// expression, so without this a deeply nested body overflows the native
/// stack (about 400 calls deep in a debug build) long before
/// `MAX_CALL_DEPTH`. The values are rustc's.
const STACK_RED_ZONE: usize = 100 * 1024;
const STACK_SEGMENT: usize = 1024 * 1024;

/// Everything that can go wrong while running a program. Each error
/// carries the span to underline in the report.
#[derive(Debug, Clone, PartialEq)]
pub enum EvalError {
    /// The divisor of `/` or `%` evaluated to zero; the span is the divisor.
    DivisionByZero(Span),
    /// A variable that is neither a local nor a global; the span is the
    /// variable.
    UnboundIdent { name: String, span: Span },
    /// An arithmetic result outside the range of `i64`; the span is the
    /// whole operation.
    IntegerOverflow(Span),
    /// A value of the wrong type; the span is the expression that produced
    /// it, e.g. the operand of `+` or the argument of a builtin.
    TypeMismatch {
        expected: Type,
        actual: Type,
        span: Span,
    },
    /// A call with too many or too few arguments; the span is the call.
    ArgumentCountMismatch {
        expected: usize,
        actual: usize,
        span: Span,
    },
    /// More than `limit` calls in progress at once, almost always infinite
    /// recursion; the span is the call that went over.
    RecursionLimit { limit: usize, span: Span },
    /// The program defines no `main` function.
    MissingMain,
    /// `main` has parameters; the span covers them.
    MainHasParams(Span),
    /// `==` or `!=` on a value that cannot be compared, such as a function;
    /// the span is that operand.
    NotComparable { ty: Type, span: Span },
    /// A top-level `def` uses a builtin's name; the span is the name.
    RedefinesBuiltin { name: String, span: Span },
    /// Writing the program's output failed, e.g. because stdout was closed;
    /// the span is the call that wrote. Holds the I/O error's message,
    /// since `io::Error` is neither `Clone` nor `PartialEq`.
    Output { message: String, span: Span },
}

/// The headline of the error; `diagnostic` adds the location and details.
impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvalError::DivisionByZero(_) => write!(f, "division by zero"),
            EvalError::UnboundIdent { name, .. } => {
                write!(f, "cannot find `{name}` in this scope")
            }
            EvalError::IntegerOverflow(_) => write!(f, "integer overflow"),
            EvalError::TypeMismatch { .. } => write!(f, "mismatched types"),
            EvalError::ArgumentCountMismatch { .. } => write!(f, "wrong number of arguments"),
            EvalError::RecursionLimit { .. } => write!(f, "recursion limit reached"),
            EvalError::MissingMain => write!(f, "no `main` function"),
            EvalError::MainHasParams(_) => write!(f, "`main` must not take parameters"),
            EvalError::NotComparable { ty, .. } => {
                write!(f, "values of type `{ty}` cannot be compared")
            }
            EvalError::RedefinesBuiltin { name, .. } => {
                write!(f, "`{name}` is a builtin function")
            }
            EvalError::Output { .. } => write!(f, "cannot write output"),
        }
    }
}

impl EvalError {
    /// The error as a report for the user: the `Display` headline plus where
    /// it happened and what to say there.
    pub fn diagnostic(&self) -> Diagnostic {
        let at = |span: &Span, label: String| Diagnostic::new(self.to_string(), *span, label);
        match self {
            EvalError::DivisionByZero(span) => at(span, "this is zero".into()),
            EvalError::UnboundIdent { span, .. } => at(span, "not found in this scope".into()),
            EvalError::IntegerOverflow(span) => {
                at(span, "the result does not fit in an `i64`".into())
            }
            EvalError::TypeMismatch {
                expected,
                actual,
                span,
            } => at(span, format!("expected `{expected}`, found `{actual}`")),
            EvalError::ArgumentCountMismatch {
                expected,
                actual,
                span,
            } => at(
                span,
                format!(
                    "expected {expected} argument{}, found {actual}",
                    if *expected == 1 { "" } else { "s" }
                ),
            ),
            EvalError::RecursionLimit { limit, span } => {
                at(span, format!("this call would exceed {limit} nested calls"))
                    .with_note("does the function have a base case that stops the recursion?")
            }
            EvalError::MissingMain => Diagnostic::without_location(self.to_string())
                .with_note("a program starts by calling `def main() = ...;`"),
            EvalError::MainHasParams(span) => at(span, "remove these parameters".into()),
            EvalError::NotComparable { span, .. } => at(span, "this is a function".into()),
            EvalError::RedefinesBuiltin { span, .. } => at(span, "cannot be redefined".into())
                .with_note("pick another name; a local `let` or parameter may shadow it"),
            EvalError::Output { message, span } => at(span, message.clone()),
        }
    }
}

/// A function value: its code, borrowed from the AST, and the environment
/// it was created in.
#[derive(Debug, Clone)]
pub struct Closure<'ast> {
    params: &'ast [Ident],
    body: &'ast Expr,
    /// The locals visible where the function was written. Empty for a
    /// top-level `def`, which sees only globals.
    env: Env<'ast>,
}

/// A runtime value. Cloning is cheap: strings and closures are shared
/// through `Rc`, never copied.
#[derive(Debug, Clone)]
pub enum Value<'ast> {
    Unit,
    Int(i64),
    Bool(bool),
    Str(Rc<str>),
    Fun(Rc<Closure<'ast>>),
    Builtin(Builtin),
}

impl Value<'_> {
    /// The value's type, for error messages.
    pub fn ty(&self) -> Type {
        match self {
            Value::Unit => Type::Unit,
            Value::Int(_) => Type::Int,
            Value::Fun { .. } => Type::Fun,
            Value::Bool(_) => Type::Bool,
            Value::Str(_) => Type::Str,
            Value::Builtin(_) => Type::Fun,
        }
    }
}

/// How a value is shown to the user, e.g. as the result of `main`. Strings
/// are quoted and escaped; functions show their source.
impl fmt::Display for Value<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Unit => write!(f, "()"),
            Value::Int(n) => write!(f, "{n}"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Str(s) => write!(f, "{s:?}"),
            Value::Fun(closure) => {
                write!(f, "FUN (")?;
                for (i, param) in closure.params.iter().enumerate() {
                    write!(f, "{}", param.name)?;
                    if i != closure.params.len() - 1 {
                        write!(f, ", ")?;
                    }
                }
                write!(f, ") => {}", closure.body)
            }
            Value::Builtin(builtin) => write!(f, "BUILTIN {builtin}"),
        }
    }
}

/// One local variable: a node in the linked list that makes up an `Env`.
#[derive(Debug, Clone)]
struct Binding<'ast> {
    id: &'ast Ident,
    value: Value<'ast>,
    /// The bindings that were in scope before this one.
    parent: Option<Rc<Binding<'ast>>>,
}

/// The local variables in scope, innermost first. It is a persistent list:
/// binding a variable makes a new head that shares the rest, and nothing is
/// ever changed in place. That makes capturing an environment in a closure
/// cheap (one `Rc` clone), and an inner scope cannot affect an outer one.
#[derive(Debug, Clone)]
struct Env<'ast> {
    head: Option<Rc<Binding<'ast>>>,
}

impl<'ast> Env<'ast> {
    fn empty() -> Self {
        Env { head: None }
    }

    /// This environment plus `id = value`. An existing variable of the same
    /// name is shadowed, not replaced: environments that share the old
    /// bindings, such as a closure's, still see it.
    fn bind(self, id: &'ast Ident, value: Value<'ast>) -> Self {
        Env {
            head: Some(Rc::new(Binding {
                id,
                value,
                parent: self.head,
            })),
        }
    }

    /// The innermost binding of `name`, so the most recent shadowing wins.
    fn search(&self, name: &str) -> Option<&Value<'ast>> {
        let mut curr = &self.head;
        loop {
            match curr {
                Some(binding) if name == binding.id.name => {
                    break Some(&binding.value);
                }
                Some(binding) => {
                    curr = &binding.parent;
                }
                None => {
                    break None;
                }
            }
        }
    }
}

/// The interpreter's state while running one program.
struct TreeEval<'ast, 'out> {
    /// Builtins and top-level `def`s, by name. Locals shadow them.
    globals: HashMap<&'ast str, Value<'ast>>,
    /// Calls currently in progress. A `Cell` because evaluation takes
    /// `&self`.
    depth: Cell<usize>,
    /// The most calls allowed in progress at once; see `MAX_CALL_DEPTH`.
    max_depth: usize,
    /// Where `println` writes. A `RefCell`, not a `Cell`, because writing
    /// needs a `&mut` to the writer, and only `RefCell` lends one out
    /// through a `&self`.
    out: RefCell<&'out mut dyn Write>,
}

/// `value` as an integer, or a type error pointing at `span`.
fn expect_int(value: Value<'_>, span: Span) -> Result<i64, EvalError> {
    match value {
        Value::Int(n) => Ok(n),
        other => Err(EvalError::TypeMismatch {
            expected: Type::Int,
            actual: other.ty(),
            span,
        }),
    }
}

/// `value` as a boolean, or a type error pointing at `span`.
fn expect_bool(value: Value<'_>, span: Span) -> Result<bool, EvalError> {
    match value {
        Value::Bool(b) => Ok(b),
        other => Err(EvalError::TypeMismatch {
            expected: Type::Bool,
            actual: other.ty(),
            span,
        }),
    }
}

impl<'ast, 'out> TreeEval<'ast, 'out> {
    fn new(out: &'out mut dyn Write) -> Self {
        Self::with_max_depth(out, MAX_CALL_DEPTH)
    }

    /// An interpreter that allows at most `max_depth` calls in progress,
    /// with only the builtins defined.
    fn with_max_depth(out: &'out mut dyn Write, max_depth: usize) -> Self {
        TreeEval {
            globals: Builtin::ALL
                .into_iter()
                .map(|b| (b.name(), Value::Builtin(b)))
                .collect(),
            depth: Cell::new(0),
            max_depth,
            out: RefCell::new(out),
        }
    }

    /// The value of the variable `name`: a local if there is one, else a
    /// global.
    fn search<'env>(&'env self, name: &str, env: &'env Env<'ast>) -> Option<&'env Value<'ast>> {
        env.search(name).or_else(|| self.globals.get(name))
    }

    /// Define a top-level function. Its body is not evaluated, and the names
    /// it uses are looked up only when it runs, so definitions can refer to
    /// each other in any order.
    fn eval_decl(&mut self, decl: &'ast Decl) -> Result<(), EvalError> {
        match &decl.kind {
            DeclKind::FunDecl { id, params, body } => {
                if let Some(Value::Builtin(_)) = self.globals.get(id.name.as_str()) {
                    return Err(EvalError::RedefinesBuiltin {
                        name: id.name.clone(),
                        span: id.span,
                    });
                }
                let f = Value::Fun(Rc::new(Closure {
                    params,
                    body,
                    env: Env::empty(),
                }));
                self.globals.insert(&id.name, f);
                Ok(())
            }
        }
    }

    /// Run one statement of a block and return the environment for the
    /// statements after it: extended by a `let`, unchanged otherwise.
    fn eval_stmt(&self, stmt: &'ast Stmt, env: Env<'ast>) -> Result<Env<'ast>, EvalError> {
        match &stmt.kind {
            StmtKind::Let { id, init } => {
                let v = self.eval_expr(init, &env)?;
                let env = env.bind(id, v);
                Ok(env)
            }
            StmtKind::ExprStmt(expr) => {
                // Run for its effects; the value is discarded.
                let _ = self.eval_expr(expr, &env)?;
                Ok(env)
            }
        }
    }

    /// Evaluate `expr` with the locals in `env`. Every recursive evaluation
    /// goes through here, so that the stack grows when it runs low.
    fn eval_expr(&self, expr: &'ast Expr, env: &Env<'ast>) -> Result<Value<'ast>, EvalError> {
        stacker::maybe_grow(STACK_RED_ZONE, STACK_SEGMENT, || {
            self.eval_expr_inner(expr, env)
        })
    }

    fn eval_expr_inner(&self, expr: &'ast Expr, env: &Env<'ast>) -> Result<Value<'ast>, EvalError> {
        match &expr.kind {
            ExprKind::Unit => Ok(Value::Unit),
            ExprKind::Int(n) => Ok(Value::Int(*n)),
            ExprKind::Bool(b) => Ok(Value::Bool(*b)),
            ExprKind::Str(s) => Ok(Value::Str(s.clone())),
            ExprKind::Var(name) => {
                self.search(name, env)
                    .cloned()
                    .ok_or_else(|| EvalError::UnboundIdent {
                        name: name.clone(),
                        span: expr.span,
                    })
            }
            // The callee is evaluated first, then the arguments from left to
            // right.
            ExprKind::Call { callee, args } => match self.eval_expr(callee, env)? {
                Value::Fun(closure) => self.call_closure(&closure, args, env, expr.span),
                Value::Builtin(builtin) => self.call_builtin(builtin, args, env, expr.span),
                other => Err(EvalError::TypeMismatch {
                    expected: Type::Fun,
                    actual: other.ty(),
                    span: callee.span,
                }),
            },
            ExprKind::Unary(op, operand) => {
                let e = self.eval_expr(operand, env)?;
                match op {
                    UnOp::Neg => {
                        let n = expect_int(e, operand.span)?;
                        let Some(res) = n.checked_neg() else {
                            return Err(EvalError::IntegerOverflow(expr.span));
                        };
                        Ok(Value::Int(res))
                    }
                    UnOp::Not => {
                        let b = expect_bool(e, operand.span)?;
                        Ok(Value::Bool(!b))
                    }
                }
            }
            // `&&` and `||` short-circuit: the right operand is evaluated only
            // when the left one does not already decide the result, so they
            // come before the arm that evaluates both operands.
            ExprKind::Binary(op @ (BinOp::And | BinOp::Or), lhs, rhs) => {
                let l = expect_bool(self.eval_expr(lhs, env)?, lhs.span)?;
                match (op, l) {
                    (BinOp::And, false) => Ok(Value::Bool(false)),
                    (BinOp::Or, true) => Ok(Value::Bool(true)),
                    _ => {
                        let r = expect_bool(self.eval_expr(rhs, env)?, rhs.span)?;
                        Ok(Value::Bool(r))
                    }
                }
            }
            ExprKind::Binary(op, lhs, rhs) => {
                let lv = self.eval_expr(lhs, env)?;
                let rv = self.eval_expr(rhs, env)?;

                match op {
                    BinOp::And | BinOp::Or => unreachable!("handled above"),
                    BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem => {
                        let l = expect_int(lv, lhs.span)?;
                        let r = expect_int(rv, rhs.span)?;
                        let result = match op {
                            BinOp::Add => l.checked_add(r),
                            BinOp::Sub => l.checked_sub(r),
                            BinOp::Mul => l.checked_mul(r),
                            BinOp::Div | BinOp::Rem if r == 0 => {
                                return Err(EvalError::DivisionByZero(rhs.span));
                            }
                            BinOp::Div => l.checked_div(r),
                            BinOp::Rem => l.checked_rem(r),
                            _ => unreachable!(),
                        };
                        result
                            .ok_or(EvalError::IntegerOverflow(expr.span))
                            .map(Value::Int)
                    }
                    BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                        let l = expect_int(lv, lhs.span)?;
                        let r = expect_int(rv, rhs.span)?;
                        Ok(Value::Bool(match op {
                            BinOp::Lt => l < r,
                            BinOp::Le => l <= r,
                            BinOp::Gt => l > r,
                            BinOp::Ge => l >= r,
                            _ => unreachable!(),
                        }))
                    }
                    BinOp::Eq | BinOp::Ne => {
                        let equal = match (&lv, &rv) {
                            (Value::Int(l), Value::Int(r)) => l == r,
                            (Value::Bool(l), Value::Bool(r)) => l == r,
                            (Value::Str(l), Value::Str(r)) => l == r,
                            (Value::Unit, Value::Unit) => true,
                            (Value::Fun(_) | Value::Builtin(_), _) => {
                                return Err(EvalError::NotComparable {
                                    ty: Type::Fun,
                                    span: lhs.span,
                                });
                            }
                            (_, Value::Fun(_) | Value::Builtin(_)) => {
                                return Err(EvalError::NotComparable {
                                    ty: Type::Fun,
                                    span: rhs.span,
                                });
                            }
                            // Two values of different types, such as an
                            // integer and a string.
                            _ => {
                                return Err(EvalError::TypeMismatch {
                                    expected: lv.ty(),
                                    actual: rv.ty(),
                                    span: rhs.span,
                                });
                            }
                        };
                        Ok(Value::Bool(if let BinOp::Eq = op { equal } else { !equal }))
                    }
                }
            }
            ExprKind::If {
                cond,
                t_branch,
                f_branch,
            } => {
                let c = expect_bool(self.eval_expr(cond, env)?, cond.span)?;
                match (c, f_branch) {
                    (true, Some(_)) => self.eval_expr(t_branch, env),
                    (false, Some(f_branch)) => self.eval_expr(f_branch, env),
                    // Without `else`, the `if` is unit either way; the branch
                    // still runs for its effects.
                    (true, None) => {
                        let _ = self.eval_expr(t_branch, env)?;
                        Ok(Value::Unit)
                    }
                    (_, None) => Ok(Value::Unit),
                }
            }
            // The closure captures the current locals by sharing them.
            ExprKind::Lambda { params, body } => {
                let f = Value::Fun(Rc::new(Closure {
                    params,
                    body,
                    env: env.clone(),
                }));
                Ok(f)
            }
            // A block's `let`s extend a copy of `env`, so they go out of
            // scope when the block ends.
            ExprKind::Block { stmts, tail } => {
                let mut new_env = env.clone();

                for stmt in stmts {
                    new_env = self.eval_stmt(stmt, new_env)?;
                }
                let res = self.eval_expr(tail, &new_env)?;
                Ok(res)
            }
        }
    }

    /// Call `closure` with `args`, evaluated in the caller's `env`; `span`
    /// is the whole call.
    fn call_closure(
        &self,
        closure: &Closure<'ast>,
        args: &'ast [Expr],
        env: &Env<'ast>,
        span: Span,
    ) -> Result<Value<'ast>, EvalError> {
        if closure.params.len() != args.len() {
            return Err(EvalError::ArgumentCountMismatch {
                expected: closure.params.len(),
                actual: args.len(),
                span,
            });
        }
        // Start from what the closure captured, not the caller's
        // environment: that is what makes scoping lexical.
        let mut closure_env = closure.env.clone();
        for (param, arg) in closure.params.iter().zip(args) {
            let v = self.eval_expr(arg, env)?;
            closure_env = closure_env.bind(param, v);
        }

        let depth = self.depth.get();
        if depth == self.max_depth {
            return Err(EvalError::RecursionLimit {
                limit: self.max_depth,
                span,
            });
        }
        // Nothing between `set` and the restore can return early,
        // so the count stays right even when the body fails.
        self.depth.set(depth + 1);
        let result = self.eval_expr(closure.body, &closure_env);
        self.depth.set(depth);
        result
    }

    /// Call `builtin` with `args`, evaluated in the caller's `env`; `span`
    /// is the whole call. Builtins cannot recurse, so they do not count
    /// towards the call depth.
    fn call_builtin(
        &self,
        builtin: Builtin,
        args: &'ast [Expr],
        env: &Env<'ast>,
        span: Span,
    ) -> Result<Value<'ast>, EvalError> {
        let params = builtin.params();
        if params.len() != args.len() {
            return Err(EvalError::ArgumentCountMismatch {
                expected: params.len(),
                actual: args.len(),
                span,
            });
        }
        let mut values = Vec::with_capacity(args.len());
        for (&expected, arg) in params.iter().zip(args) {
            let v = self.eval_expr(arg, env)?;
            if v.ty() != expected {
                return Err(EvalError::TypeMismatch {
                    expected,
                    actual: v.ty(),
                    span: arg.span,
                });
            }
            values.push(v);
        }

        match (builtin, values.as_slice()) {
            (Builtin::Print, [Value::Str(s)]) => {
                write!(self.out.borrow_mut(), "{s}").map_err(|e| EvalError::Output {
                    message: e.to_string(),
                    span,
                })?;
                Ok(Value::Unit)
            }
            (Builtin::Println, [Value::Str(s)]) => {
                writeln!(self.out.borrow_mut(), "{s}").map_err(|e| EvalError::Output {
                    message: e.to_string(),
                    span,
                })?;
                Ok(Value::Unit)
            }
            (Builtin::IntToStr, [Value::Int(n)]) => Ok(Value::Str(n.to_string().into())),
            (Builtin::Concat, [Value::Str(a), Value::Str(b)]) => {
                Ok(Value::Str(format!("{a}{b}").into()))
            }
            _ => unreachable!("the arguments were checked against `params`"),
        }
    }

    /// Define every top-level function, then run `main`'s body. `main` is
    /// entered directly rather than called, so it does not count towards
    /// the call depth.
    pub fn eval(&mut self, ast: &'ast Ast) -> Result<Value<'ast>, EvalError> {
        for decl in &ast.decls {
            self.eval_decl(decl)?;
        }
        let main = self.globals.get("main").ok_or(EvalError::MissingMain)?;

        if let Value::Fun(closure) = main {
            if let (Some(first), Some(last)) = (closure.params.first(), closure.params.last()) {
                return Err(EvalError::MainHasParams(Span {
                    start: first.span.start,
                    end: last.span.end,
                }));
            }
            self.eval_expr(closure.body, &closure.env)
        } else {
            // Globals other than `def`s are builtins, and none of them is
            // called `main`, so this cannot happen today.
            Err(EvalError::MissingMain)
        }
    }
}

/// Run a program: define every top-level function, then call `main`.
/// The program's output goes to stdout.
pub fn eval(ast: &Ast) -> Result<Value<'_>, EvalError> {
    eval_with_output(ast, &mut io::stdout().lock())
}

/// Like `eval`, but the program's output goes to `out`.
pub fn eval_with_output<'ast>(
    ast: &'ast Ast,
    out: &mut dyn Write,
) -> Result<Value<'ast>, EvalError> {
    TreeEval::new(out).eval(ast)
}

#[cfg(test)]
mod tests;
