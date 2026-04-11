# elm-deps

Module dependency graph analyzer for Elm projects. Built on [elm-ast-rs](../../).

## Features

- Dependency tree visualization
- Circular dependency detection
- Coupling metrics (afferent/efferent)
- DOT output for Graphviz
- Mermaid diagram output for GitHub/GitLab markdown

## Usage

```bash
# Summary with dependency tree + stats
elm-deps [src-directory]

# Graphviz DOT format (pipe to `dot -Tsvg -o deps.svg`)
elm-deps --dot src

# Mermaid diagram (paste into markdown)
elm-deps --mermaid src

# Only check for circular dependencies
elm-deps --cycles src

# Coupling statistics
elm-deps --stats src
```

## Example output

```
$ elm-deps src

Analyzed 13 modules in 18.2ms

Element
  -> Internal.Flag
  -> Internal.Model
  -> Internal.Style
Element.Input
  -> Element
  -> Element.Background
  -> Element.Border

No circular dependencies found.

Dependency statistics:
  13 modules, 36 internal edges
  2.8 avg imports per module
  2 leaf modules (no internal imports)
  4 root modules (not imported by others)

Most depended on (highest efferent coupling):
   10 Internal.Model
    8 Element
    7 Internal.Flag
```

## Build

```bash
cargo build -p elm-deps --release
```
