# Elm Assist LSP

A fast Elm development extension for VS Code, powered by a native Rust language server.

Real-time diagnostics, auto-fix code actions, and hover documentation for **54 built-in lint rules** — no Node.js runtime, no Elm compiler dependency, no plugin system to configure.

## Features

### Real-time diagnostics

Lint errors appear as you type with 150ms debouncing. Parse errors are shown inline when syntax is invalid, while lint rules continue to run on the valid portions of your file.

### Quick-fix code actions

25 rules offer auto-fixes via the lightbulb menu or `Ctrl+.` / `Cmd+.`. Fixes include removing unused imports, simplifying boolean expressions, collapsing redundant patterns, and more.

### Hover documentation

Hover over any diagnostic to see the rule name, description, and whether an auto-fix is available.

### File watching

External changes (git checkout, build tools) are detected automatically. Config changes to `elm-assist.toml` trigger a live reload without restarting the server.

### Project-aware analysis

The server scans your entire project at startup, enabling cross-file rules like unused exports, unused modules, and unused custom type constructors.

## Rules

54 rules covering simplification, unused code, code style, debugging, complexity, and port safety. 25 of these offer auto-fixes.

| Category | Rules | Auto-fixable |
|----------|-------|-------------|
| Simplification | 16 | 16 |
| Unused code | 9 | 4 |
| Code style | 14 | 7 |
| Debugging | 1 | 1 |
| Complexity | 5 | 0 |
| Port safety / other | 9 | 2 |

<details>
<summary>Full rule list</summary>

**Simplification**
NoIfTrueFalse (fix), NoBooleanCase (fix), NoAlwaysIdentity (fix), NoRedundantCons (fix), NoUnnecessaryParens (fix), NoNegationOfBooleanOperator (fix), NoFullyAppliedPrefixOperator (fix), NoIdentityFunction (fix), NoListLiteralConcat (fix), NoEmptyListConcat (fix), NoStringConcat (fix), NoBoolOperatorSimplify (fix), NoMaybeMapWithNothing (fix), NoResultMapWithErr (fix), NoPipelineSimplify (fix), NoNestedNegation (fix)

**Unused code**
NoUnusedImports (fix), NoUnusedVariables (fix), NoUnusedExports, NoUnusedCustomTypeConstructors, NoUnusedCustomTypeConstructorArgs, NoUnusedModules, NoUnusedParameters (fix), NoUnusedLetBinding (fix), NoUnusedPatterns

**Code style**
NoMissingTypeAnnotation, NoSinglePatternCase (fix), NoExposingAll (fix), NoImportExposingAll (fix), NoDeprecated, NoMissingDocumentation, NoUnnecessaryTrailingUnderscore, NoPrematureLetComputation, NoSimpleLetBody (fix), NoUnnecessaryPortModule (fix), NoMissingTypeAnnotationInLetIn, NoMissingTypeExpose, NoRedundantlyQualifiedType (fix), NoRecordPatternInFunctionArgs

**Debugging**
NoDebug (fix)

**Complexity**
CognitiveComplexity, NoUnoptimizedRecursion, NoRecursiveUpdate, NoConfusingPrefixOperator, NoShadowing

**Port safety**
NoDuplicatePorts, NoUnsafePorts, NoInconsistentAliases, NoUnusedDependencies

**Other**
NoEmptyLet (fix), NoEmptyRecordUpdate (fix), NoWildcardPatternLast, NoMaxLineLength, NoTodoComment

</details>

## Requirements

The `elm-assist-lsp` binary must be available on your system. Install it via:

```sh
npm install elm-assist
```

Or download a pre-built binary from [GitHub Releases](https://github.com/joshburgess/elm-assist/releases).

The extension searches for the binary in this order:

1. The `elm-assist.serverPath` setting (if set)
2. `./node_modules/.bin/elm-assist-lsp` (project-local npm install)
3. `elm-assist-lsp` on PATH

## Configuration

### Extension settings

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `elm-assist.serverPath` | string | `""` | Path to the `elm-assist-lsp` binary. If empty, searches PATH. |
| `elm-assist.enable` | boolean | `true` | Enable or disable the language server. |

### Project configuration

Create an `elm-assist.toml` in your project root to customize rules:

```toml
[rules]
disable = ["NoMissingTypeAnnotation"]

[rules.severity]
NoDebug = "error"
NoUnusedImports = "warning"

[rules.CognitiveComplexity]
threshold = 20

[rules.NoInconsistentAliases]
aliases = { "Json.Decode" = "Decode", "Json.Encode" = "Encode" }
```

Changes to `elm-assist.toml` are picked up automatically — no restart needed.

## CLI tools

The `elm-assist` package also includes CLI tools that complement the editor experience:

- **elm-lint** — run lint checks from the command line with `--fix`, `--watch`, and `--json` output
- **elm-unused** — project-wide dead code detection
- **elm-deps** — module dependency graph analysis (DOT, Mermaid, cycle detection)
- **elm-refactor** — automated refactoring (rename, sort imports, qualify imports)
- **elm-search** — semantic AST-aware code search

See the [elm-assist repository](https://github.com/joshburgess/elm-assist) for full documentation.

## License

MIT
