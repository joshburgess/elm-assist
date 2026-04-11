pub mod cognitive_complexity;
pub mod no_always_identity;
pub mod no_bool_operator_simplify;
pub mod no_boolean_case;
pub mod no_confusing_prefix_operator;
pub mod no_debug;
pub mod no_deprecated;
pub mod no_duplicate_ports;
pub mod no_unused_dependencies;
pub mod no_empty_let;
pub mod no_empty_list_concat;
pub mod no_empty_record_update;
pub mod no_exposing_all;
pub mod no_fully_applied_prefix_operator;
pub mod no_identity_function;
pub mod no_if_true_false;
pub mod no_import_exposing_all;
pub mod no_inconsistent_aliases;
pub mod no_list_literal_concat;
pub mod no_max_line_length;
pub mod no_maybe_map_with_nothing;
pub mod no_missing_documentation;
pub mod no_missing_type_annotation;
pub mod no_missing_type_annotation_in_let_in;
pub mod no_missing_type_expose;
pub mod no_negation_of_boolean_operator;
pub mod no_nested_negation;
pub mod no_pipeline_simplify;
pub mod no_premature_let_computation;
pub mod no_record_pattern_in_function_args;
pub mod no_recursive_update;
pub mod no_redundant_cons;
pub mod no_redundantly_qualified_type;
pub mod no_result_map_with_err;
pub mod no_shadowing;
pub mod no_simple_let_body;
pub mod no_single_pattern_case;
pub mod no_string_concat;
pub mod no_todo_comment;
pub mod no_unnecessary_parens;
pub mod no_unnecessary_port_module;
pub mod no_unnecessary_trailing_underscore;
pub mod no_unsafe_ports;
pub mod no_unoptimized_recursion;
pub mod no_unused_custom_type_constructor_args;
pub mod no_unused_custom_type_constructors;
pub mod no_unused_exports;
pub mod no_unused_imports;
pub mod no_unused_let_binding;
pub mod no_unused_modules;
pub mod no_unused_parameters;
pub mod no_unused_patterns;
pub mod no_unused_variables;
pub mod no_wildcard_pattern_last;

use crate::rule::Rule;

/// Return all built-in rules.
pub fn all_rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(no_unused_imports::NoUnusedImports),
        Box::new(no_unused_variables::NoUnusedVariables),
        Box::new(no_debug::NoDebug),
        Box::new(no_missing_type_annotation::NoMissingTypeAnnotation),
        Box::new(no_single_pattern_case::NoSinglePatternCase),
        Box::new(no_boolean_case::NoBooleanCase),
        Box::new(no_if_true_false::NoIfTrueFalse),
        Box::new(no_unnecessary_parens::NoUnnecessaryParens),
        Box::new(no_nested_negation::NoNestedNegation),
        Box::new(no_empty_let::NoEmptyLet),
        Box::new(no_empty_record_update::NoEmptyRecordUpdate),
        Box::new(no_always_identity::NoAlwaysIdentity),
        Box::new(no_redundant_cons::NoRedundantCons),
        Box::new(no_wildcard_pattern_last::NoWildcardPatternLast),
        // Simplify rules (Phase 3).
        Box::new(no_bool_operator_simplify::NoBoolOperatorSimplify),
        Box::new(no_empty_list_concat::NoEmptyListConcat),
        Box::new(no_list_literal_concat::NoListLiteralConcat),
        Box::new(no_pipeline_simplify::NoPipelineSimplify),
        Box::new(no_negation_of_boolean_operator::NoNegationOfBooleanOperator),
        Box::new(no_string_concat::NoStringConcat),
        Box::new(no_fully_applied_prefix_operator::NoFullyAppliedPrefixOperator),
        Box::new(no_identity_function::NoIdentityFunction),
        // Code quality rules (Phase 3).
        Box::new(no_simple_let_body::NoSimpleLetBody),
        Box::new(no_unused_let_binding::NoUnusedLetBinding),
        Box::new(no_todo_comment::NoTodoComment),
        Box::new(no_maybe_map_with_nothing::NoMaybeMapWithNothing),
        Box::new(no_result_map_with_err::NoResultMapWithErr),
        // Project-level rules (require cross-module context).
        Box::new(no_unused_exports::NoUnusedExports),
        Box::new(no_unused_custom_type_constructors::NoUnusedCustomTypeConstructors),
        Box::new(no_unused_modules::NoUnusedModules),
        // New rules.
        Box::new(no_unused_parameters::NoUnusedParameters),
        Box::new(no_unused_custom_type_constructor_args::NoUnusedCustomTypeConstructorArgs),
        Box::new(no_exposing_all::NoExposingAll),
        Box::new(no_import_exposing_all::NoImportExposingAll),
        Box::new(no_deprecated::NoDeprecated),
        Box::new(no_missing_documentation::NoMissingDocumentation),
        Box::new(no_unnecessary_trailing_underscore::NoUnnecessaryTrailingUnderscore),
        Box::new(no_premature_let_computation::NoPrematureLetComputation),
        Box::new(no_unnecessary_port_module::NoUnnecessaryPortModule),
        Box::new(no_max_line_length::NoMaxLineLength::default()),
        Box::new(no_shadowing::NoShadowing),
        Box::new(no_record_pattern_in_function_args::NoRecordPatternInFunctionArgs),
        // Batch 2 rules.
        Box::new(no_unused_patterns::NoUnusedPatterns),
        Box::new(cognitive_complexity::CognitiveComplexity::default()),
        Box::new(no_missing_type_annotation_in_let_in::NoMissingTypeAnnotationInLetIn),
        Box::new(no_confusing_prefix_operator::NoConfusingPrefixOperator),
        Box::new(no_missing_type_expose::NoMissingTypeExpose),
        Box::new(no_redundantly_qualified_type::NoRedundantlyQualifiedType),
        Box::new(no_unoptimized_recursion::NoUnoptimizedRecursion),
        Box::new(no_recursive_update::NoRecursiveUpdate),
        // Port rules.
        Box::new(no_duplicate_ports::NoDuplicatePorts),
        Box::new(no_unsafe_ports::NoUnsafePorts),
        // Config-driven rules.
        Box::new(no_inconsistent_aliases::NoInconsistentAliases::default()),
        // Project-level: elm.json dependency check.
        Box::new(no_unused_dependencies::NoUnusedDependencies),
    ]
}
