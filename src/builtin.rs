//! The functions that come with the language. This module says what they
//! are called and what they take; the evaluator runs them.

use crate::types::Type;
use std::fmt;

/// A function implemented in Rust rather than in etude. Programs see it as
/// a global under its `name`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Builtin {
    Print,
    Println,
    IntToStr,
    Concat,
}

impl Builtin {
    /// Every builtin, for pre-filling the globals.
    pub(crate) const ALL: [Builtin; 4] = [
        Builtin::Print,
        Builtin::Println,
        Builtin::IntToStr,
        Builtin::Concat,
    ];

    /// The name the program calls it by.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Builtin::Print => "print",
            Builtin::Println => "println",
            Builtin::IntToStr => "int_to_str",
            Builtin::Concat => "concat",
        }
    }

    /// The type of each parameter. Arguments are checked against these
    /// before the builtin runs.
    pub(crate) fn params(self) -> &'static [Type] {
        match self {
            Builtin::Print | Builtin::Println => &[Type::Str],
            Builtin::IntToStr => &[Type::Int],
            Builtin::Concat => &[Type::Str, Type::Str],
        }
    }
}

impl fmt::Display for Builtin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_unique() {
        let mut names: Vec<_> = Builtin::ALL.iter().map(|b| b.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), Builtin::ALL.len());
    }
}
