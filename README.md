# etude

[![CI](https://github.com/guo543/etude/actions/workflows/ci.yml/badge.svg)](https://github.com/guo543/etude/actions/workflows/ci.yml)

A small, expression-oriented programming language that leans functional
(immutable bindings, first-class functions, closures) while still allowing
side effects. Written in Rust as a learning project.

```
def make_adder(n) = fun(x) => x + n;

def compose(f, g) = fun(x) => f(g(x));

def main() = {
    let add10 = make_adder(10);
    let double = fun(x) => x * 2;
    compose(double, add10)(1)
};
```

The planned language is described in [`docs/grammar.ebnf`](docs/grammar.ebnf).

## Usage

```
cargo run -- examples/closures.etd          # run a program: prints `Result: …`
cargo run -- --ast examples/closures.etd    # print the parsed program first
cargo run -- --help
```

Tests:

```
cargo test                                  # unit tests and all examples
BLESS=1 cargo test --test examples          # rewrite expected example output
```

`examples/NAME.etd` must run and print `examples/NAME.out`;
`examples/errors/NAME.etd` must fail with the report in
`examples/errors/NAME.err`.

## Where things stand

- **Lexer** (`src/lexer.rs`): tokens carry only a kind and a span; it never
  fails (bad characters become `Unknown`). Tokens: `+ - * / %`,
  `== != < <= > >=`, `! && ||`, `= => , ; ( ) { }`, keywords
  `let def fun if else true false`, identifiers (ASCII letters, digits, `_`),
  integers and string literals. Whitespace and `//` line comments are
  skipped.
- **Parser** (`src/parser.rs`): rustc-style recursive descent that always
  holds the current token. A program is a list of `def name(params) = expr;`.
  Expressions: integers, strings (escapes `\n \r \t \\ \" \0`),
  `true`/`false`, unit `()`, variables, unary `-` and `!`, binary operators
  from a precedence table in the parser (`||` < `&&` < comparisons < `+ -` <
  `* / %`), calls `f(a)(b)` on any expression, lambdas `fun(params) => body`,
  `if c { … } else if d { … } else { … }`, and blocks
  `{ let x = e; e; tail }`. Parameter and argument lists allow a trailing
  comma. Every node has a span; errors at end of input point right after the
  last token.
- **Evaluator** (`src/eval.rs`): a tree walker that borrows from the AST.
  Top-level `def`s live in a global table and are closures with an empty
  captured environment; locals live in an immutable linked environment, so
  closures capture by sharing it. Values: integers, booleans, unit `()`,
  strings, functions and builtins. Builtins are an enum dispatched by
  `match`: `print(s)` and `println(s)` write a string to stdout (the latter
  with a newline), `int_to_str(n)` formats an integer and `concat(a, b)`
  joins two strings. `%` is the remainder, with the sign of the dividend as
  in Rust. `&&` and `||` short-circuit; `if` evaluates only the chosen
  branch, so recursion terminates. Integer overflow, division by zero, wrong
  argument counts and type mismatches are runtime errors, and so is
  recursion deeper than 10,000 calls; `stacker` grows the native stack so
  deeply nested evaluation does not overflow it first.
- **Checks** (`src/check.rs`): run after parsing, before anything is
  evaluated. They report every variable that refers to nothing (even in
  code that never runs), duplicate top-level definitions, duplicate
  parameters and definitions that reuse a builtin's name, all at once.
- **Errors** (`src/diagnostic.rs`): reported with ariadne. Each error's
  `Display` is the headline; `diagnostic()` adds the underlined location,
  label, secondary labels (e.g. "first defined here") and notes.
- **CLI** (`src/main.rs`, clap): `etude [--ast] [--color auto|always|never]
  <file>`.
- **Tests:** 201 unit tests; 50 example programs (19 that run, including
  factorial, fibonacci, FizzBuzz and Church numerals, and 31 that must fail
  with a specific report).

## Design decisions so far

- **Blocks, not `let … in`.** `{ let x = e; …; tail }` is an expression whose
  value is its tail; it means `let x = e in { … }`. The tail is required:
  there is no implicit unit, so a block that ends in `;` is an error.
- **`def` defines functions only,** always with a parameter list, and ends
  with `;`. `def f(a) = body;` means `f = fun(a) => body`.
- **Lambdas:** `fun(params) => body`. `=` is for definitions, `=>` maps
  parameters to a body. The body extends as far right as possible.
- **Scoping is lexical.** A function sees its parameters, its own locals,
  whatever it captured where it was written, and globals — never its
  caller's locals.
- **Shadowing is allowed for locals,** in nested and in the same scope
  (`let x = x + 1;`), as in OCaml and Rust. Top-level names must be unique,
  because top-level definitions see each other in any order.
- **Functions take a parameter list and are not curried.** Currying is
  written out: `def add(a) = fun(b) => a + b;`.
- **`if` requires braces;** `else` is optional, and without it the `if` is
  unit. Braces go where expressions follow each other (condition, then
  branch); a construct that ends in a single expression (`def` and lambda
  bodies) needs none.
- **Comparisons cannot be chained:** `a < b < c` and `a == b < c` are
  errors. `==`/`!=` share one precedence level with `<`, `<=`, `>`, `>=`, as
  in Rust and Swift.
- **`&&` and `||` short-circuit.** `==` works on integers, booleans,
  strings and unit; functions cannot be compared.
- **Strings are UTF-8** with a fixed set of escapes; any other escape is an
  error.
- **Builtins are ordinary global functions** that cannot be redefined at
  the top level; a local `let` or parameter may shadow them.
- **The lexer never fails;** bad input becomes a token that the parser
  reports where it reaches it.

---

# Roadmap

Each step ends with something that runs and is tested.

## Done

- Lexer, parser and AST with spans; `def` items with `;`; blocks with `let`
  and expression statements; unary minus; calls on any expression.
- Tree-walking evaluator: globals table, immutable linked environment,
  borrowing from the AST; `main` must exist and take no parameters.
- Lambdas and closures (`fun(params) => body`), currying, functions as values.
- Booleans, comparisons, `!`/`&&`/`||` with short-circuiting, `if`/`else if`/
  `else` as expressions, unit; factorial, fibonacci and mutual recursion run.
- Unit literal `()`, `//` comments, trailing commas, identifiers starting
  with `_`.
- Strings: literals with escapes, `==`, `concat`; `%`.
- Builtins: `print`, `println`, `int_to_str`, `concat`.
- Call-depth limit and stack growth with `stacker`.
- Error reports with ariadne; clap CLI with `--ast` and `--color`.
- End-to-end example tests with `BLESS=1`.

## 1. Loose ends

- [x] **Duplicate top-level names are an error:** "`f` is defined more than
      once", pointing at both definitions.
- [x] **Call-depth limit:** more than 10,000 nested calls is a "recursion
      limit reached" error; `stacker` keeps the native stack from
      overflowing first.
- [x] **`parse_comma_list`:** one loop for `def` parameters, lambda
      parameters and call arguments.
- [ ] **Share `def` and lambda:** a `Lambda { params, body }` struct used by
      both `DeclKind::FunDecl { id, lambda }` and `ExprKind::Lambda`, since a
      `def` is a name bound to a lambda.

## 2. Booleans, comparisons and `if` — done

See "Done" above.

## 3. Unit and `print`

- [x] `Value::Unit` and `Type::Unit`; `if` without `else` is unit.
- [x] `()` literal: `(` directly followed by `)` in `parse_atom`.
- [x] Built-in functions (`Value::Builtin`): `print(s)` and `println(s)`,
      returning unit, `int_to_str(n)` and `concat(a, b)`.
- [x] ~~Optional block tail~~: decided against; a block needs a tail.

## 4. Global `let`

Undecided: maybe no global `let` at all, or only constants.

- [ ] Top-level `let`, evaluated eagerly and in order before `main`.
- [ ] Reading a global before it is initialized is a runtime error.
- [ ] Top-level parsing dispatches on `def` / `let` (`parse_item`), with an
      "expected a definition" error otherwise.

## 5. Strings

- [x] String literals with escapes, `Value::Str`, a `string` type, and
      concatenation (the `concat` builtin).
- [ ] A `++` operator; ordering; length and slicing without integer
      indexing (UTF-8), once there is an `option` type.

## 6. Algebraic data types and `match` (checked at run time)

```
type list<a> = | Nil | Cons(a, list<a>);
type shape = | Circle(int) | Rect { w: int, h: int };

def sum(xs) = match xs {
    Nil => 0,
    Cons(x, rest) => x + sum(rest),
};
```

- [ ] `type` items with type parameters; constructors start with an
      uppercase letter, variables and types with a lowercase one.
- [ ] Type syntax (`int`, `list<a>`, `(int, int) -> int`), shared with the
      later optional annotations.
- [ ] Constructor expressions and `match` with patterns: `_`, variables,
      literals, nested constructors; later `|` alternatives and `if` guards.
- [ ] `Value::Data { tag, fields }`.
- [ ] Possibly: `bool` and `option` as ordinary ADTs, `if` as sugar for
      `match`.

## 7. Name resolution

- [x] A pass between parsing and evaluation: "cannot find `x`" before
      running, duplicate definitions and parameters.
- [ ] Record what each variable use refers to (a local, a `def` or a
      builtin), and the call graph between `def`s, for the type checker.
- [ ] Move the remaining program-level checks (`main` exists and takes no
      parameters) from the evaluator into it.

## 8. Hindley-Milner type checking

A separate pass between parsing and running.

- [ ] Types `int`, `bool`, `unit`, `string`, functions, ADTs with parameters,
      and type variables in a union-find table with levels.
- [ ] `unify` with occurs check; level-based `generalize`; `instantiate`;
      let-polymorphism in blocks.
- [ ] Top-level `def`s and `type`s: dependency graph and strongly connected
      components (Tarjan); each group is checked together, then generalized.
- [ ] Exhaustiveness and redundancy checking for `match` (Maranget).
- [ ] Decide: is `==` polymorphic or only for `int` and `bool`?
- [ ] Decide: what does `main` return (any value, printed; or `int` as an
      exit code)?
- [ ] Discarding a non-unit value in a statement (`1 + 1;`) is an error.
- [ ] Runtime type checks in the evaluator become internal errors.

## 9. Records

- [ ] Nominal records: `type person = { name: string, age: int };`, field
      access `p.name`, functional update.
- [ ] Record literals vs. blocks: `{ name: …` (identifier then `:`) starts a
      record; in `if`/`match` conditions record literals need parentheses.

## 10. Error recovery

- [ ] Collect errors in a `Vec` instead of stopping at the first one.
- [ ] Skip ahead to `;`, `}`, `def`, `let` or `type` after an error.
- [ ] `ExprKind::Error` nodes that later passes accept without producing
      follow-up errors.

## Open questions

- **Top-level `;`:** drop it entirely? Unambiguous, since every top-level
  item starts with a keyword; the cost is that a top-level `let` would look
  different from one in a block. Decide together with global `let`.
- **Statements ending in `}` without `;`** (`if c { print(x) }`): allow when
  the statement is a block or an `if`? A following line starting with `-` or
  `(` would continue the expression. Today the `;` is required:
  `{ if c { println(x) } 1 }` is a syntax error.

## Later

- Optional type annotations: `def f(x: int) -> int`, `fun(x: int) => …`;
  polymorphic recursion with an annotated signature.
- Symbol interning (`Symbol(u32)`), so names are copied and compared as
  integers.
- Faster closures: with name resolution, array frames or flat closures
  instead of the linked environment.
- Local recursive functions (`def` or `let rec` inside blocks).
- A depth limit in the parser for very deeply nested input.
- Warnings: unused variables, shadowed bindings that are never read.
