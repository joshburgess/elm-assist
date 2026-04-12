//! Public lint pipeline — callable steps for the TUI and other consumers.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use rayon::prelude::*;

use crate::cache::{self, CachedError, LintCache};
use crate::collect::{self, ModuleInfo};
use crate::config::Config;
use crate::elm_json;
use crate::rule::{LintContext, LintError, ProjectContext, Rule, Severity};

// ── File discovery ────────────────────────────────────────────────

/// Recursively discover all `.elm` files under `dir`.
pub fn discover_files(dir: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_elm_files_recursive(&PathBuf::from(dir), &mut files);
    files.sort();
    files
}

fn collect_elm_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_elm_files_recursive(&path, files);
            } else if path.extension().is_some_and(|ext| ext == "elm") {
                files.push(path);
            }
        }
    }
}

// ── Parsed project ────────────────────────────────────────────────

/// A single parsed Elm file.
pub struct ParsedFile {
    pub path: String,
    pub module_name: String,
    pub module: elm_ast::file::ElmModule,
    pub source: String,
}

/// The result of parsing all files.
pub struct ParseResult {
    pub files: Vec<ParsedFile>,
    pub parse_error_count: usize,
    /// Parse error messages (file path + error description).
    pub parse_errors: Vec<String>,
}

/// Read and parse all `.elm` files in parallel.
pub fn parse_files(files: &[PathBuf]) -> ParseResult {
    let file_contents: Vec<(PathBuf, String)> = files
        .par_iter()
        .filter_map(|file| {
            let source = fs::read_to_string(file).ok()?;
            Some((file.clone(), source))
        })
        .collect();

    let results: Vec<Result<ParsedFile, String>> = file_contents
        .into_iter()
        .map(|(path, source)| {
            let path_str = path.display().to_string();
            match elm_ast::parse(&source) {
                Ok(module) => {
                    let module_name = extract_module_name(&module);
                    Ok(ParsedFile {
                        path: path_str,
                        module_name,
                        module,
                        source,
                    })
                }
                Err(errors) => Err(format!("{}: {}", path_str, errors[0])),
            }
        })
        .collect();

    let mut parsed = Vec::new();
    let mut parse_error_count = 0;
    let mut parse_errors = Vec::new();

    for result in results {
        match result {
            Ok(file) => parsed.push(file),
            Err(msg) => {
                parse_errors.push(msg);
                parse_error_count += 1;
            }
        }
    }

    ParseResult {
        files: parsed,
        parse_error_count,
        parse_errors,
    }
}

/// Extract the module name from a parsed Elm module header.
pub fn extract_module_name(module: &elm_ast::file::ElmModule) -> String {
    match &module.header.value {
        elm_ast::module_header::ModuleHeader::Normal { name, .. }
        | elm_ast::module_header::ModuleHeader::Port { name, .. }
        | elm_ast::module_header::ModuleHeader::Effect { name, .. } => name.value.join("."),
    }
}

// ── Project context ───────────────────────────────────────────────

/// Build module info and project context from parsed files.
pub fn build_project_context(
    parsed: &ParseResult,
    config: &Config,
) -> (ProjectContext, Vec<String>) {
    let module_infos: HashMap<String, ModuleInfo> = parsed
        .files
        .par_iter()
        .map(|f| {
            (
                f.module_name.clone(),
                collect::collect_module_info(&f.module),
            )
        })
        .collect();

    let project_modules: Vec<String> = parsed.files.iter().map(|f| f.module_name.clone()).collect();

    let elm_json_info = elm_json::load_elm_json(Path::new(".")).ok();
    let _ = config; // config is used for severity, handled in run_rules

    let project_context = ProjectContext::build_with_elm_json(module_infos, elm_json_info);

    (project_context, project_modules)
}

// ── Rule execution ────────────────────────────────────────────────

