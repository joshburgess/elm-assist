/// A parsed search query.
#[derive(Debug, Clone)]
pub enum Query {
    /// Functions whose return type contains a given type name.
    /// `returns Maybe`, `returns Result`
    ReturnsType(String),

    /// Functions whose signature mentions a given type anywhere.
    /// `type Html`, `type Decoder`
    UsesType(String),

    /// Case expressions matching on a given constructor/type.
    /// `case-on Result`, `case-on Maybe`
    CaseOn(String),

    /// Record updates touching a given field.
    /// `update .name`, `update .count`
    RecordUpdateField(String),

    /// All qualified calls to a module.
    /// `calls Http`, `calls Json.Decode`
    CallsTo(String),

    /// Functions with arguments that are never used in the body.
    /// `unused-args`
    UnusedArgs,

    /// Lambdas with at least N arguments.
    /// `lambda 3` (lambdas with 3+ args)
    LambdaArity(usize),

    /// Functions that use a specific function/value name.
    /// `uses map`, `uses andThen`
    Uses(String),

    /// Top-level functions matching a name pattern (substring).
    /// `def update`, `def view`
    Defined(String),

    /// Expressions of a specific shape.
    /// `expr let` (let expressions), `expr if` (if expressions)
    ExprKind(ExprKindQuery),
}

#[derive(Debug, Clone)]
pub enum ExprKindQuery {
    Let,
    Case,
    If,
    Lambda,
    Record,
    List,
    Tuple,
}

/// Parse a query string into a Query.
pub fn parse_query(input: &str) -> Result<Query, String> {
    let parts: Vec<&str> = input.splitn(2, ' ').collect();
    let cmd = parts[0].to_lowercase();
    let arg = parts.get(1).map(|s| s.trim()).unwrap_or("");

    match cmd.as_str() {
        "returns" => {
            if arg.is_empty() {
                return Err("'returns' requires a type name, e.g. 'returns Maybe'".into());
            }
            Ok(Query::ReturnsType(arg.to_string()))
        }
        "type" => {
            if arg.is_empty() {
                return Err("'type' requires a type name, e.g. 'type Html'".into());
            }
            Ok(Query::UsesType(arg.to_string()))
        }
        "case-on" | "case" => {
            if arg.is_empty() {
                return Err("'case-on' requires a type/constructor name".into());
            }
            Ok(Query::CaseOn(arg.to_string()))
        }
        "update" => {
            let field = arg.trim_start_matches('.');
            if field.is_empty() {
                return Err("'update' requires a field name, e.g. 'update .name'".into());
            }
            Ok(Query::RecordUpdateField(field.to_string()))
        }
        "calls" => {
            if arg.is_empty() {
                return Err("'calls' requires a module name, e.g. 'calls Http'".into());
            }
            Ok(Query::CallsTo(arg.to_string()))
        }
        "unused-args" => Ok(Query::UnusedArgs),
        "lambda" => {
            let n: usize = arg
                .parse()
                .map_err(|_| "'lambda' requires a number, e.g. 'lambda 3'")?;
            Ok(Query::LambdaArity(n))
        }
        "uses" => {
            if arg.is_empty() {
                return Err("'uses' requires a name, e.g. 'uses map'".into());
            }
            Ok(Query::Uses(arg.to_string()))
        }
        "def" => {
            if arg.is_empty() {
                return Err("'def' requires a name pattern, e.g. 'def update'".into());
            }
            Ok(Query::Defined(arg.to_string()))
        }
        "expr" => {
            let kind = match arg.to_lowercase().as_str() {
                "let" => ExprKindQuery::Let,
                "case" => ExprKindQuery::Case,
                "if" => ExprKindQuery::If,
                "lambda" => ExprKindQuery::Lambda,
                "record" => ExprKindQuery::Record,
                "list" => ExprKindQuery::List,
                "tuple" => ExprKindQuery::Tuple,
                _ => {
                    return Err(format!(
                        "Unknown expr kind: {arg}. Try: let, case, if, lambda, record, list, tuple"
                    ));
                }
            };
            Ok(Query::ExprKind(kind))
        }
        _ => Err(format!(
            "Unknown query: '{cmd}'. Available: returns, type, case-on, update, calls, unused-args, lambda, uses, def, expr"
        )),
    }
}
