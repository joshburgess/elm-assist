use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use elm_ast::module_header::ModuleHeader;
use elm_deps::graph;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let mut dir = "src";
    let mut format = OutputFormat::Summary;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--dot" => format = OutputFormat::Dot,
            "--mermaid" => format = OutputFormat::Mermaid,
            "--cycles" => format = OutputFormat::CyclesOnly,
            "--stats" => format = OutputFormat::Stats,
            "--help" | "-h" => {
                print_help();
                return;
            }
            s if !s.starts_with('-') => dir = &args[i],
            _ => {
                eprintln!("Unknown flag: {}", args[i]);
                print_help();
                std::process::exit(1);
            }
        }
        i += 1;
    }

    if !Path::new(dir).exists() {
        eprintln!("Error: directory '{dir}' not found.");
        print_help();
        std::process::exit(1);
    }

    let start = Instant::now();

    let files = find_elm_files(dir);
    if files.is_empty() {
        eprintln!("No .elm files found in '{dir}'.");
        std::process::exit(1);
    }

    // Parse all files and collect module -> imports mapping.
    let mut graph: HashMap<String, Vec<String>> = HashMap::new();
    let mut project_modules: HashSet<String> = HashSet::new();
    let mut parse_errors = 0;

    for file in &files {
        let source = match fs::read_to_string(file) {
            Ok(s) => s,
            Err(_) => continue,
        };
        match elm_ast::parse(&source) {
            Ok(module) => {
                let mod_name = match &module.header.value {
                    ModuleHeader::Normal { name, .. }
                    | ModuleHeader::Port { name, .. }
                    | ModuleHeader::Effect { name, .. } => name.value.join("."),
                };
                let imports: Vec<String> = module
                    .imports
                    .iter()
                    .map(|imp| imp.value.module_name.value.join("."))
                    .collect();
                project_modules.insert(mod_name.clone());
                graph.insert(mod_name, imports);
            }
            Err(errors) => {
                eprintln!("  warning: {}: {}", file.display(), errors[0]);
                parse_errors += 1;
            }
        }
    }

    let elapsed = start.elapsed();
    eprintln!(
        "Analyzed {} modules in {:.1}ms",
        graph.len(),
        elapsed.as_secs_f64() * 1000.0
    );
    if parse_errors > 0 {
        eprintln!("  ({parse_errors} files had parse errors)");
    }
    eprintln!();

    // Build graph data for library functions.
    let module_data: Vec<(String, Vec<String>)> = graph
        .iter()
        .map(|(name, imports)| (name.clone(), imports.clone()))
        .collect();
    let (internal_graph, _) = graph::build_graph(&module_data);

    match format {
        OutputFormat::Summary => print_summary(&internal_graph, &project_modules),
        OutputFormat::Dot => print_dot(&internal_graph),
        OutputFormat::Mermaid => print_mermaid(&internal_graph),
        OutputFormat::CyclesOnly => print_cycles(&internal_graph),
        OutputFormat::Stats => print_stats(&internal_graph, &project_modules),
    }
}

#[derive(Clone, Copy)]
enum OutputFormat {
    Summary,
    Dot,
    Mermaid,
    CyclesOnly,
    Stats,
}

// ── Output formats ───────────────────────────────────────────────────

fn print_summary(dep_graph: &HashMap<&str, Vec<&str>>, project_modules: &HashSet<String>) {
    // Print each module and its internal imports.
    let mut modules: Vec<&&str> = dep_graph.keys().collect();
    modules.sort();

    for module in &modules {
        let deps = &dep_graph[**module];
        if deps.is_empty() {
            println!("{module} (no internal imports)");
        } else {
            println!("{module}");
            for dep in deps {
                println!("  -> {dep}");
            }
        }
    }

    println!();

    // Cycles.
    let cycles = graph::find_cycles(dep_graph);
    if cycles.is_empty() {
        println!("No circular dependencies found.");
    } else {
        println!("{} circular dependency chain(s) found:", cycles.len());
        for cycle in &cycles {
            println!("  {}", cycle.join(" -> "));
        }
    }

    println!();
    print_stats(dep_graph, project_modules);
}

fn print_dot(dep_graph: &HashMap<&str, Vec<&str>>) {
    println!("digraph elm_deps {{");
    println!("  rankdir=LR;");
    println!("  node [shape=box, style=filled, fillcolor=lightblue];");

    let mut modules: Vec<&&str> = dep_graph.keys().collect();
    modules.sort();

    for module in &modules {
        let safe_name = module.replace('.', "_");
        println!("  {safe_name} [label=\"{module}\"];");
    }

    for module in &modules {
        let safe_from = module.replace('.', "_");
        for dep in &dep_graph[**module] {
            let safe_to = dep.replace('.', "_");
            println!("  {safe_from} -> {safe_to};");
        }
    }

    println!("}}");
}

fn print_mermaid(dep_graph: &HashMap<&str, Vec<&str>>) {
    println!("graph LR");

    let mut modules: Vec<&&str> = dep_graph.keys().collect();
    modules.sort();

    for module in &modules {
        for dep in &dep_graph[**module] {
            println!("  {module} --> {dep}");
        }
    }
}

fn print_cycles(dep_graph: &HashMap<&str, Vec<&str>>) {
    let cycles = graph::find_cycles(dep_graph);
    if cycles.is_empty() {
        println!("No circular dependencies found.");
    } else {
        println!("{} circular dependency chain(s):", cycles.len());
        for cycle in &cycles {
            println!("  {}", cycle.join(" -> "));
        }
    }
}

fn print_stats(dep_graph: &HashMap<&str, Vec<&str>>, _project_modules: &HashSet<String>) {
    let stats = graph::compute_stats(dep_graph);

    println!("Dependency statistics:");
    println!(
        "  {} modules, {} internal edges",
        stats.total_modules, stats.total_edges
    );
    if stats.total_modules > 0 {
        println!("  {:.1} avg imports per module", stats.avg_imports);
    }
    println!("  {} leaf modules (no internal imports)", stats.leaf_count);
    println!(
        "  {} root modules (not imported by others)",
        stats.root_count
    );

    let top_imports: Vec<_> = stats
        .most_imports
        .iter()
        .filter(|(_, c)| *c > 0)
        .take(5)
        .collect();
    if !top_imports.is_empty() {
        println!();
        println!("Most imports (highest afferent coupling):");
        for (m, c) in &top_imports {
            println!("  {c:>3} {m}");
        }
    }

    if !stats.most_depended_on.is_empty() {
        println!();
        println!("Most depended on (highest efferent coupling):");
        for (m, c) in stats.most_depended_on.iter().take(5) {
            println!("  {c:>3} {m}");
        }
    }

    println!();
    if stats.cycle_count == 0 {
        println!("No circular dependencies.");
    } else {
        println!("{} circular dependency chain(s).", stats.cycle_count);
    }
}

// ── File discovery ───────────────────────────────────────────────────

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

fn print_help() {
    eprintln!("Usage: elm-deps [options] [src-directory]");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --dot        Output DOT format (for Graphviz)");
    eprintln!("  --mermaid    Output Mermaid diagram format");
    eprintln!("  --cycles     Only check for circular dependencies");
    eprintln!("  --stats      Show coupling statistics");
    eprintln!("  --help       Show this help");
}
