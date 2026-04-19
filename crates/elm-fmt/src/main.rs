use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use clap::{Parser, ValueEnum};
use elm_ast::{parse, pretty_print, pretty_print_converged};
use rayon::prelude::*;

/// Fast Elm formatter matching elm-format output.
#[derive(Parser)]
#[command(name = "elm-fmt", version, about)]
struct Cli {
    /// Files or directories to format. Directories are searched recursively
    /// for `.elm` files. When omitted, source is read from stdin.
    paths: Vec<PathBuf>,

    /// Output style.
    #[arg(long, value_enum, default_value_t = Style::ElmFormat)]
    style: Style,

    /// Write formatted output back to each file in place.
    #[arg(short, long, conflicts_with_all = ["check", "stdout"])]
    write: bool,

    /// Check whether files are already formatted. Exits 0 if unchanged,
    /// 1 if any file would be rewritten, 2 on parse errors.
    #[arg(long, conflicts_with_all = ["write", "stdout"])]
    check: bool,

    /// Force printing to stdout even when paths are given (default when no
    /// `--write`/`--check` is set and a single path is provided).
    #[arg(long)]
    stdout: bool,
}

#[derive(Copy, Clone, ValueEnum)]
enum Style {
    /// Match `elm-format <source>` output exactly.
    ElmFormat,
    /// Pre-apply elm-format's second-pass mutations so the output is a
    /// fixed point under elm-format (useful for code generation).
    ElmFormatConverged,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    if cli.paths.is_empty() {
        return run_stdin(&cli);
    }

    let files = match collect_elm_files(&cli.paths) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("elm-fmt: {e}");
            return ExitCode::from(2);
        }
    };

    if files.is_empty() {
        eprintln!("elm-fmt: no .elm files found");
        return ExitCode::from(2);
    }

    let parse_failed = AtomicBool::new(false);
    let changed_count = AtomicUsize::new(0);

    let results: Vec<_> = files
        .par_iter()
        .map(|path| process_file(path, &cli, &parse_failed, &changed_count))
        .collect();

    for msg in results.into_iter().flatten() {
        eprintln!("{msg}");
    }

    if parse_failed.load(Ordering::Relaxed) {
        return ExitCode::from(2);
    }
    if cli.check && changed_count.load(Ordering::Relaxed) > 0 {
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

fn run_stdin(cli: &Cli) -> ExitCode {
    if cli.write || cli.check {
        eprintln!("elm-fmt: --write and --check require path arguments");
        return ExitCode::from(2);
    }
    let mut source = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut source) {
        eprintln!("elm-fmt: reading stdin: {e}");
        return ExitCode::from(2);
    }
    match format_source(&source, cli.style) {
        Ok(out) => {
            let mut stdout = io::stdout().lock();
            if let Err(e) = stdout.write_all(out.as_bytes()) {
                eprintln!("elm-fmt: writing stdout: {e}");
                return ExitCode::from(2);
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("elm-fmt: parse error: {e}");
            ExitCode::from(2)
        }
    }
}

fn process_file(
    path: &Path,
    cli: &Cli,
    parse_failed: &AtomicBool,
    changed_count: &AtomicUsize,
) -> Option<String> {
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            parse_failed.store(true, Ordering::Relaxed);
            return Some(format!("{}: read error: {}", path.display(), e));
        }
    };

    let formatted = match format_source(&source, cli.style) {
        Ok(s) => s,
        Err(e) => {
            parse_failed.store(true, Ordering::Relaxed);
            return Some(format!("{}: parse error: {}", path.display(), e));
        }
    };

    let changed = formatted != source;

    if cli.check {
        if changed {
            changed_count.fetch_add(1, Ordering::Relaxed);
            return Some(format!("would reformat {}", path.display()));
        }
        return None;
    }

    if cli.write {
        if changed {
            changed_count.fetch_add(1, Ordering::Relaxed);
            if let Err(e) = fs::write(path, &formatted) {
                parse_failed.store(true, Ordering::Relaxed);
                return Some(format!("{}: write error: {}", path.display(), e));
            }
        }
        return None;
    }

    let mut stdout = io::stdout().lock();
    if let Err(e) = stdout.write_all(formatted.as_bytes()) {
        parse_failed.store(true, Ordering::Relaxed);
        return Some(format!("{}: stdout error: {}", path.display(), e));
    }
    None
}

fn format_source(source: &str, style: Style) -> Result<String, String> {
    let module = parse(source).map_err(|errors| {
        errors
            .iter()
            .map(|e| {
                format!(
                    "line {}, col {}: {}",
                    e.span.start.line, e.span.start.column, e.message
                )
            })
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    Ok(match style {
        Style::ElmFormat => pretty_print(&module),
        Style::ElmFormatConverged => pretty_print_converged(&module),
    })
}

fn collect_elm_files(paths: &[PathBuf]) -> io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for p in paths {
        collect_into(p, &mut out)?;
    }
    Ok(out)
}

fn collect_into(path: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    let meta = fs::metadata(path)?;
    if meta.is_file() {
        out.push(path.to_path_buf());
        return Ok(());
    }
    if meta.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let p = entry.path();
            if p.is_dir() {
                collect_into(&p, out)?;
            } else if p.extension().and_then(|s| s.to_str()) == Some("elm") {
                out.push(p);
            }
        }
    }
    Ok(())
}
