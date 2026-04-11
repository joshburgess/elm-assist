use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use elm_ast::module_header::ModuleHeader;

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

    // Filter to only project-internal imports.
    let internal_graph: HashMap<&str, Vec<&str>> = graph
        .iter()
        .map(|(mod_name, imports)| {
            let internal: Vec<&str> = imports
                .iter()
                .filter(|imp| project_modules.contains(*imp))
                .map(|s| s.as_str())
                .collect();
            (mod_name.as_str(), internal)
        })
        .collect();

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

fn print_summary(graph: &HashMap<&str, Vec<&str>>, project_modules: &HashSet<String>) {
    // Print each module and its internal imports.
    let mut modules: Vec<&&str> = graph.keys().collect();
    modules.sort();

    for module in &modules {
        let deps = &graph[**module];
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
    let cycles = find_cycles(graph);
    if cycles.is_empty() {
        println!("No circular dependencies found.");
    } else {
        println!("{} circular dependency chain(s) found:", cycles.len());
        for cycle in &cycles {
            println!("  {}", cycle.join(" -> "));
        }
    }

    println!();
    print_stats(graph, project_modules);
}

fn print_dot(graph: &HashMap<&str, Vec<&str>>) {
    println!("digraph elm_deps {{");
    println!("  rankdir=LR;");
    println!("  node [shape=box, style=filled, fillcolor=lightblue];");

    let mut modules: Vec<&&str> = graph.keys().collect();
    modules.sort();

    for module in &modules {
        let safe_name = module.replace('.', "_");
        println!("  {safe_name} [label=\"{module}\"];");
    }

    for module in &modules {
        let safe_from = module.replace('.', "_");
        for dep in &graph[**module] {
            let safe_to = dep.replace('.', "_");
            println!("  {safe_from} -> {safe_to};");
        }
    }

    println!("}}");
}

fn print_mermaid(graph: &HashMap<&str, Vec<&str>>) {
    println!("graph LR");

    let mut modules: Vec<&&str> = graph.keys().collect();
    modules.sort();

    for module in &modules {
        for dep in &graph[**module] {
            println!("  {module} --> {dep}");
        }
    }
}

fn print_cycles(graph: &HashMap<&str, Vec<&str>>) {
    let cycles = find_cycles(graph);
    if cycles.is_empty() {
        println!("No circular dependencies found.");
    } else {
        println!("{} circular dependency chain(s):", cycles.len());
        for cycle in &cycles {
            println!("  {}", cycle.join(" -> "));
        }
    }
}

fn print_stats(graph: &HashMap<&str, Vec<&str>>, _project_modules: &HashSet<String>) {
    let total = graph.len();
    let total_edges: usize = graph.values().map(|v| v.len()).sum();

    // Modules with most imports (afferent coupling).
    let mut import_counts: Vec<(&str, usize)> =
        graph.iter().map(|(m, deps)| (*m, deps.len())).collect();
    import_counts.sort_by_key(|(_, c)| std::cmp::Reverse(*c));

    // Modules most depended on (efferent coupling).
    let mut depended_on: HashMap<&str, usize> = HashMap::new();
    for deps in graph.values() {
        for dep in deps {
            *depended_on.entry(dep).or_default() += 1;
        }
    }
    let mut dep_counts: Vec<(&str, usize)> = depended_on.into_iter().collect();
    dep_counts.sort_by_key(|(_, c)| std::cmp::Reverse(*c));

    // Leaf modules (no internal imports).
    let leaves: Vec<&&str> = graph
        .iter()
        .filter(|(_, deps)| deps.is_empty())
        .map(|(m, _)| m)
        .collect();

    // Root modules (not imported by anyone).
    let all_imported: HashSet<&str> = graph.values().flat_map(|v| v.iter().copied()).collect();
    let roots: Vec<&&str> = graph
        .keys()
        .filter(|m| !all_imported.contains(**m))
        .collect();

    println!("Dependency statistics:");
    println!("  {total} modules, {total_edges} internal edges");
    if total > 0 {
        println!(
            "  {:.1} avg imports per module",
            total_edges as f64 / total as f64
        );
    }
    println!("  {} leaf modules (no internal imports)", leaves.len());
    println!("  {} root modules (not imported by others)", roots.len());

    if !import_counts.is_empty() {
        println!();
        println!("Most imports (highest afferent coupling):");
        for (m, c) in import_counts.iter().take(5) {
            if *c > 0 {
                println!("  {c:>3} {m}");
            }
        }
    }

    if !dep_counts.is_empty() {
        println!();
        println!("Most depended on (highest efferent coupling):");
        for (m, c) in dep_counts.iter().take(5) {
            println!("  {c:>3} {m}");
        }
    }

    let cycles = find_cycles(graph);
    println!();
    if cycles.is_empty() {
        println!("No circular dependencies.");
    } else {
        println!("{} circular dependency chain(s).", cycles.len());
    }
}

// ── Cycle detection ──────────────────────────────────────────────────

fn find_cycles<'a>(graph: &HashMap<&'a str, Vec<&'a str>>) -> Vec<Vec<&'a str>> {
    let mut visited: HashSet<&str> = HashSet::new();
    let mut on_stack: HashSet<&str> = HashSet::new();
    let mut path: Vec<&str> = Vec::new();
    let mut cycles: Vec<Vec<&str>> = Vec::new();

    let mut modules: Vec<&&str> = graph.keys().collect();
    modules.sort();

    for module in modules {
        if !visited.contains(*module) {
            dfs(
                module,
                graph,
                &mut visited,
                &mut on_stack,
                &mut path,
                &mut cycles,
            );
        }
    }

    // Deduplicate cycles (same cycle can be found from different starting points).
    let mut unique: Vec<Vec<&str>> = Vec::new();
    for cycle in cycles {
        let normalized = normalize_cycle(&cycle);
        if !unique.iter().any(|c| normalize_cycle(c) == normalized) {
            unique.push(cycle);
        }
    }

    unique
}

fn dfs<'a>(
    node: &'a str,
    graph: &HashMap<&'a str, Vec<&'a str>>,
    visited: &mut HashSet<&'a str>,
    on_stack: &mut HashSet<&'a str>,
    path: &mut Vec<&'a str>,
    cycles: &mut Vec<Vec<&'a str>>,
) {
    visited.insert(node);
    on_stack.insert(node);
    path.push(node);

    if let Some(deps) = graph.get(node) {
        for dep in deps {
            if !visited.contains(dep) {
                dfs(dep, graph, visited, on_stack, path, cycles);
            } else if on_stack.contains(dep) {
                // Found a cycle. Extract it from the path.
                if let Some(start) = path.iter().position(|n| n == dep) {
                    let mut cycle: Vec<&str> = path[start..].to_vec();
                    cycle.push(dep); // close the loop
                    cycles.push(cycle);
                }
            }
        }
    }

    path.pop();
    on_stack.remove(node);
}

fn normalize_cycle<'a>(cycle: &[&'a str]) -> Vec<&'a str> {
    if cycle.len() <= 1 {
        return cycle.to_vec();
    }
    // Remove the closing duplicate.
    let core = &cycle[..cycle.len() - 1];
    // Rotate so the lexicographically smallest element is first.
    let min_pos = core
        .iter()
        .enumerate()
        .min_by_key(|(_, n)| **n)
        .map(|(i, _)| i)
        .unwrap_or(0);
    let mut normalized: Vec<&str> = core[min_pos..].to_vec();
    normalized.extend_from_slice(&core[..min_pos]);
    normalized.push(normalized[0]); // close the loop
    normalized
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
