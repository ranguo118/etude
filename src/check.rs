//! Checks that need only names and scopes, run before evaluation: every
//! variable must refer to something, and names must not clash.

use std::{collections::HashMap, fmt};

use crate::{
    ast::{Ast, Decl, DeclKind, Expr, ExprKind, Ident, StmtKind},
    builtin::Builtin,
    diagnostic::Diagnostic,
    span::Span,
};

/// A problem found before the program runs.
#[derive(Debug, Clone, PartialEq)]
pub enum SemError {
    /// A top-level `def` uses a builtin's name; the span is the name.
    RedefinesBuiltin { name: String, span: Span },
    /// A second top-level `def` of the same name; `span` is its name and
    /// `first` the name in the first definition.
    DuplicateDef {
        name: String,
        span: Span,
        first: Span,
    },
    /// A name listed twice in one parameter list; `span` is the second
    /// occurrence and `first` the first.
    DuplicateParam {
        name: String,
        span: Span,
        first: Span,
    },
    /// A variable that is neither a local, a top-level `def` nor a builtin;
    /// the span is the variable.
    UnboundIdent { name: String, span: Span },
}

/// The headline of the error.
impl fmt::Display for SemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SemError::RedefinesBuiltin { name, .. } => {
                write!(f, "`{name}` is a builtin function")
            }
            SemError::DuplicateDef { name, .. } => {
                write!(f, "`{name}` is defined more than once")
            }
            SemError::DuplicateParam { name, .. } => {
                write!(f, "parameter `{name}` is listed more than once")
            }
            SemError::UnboundIdent { name, .. } => {
                write!(f, "cannot find `{name}` in this scope")
            }
        }
    }
}

impl SemError {
    /// The error as a report for the user: the `Display` headline plus where
    /// it happened and what to say there.
    pub fn diagnostic(&self) -> Diagnostic {
        let at = |span: &Span, label: &str| Diagnostic::new(self.to_string(), *span, label);
        match self {
            SemError::RedefinesBuiltin { span, .. } => at(span, "cannot be redefined")
                .with_note("pick another name; a local `let` or parameter may shadow it"),
            SemError::DuplicateDef { span, first, .. } => {
                at(span, "defined again here").with_secondary(*first, "first defined here")
            }
            SemError::DuplicateParam { span, first, .. } => {
                at(span, "listed again here").with_secondary(*first, "first listed here")
            }
            SemError::UnboundIdent { span, .. } => at(span, "not found in this scope"),
        }
    }
}

struct Checker<'ast> {
    /// Top-level names and where each was first defined.
    globals: HashMap<&'ast str, Span>,
    /// The locals in scope, one entry per open scope, innermost last.
    scopes: Vec<Vec<&'ast Ident>>,
    errors: Vec<SemError>,
}

