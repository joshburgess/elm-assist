use std::collections::{HashMap, HashSet};

use elm_ast::declaration::Declaration;
use elm_ast::exposing::{ExposedItem, Exposing};
use elm_ast::expr::Expr;
use elm_ast::file::ElmModule;
use elm_ast::module_header::ModuleHeader;
use elm_ast::node::Spanned;
use elm_ast::pattern::Pattern;
use elm_ast::type_annotation::TypeAnnotation;
use elm_ast::visit::{self, Visit};

/// All information collected from a single Elm module.
#[derive(Debug)]
pub struct ModuleInfo {
    /// The module's dotted name: `["Html", "Attributes"]`
    pub module_name: Vec<String>,

    /// What this module exposes.
    pub exposing: ExposingInfo,

    /// Functions/values defined in this module.
    pub defined_values: HashSet<String>,

    /// Types defined in this module (type aliases + custom types).
    pub defined_types: HashSet<String>,

    /// Custom type constructors defined in this module.
    pub defined_constructors: HashMap<String, String>, // constructor -> parent type

    /// Imported modules: module_name -> (alias, exposing)
    pub imports: Vec<ImportInfo>,

    /// All value/function references used in expressions.
    pub used_values: HashSet<String>,

    /// All qualified references: ("Module", "function")
    pub used_qualified: HashSet<(String, String)>,

    /// All type names referenced in type annotations.
    pub used_types: HashSet<String>,

    /// All qualified type references.
    pub used_qualified_types: HashSet<(String, String)>,

