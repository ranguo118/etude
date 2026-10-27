//! User-facing error reports. This is the only module that knows about
//! ariadne; the lexer, parser and evaluator return plain error values and
//! turn them into a `Diagnostic` for printing.

use std::io::Write;

use ariadne::{Color, Config, IndexType, Label, Report, ReportKind, Source};

use crate::span::Span;

/// What went wrong, where, and anything else worth saying about it.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    /// The headline, e.g. "expected `;`, found end of input".
    pub message: String,
    /// Where in the source, and the text shown next to the underline, e.g.
    /// "expected `;`". `None` for errors about the program as a whole.
    pub label: Option<(Span, String)>,
    pub notes: Vec<String>,
}

impl Diagnostic {
    pub fn new(message: impl Into<String>, span: Span, label: impl Into<String>) -> Self {
        Diagnostic {
            message: message.into(),
            label: Some((span, label.into())),
            notes: Vec::new(),
        }
    }

    /// An error with no particular place in the source, such as a missing
    /// `main`.
    pub fn without_location(message: impl Into<String>) -> Self {
        Diagnostic {
            message: message.into(),
            label: None,
            notes: Vec::new(),
        }
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    /// Print the report to stderr, quoting `src` and naming the file `path`.
    pub fn eprint(&self, path: &str, src: &str, color: bool) {
        // Nothing sensible to do if writing to stderr itself fails.
        let _ = self.write(path, src, color, std::io::stderr());
    }

    pub fn write(
        &self,
        path: &str,
        src: &str,
        color: bool,
        mut w: impl Write,
    ) -> std::io::Result<()> {
        let Some((span, label)) = &self.label else {
            // ariadne prints only the headline when nothing is underlined, so
            // the file and notes are written by hand.
            writeln!(w, "Error: {}", self.message)?;
            writeln!(w, "  in {path}")?;
            for note in &self.notes {
                writeln!(w, "  Note: {note}")?;
            }
            return Ok(());
        };

        let range = span.start..span.end;
        let config = Config::default()
            // Spans are byte offsets; ariadne counts chars by default.
            .with_index_type(IndexType::Byte)
            .with_color(color);
        let mut report = Report::build(ReportKind::Error, (path, range.clone()))
            .with_config(config)
            .with_message(&self.message)
            .with_label(
                Label::new((path, range))
                    .with_message(label)
                    .with_color(Color::Red),
            );
        for note in &self.notes {
            report = report.with_note(note);
        }
        report.finish().write((path, Source::from(src)), w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(diag: &Diagnostic, src: &str) -> String {
        let mut out = Vec::new();
        diag.write("test.etd", src, false, &mut out).unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn line_and_column_use_byte_offsets() {
        // `é` is two bytes; with char offsets the column would be off by one.
        let src = "let é = 1;\nfoo bar";
        let start = src.find("bar").unwrap();
        let span = Span {
            start,
            end: start + 3,
        };
        let out = render(&Diagnostic::new("oops", span, "here"), src);
        assert!(out.starts_with("Error: oops"), "{out}");
        assert!(out.contains("test.etd:2:5"), "{out}");
        assert!(out.contains("here"), "{out}");
    }

    #[test]
    fn notes_are_shown() {
        let span = Span { start: 0, end: 1 };
        let diag = Diagnostic::new("oops", span, "here").with_note("a hint");
        assert!(render(&diag, "x").contains("Note: a hint"));
    }

    #[test]
    fn without_location() {
        let diag = Diagnostic::without_location("no `main` function").with_note("add one");
        assert_eq!(
            render(&diag, "def f() = 1;"),
            "Error: no `main` function\n  in test.etd\n  Note: add one\n"
        );
    }

    #[test]
    fn no_color_codes_when_disabled() {
        let span = Span { start: 0, end: 1 };
        assert!(!render(&Diagnostic::new("oops", span, "here"), "x").contains('\x1b'));
    }
}