impl<'ast> Checker<'ast> {
    fn new() -> Self {
        Checker {
            globals: HashMap::new(),
            scopes: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Check the whole program: first every top-level name, so that
    /// definitions can refer to each other in any order, then every body.
    fn check(&mut self, ast: &'ast Ast) {
        for decl in &ast.decls {
            self.check_decl(decl);
        }
        for decl in &ast.decls {
            self.check_decl_body(decl);
        }
    }

    /// Check a definition's name and parameters, and add it to the globals.
    /// A clashing definition is reported and the first one kept.
    fn check_decl(&mut self, decl: &'ast Decl) {
        match &decl.kind {
            DeclKind::FunDecl { id, params, .. } => {
                self.check_params(params);
                if Builtin::from_name(&id.name).is_some() {
                    self.errors.push(SemError::RedefinesBuiltin {
                        name: id.name.clone(),
                        span: id.span,
                    });
                } else if let Some(&first) = self.globals.get(id.name.as_str()) {
                    self.errors.push(SemError::DuplicateDef {
                        name: id.name.clone(),
                        span: id.span,
                        first,
                    });
                } else {
                    self.globals.insert(&id.name, id.span);
                }
            }
        }
    }

    /// Check a definition's body, with its parameters in scope.
    fn check_decl_body(&mut self, decl: &'ast Decl) {
        match &decl.kind {
            DeclKind::FunDecl { params, body, .. } => {
                debug_assert!(self.scopes.is_empty(), "a scope was not popped");
                self.push_scope();
                for param in params {
                    self.bind(param);
                }
                self.check_expr(body);
                self.pop_scope();
            }
        }
    }

    /// Report names listed more than once in one parameter list.
    fn check_params(&mut self, params: &'ast [Ident]) {
        let mut seen: HashMap<&str, Span> = HashMap::new();
        for param in params {
            if let Some(&first) = seen.get(param.name.as_str()) {
                self.errors.push(SemError::DuplicateParam {
                    name: param.name.clone(),
                    span: param.span,
                    first,
                });
            } else {
                seen.insert(&param.name, param.span);
            }
        }
    }

    fn check_expr(&mut self, expr: &'ast Expr) {
        match &expr.kind {
            ExprKind::Unit | ExprKind::Int(_) | ExprKind::Bool(_) | ExprKind::Str(_) => {}
            ExprKind::Var(name) => {
                if !self.resolves(name) {
                    self.errors.push(SemError::UnboundIdent {
                        name: name.clone(),
                        span: expr.span,
                    });
                }
            }
            ExprKind::Call { callee, args } => {
                self.check_expr(callee);
                for arg in args {
                    self.check_expr(arg);
                }
            }
            ExprKind::Unary(_, operand) => self.check_expr(operand),
            ExprKind::Binary(_, lhs, rhs) => {
                self.check_expr(lhs);
                self.check_expr(rhs);
            }
            ExprKind::If {
                cond,
                t_branch,
                f_branch,
            } => {
                self.check_expr(cond);
                self.check_expr(t_branch);
                if let Some(f_branch) = f_branch {
                    self.check_expr(f_branch);
                }
            }
            // The body is checked here, where the lambda is written, with
            // the locals in scope at this point: exactly what the closure
            // captures when it is created.
            ExprKind::Lambda { params, body } => {
                self.check_params(params);
                self.push_scope();
                for param in params {
                    self.bind(param);
                }
                self.check_expr(body);
                self.pop_scope();
            }
            ExprKind::Block { stmts, tail } => {
                self.push_scope();
                for stmt in stmts {
                    match &stmt.kind {
                        // The initializer comes first: in `let x = x + 1`
                        // the `x` on the right is the outer one.
                        StmtKind::Let { id, init } => {
                            self.check_expr(init);
                            self.bind(id);
                        }
                        StmtKind::ExprStmt(expr) => self.check_expr(expr),
                    }
                }
                self.check_expr(tail);
                self.pop_scope();
            }
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(Vec::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    /// Bring `id` into the innermost scope, shadowing any earlier binding of
    /// the same name.
    fn bind(&mut self, id: &'ast Ident) {
        self.scopes
            .last_mut()
            .expect("a binding outside any scope")
            .push(id);
    }

    /// Whether `name` refers to something: a local, else a top-level `def`,
    /// else a builtin, the same order the evaluator looks names up in.
    fn resolves(&self, name: &str) -> bool {
        let local = self.scopes.iter().flatten().any(|id| id.name == name);
        local || self.globals.contains_key(name) || Builtin::from_name(name).is_some()
    }
}

/// Check `ast` before it runs. Returns every error found, in the order the
/// checks ran: names and parameters of all definitions first, then bodies.
pub fn check(ast: &Ast) -> Result<(), Vec<SemError>> {
    let mut checker = Checker::new();
    checker.check(ast);
    if checker.errors.is_empty() {
        Ok(())
    } else {
        Err(checker.errors)
    }
}

#[cfg(test)]
mod tests;
