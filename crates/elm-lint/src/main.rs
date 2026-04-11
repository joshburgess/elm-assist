#![allow(clippy::collapsible_if)]

use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use clap::Parser;
use rayon::prelude::*;

use elm_lint::cache::{self, LintCache};
use elm_lint::collect::collect_module_info;
use elm_lint::config::Config;
use elm_lint::elm_json;
use elm_lint::fix::apply_fixes;
use elm_lint::output;
use elm_lint::rule::{self, LintContext, ProjectContext};
use elm_lint::rules;
use elm_lint::watch;

/// Fast Elm linter with built-in rules.
#[derive(Parser)]
#[command(name = "elm-lint", version, about)]
struct Cli {
    /// Source directory to lint.
    #[arg(default_value = "src")]
    dir: String,

    /// List all available rules.
    #[arg(long)]
    list: bool,

    /// Only run specified rules (comma-separated).
    #[arg(long, value_delimiter = ',')]
    rules: Option<Vec<String>>,

    /// Disable specified rules (comma-separated).
    #[arg(long, value_delimiter = ',')]
    disable: Option<Vec<String>>,

    /// Apply auto-fixes interactively.
    #[arg(long, conflicts_with_all = ["fix_all", "watch"])]
    fix: bool,

    /// Apply all auto-fixes without prompting.
    #[arg(long, conflicts_with = "watch")]
    fix_all: bool,

    /// Show what auto-fixes would change without writing to disk.
    #[arg(long, conflicts_with_all = ["fix", "fix_all", "watch"])]
    fix_dry_run: bool,

    /// Output findings as JSON for editor integration.
    #[arg(long)]
    json: bool,

    /// Force colored output.
    #[arg(long, conflicts_with = "no_color")]
    color: bool,

    /// Disable colored output.
    #[arg(long)]
    no_color: bool,

    /// Path to config file (default: auto-discover elm-assist.toml).
    #[arg(long)]
    config: Option<String>,

    /// Re-run on file changes.
    #[arg(long)]
    watch: bool,
}

fn main() {
    let cli = Cli::parse();

    // --fix/--fix-all/--fix-dry-run conflict with --json.
    if cli.json && (cli.fix || cli.fix_all || cli.fix_dry_run) {
        eprintln!("Error: --json cannot be combined with --fix, --fix-all, or --fix-dry-run");
        std::process::exit(2);
    }

    // Load config.
    let config = if let Some(path) = &cli.config {
        match Config::load(Path::new(path)) {
            Ok(c) => {
                eprintln!("Using config: {path}");
                c
            }
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(2);
            }
        }
    } else if let Some((path, c)) = Config::discover() {
        eprintln!("Using config: {}", path.display());
        c
    } else {
        Config::default()
    };

    let mut all_rules = rules::all_rules();

    // Apply per-rule config options.
    for rule in &mut all_rules {
        if let Some(options) = config.rule_options(rule.name()) {
            if let Err(e) = rule.configure(options) {
                eprintln!("Error configuring rule {}: {e}", rule.name());
                std::process::exit(2);
            }
        }
    }

    if cli.list {
        println!("Available rules ({}):\n", all_rules.len());
        for rule in &all_rules {
            let disabled = config.is_rule_disabled(rule.name());
            let marker = if disabled { " (disabled)" } else { "" };
            println!("  {:40} {}{}", rule.name(), rule.description(), marker);
        }
        return;
    }

    // Determine active rules: --rules overrides everything, then --disable + config.
    let active_rules: Vec<&dyn rule::Rule> = match &cli.rules {
        Some(names) => all_rules
            .iter()
            .filter(|r| names.iter().any(|n| n == r.name()))
            .map(|r| r.as_ref())
            .collect(),
        None => {
            let cli_disabled: Vec<&str> = cli
                .disable
                .as_ref()
                .map(|v| v.iter().map(|s| s.as_str()).collect())
                .unwrap_or_default();

            all_rules
                .iter()
                .filter(|r| !config.is_rule_disabled(r.name()) && !cli_disabled.contains(&r.name()))
                .map(|r| r.as_ref())
                .collect()
        }
    };

    // Resolve source directory (CLI overrides config).
    let dir = if cli.dir != "src" {
        &cli.dir
    } else {
        config.src.as_deref().unwrap_or("src")
    };

    if !Path::new(dir).exists() {
        eprintln!("Error: directory '{dir}' not found.");
        std::process::exit(2);
    }

    // Determine output format.
    let format = output::resolve_format(cli.json, cli.color, cli.no_color);

    if cli.watch {
        watch::run_watch_loop(dir, || {
            run_lint(dir, &active_rules, &config, &format);
        });
    }

    // One-shot mode.
    let (total_errors, file_errors, sources) = run_lint(dir, &active_rules, &config, &format);

    // Show dry-run diffs if requested.
    if cli.fix_dry_run && total_errors > 0 {
        println!();
        show_fix_diffs(&file_errors, &sources);
    }

    // Apply fixes if requested.
    if (cli.fix || cli.fix_all) && total_errors > 0 {
        println!();
        let fix_mode = if cli.fix_all {
            FixMode::All
        } else {
            FixMode::Interactive
        };
        let applied = apply_all_fixes(&file_errors, &sources, &fix_mode);
        if applied > 0 {
            println!("{applied} fixes applied.");
        } else {
            println!("No fixes applied.");
        }
    }

    // Exit code: 0 = clean, 1 = findings, 2 = error (handled above).
    if total_errors > 0 {
        std::process::exit(1);
    }
}

