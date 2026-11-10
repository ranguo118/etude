use std::collections::HashMap;

use crate::{
    ast::{Ast, Decl, DeclKind, Ident},
    builtin::Builtin,
    span::Span,
};

enum SemError {
    RedefinesBuiltin { name: String, span: Span },
}

struct Checker<'ast> {
    /// Top-level names and where each was first defined.
    globals: HashMap<&'ast str, Span>,
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

    pub fn check(ast: &Ast) {
        for decl in &ast.decls {}
    }
}
