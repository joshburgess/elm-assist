# elm-unused

Fast project-wide dead code detector for Elm projects. Built on [elm-ast-rs](../../).

## What it detects

- **Unused imports** -- module imported but nothing from it is used
- **Unused import exposings** -- specific name in exposing list never referenced
- **Unused functions** -- defined but never called or exported
- **Unused exports** -- exported but never imported by other project modules
- **Unused constructors** -- custom type constructors never constructed or pattern-matched
- **Unused types** -- type aliases or custom types never referenced

## Usage

```bash
elm-unused [src-directory]
```

Defaults to `src` if no directory is given.

## Example output

```
$ elm-unused src

Scanned 18 files (18 modules) in 10.3ms

Array:
    export append
    export filter
    function emptyBuilder
Dict:
    export fromList
    export isEmpty

Summary: 6 findings
  4 unused export
  2 unused function
```

## Note on library packages

For library packages (like `elm/core`), most "unused exports" are expected -- they're used by consumers, not within the package itself. This tool is most useful for application code.

## Build

```bash
cargo build -p elm-unused --release
```
