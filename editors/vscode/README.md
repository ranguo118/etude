# Etude for VS Code

Syntax highlighting for [etude](https://github.com/guo543/etude) (`.etd`
files): comments, strings and their escapes, keywords, numbers, builtins,
function names and operators. Invalid escapes and number literals are
marked the way the parser reports them. `Cmd+/` toggles `//` comments, and
brackets and quotes close automatically.

There is no language server yet, so errors only show up when you run the
program.

## Install

Link this folder into VS Code's extensions directory and reload the window
(`Developer: Reload Window`):

```
ln -s "$PWD/editors/vscode" ~/.vscode/extensions/etude
```

Or open this folder in VS Code and press `F5` to try it in an Extension
Development Host window.

To build an installable package:

```
npx @vscode/vsce package
code --install-extension etude-0.1.0.vsix
```

## Changing the grammar

The rules are in `syntaxes/etude.tmLanguage.json`, a TextMate grammar: each
rule is an Oniguruma regex and a scope name that color themes style. Keep it
in sync with the lexer (`src/lexer.rs`): keywords, operators and builtins are
listed in it by hand. `Developer: Inspect Editor Tokens and Scopes` shows
which scope a token got.
