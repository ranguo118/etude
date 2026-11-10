use std::fs;
use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser as _, ValueEnum};

use etude::check::{self, SemError};
use etude::diagnostic::Diagnostic;
use etude::eval::{self, EvalError};
use etude::lexer::Lexer;
use etude::parser::{Parser, ParserError};

/// Run an etude program: define its functions, then call `main`.
#[derive(clap::Parser)]
#[command(version, about)]
struct Cli {
    /// The program to run.
    file: PathBuf,

    /// Print the parsed program before running it.
    #[arg(long)]
    ast: bool,

    /// When to color error messages.
    #[arg(long, value_enum, value_name = "WHEN", default_value_t = Color::Auto)]
    color: Color,
}

#[derive(Clone, Copy, ValueEnum)]
enum Color {
    /// Only when writing to a terminal.
    Auto,
    /// Even when writing to a file or pipe, e.g. into `less -R`.
    Always,
    /// Plain text.
    Never,
}

impl Color {
    fn enabled(self) -> bool {
        match self {
            Color::Auto => io::stderr().is_terminal(),
            Color::Always => true,
            Color::Never => false,
        }
    }
}

enum Error {
    Parse(ParserError),
    /// Every problem the checks found; the program did not run.
    Check(Vec<SemError>),
    Eval(EvalError),
}

impl Error {
    fn diagnostics(&self) -> Vec<Diagnostic> {
        match self {
            Error::Parse(e) => vec![e.diagnostic()],
            Error::Check(errors) => errors.iter().map(SemError::diagnostic).collect(),
            Error::Eval(e) => vec![e.diagnostic()],
        }
    }
}

/// Lex, parse, check and evaluate a source file, returning the printed
/// result. With `print_ast`, the parsed program is printed before it is
/// checked.
///
/// The result is formatted here because values borrow from the AST, which
/// only lives inside this function.
fn run(input: &str, print_ast: bool) -> Result<String, Error> {
    let ast = Parser::new(Lexer::new(input))
        .parse()
        .map_err(Error::Parse)?;
    if print_ast {
        println!("{ast}");
    }
    check::check(&ast).map_err(Error::Check)?;
    eval::eval(&ast)
        .map(|value| value.to_string())
        .map_err(Error::Eval)
}

fn main() -> ExitCode {
    // On bad arguments clap prints the error and usage and exits with code 2.
    let cli = Cli::parse();
    let path = cli.file.display().to_string();

    let src = match fs::read_to_string(&cli.file) {
        Ok(src) => src,
        Err(e) => {
            eprintln!("error: cannot read {path}: {e}");
            return ExitCode::FAILURE;
        }
    };

    match run(&src, cli.ast) {
        Ok(value) => {
            println!("Result: {value}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            for diagnostic in e.diagnostics() {
                diagnostic.eprint(&path, &src, cli.color.enabled());
            }
            ExitCode::FAILURE
        }
    }
}
