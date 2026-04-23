# elm-fmt

Fast Elm formatter matching `elm-format` output. Built on [elm-ast](https://crates.io/crates/elm-ast).

## Features

- Parallel per-file formatting via rayon
- Two output styles (see [Styles](#styles) below):
  - `elm-format` (default): matches `elm-format <source>` output byte-for-byte
  - `elm-format-converged`: pre-applies elm-format's second-pass mutations, producing a fixed point under repeated `elm-format` runs

## Usage

```bash
# Print formatted output to stdout (default when paths are given without --write/--check)
elm-fmt src/Main.elm

# Read from stdin
cat src/Main.elm | elm-fmt

# Format files in place
elm-fmt --write src/

# Check whether files are already formatted (exits 1 if any would be rewritten)
elm-fmt --check src/

# Use converged style
elm-fmt --style elm-format-converged --write src/
```

Directories are searched recursively for `.elm` files.

## Styles

### `elm-format` (default)

Matches `elm-format <source>` byte-for-byte on real-world packages. Pipelines (`|>`, `|.`, `|=`) are always vertical; records and lists with 2+ entries are always multi-line; `if`-`else` is always multi-line; multi-line source layout is preserved across containers and pipeline chains. Verified across 291 real-world `.elm` files from 50 packages.

Use this style when you want a drop-in replacement for `elm-format` with no behavior difference, including in existing CI pipelines.

### `elm-format-converged`

Same formatting rules as `elm-format`, but pre-applies the mutations `elm-format` would make on a second pass over its own output.

`elm-format` is not fully idempotent. On one specific shape, a line comment followed by a blank line followed by an `import` statement, appearing inside a doc-comment code block (`{-| ... -}`), `elm-format` keeps 1 blank line on the first pass and inserts a second on the next pass. The `elm-format-converged` style skips straight to the 2-blank form that `elm-format` would settle on after repeated passes.

Properties:

- `elm-fmt --style elm-format-converged` output is a fixed point under `elm-format`: running `elm-format` over it produces no changes.
- On every input *except* the 1-blank doc-comment shape, the output is identical to the default `elm-format` style.
- On the 1-blank shape, output differs from `elm-format <source>` on the first pass (agreeing on every subsequent pass).

Use this style when:

- Code is generated programmatically and must remain stable if `elm-format` runs later in the pipeline.
- Downstream tooling (CI formatters, editor integrations, git hooks) re-runs `elm-format`, and you want that re-format to be a no-op.

Use the default `elm-format` style otherwise.

For the full story on these modes (they come from the underlying `elm-ast` printer), see the [elm-ast printing guide](https://github.com/joshburgess/elm-ast/blob/main/docs/printing.md).

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success (or `--check` found no changes) |
| 1 | `--check` found files that would be rewritten |
| 2 | Parse error, I/O error, or bad arguments |

## Build

```bash
cargo build -p elm-fmt --release
```