// ── Lint pipeline ──────────────────────────────────────────────────

/// Run the full lint pipeline: discover files, parse in parallel, collect module
/// info, build project context, run rules in parallel, report. Returns total
/// error count and the collected data (for fix application).
fn run_lint(
    dir: &str,
    active_rules: &[&dyn rule::Rule],
    config: &Config,
    format: &output::OutputFormat,
) -> (
    usize,
    HashMap<String, Vec<rule::LintError>>,
    HashMap<String, String>,
) {
    let start = Instant::now();

    let files = find_elm_files(dir);
    if files.is_empty() {
        eprintln!("No .elm files found in '{dir}'.");
        return (0, HashMap::new(), HashMap::new());
    }

    // Phase 0: Read files and compute hashes in parallel.
    let file_contents: Vec<(PathBuf, String)> = files
        .par_iter()
        .filter_map(|file| {
            let source = fs::read_to_string(file).ok()?;
            Some((file.clone(), source))
        })
        .collect();

    let file_hashes: HashMap<String, u64> = file_contents
        .iter()
        .map(|(path, source)| {
            (
                path.display().to_string(),
                cache::hash_contents(source.as_bytes()),
            )
        })
        .collect();

    // Check cache.
    let rule_names: Vec<String> = active_rules.iter().map(|r| r.name().to_string()).collect();
    let lint_cache = LintCache::load(Path::new(dir), rule_names);

    if lint_cache.is_valid_for(&file_hashes) {
        let elapsed = start.elapsed();
        let cached_errors = lint_cache.get_all_errors();

        // Read sources for fix application (only files with errors).
        let mut sources: HashMap<String, String> = HashMap::new();
        for (path, source) in &file_contents {
            sources.insert(path.display().to_string(), source.clone());
        }

        // Convert cached errors back to LintErrors for reporting.
        let file_errors = cached_to_lint_errors(&cached_errors);

        eprintln!(
            "Linted {} files with {} rules in {:.1}ms (cached)",
            file_contents.len(),
            active_rules.len(),
            elapsed.as_secs_f64() * 1000.0,
        );

        let total_errors: usize = file_errors.values().map(|v| v.len()).sum();
        output::report(
            format,
            &file_errors,
            &sources,
            file_contents.len(),
            active_rules.len(),
        );
        output::report_summary(format, &file_errors);

        return (total_errors, file_errors, sources);
    }

    // Phase 1: Parse in parallel.
    let parse_results: Vec<Result<(String, String, elm_ast::file::ElmModule, String), String>> =
        file_contents
            .into_iter()
            .map(|(path, source)| {
                let path_str = path.display().to_string();
                match elm_ast::parse(&source) {
                    Ok(module) => {
                        let mod_name = extract_module_name(&module);
                        Ok((path_str, mod_name, module, source))
                    }
                    Err(errors) => Err(format!("{}: {}", path_str, errors[0])),
                }
            })
            .collect();

    // Split results (sequential, cheap).
    let mut parsed: Vec<(String, String, elm_ast::file::ElmModule, String)> = Vec::new();
    let mut project_modules = Vec::new();
    let mut parse_errors = 0;

    for result in parse_results {
        match result {
            Ok((path, mod_name, module, source)) => {
                project_modules.push(mod_name.clone());
                parsed.push((path, mod_name, module, source));
            }
            Err(msg) => {
                eprintln!("  warning: {msg}");
                parse_errors += 1;
            }
        }
    }

    // Phase 2: Collect ModuleInfo in parallel.
    let module_infos: HashMap<String, elm_lint::collect::ModuleInfo> = parsed
        .par_iter()
        .map(|(_path, mod_name, module, _source)| (mod_name.clone(), collect_module_info(module)))
        .collect();

    // Load elm.json for dependency checking.
    let elm_json_info = elm_json::load_elm_json(Path::new(dir)).ok();

    // Phase 3: Build ProjectContext (sequential, single-shot aggregation).
    let project_context = ProjectContext::build_with_elm_json(module_infos, elm_json_info);

    // Phase 4: Run all rules on all files in parallel.
    let results: Vec<(String, String, Vec<rule::LintError>)> = parsed
        .par_iter()
        .map(|(file_path, mod_name, module, source)| {
            let ctx = LintContext {
                module,
                source,
                file_path,
                project_modules: &project_modules,
                module_info: project_context.modules.get(mod_name),
                project: Some(&project_context),
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

            (file_path.clone(), source.clone(), file_lint_errors)
        })
        .collect();

    let elapsed = start.elapsed();

    // Assemble results.
    let mut file_errors: HashMap<String, Vec<rule::LintError>> = HashMap::new();
    let mut sources: HashMap<String, String> = HashMap::new();
    for (path, source, errors) in results {
        if !errors.is_empty() {
            file_errors.insert(path.clone(), errors);
        }
        sources.insert(path, source);
    }

    // Save cache.
    let cached_errors = lint_errors_to_cached(&file_errors);
    lint_cache.save(&file_hashes, &cached_errors);

    eprintln!(
        "Linted {} files with {} rules in {:.1}ms",
        parsed.len(),
        active_rules.len(),
        elapsed.as_secs_f64() * 1000.0,
    );
    if parse_errors > 0 {
        eprintln!("  ({parse_errors} files had parse errors and were skipped)");
    }

    let total_errors: usize = file_errors.values().map(|v| v.len()).sum();

    // Report findings.
    output::report(
        format,
        &file_errors,
        &sources,
        parsed.len(),
        active_rules.len(),
    );
    output::report_summary(format, &file_errors);

    (total_errors, file_errors, sources)
}

/// Convert LintErrors to cached representation.
fn lint_errors_to_cached(
    file_errors: &HashMap<String, Vec<rule::LintError>>,
) -> HashMap<String, Vec<cache::CachedError>> {
    file_errors
        .iter()
        .map(|(path, errors)| {
            let cached: Vec<cache::CachedError> = errors
                .iter()
                .map(|e| cache::CachedError {
                    rule: e.rule.to_string(),
                    message: e.message.clone(),
                    severity: match e.severity {
                        rule::Severity::Error => "error".into(),
                        rule::Severity::Warning => "warning".into(),
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

/// Convert cached errors back to LintErrors for reporting.
/// Note: cached errors don't carry Fix data, so --fix won't work from cache.
fn cached_to_lint_errors(
    cached: &HashMap<String, Vec<cache::CachedError>>,
) -> HashMap<String, Vec<rule::LintError>> {
    cached
        .iter()
        .map(|(path, errors)| {
            let lint_errors: Vec<rule::LintError> = errors
                .iter()
                .map(|e| rule::LintError {
                    rule: leak_str(&e.rule),
                    severity: match e.severity.as_str() {
                        "error" => rule::Severity::Error,
                        _ => rule::Severity::Warning,
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
                    fix: None, // Fixes are not cached.
                })
                .collect();
            (path.clone(), lint_errors)
        })
        .collect()
}

/// Leak a String to get a &'static str. Used for cached rule names since
/// LintError.rule is &'static str. Only used for cache hits (bounded count).
fn leak_str(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

fn extract_module_name(module: &elm_ast::file::ElmModule) -> String {
    match &module.header.value {
        elm_ast::module_header::ModuleHeader::Normal { name, .. }
        | elm_ast::module_header::ModuleHeader::Port { name, .. }
        | elm_ast::module_header::ModuleHeader::Effect { name, .. } => name.value.join("."),
    }
}

// ── Fix dry-run ───────────────────────────────────────────────────

fn show_fix_diffs(
    file_errors: &HashMap<String, Vec<rule::LintError>>,
    sources: &HashMap<String, String>,
) {
    let mut file_paths: Vec<&String> = file_errors.keys().collect();
    file_paths.sort();

    let mut total_fixable = 0;

    for path in file_paths {
        let Some(source) = sources.get(path) else {
            continue;
        };

        let errors = &file_errors[path];
        let mut sorted: Vec<_> = errors.iter().filter(|e| e.fix.is_some()).collect();
        sorted.sort_by_key(|e| (e.span.start.line, e.span.start.column));

        for err in &sorted {
            let fix = err.fix.as_ref().unwrap();
            match apply_fixes(source, &fix.edits) {
                Ok(fixed) => {
                    total_fixable += 1;
                    println!(
                        "--- {}:{}:{} [{}] {}",
                        path, err.span.start.line, err.span.start.column, err.rule, err.message
                    );
                    print_unified_diff(source, &fixed);
                    println!();
                }
                Err(e) => {
                    eprintln!(
                        "  warning: could not compute fix for {} [{}]: {e}",
                        path, err.rule
                    );
                }
            }
        }
    }

    if total_fixable > 0 {
        println!("{total_fixable} fixes available. Run with --fix-all to apply.");
    } else {
        println!("No auto-fixable findings.");
    }
}

/// Print a minimal unified diff between two strings, showing only changed
/// lines with 2 lines of surrounding context.
fn print_unified_diff(old: &str, new: &str) {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    // Find common prefix and suffix to narrow the diff region.
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
        return; // No differences.
    }

    // Context: 2 lines before and after.
    let ctx = 2;
    let ctx_start = common_prefix.saturating_sub(ctx);
    let ctx_end_old = (old_changed_end + ctx).min(old_lines.len());
    let ctx_end_new = (new_changed_end + ctx).min(new_lines.len());

    println!(
        "@@ -{},{} +{},{} @@",
        ctx_start + 1,
        ctx_end_old - ctx_start,
        ctx_start + 1,
        ctx_end_new - ctx_start,
    );

    // Context before.
    for line in &old_lines[ctx_start..common_prefix] {
        println!(" {line}");
    }
    // Removed lines.
    for line in &old_lines[common_prefix..old_changed_end] {
        println!("-{line}");
    }
    // Added lines.
    for line in &new_lines[common_prefix..new_changed_end] {
        println!("+{line}");
    }
    // Context after.
    for line in &old_lines[old_changed_end..ctx_end_old] {
        println!(" {line}");
    }
}

// ── Fix application ────────────────────────────────────────────────

enum FixMode {
    Interactive,
    All,
}

fn apply_all_fixes(
    file_errors: &HashMap<String, Vec<rule::LintError>>,
    sources: &HashMap<String, String>,
    fix_mode: &FixMode,
) -> usize {
    let mut total_applied = 0;

    let mut file_paths: Vec<&String> = file_errors.keys().collect();
    file_paths.sort();

    let stdin = io::stdin();
    let mut stdin_lines = stdin.lock().lines();

    for path in file_paths {
        let Some(source) = sources.get(path) else {
            continue;
        };

        let errors = &file_errors[path];
        let mut sorted = errors.clone();
        sorted.sort_by_key(|e| (e.span.start.line, e.span.start.column));

        let mut edits_to_apply = Vec::new();

        for err in &sorted {
            let Some(fix) = &err.fix else {
                continue;
            };

            match fix_mode {
                FixMode::All => {
                    edits_to_apply.extend(fix.edits.iter().cloned());
                }
                FixMode::Interactive => {
                    eprint!(
                        "{}:{}:{}: [{}] {} — apply fix? [y/n/q] ",
                        path, err.span.start.line, err.span.start.column, err.rule, err.message
                    );
                    io::stderr().flush().ok();

                    if let Some(Ok(line)) = stdin_lines.next() {
                        let answer = line.trim().to_lowercase();
                        if answer == "q" {
                            return total_applied;
                        }
                        if answer == "y" || answer == "yes" {
                            edits_to_apply.extend(fix.edits.iter().cloned());
                        }
                    }
                }
            }
        }

        if edits_to_apply.is_empty() {
            continue;
        }

        match apply_fixes(source, &edits_to_apply) {
            Ok(fixed) => {
                if elm_ast::parse(&fixed).is_err() {
                    eprintln!("  warning: fix for {path} produced invalid Elm, skipping");
                    continue;
                }
                match fs::write(path, &fixed) {
                    Ok(()) => {
                        let count = edits_to_apply.len();
                        total_applied += count;
                        eprintln!("  fixed {path} ({count} edits)");
                    }
                    Err(e) => {
                        eprintln!("  warning: could not write {path}: {e}");
                    }
                }
            }
            Err(e) => {
                eprintln!("  warning: could not apply fixes to {path}: {e}");
            }
        }
    }

    total_applied
}

// ── File discovery ─────────────────────────────────────────────────

fn find_elm_files(dir: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_elm_files(&PathBuf::from(dir), &mut files);
    files.sort();
    files
}

fn collect_elm_files(dir: &Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_elm_files(&path, files);
            } else if path.extension().is_some_and(|ext| ext == "elm") {
                files.push(path);
            }
        }
    }
}