/// The result of a lint run.
pub struct LintResult {
    /// Per-file lint errors.
    pub file_errors: HashMap<String, Vec<LintError>>,
    /// Per-file source text.
    pub sources: HashMap<String, String>,
    /// Total error count.
    pub total_errors: usize,
    /// Total warning count.
    pub total_warnings: usize,
    /// Total fixable error count.
    pub total_fixable: usize,
    /// Whether the result came from cache.
    pub cached: bool,
    /// Duration of the lint run.
    pub elapsed: std::time::Duration,
    /// Number of files that failed to parse.
    pub parse_error_count: usize,
    /// Number of files linted.
    pub files_linted: usize,
    /// Number of active rules.
    pub rules_active: usize,
}

/// Run all active rules against all parsed files.
pub fn run_rules(
    parsed: &ParseResult,
    project_context: &ProjectContext,
    project_modules: &[String],
    active_rules: &[&dyn Rule],
    config: &Config,
) -> LintResult {
    let start = Instant::now();

    let results: Vec<(String, String, Vec<LintError>)> = parsed
        .files
        .par_iter()
        .map(|f| {
            let ctx = LintContext {
                module: &f.module,
                source: &f.source,
                file_path: &f.path,
                project_modules,
                module_info: project_context.modules.get(&f.module_name),
                project: Some(project_context),
            };

            let mut file_lint_errors = Vec::new();
            for rule in active_rules {
                let mut errors = rule.check(&ctx);
                let severity = config
                    .severity_for(rule.name())
                    .unwrap_or(rule.default_severity());
                for err in &mut errors {
                    err.severity = severity;
                }
                file_lint_errors.extend(errors);
            }

            (f.path.clone(), f.source.clone(), file_lint_errors)
        })
        .collect();

    let elapsed = start.elapsed();

    let mut file_errors: HashMap<String, Vec<LintError>> = HashMap::new();
    let mut sources: HashMap<String, String> = HashMap::new();
    let mut total_errors = 0;
    let mut total_warnings = 0;
    let mut total_fixable = 0;

    for (path, source, errors) in results {
        for err in &errors {
            match err.severity {
                Severity::Error => total_errors += 1,
                Severity::Warning => total_warnings += 1,
            }
            if err.fix.is_some() {
                total_fixable += 1;
            }
        }
        if !errors.is_empty() {
            file_errors.insert(path.clone(), errors);
        }
        sources.insert(path, source);
    }

    LintResult {
        total_errors: total_errors + total_warnings, // total findings
        total_warnings,
        total_fixable,
        file_errors,
        sources,
        cached: false,
        elapsed,
        parse_error_count: parsed.parse_error_count,
        files_linted: parsed.files.len(),
        rules_active: active_rules.len(),
    }
}

// ── Full pipeline (convenience) ───────────────────────────────────

