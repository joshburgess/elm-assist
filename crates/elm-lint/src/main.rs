#![allow(clippy::collapsible_if)]

use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;

use clap::Parser;

use elm_lint::config::Config;
use elm_lint::fix::apply_fixes;
use elm_lint::output;
use elm_lint::pipeline;
use elm_lint::rule;
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

/// Run the full lint pipeline via the library, then report results.
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
    let result = pipeline::run_full(dir, active_rules, config);

    if result.files_linted == 0 {
        eprintln!("No .elm files found in '{dir}'.");
        return (0, HashMap::new(), HashMap::new());
    }

    let cache_note = if result.cached { " (cached)" } else { "" };
    eprintln!(
        "Linted {} files with {} rules in {:.1}ms{cache_note}",
        result.files_linted,
        result.rules_active,
        result.elapsed.as_secs_f64() * 1000.0,
    );
    if result.parse_error_count > 0 {
        eprintln!(
            "  ({} files had parse errors and were skipped)",
            result.parse_error_count
        );
    }

    output::report(
        format,
        &result.file_errors,
        &result.sources,
        result.files_linted,
        result.rules_active,
    );
    output::report_summary(format, &result.file_errors);

    let total: usize = result.file_errors.values().map(|v| v.len()).sum();
    (total, result.file_errors, result.sources)
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
                    let hunks = pipeline::compute_diff(source, &fixed);
                    for hunk in &hunks {
                        println!(
                            "@@ -{},{} +{},{} @@",
                            hunk.old_start, hunk.old_count, hunk.new_start, hunk.new_count,
                        );
                        for line in &hunk.lines {
                            match line {
                                pipeline::DiffLine::Context(l) => println!(" {l}"),
                                pipeline::DiffLine::Removed(l) => println!("-{l}"),
                                pipeline::DiffLine::Added(l) => println!("+{l}"),
                            }
                        }
                    }
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
