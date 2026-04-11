# elm-search

Semantic AST-aware code search for Elm projects. Built on [elm-ast-rs](../../).

Unlike `grep`, `elm-search` understands Elm syntax and can answer structural questions about your code.

## Query types

| Query | Description | Example |
|---|---|---|
| `returns <Type>` | Functions whose return type contains a type | `returns Maybe` |
| `type <Type>` | Functions mentioning a type anywhere in signature | `type Decoder` |
| `case-on <Name>` | Case expressions matching on a constructor | `case-on Result` |
| `update .<field>` | Record updates touching a field | `update .name` |
| `calls <Module>` | All qualified calls to a module | `calls Http` |
| `unused-args` | Functions with arguments never used in the body | `unused-args` |
| `lambda <N>` | Lambdas with N or more arguments | `lambda 3` |
| `uses <name>` | All references to a function or value | `uses andThen` |
| `def <pattern>` | Definitions matching a name (substring) | `def update` |
| `expr <kind>` | Expressions by kind: let, case, if, lambda, record, list, tuple | `expr let` |

## Usage

```bash
elm-search [--dir <path>] <query>
```

Defaults to `src` if `--dir` is not specified.

## Examples

```bash
# Find all functions that return Maybe
$ elm-search --dir src returns Maybe

src/Dict.elm:94:1: get : ... -> Maybe
src/List.elm:311:1: maximum : ... -> Maybe
src/List.elm:536:1: head : ... -> Maybe

# Find unused function arguments
$ elm-search --dir src unused-args

src/Array.elm:572:8: append: argument `aTail` is never used
src/Dict.elm:264:20: removeHelpPrepEQGT: argument `targetKey` is never used

# Find all case expressions matching on Just
$ elm-search --dir src case-on Just

src/Dict.elm:115:3: case matching on Just
src/Maybe.elm:61:5: case matching on Just

# Find all record updates to the `count` field
$ elm-search --dir src update .count

src/Main.elm:45:5: { model | count = ... }

# Find all qualified calls to the Http module
$ elm-search --dir src calls Http

src/Api.elm:23:5: Http.get
src/Api.elm:45:5: Http.post
```

Output format is `file:line:col: description`, compatible with editor jump-to-location.

## Build

```bash
cargo build -p elm-search --release
```
