# elm-fmt

Fast Elm formatter matching `elm-format` output. Built on [elm-ast-rs](../../).

## Features

- Native Rust, no Node.js or Haskell runtime required
- Parallel per-file formatting via rayon
- Two output styles:
  - `elm-format` (default): matches `elm-format <source>` exactly
  - `elm-format-converged`: pre-applies elm-format's second-pass mutations so the output is a fixed point under elm-format (useful for code generation)

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