    /// All constructor names used in patterns or expressions.
    pub used_constructors: HashSet<String>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct ExposingInfo {
    pub exposes_all: bool,
    pub exposed_values: HashSet<String>,
    pub exposed_types: HashSet<String>,
    /// Types exposed with `(..)` (all constructors exposed)
    pub exposed_types_open: HashSet<String>,
}

#[derive(Debug)]
pub struct ImportInfo {
    pub module_name: Vec<String>,
    pub alias: Option<String>,
    pub exposing: ImportExposingInfo,
}

#[derive(Debug)]
pub enum ImportExposingInfo {
    None,
    All,
    Explicit(Vec<String>),
}

/// Collect definitions and references from a parsed Elm module.
pub fn collect_module_info(module: &ElmModule) -> ModuleInfo {
    let module_name = match &module.header.value {
        ModuleHeader::Normal { name, .. }
        | ModuleHeader::Port { name, .. }
        | ModuleHeader::Effect { name, .. } => name.value.clone(),
    };

    let exposing = collect_exposing(module);
    let imports = collect_imports(module);

    let mut defined_values = HashSet::new();
    let mut defined_types = HashSet::new();
    let mut defined_constructors = HashMap::new();

    for decl in &module.declarations {
        match &decl.value {
            Declaration::FunctionDeclaration(func) => {
                defined_values.insert(func.declaration.value.name.value.clone());
            }
            Declaration::AliasDeclaration(alias) => {
                defined_types.insert(alias.name.value.clone());
            }
            Declaration::CustomTypeDeclaration(ct) => {
                defined_types.insert(ct.name.value.clone());
                for ctor in &ct.constructors {
                    defined_constructors
                        .insert(ctor.value.name.value.clone(), ct.name.value.clone());
                }
            }
            Declaration::PortDeclaration(sig) => {
                defined_values.insert(sig.name.value.clone());
            }
            Declaration::InfixDeclaration(_) | Declaration::Destructuring { .. } => {}
        }
    }

    // Collect references using the visitor.
    let mut collector = RefCollector::new();
    collector.visit_module(module);

    ModuleInfo {
        module_name,
        exposing,
        defined_values,
        defined_types,
        defined_constructors,
        imports,
        used_values: collector.used_values,
        used_qualified: collector.used_qualified,
        used_types: collector.used_types,
        used_qualified_types: collector.used_qualified_types,
        used_constructors: collector.used_constructors,
    }
}

fn collect_exposing(module: &ElmModule) -> ExposingInfo {
    let exposing_node = match &module.header.value {
        ModuleHeader::Normal { exposing, .. }
        | ModuleHeader::Port { exposing, .. }
        | ModuleHeader::Effect { exposing, .. } => exposing,
    };

    match &exposing_node.value {
        Exposing::All(_) => ExposingInfo {
            exposes_all: true,
            exposed_values: HashSet::new(),
            exposed_types: HashSet::new(),
            exposed_types_open: HashSet::new(),
        },
        Exposing::Explicit(items) => {
            let mut exposed_values = HashSet::new();
            let mut exposed_types = HashSet::new();
            let mut exposed_types_open = HashSet::new();

            for item in items {
                match &item.value {
                    ExposedItem::Function(name) => {
                        exposed_values.insert(name.clone());
                    }
                    ExposedItem::TypeOrAlias(name) => {
                        exposed_types.insert(name.clone());
                    }
                    ExposedItem::TypeExpose { name, open } => {
                        exposed_types.insert(name.clone());
                        if open.is_some() {
                            exposed_types_open.insert(name.clone());
                        }
                    }
                    ExposedItem::Infix(op) => {
                        exposed_values.insert(op.clone());
                    }
                }
            }

            ExposingInfo {
                exposes_all: false,
                exposed_values,
                exposed_types,
                exposed_types_open,
            }
        }
    }
}

fn collect_imports(module: &ElmModule) -> Vec<ImportInfo> {
    module
        .imports
        .iter()
        .map(|imp| {
            let alias = imp.value.alias.as_ref().map(|a| a.value.join("."));
            let exposing = match &imp.value.exposing {
                None => ImportExposingInfo::None,
                Some(exp) => match &exp.value {
                    Exposing::All(_) => ImportExposingInfo::All,
                    Exposing::Explicit(items) => {
                        let names: Vec<String> = items
                            .iter()
                            .map(|item| match &item.value {
                                ExposedItem::Function(n) => n.clone(),
                                ExposedItem::TypeOrAlias(n) => n.clone(),
                                ExposedItem::TypeExpose { name, .. } => name.clone(),
                                ExposedItem::Infix(n) => n.clone(),
                            })
                            .collect();
                        ImportExposingInfo::Explicit(names)
                    }
                },
            };

            ImportInfo {
                module_name: imp.value.module_name.value.clone(),
                alias,
                exposing,
            }
        })
        .collect()
}

/// Visitor that collects all value/type/constructor references.
struct RefCollector {
    used_values: HashSet<String>,
    used_qualified: HashSet<(String, String)>,
    used_types: HashSet<String>,
    used_qualified_types: HashSet<(String, String)>,
    used_constructors: HashSet<String>,
}

impl RefCollector {
    fn new() -> Self {
        Self {
            used_values: HashSet::new(),
            used_qualified: HashSet::new(),
            used_types: HashSet::new(),
            used_qualified_types: HashSet::new(),
            used_constructors: HashSet::new(),
        }
    }
}

impl Visit for RefCollector {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        if let Expr::FunctionOrValue { module_name, name } = &expr.value {
            if module_name.is_empty() {
                // Check if it starts with uppercase (constructor) or lowercase (value).
                if name.chars().next().is_some_and(|c| c.is_uppercase()) {
                    self.used_constructors.insert(name.clone());
                } else {
                    self.used_values.insert(name.clone());
                }
            } else {
                let module = module_name.join(".");
                self.used_qualified.insert((module, name.clone()));
            }
        }
        visit::walk_expr(self, expr);
    }

    fn visit_pattern(&mut self, pattern: &Spanned<Pattern>) {
        if let Pattern::Constructor {
            module_name, name, ..
        } = &pattern.value
        {
            if module_name.is_empty() {
                self.used_constructors.insert(name.clone());
            } else {
                let module = module_name.join(".");
                self.used_qualified.insert((module, name.clone()));
            }
        }
        visit::walk_pattern(self, pattern);
    }

    fn visit_type_annotation(&mut self, ty: &Spanned<TypeAnnotation>) {
        if let TypeAnnotation::Typed {
            module_name, name, ..
        } = &ty.value
        {
            if module_name.is_empty() {
                self.used_types.insert(name.value.clone());
            } else {
                let module = module_name.join(".");
                self.used_qualified_types
                    .insert((module, name.value.clone()));
            }
        }
        visit::walk_type_annotation(self, ty);
    }
}
