use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use elm_search::query::parse_query;
use elm_search::search::search;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 || args[1] == "--help" || args[1] == "-h" {
        print_help();
        if args.len() < 2 {
            std::process::exit(1);
        }
        return;
    }

    // The query is everything after the optional directory flag.
    let mut dir = "src";
    let mut query_start = 1;

    // Check if first arg is --dir or a directory.
    if args.len() > 2 && args[1] == "--dir" {
        dir = &args[2];
        query_start = 3;
    }

    let query_str = args[query_start..].join(" ");
    let query = match parse_query(&query_str) {
        Ok(q) => q,
        Err(e) => {
            eprintln!("Error: {e}");
            eprintln!();
            print_help();
            std::process::exit(1);
        }
    };

    if !Path::new(dir).exists() {
        eprintln!("Error: directory '{dir}' not found.");
        std::process::exit(1);
    }

    let start = Instant::now();

    let files = find_elm_files(dir);
    if files.is_empty() {
        eprintln!("No .elm files found in '{dir}'.");
        std::process::exit(1);
    }

    let mut total_matches = 0;

    for file in &files {
        let source = match fs::read_to_string(file) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let module = match elm_ast::parse(&source) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let matches = search(&module, &query);
        if !matches.is_empty() {
            for m in &matches {
                println!(
                    "{}:{}:{}: {}",
                    file.display(),
                    m.span.start.line,
                    m.span.start.column,
                    m.context
                );
            }
            total_matches += matches.len();
        }
    }

    let elapsed = start.elapsed();
    eprintln!(
        "\n{total_matches} match(es) in {} files ({:.1}ms)",
        files.len(),
        elapsed.as_secs_f64() * 1000.0
    );
}

fn print_help() {
    eprintln!("elm-search - Semantic AST-aware code search for Elm");
    eprintln!();
    eprintln!("Usage: elm-search [--dir <path>] <query>");
    eprintln!();
    eprintln!("Queries:");
    eprintln!("  returns <Type>       Functions returning a type (e.g. 'returns Maybe')");
    eprintln!("  type <Type>          Functions using a type anywhere in signature");
    eprintln!("  case-on <Name>       Case expressions matching on a constructor");
    eprintln!("  update .<field>      Record updates touching a field");
    eprintln!("  calls <Module>       Qualified calls to a module (e.g. 'calls Http')");
    eprintln!("  unused-args          Functions with unused arguments");
    eprintln!("  lambda <N>           Lambdas with N or more arguments");
    eprintln!("  uses <name>          All references to a function/value");
    eprintln!("  def <pattern>        Definitions matching a name (substring)");
    eprintln!(
        "  expr <kind>          Expressions by kind: let, case, if, lambda, record, list, tuple"
    );
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  elm-search returns Maybe");
    eprintln!("  elm-search --dir src case-on Result");
    eprintln!("  elm-search calls Http");
    eprintln!("  elm-search unused-args");
    eprintln!("  elm-search lambda 3");
    eprintln!("  elm-search def update");
}

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
