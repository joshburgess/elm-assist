#![allow(clippy::collapsible_if)]

use elm_refactor::commands;
use elm_refactor::project;

use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_help();
        std::process::exit(1);
    }

    match args[1].as_str() {
        "rename" => cmd_rename(&args[2..]),
        "sort-imports" => cmd_sort_imports(&args[2..]),
        "qualify-imports" => cmd_qualify_imports(&args[2..]),
        "--help" | "-h" | "help" => print_help(),
        cmd => {
            eprintln!("Unknown command: {cmd}");
            print_help();
            std::process::exit(1);
        }
    }
}

fn cmd_rename(args: &[String]) {
    if args.len() < 3 {
        eprintln!("Usage: elm-refactor rename <Module.name> <old_name> <new_name> [src-dir]");
        eprintln!();
        eprintln!("Renames a function/value across the entire project.");
        eprintln!("Updates the definition, all references, and import/exposing lists.");
        eprintln!();
        eprintln!("Example:");
        eprintln!("  elm-refactor rename Main.myFunc oldName newName src");
        std::process::exit(1);
    }

    let qualified = &args[0];
    let (module, _fn_name) = match qualified.rsplit_once('.') {
        Some((m, f)) => (m, f),
        None => {
            eprintln!("Error: first argument must be a qualified name like Module.functionName");
            std::process::exit(1);
        }
    };

    let from = &args[1];
    let to = &args[2];
    let dir = args.get(3).map(|s| s.as_str()).unwrap_or("src");

    let start = Instant::now();
    let mut project = project::Project::load(dir);
    eprintln!(
        "Loaded {} files in {:.1}ms",
        project.files.len(),
        start.elapsed().as_secs_f64() * 1000.0
    );

    let changes = commands::rename::rename(&mut project, module, from, to);

    if changes == 0 {
        println!("No occurrences of `{from}` found in module `{module}`.");
        return;
    }

    let written = project.write_changes();
    println!(
        "Renamed `{from}` to `{to}` in `{module}`: {changes} occurrence(s), {written} file(s) modified."
    );
}

fn cmd_sort_imports(args: &[String]) {
    let dir = args.first().map(|s| s.as_str()).unwrap_or("src");
    let dry_run = args.iter().any(|a| a == "--dry-run");

    let start = Instant::now();
    let mut project = project::Project::load(dir);
    eprintln!(
        "Loaded {} files in {:.1}ms",
        project.files.len(),
        start.elapsed().as_secs_f64() * 1000.0
    );

    let changes = commands::sort_imports::sort_imports(&mut project);

    if changes == 0 {
        println!("All imports are already sorted.");
        return;
    }

    if dry_run {
        println!("Would sort imports in {changes} file(s). (dry run)");
    } else {
        let written = project.write_changes();
        println!("Sorted imports in {written} file(s).");
    }
}

fn cmd_qualify_imports(args: &[String]) {
    let dir = args.first().map(|s| s.as_str()).unwrap_or("src");
    let dry_run = args.iter().any(|a| a == "--dry-run");

    let start = Instant::now();
    let mut project = project::Project::load(dir);
    eprintln!(
        "Loaded {} files in {:.1}ms",
        project.files.len(),
        start.elapsed().as_secs_f64() * 1000.0
    );

    let changes = commands::qualify_imports::qualify_imports(&mut project);

    if changes == 0 {
        println!("No unqualified imports to qualify.");
        return;
    }

    if dry_run {
        println!("Would qualify {changes} reference(s). (dry run)");
    } else {
        let written = project.write_changes();
        println!("Qualified {changes} reference(s), {written} file(s) modified.");
    }
}

fn print_help() {
    eprintln!("elm-refactor - Automated refactoring for Elm projects");
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  rename           Rename a function/value across the project");
    eprintln!("  sort-imports     Sort import declarations alphabetically");
    eprintln!("  qualify-imports  Convert exposed imports to qualified form");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  elm-refactor rename Module.func oldName newName [src-dir]");
    eprintln!("  elm-refactor sort-imports [src-dir] [--dry-run]");
    eprintln!("  elm-refactor qualify-imports [src-dir] [--dry-run]");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --dry-run    Show what would change without modifying files");
    eprintln!("  --help       Show this help");
}
