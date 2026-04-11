mod analyze;
mod collect;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use analyze::FindingKind;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = args.get(1).map(|s| s.as_str()).unwrap_or("src");

    if !Path::new(dir).exists() {
        eprintln!("Error: directory '{dir}' not found.");
        eprintln!("Usage: elm-unused [src-directory]");
        std::process::exit(1);
    }

    let start = Instant::now();

    // Discover .elm files.
    let files = find_elm_files(dir);
    if files.is_empty() {
        eprintln!("No .elm files found in '{dir}'.");
        std::process::exit(1);
    }

    // Parse all files and collect module info.
    let mut modules: HashMap<String, collect::ModuleInfo> = HashMap::new();
    let mut parse_errors = 0;

    for file in &files {
        let source = match fs::read_to_string(file) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("  warning: could not read {}: {e}", file.display());
                continue;
            }
        };

        match elm_ast::parse(&source) {
            Ok(module) => {
                let info = collect::collect_module_info(&module);
                let mod_name = info.module_name.join(".");
                modules.insert(mod_name, info);
            }
            Err(errors) => {
                eprintln!(
                    "  warning: parse error in {}: {}",
                    file.display(),
                    errors[0]
                );
                parse_errors += 1;
            }
        }
    }

    let elapsed = start.elapsed();

    // Run analysis.
    let findings = analyze::analyze(&modules);

    // Report.
    eprintln!(
        "Scanned {} files ({} modules) in {:.1}ms",
        files.len(),
        modules.len(),
        elapsed.as_secs_f64() * 1000.0
    );
    if parse_errors > 0 {
        eprintln!("  ({parse_errors} files had parse errors and were skipped)");
    }
    eprintln!();

    if findings.is_empty() {
        println!("No unused code found.");
        return;
    }

    // Group by module.
    let mut by_module: HashMap<&str, Vec<&analyze::Finding>> = HashMap::new();
    for f in &findings {
        by_module.entry(&f.module_name).or_default().push(f);
    }

    let mut module_names: Vec<&&str> = by_module.keys().collect();
    module_names.sort();

    let mut counts: HashMap<&str, usize> = HashMap::new();
    for f in &findings {
        *counts.entry(f.kind.label()).or_default() += 1;
    }

    for module_name in module_names {
        let module_findings = &by_module[*module_name];
        println!("{}:", module_name);
        for f in module_findings {
            let icon = match f.kind {
                FindingKind::UnusedImport => "  import",
                FindingKind::UnusedImportExposing => "  exposing",
                FindingKind::UnusedFunction => "  function",
                FindingKind::UnusedExport => "  export",
                FindingKind::UnusedConstructor => "  constructor",
                FindingKind::UnusedType => "  type",
            };
            println!("  {icon} {}", f.name);
        }
    }

    println!();
    println!("Summary: {} findings", findings.len());
    let mut count_entries: Vec<_> = counts.iter().collect();
    count_entries.sort_by_key(|(_, v)| std::cmp::Reverse(**v));
    for (kind, count) in count_entries {
        println!("  {count} {kind}");
    }
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
