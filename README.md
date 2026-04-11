# elm-assist

A fast, native Rust toolchain for Elm projects. Single binaries, no Node.js, no runtime dependencies.

Built on [elm-ast](https://crates.io/crates/elm-ast), a complete Elm 0.19.1 parser with 100% parse/round-trip/idempotency on 291 real-world files from 50 packages.

## Tools

### elm-lint

Fast Elm linter with **54 built-in rules** and **25 auto-fixes**. Covers the most popular elm-review rule packages without the plugin system, Node.js dependency, or Elm runtime.

```
elm-lint [src-directory]

Options:
  --fix              Apply auto-fixes interactively
  --fix-all          Apply all auto-fixes without prompting
  --fix-dry-run      Show what auto-fixes would change
  --watch            Re-run on file changes
  --rules <list>     Enable only specific rules (comma-separated)
  --disable <list>   Disable specific rules
  --config <path>    Path to elm-assist.toml
  --json             Output as JSON
  --list             List all available rules
```

Configurable via `elm-assist.toml`:

```toml
[rules]
disable = ["NoMissingTypeAnnotation"]

[rules.severity]
NoDebug = "error"
NoUnusedImports = "warning"

[rules.NoMaxLineLength]
max_length = 100

[rules.CognitiveComplexity]
threshold = 20

[rules.NoInconsistentAliases]
aliases = { "Json.Decode" = "Decode", "Json.Encode" = "Encode" }
```

<details>
<summary>All 54 rules</summary>

**Simplification** (from elm-review-simplify)
- NoIfTrueFalse (fix)
- NoBooleanCase (fix)
- NoAlwaysIdentity (fix)
- NoRedundantCons (fix)
- NoUnnecessaryParens (fix)
- NoNegationOfBooleanOperator (fix)
- NoFullyAppliedPrefixOperator (fix)
- NoIdentityFunction (fix)
- NoListLiteralConcat (fix)
- NoEmptyListConcat (fix)
- NoStringConcat (fix)
- NoBoolOperatorSimplify (fix)
- NoMaybeMapWithNothing (fix)
- NoResultMapWithErr (fix)
- NoPipelineSimplify (fix)
- NoNestedNegation (fix)

**Unused code** (from elm-review-unused)
- NoUnusedImports (fix)
- NoUnusedVariables (fix)
- NoUnusedExports
- NoUnusedCustomTypeConstructors
- NoUnusedCustomTypeConstructorArgs
- NoUnusedModules
- NoUnusedParameters (fix)
- NoUnusedLetBinding (fix)
- NoUnusedPatterns

**Code style** (from elm-review-common and elm-review-code-style)
- NoMissingTypeAnnotation
- NoSinglePatternCase (fix)
- NoExposingAll (fix)
- NoImportExposingAll (fix)
- NoDeprecated
- NoMissingDocumentation
- NoUnnecessaryTrailingUnderscore
- NoPrematureLetComputation
- NoSimpleLetBody (fix)
- NoUnnecessaryPortModule (fix)
- NoMissingTypeAnnotationInLetIn
- NoMissingTypeExpose
- NoRedundantlyQualifiedType (fix)
- NoRecordPatternInFunctionArgs

**Debugging**
- NoDebug (fix)

**Complexity**
- CognitiveComplexity
- NoUnoptimizedRecursion
- NoRecursiveUpdate
- NoConfusingPrefixOperator
- NoShadowing

**Port safety**
- NoDuplicatePorts
- NoUnsafePorts
- NoInconsistentAliases
- NoUnusedDependencies

**Other**
- NoEmptyLet (fix)
- NoEmptyRecordUpdate (fix)
- NoWildcardPatternLast
- NoMaxLineLength
- NoTodoComment

</details>

### elm-unused

Project-wide dead code detection. Finds unused imports, functions, exports, custom type constructors, and types across your entire project.

```
elm-unused [src-directory]
```

### elm-deps

Module dependency graph analyzer. Detects circular dependencies, outputs DOT/Mermaid diagrams, and reports coupling statistics.

```
elm-deps [src-directory]
elm-deps --dot          # DOT format for Graphviz
elm-deps --mermaid      # Mermaid diagram
elm-deps --cycles       # Only show circular dependencies
elm-deps --stats        # Coupling statistics
```

### elm-refactor

Automated refactoring tool.

```
elm-refactor rename Module.name oldName newName
elm-refactor sort-imports [src-directory]
elm-refactor qualify-imports [src-directory]
elm-refactor --dry-run ...
```

### elm-search

Semantic AST-aware code search.

```
elm-search "returns Html" [src-directory]
elm-search "calls Http"
elm-search "case-on Msg"
elm-search "update .name"
elm-search "unused-args"
elm-search "lambda 3"
```

### elm-assist-lsp

Language Server Protocol server providing real-time diagnostics and code actions from elm-lint. Works with any LSP-compatible editor.

A VS Code extension is included in `editors/vscode/`.

## Installation

```sh
# From source
cargo install --path crates/elm-lint
cargo install --path crates/elm-unused
cargo install --path crates/elm-deps
cargo install --path crates/elm-refactor
cargo install --path crates/elm-search
cargo install --path crates/elm-assist-lsp
```

## License

Dual licensed under [Apache 2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT).