/// Run the entire lint pipeline from discovery through results.
/// This is the equivalent of the old `run_lint()` in main.rs.
pub fn run_full(dir: &str, active_rules: &[&dyn Rule], config: &Config) -> LintResult {
    let files = discover_files(dir);
    if files.is_empty() {
        return LintResult {
            file_errors: HashMap::new(),
            sources: HashMap::new(),
            total_errors: 0,
            total_warnings: 0,
            total_fixable: 0,
            cached: false,
            elapsed: std::time::Duration::ZERO,
            parse_error_count: 0,
            files_linted: 0,
            rules_active: active_rules.len(),
        };
    }

    // Check cache.
    let start = Instant::now();
    let file_contents: Vec<(String, String)> = files
        .par_iter()
        .filter_map(|file| {
            let source = fs::read_to_string(file).ok()?;
            Some((file.display().to_string(), source))
        })
        .collect();

    let file_hashes: HashMap<String, u64> = file_contents
        .iter()
        .map(|(path, source)| (path.clone(), cache::hash_contents(source.as_bytes())))
        .collect();

    let rule_names: Vec<String> = active_rules.iter().map(|r| r.name().to_string()).collect();
    let lint_cache = LintCache::load(Path::new(dir), rule_names);

    if lint_cache.is_valid_for(&file_hashes) {
        let elapsed = start.elapsed();
        let cached_errors = lint_cache.get_all_errors();
        let file_errors = cached_to_lint_errors(&cached_errors);

        let mut sources = HashMap::new();
        for (path, source) in &file_contents {
            sources.insert(path.clone(), source.clone());
        }

        let mut total_errors = 0;
        let mut total_warnings = 0;
        let mut total_fixable = 0;
        for errors in file_errors.values() {
            for err in errors {
                match err.severity {
                    Severity::Error => total_errors += 1,
                    Severity::Warning => total_warnings += 1,
                }
                if err.fix.is_some() {
                    total_fixable += 1;
                }
            }
        }

        return LintResult {
            total_errors: total_errors + total_warnings,
            total_warnings,
            total_fixable,
            file_errors,
            sources,
            cached: true,
            elapsed,
            parse_error_count: 0,
            files_linted: file_contents.len(),
            rules_active: active_rules.len(),
        };
    }

    // Full pipeline.
    let parsed = parse_files(&files);
    let (project_context, project_modules) = build_project_context(&parsed, config);
    let mut result = run_rules(
        &parsed,
        &project_context,
        &project_modules,
        active_rules,
        config,
    );
    result.elapsed = start.elapsed();

    // Save cache.
    let cached_errors = lint_errors_to_cached(&result.file_errors);
    lint_cache.save(&file_hashes, &cached_errors);

    result
}

// ── Combined analysis pass (TUI / incremental use) ────────────────

/// Combined result of a single-parse analysis pass: lint + module import data
/// suitable for downstream dependency-graph analysis.
pub struct AnalysisResult {
    pub lint: LintResult,
    /// (module_name, imports) pairs for each parsed module, suitable for
    /// feeding into `elm_deps::graph::build_graph`.
    pub module_data: Vec<(String, Vec<String>)>,
}

/// Discover, parse, and run rules **once**, returning both lint results and
/// dependency-graph input data. Unlike `run_full`, this does not use the
/// on-disk lint cache — it is intended for interactive (TUI) use where the
/// caller needs a fresh full parse anyway.
pub fn run_all(dir: &str, active_rules: &[&dyn Rule], config: &Config) -> AnalysisResult {
    let start = Instant::now();

    let files = discover_files(dir);
    if files.is_empty() {
        return AnalysisResult {
            lint: LintResult {
                file_errors: HashMap::new(),
                sources: HashMap::new(),
                total_errors: 0,
                total_warnings: 0,
                total_fixable: 0,
                cached: false,
                elapsed: std::time::Duration::ZERO,
                parse_error_count: 0,
                files_linted: 0,
                rules_active: active_rules.len(),
            },
            module_data: Vec::new(),
        };
    }

    let parsed = parse_files(&files);

    // Extract deps data directly from parsed modules — no re-read, no re-parse.
    let mut module_data: Vec<(String, Vec<String>)> = parsed
        .files
        .iter()
        .map(|f| {
            let imports: Vec<String> = f
                .module
                .imports
                .iter()
                .map(|imp| imp.value.module_name.value.join("."))
                .collect();
            (f.module_name.clone(), imports)
        })
        .collect();
    module_data.sort_by(|(a, _), (b, _)| a.cmp(b));

    let (project_context, project_modules) = build_project_context(&parsed, config);
    let mut lint = run_rules(
        &parsed,
        &project_context,
        &project_modules,
        active_rules,
        config,
    );
    lint.elapsed = start.elapsed();

    AnalysisResult { lint, module_data }
}

// ── Diff computation ──────────────────────────────────────────────

/// A line in a unified diff.
#[derive(Debug, Clone)]
pub enum DiffLine {
    Context(String),
    Added(String),
    Removed(String),
}

