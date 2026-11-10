//! Types, shared by the evaluator, the builtins and the checks.

use std::fmt;

/// The type of a value, as reported in errors. There is no type checker
/// yet, so this is only known at run time; closures and builtins are both
/// `Fun`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Type {
    Unit,
    Int,
    Bool,
    Fun,
    Str,
}

/// The type's name, as it will appear in signatures and type errors.
impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Unit => write!(f, "()"),
            Type::Int => write!(f, "int"),
            Type::Fun => write!(f, "fun"),
            Type::Bool => write!(f, "bool"),
            Type::Str => write!(f, "string"),
        }
    }
}
