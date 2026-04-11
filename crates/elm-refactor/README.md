# elm-refactor

Automated cross-file refactoring tool for Elm projects. Built on [elm-ast-rs](../../).

## Commands

### `rename`

Rename a function or value across the entire project. Updates the definition, all local references, all qualified references from other modules, and import/exposing lists.

```bash
elm-refactor rename Module.func oldName newName [src-dir]
```

**Example:**
```bash
$ elm-refactor rename Main.helper oldHelper newHelper src

Renamed `oldHelper` to `newHelper` in `Main`: 5 occurrence(s), 3 file(s) modified.
```

### `sort-imports`

Sort all import declarations alphabetically in every module.

```bash
elm-refactor sort-imports [src-dir] [--dry-run]
```

### `qualify-imports`

Convert unqualified exposed imports to qualified form. Transforms `import Html exposing (div)` with usage of `div` into `Html.div`, and removes `div` from the exposing list.

```bash
elm-refactor qualify-imports [src-dir] [--dry-run]
```

**Example:**
```bash
$ elm-refactor qualify-imports src --dry-run

Would qualify 44 reference(s). (dry run)
```

## Options

All commands support:
- `--dry-run` -- show what would change without modifying files
- The source directory defaults to `src` if not specified

## Build

```bash
cargo build -p elm-refactor --release
```
