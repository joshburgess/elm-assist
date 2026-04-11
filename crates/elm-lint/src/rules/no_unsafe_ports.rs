use elm_ast::declaration::Declaration;
use elm_ast::span::Span;
use elm_ast::type_annotation::TypeAnnotation;

use crate::rule::{LintContext, LintError, Rule, Severity};

/// Reports port declarations whose type signatures include types that are not
/// safe for JavaScript interop. Elm ports can only carry JSON-compatible values;
/// using unsupported types causes runtime crashes.
///
/// Safe types: Int, Float, String, Bool, List, Array, Maybe, Json.Decode.Value,
/// Json.Encode.Value, tuples (2/3), and records composed entirely of safe types.
///
/// Unsafe: custom types, type variables (except `msg` in `Cmd msg`/`Sub msg`),
/// function types, and extensible records.
pub struct NoUnsafePorts;

impl Rule for NoUnsafePorts {
    fn name(&self) -> &'static str {
        "NoUnsafePorts"
    }

    fn description(&self) -> &'static str {
        "Port signatures must only use JSON-compatible types"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let mut errors = Vec::new();

        for decl in &ctx.module.declarations {
            if let Declaration::PortDeclaration(sig) = &decl.value {
                // Port signatures are always function types:
                //   port sendMessage : String -> Cmd msg       (outgoing)
                //   port onMessage : (String -> msg) -> Sub msg (incoming)
                //
                // For outgoing ports (X -> Cmd msg): validate X.
                // For incoming ports ((X -> msg) -> Sub msg): validate X.
                check_port_type(
                    &sig.name.value,
                    sig.type_annotation.span,
                    &sig.type_annotation.value,
                    &mut errors,
                );
            }
        }

        errors
    }
}

fn check_port_type(port_name: &str, span: Span, ty: &TypeAnnotation, errors: &mut Vec<LintError>) {
    match classify_port(ty) {
        PortKind::Outgoing(payload) => {
            check_payload_type(port_name, span, payload, errors);
        }
        PortKind::Incoming(payload) => {
            check_payload_type(port_name, span, payload, errors);
        }
        PortKind::Unknown => {
            // Not a recognized port shape — skip (the Elm compiler will catch this).
        }
    }
}

enum PortKind<'a> {
    /// `payload -> Cmd msg`
    Outgoing(&'a TypeAnnotation),
    /// `(payload -> msg) -> Sub msg`
    Incoming(&'a TypeAnnotation),
    /// Not a recognized port type shape.
    Unknown,
}

fn classify_port(ty: &TypeAnnotation) -> PortKind<'_> {
    // Outgoing: payload -> Cmd msg
    if let TypeAnnotation::FunctionType { from, to } = ty {
        if is_cmd_type(&to.value) {
            return PortKind::Outgoing(&from.value);
        }
        // Incoming: (payload -> msg) -> Sub msg
        if is_sub_type(&to.value) {
            if let TypeAnnotation::FunctionType { from: inner, .. } = &from.value {
                return PortKind::Incoming(&inner.value);
            }
        }
    }
    PortKind::Unknown
}

fn is_cmd_type(ty: &TypeAnnotation) -> bool {
    matches!(ty, TypeAnnotation::Typed { module_name, name, .. }
        if (module_name.is_empty() || module_name == &["Platform", "Cmd"])
            && name.value == "Cmd")
}

fn is_sub_type(ty: &TypeAnnotation) -> bool {
    matches!(ty, TypeAnnotation::Typed { module_name, name, .. }
        if (module_name.is_empty() || module_name == &["Platform", "Sub"])
            && name.value == "Sub")
}

fn check_payload_type(
    port_name: &str,
    span: Span,
    ty: &TypeAnnotation,
    errors: &mut Vec<LintError>,
) {
    if let Some(reason) = find_unsafe_type(ty) {
        errors.push(LintError {
            rule: "NoUnsafePorts",
            severity: Severity::Error,
            message: format!(
                "Port `{port_name}` uses {reason}, which is not safe for JavaScript interop"
            ),
            span,
            fix: None,
        });
    }
}

/// Recursively check a type for safety. Returns `Some(reason)` if unsafe.
fn find_unsafe_type(ty: &TypeAnnotation) -> Option<String> {
    match ty {
        TypeAnnotation::Unit => None,

        TypeAnnotation::GenericType(name) => {
            // Type variables are unsafe in port payloads — Elm can't serialize them.
            Some(format!("a type variable `{name}`"))
        }

        TypeAnnotation::Typed {
            module_name,
            name,
            args,
            ..
        } => {
            let unqualified = &name.value;

            // Check if this is a known safe type.
            if is_safe_named_type(module_name, unqualified) {
                // Recurse into type arguments.
                for arg in args {
                    if let Some(reason) = find_unsafe_type(&arg.value) {
                        return Some(reason);
                    }
                }
                None
            } else {
                let qualified = if module_name.is_empty() {
                    unqualified.clone()
                } else {
                    format!("{}.{}", module_name.join("."), unqualified)
                };
                Some(format!("a custom type `{qualified}`"))
            }
        }

        TypeAnnotation::Tupled(elems) => {
            if elems.len() > 3 {
                return Some("a tuple with more than 3 elements".to_string());
            }
            for elem in elems {
                if let Some(reason) = find_unsafe_type(&elem.value) {
                    return Some(reason);
                }
            }
            None
        }

        TypeAnnotation::Record(fields) => {
            for field in fields {
                if let Some(reason) = find_unsafe_type(&field.value.type_annotation.value) {
                    return Some(reason);
                }
            }
            None
        }

        TypeAnnotation::GenericRecord { .. } => Some("an extensible record type".to_string()),

        TypeAnnotation::FunctionType { .. } => Some("a function type".to_string()),
    }
}

/// Check whether a named type (possibly qualified) is safe for ports.
fn is_safe_named_type(module_name: &[String], name: &str) -> bool {
    // Primitive types (unqualified or from Basics).
    if module_name.is_empty() || module_name == ["Basics"] {
        match name {
            "Int" | "Float" | "String" | "Bool" => return true,
            _ => {}
        }
    }

    // Container types.
    if module_name.is_empty() || module_name == ["List"] {
        if name == "List" {
            return true;
        }
    }
    if module_name.is_empty() || module_name == ["Array"] {
        if name == "Array" {
            return true;
        }
    }
    if module_name.is_empty() || module_name == ["Maybe"] {
        if name == "Maybe" {
            return true;
        }
    }

    // Json.Decode.Value and Json.Encode.Value — the escape hatch.
    if (module_name == ["Json", "Decode"] || module_name == ["Json", "Encode"]) && name == "Value" {
        return true;
    }
    // Also handle the common alias patterns.
    if (module_name == ["Decode"] || module_name == ["Encode"]) && name == "Value" {
        return true;
    }
    // Unqualified Value (if imported).
    if module_name.is_empty() && name == "Value" {
        return true;
    }

    false
}
