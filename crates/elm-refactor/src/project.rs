use std::fs;
use std::path::{Path, PathBuf};

use elm_ast::file::ElmModule;
use elm_ast::module_header::ModuleHeader;

/// A parsed Elm project: all source files with their ASTs.
pub struct Project {
    pub files: Vec<ProjectFile>,
}

pub struct ProjectFile {
    pub path: PathBuf,
    pub source: String,
    pub module: ElmModule,
    pub module_name: String,
}

#[allow(dead_code)]
impl Project {
    /// Scan a directory for .elm files, parse them all.
    pub fn load(dir: &str) -> Self {
        let paths = find_elm_files(dir);
        let mut files = Vec::new();

        for path in paths {
            let source = match fs::read_to_string(&path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("  warning: {}: {e}", path.display());
                    continue;
                }
            };
            match elm_ast::parse(&source) {
                Ok(module) => {
                    let module_name = match &module.header.value {
                        ModuleHeader::Normal { name, .. }
                        | ModuleHeader::Port { name, .. }
                        | ModuleHeader::Effect { name, .. } => name.value.join("."),
                    };
                    files.push(ProjectFile {
                        path,
                        source,
                        module,
                        module_name,
                    });
                }
                Err(errors) => {
                    eprintln!("  warning: {}: {}", path.display(), errors[0]);
                }
            }
        }

        Self { files }
    }

    /// Write all modified files back to disk.
    /// Only writes files whose printed output differs from the original source.
    pub fn write_changes(&self) -> usize {
        let mut written = 0;
        for file in &self.files {
            let printed = elm_ast::print(&file.module);
            if printed != file.source {
                if let Err(e) = fs::write(&file.path, &printed) {
                    eprintln!("  error writing {}: {e}", file.path.display());
                } else {
                    written += 1;
                }
            }
        }
        written
    }

    /// Find a module by name.
    pub fn find_module(&self, name: &str) -> Option<&ProjectFile> {
        self.files.iter().find(|f| f.module_name == name)
    }

    /// Get all module names.
    pub fn module_names(&self) -> Vec<&str> {
        self.files.iter().map(|f| f.module_name.as_str()).collect()
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