/// A hunk in a unified diff.
#[derive(Debug, Clone)]
pub struct DiffHunk {
    pub old_start: usize,
    pub old_count: usize,
    pub new_start: usize,
    pub new_count: usize,
    pub lines: Vec<DiffLine>,
}

/// Compute a unified diff between two strings.
pub fn compute_diff(old: &str, new: &str) -> Vec<DiffHunk> {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    let common_prefix = old_lines
        .iter()
        .zip(new_lines.iter())
        .take_while(|(a, b)| a == b)
        .count();

    let common_suffix = old_lines
        .iter()
        .rev()
        .zip(new_lines.iter().rev())
        .take_while(|(a, b)| a == b)
        .count()
        .min(old_lines.len() - common_prefix)
        .min(new_lines.len() - common_prefix);

    let old_changed_end = old_lines.len() - common_suffix;
    let new_changed_end = new_lines.len() - common_suffix;

    if common_prefix == old_changed_end && common_prefix == new_changed_end {
        return Vec::new();
    }

    let ctx = 2;
    let ctx_start = common_prefix.saturating_sub(ctx);
    let ctx_end_old = (old_changed_end + ctx).min(old_lines.len());
    let ctx_end_new = (new_changed_end + ctx).min(new_lines.len());

    let mut lines = Vec::new();

    for line in &old_lines[ctx_start..common_prefix] {
        lines.push(DiffLine::Context((*line).to_string()));
    }
    for line in &old_lines[common_prefix..old_changed_end] {
        lines.push(DiffLine::Removed((*line).to_string()));
    }
    for line in &new_lines[common_prefix..new_changed_end] {
        lines.push(DiffLine::Added((*line).to_string()));
    }
    for line in &old_lines[old_changed_end..ctx_end_old] {
        lines.push(DiffLine::Context((*line).to_string()));
    }

    vec![DiffHunk {
        old_start: ctx_start + 1,
        old_count: ctx_end_old - ctx_start,
        new_start: ctx_start + 1,
        new_count: ctx_end_new - ctx_start,
        lines,
    }]
}

// ── Cache helpers ─────────────────────────────────────────────────

fn lint_errors_to_cached(
    file_errors: &HashMap<String, Vec<LintError>>,
) -> HashMap<String, Vec<CachedError>> {
    file_errors
        .iter()
        .map(|(path, errors)| {
            let cached: Vec<CachedError> = errors
                .iter()
                .map(|e| CachedError {
                    rule: e.rule.to_string(),
                    message: e.message.clone(),
                    severity: match e.severity {
                        Severity::Error => "error".into(),
                        Severity::Warning => "warning".into(),
                    },
                    start_line: e.span.start.line,
                    start_col: e.span.start.column,
                    start_offset: e.span.start.offset,
                    end_line: e.span.end.line,
                    end_col: e.span.end.column,
                    end_offset: e.span.end.offset,
                    fixable: e.fix.is_some(),
                })
                .collect();
            (path.clone(), cached)
        })
        .collect()
}

fn cached_to_lint_errors(
    cached: &HashMap<String, Vec<CachedError>>,
) -> HashMap<String, Vec<LintError>> {
    cached
        .iter()
        .map(|(path, errors)| {
            let lint_errors: Vec<LintError> = errors
                .iter()
                .map(|e| LintError {
                    rule: leak_str(&e.rule),
                    severity: match e.severity.as_str() {
                        "error" => Severity::Error,
                        _ => Severity::Warning,
                    },
                    message: e.message.clone(),
                    span: elm_ast::span::Span {
                        start: elm_ast::span::Position {
                            offset: e.start_offset,
                            line: e.start_line,
                            column: e.start_col,
                        },
                        end: elm_ast::span::Position {
                            offset: e.end_offset,
                            line: e.end_line,
                            column: e.end_col,
                        },
                    },
                    fix: None,
                })
                .collect();
            (path.clone(), lint_errors)
        })
        .collect()
}

fn leak_str(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}
