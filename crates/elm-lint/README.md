# elm-lint

Fast Elm linter with 14 built-in rules. Built on [elm-ast-rs](../../).

## Rules

| Rule | Description |
|---|---|
| `NoUnusedImports` | Imports where nothing from the module is used |
| `NoUnusedVariables` | Let bindings that are never referenced |
| `NoDebug` | `Debug.log`, `Debug.todo`, `Debug.toString` usage |
| `NoMissingTypeAnnotation` | Top-level functions without type signatures |
| `NoSinglePatternCase` | Case with one branch (use `let` destructuring) |
| `NoBooleanCase` | Case on `True`/`False` (use `if`/`else`) |
| `NoIfTrueFalse` | `if x then True else False` (simplify to `x`) |
| `NoUnnecessaryParens` | Parentheses around simple atoms |
| `NoNestedNegation` | `not (not x)` or double negation |
| `NoEmptyLet` | `let ... in` with no declarations |
| `NoEmptyRecordUpdate` | `{ r \| }` with no actual updates |
| `NoAlwaysIdentity` | `always identity`, `identity >> f` |
| `NoRedundantCons` | `x :: []` (use `[ x ]`) |
| `NoWildcardPatternLast` | Wildcard `_` not as last case branch |

## Usage

```bash
# Lint with all rules
elm-lint [src-directory]

# List available rules
elm-lint --list

# Run only specific rules
elm-lint --rules NoDebug,NoUnusedImports src
```

## Example output

```
$ elm-lint src

Linted 26 files with 14 rules in 7.4ms

src/VirtualDom/Styled.elm:45:1: [NoUnusedImports] Import `Hex` is not used
src/View.elm:12:1: [NoMissingTypeAnnotation] `helper` is missing a type annotation

2 errors in 2 files
  1 NoUnusedImports
  1 NoMissingTypeAnnotation
```

Output format is `file:line:col: [Rule] message`, compatible with editor error parsers.

## Build

```bash
cargo build -p elm-lint --release
```
