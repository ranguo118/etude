use crate::span::Span;
use std::{fmt, rc::Rc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnOp {
    /// `-x`: integer negation.
    Neg,
    /// `!x`: boolean negation.
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BinOp {
    // Arithmetic: integers to an integer.
    Add,
    Sub,
    Mul,
    Div,
    /// The remainder of `Div`, with the sign of the dividend, as in Rust.
    Rem,

    // Comparison: two operands to a boolean.
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,

    // Logical: booleans to a boolean. The right operand is only evaluated
    // when the left one does not already decide the result.
    And,
    Or,
}

impl fmt::Display for UnOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            UnOp::Neg => "-",
            UnOp::Not => "!",
        };
        f.write_str(text)
    }
}

impl fmt::Display for BinOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            // Arithmetic
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Rem => "%",
            // Comparison
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
            // Logical
            BinOp::And => "&&",
            BinOp::Or => "||",
        };
        f.write_str(text)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Ident {
    pub(crate) name: String,
    pub(crate) span: Span,
}

#[derive(Debug, Clone)]
pub(crate) enum StmtKind {
    Let { id: Ident, init: Expr },
    ExprStmt(Expr),
}

#[derive(Debug, Clone)]
pub(crate) enum ExprKind {
    Unit,
    Int(i64),
    Bool(bool),
    Str(Rc<str>),
    Var(String),
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    /// `if cond { … } else { … }`; `else if` is an `If` as the else branch.
    If {
        cond: Box<Expr>,
        t_branch: Box<Expr>,
        f_branch: Option<Box<Expr>>,
    },
    Lambda {
        params: Vec<Ident>,
        body: Box<Expr>,
    },
    Block {
        stmts: Vec<Stmt>,
        tail: Box<Expr>,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct Stmt {
    pub(crate) kind: StmtKind,
    #[allow(dead_code, reason = "only tests read it so far; diagnostics will")]
    pub(crate) span: Span,
}

#[derive(Debug, Clone)]
pub(crate) struct Expr {
    pub(crate) kind: ExprKind,
    pub(crate) span: Span,
}

impl Stmt {
    pub(crate) fn let_decl(id: Ident, init: Expr, start: usize, end: usize) -> Self {
        Stmt {
            kind: StmtKind::Let { id, init },
            span: Span { start, end },
        }
    }

    pub(crate) fn expr_stmt(expr: Expr, start: usize, end: usize) -> Self {
        Stmt {
            kind: StmtKind::ExprStmt(expr),
            span: Span { start, end },
        }
    }
}

impl Expr {
    pub(crate) fn unit(start: usize, end: usize) -> Self {
        Expr {
            kind: ExprKind::Unit,
            span: Span { start, end },
        }
    }

    pub(crate) fn int(n: i64, start: usize, end: usize) -> Self {
        Expr {
            kind: ExprKind::Int(n),
            span: Span { start, end },
        }
    }

    pub(crate) fn bool(b: bool, start: usize, end: usize) -> Self {
        Expr {
            kind: ExprKind::Bool(b),
            span: Span { start, end },
        }
    }

    pub(crate) fn str(s: String, span: Span) -> Self {
        Expr {
            kind: ExprKind::Str(Rc::from(s)),
            span,
        }
    }

    pub(crate) fn var(s: String, start: usize, end: usize) -> Self {
        Expr {
            kind: ExprKind::Var(s),
            span: Span { start, end },
        }
    }

    pub(crate) fn call(callee: Expr, args: Vec<Expr>, start: usize, end: usize) -> Self {
        Expr {
            kind: ExprKind::Call {
                callee: Box::new(callee),
                args,
            },
            span: Span { start, end },
        }
    }

    pub(crate) fn unary(op: UnOp, expr: Expr, start: usize, end: usize) -> Self {
        Expr {
            kind: ExprKind::Unary(op, Box::new(expr)),
            span: Span { start, end },
        }
    }

    pub(crate) fn binary(op: BinOp, lhs: Expr, rhs: Expr, start: usize, end: usize) -> Self {
        Expr {
            kind: ExprKind::Binary(op, Box::new(lhs), Box::new(rhs)),
            span: Span { start, end },
        }
    }

    pub(crate) fn if_expr(
        cond: Expr,
        t_branch: Expr,
        f_branch: Option<Expr>,
        start: usize,
        end: usize,
    ) -> Self {
        Expr {
            kind: ExprKind::If {
                cond: Box::new(cond),
                t_branch: Box::new(t_branch),
                f_branch: f_branch.map(Box::new),
            },
            span: Span { start, end },
        }
    }

    pub(crate) fn lambda(params: Vec<Ident>, body: Expr, start: usize, end: usize) -> Self {
        Expr {
            kind: ExprKind::Lambda {
                params,
                body: Box::new(body),
            },
            span: Span { start, end },
        }
    }

    pub(crate) fn block(stmts: Vec<Stmt>, tail: Expr, start: usize, end: usize) -> Self {
        Expr {
            kind: ExprKind::Block {
                stmts,
                tail: Box::new(tail),
            },
            span: Span { start, end },
        }
    }
}

impl fmt::Display for Stmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            StmtKind::Let { id, init } => write!(f, "LET: {} = {init} ;", id.name),
            StmtKind::ExprStmt(expr) => write!(f, "EXPRSTMT: {expr} ;"),
        }
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ExprKind::Unit => write!(f, "()"),
            ExprKind::Int(n) => write!(f, "{n}"),
            ExprKind::Bool(b) => write!(f, "{b}"),
            ExprKind::Str(s) => write!(f, "{s:?}"),
            ExprKind::Var(s) => write!(f, "{s}"),
            ExprKind::Call { callee, args } => {
                write!(f, "{}(", callee)?;
                for (i, arg) in args.iter().enumerate() {
                    write!(f, "{arg}")?;
                    if i != args.len() - 1 {
                        write!(f, ", ")?;
                    }
                }
                write!(f, ")")
            }
            ExprKind::Unary(op, expr) => write!(f, "({op} {expr})"),
            ExprKind::Binary(op, lhs, rhs) => write!(f, "({lhs} {op} {rhs})"),
            ExprKind::If {
                cond,
                t_branch,
                f_branch,
            } => {
                write!(f, "(IF {cond} {t_branch}")?;
                if let Some(f_branch) = f_branch {
                    write!(f, " {f_branch}")?;
                }
                write!(f, ")")
            }
            ExprKind::Lambda { params, body } => {
                write!(f, "FUN (")?;
                for (i, param) in params.iter().enumerate() {
                    write!(f, "{}", param.name)?;
                    if i != params.len() - 1 {
                        write!(f, ", ")?;
                    }
                }
                write!(f, ") => {body}")
            }
            ExprKind::Block { stmts, tail } => {
                write!(f, "BLOCK: {{ ")?;
                for stmt in stmts {
                    write!(f, "{stmt} ")?;
                }
                write!(f, "{tail} ")?;
                write!(f, "}}")
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum DeclKind {
    // VarDecl {
    //     id: String,
    //     init: Expr,
    // },
    FunDecl {
        id: Ident,
        params: Vec<Ident>,
        body: Expr,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct Decl {
    pub(crate) kind: DeclKind,
    #[allow(dead_code, reason = "only tests read it so far; diagnostics will")]
    pub(crate) span: Span,
}

impl Decl {
    pub(crate) fn function(
        id: Ident,
        params: Vec<Ident>,
        body: Expr,
        start: usize,
        end: usize,
    ) -> Self {
        Decl {
            kind: DeclKind::FunDecl { id, params, body },
            span: Span { start, end },
        }
    }
}

impl fmt::Display for Decl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            DeclKind::FunDecl { id, params, body } => {
                write!(f, "DEF {} (", id.name)?;
                for (i, param) in params.iter().enumerate() {
                    write!(f, "{}", param.name)?;
                    if i != params.len() - 1 {
                        write!(f, ", ")?;
                    }
                }
                write!(f, ") = {body}")
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Ast {
    pub(crate) decls: Vec<Decl>,
}

impl fmt::Display for Ast {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "AST:")?;
        for decl in &self.decls {
            writeln!(f, "{decl}")?;
        }
        Ok(())
    }
}
