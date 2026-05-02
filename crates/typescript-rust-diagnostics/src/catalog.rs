use crate::{DiagnosticCategory, DiagnosticSupport, TypeScriptDiagnosticDefinition};

macro_rules! define_typescript_diagnostics_catalog {
    (
        $(
            $variant:ident => {
                code: $code:literal,
                key: $key:literal,
                category: $category:ident,
                message_template: $message_template:literal,
                argument_count: $argument_count:literal,
                support: $support:ident,
            }
        ),+ $(,)?
    ) => {
        /// Stable internal catalog for TypeScript-compatible diagnostics.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[repr(usize)]
        pub enum TypeScriptDiagnosticKind {
            $($variant),+
        }

        pub static TYPE_SCRIPT_DIAGNOSTIC_DEFINITIONS: &[TypeScriptDiagnosticDefinition] = &[
            $(
                TypeScriptDiagnosticDefinition {
                    code: $code,
                    key: $key,
                    category: DiagnosticCategory::$category,
                    message_template: $message_template,
                    argument_count: $argument_count,
                    support: DiagnosticSupport::$support,
                }
            ),+
        ];

        impl TypeScriptDiagnosticKind {
            pub fn definition(self) -> &'static TypeScriptDiagnosticDefinition {
                &TYPE_SCRIPT_DIAGNOSTIC_DEFINITIONS[self as usize]
            }

            pub fn code(self) -> u32 {
                self.definition().code
            }

            pub fn message_template(self) -> &'static str {
                self.definition().message_template
            }

            pub fn support(self) -> DiagnosticSupport {
                self.definition().support
            }
        }
    };
}

define_typescript_diagnostics_catalog! {
    TsconfigNotLoaded => {
        code: 5112,
        key: "tsconfig_json_is_present_but_will_not_be_loaded_if_files_are_specified_on_commandline_Use_ignoreConfig_to_skip_this_error",
        category: Error,
        message_template: "tsconfig.json is present but will not be loaded if files are specified on commandline. Use '--ignoreConfig' to skip this error.",
        argument_count: 0,
        support: Emitted,
    },

    CannotFindName => {
        code: 2304,
        key: "Cannot_find_name_0",
        category: Error,
        message_template: "Cannot find name '{0}'.",
        argument_count: 1,
        support: Emitted,
    },
    TypeNotAssignable => {
        code: 2322,
        key: "Type_0_is_not_assignable_to_type_1",
        category: Error,
        message_template: "Type '{0}' is not assignable to type '{1}'.",
        argument_count: 2,
        support: Emitted,
    },
    PropertyDoesNotExist => {
        code: 2339,
        key: "Property_0_does_not_exist_on_type_1",
        category: Error,
        message_template: "Property '{0}' does not exist on type '{1}'.",
        argument_count: 2,
        support: Emitted,
    },
    ArgumentNotAssignableToParameter => {
        code: 2345,
        key: "Argument_of_type_0_is_not_assignable_to_parameter_of_type_1",
        category: Error,
        message_template: "Argument of type '{0}' is not assignable to parameter of type '{1}'.",
        argument_count: 2,
        support: Emitted,
    },
    ParameterImplicitlyHasAny => {
        code: 7006,
        key: "Parameter_0_implicitly_has_an_any_type",
        category: Error,
        message_template: "Parameter '{0}' implicitly has an 'any' type.",
        argument_count: 1,
        support: Emitted,
    },
    ThisExpressionIsNotCallable => {
        code: 2349,
        key: "This_expression_is_not_callable",
        category: Error,
        message_template: "This expression is not callable.",
        argument_count: 0,
        support: Emitted,
    },
    CannotRedeclareBlockScopedVariable => {
        code: 2451,
        key: "Cannot_redeclare_block_scoped_variable_0",
        category: Error,
        message_template: "Cannot redeclare block-scoped variable '{0}'.",
        argument_count: 1,
        support: Emitted,
    },
    ExpectedArguments => {
        code: 2554,
        key: "Expected_0_arguments_but_got_1",
        category: Error,
        message_template: "Expected {0} arguments, but got {1}.",
        argument_count: 2,
        support: Emitted,
    },
    CannotAssignToConstant => {
        code: 2588,
        key: "Cannot_assign_to_0_because_it_is_a_constant",
        category: Error,
        message_template: "Cannot assign to '{0}' because it is a constant.",
        argument_count: 1,
        support: Emitted,
    },
    LeftHandSideOfArithmeticOperationMustBeNumberLike => {
        code: 2362,
        key: "The_left_hand_side_of_an_arithmetic_operation_must_be_number_like",
        category: Error,
        message_template: "The left-hand side of an arithmetic operation must be of type 'any', 'number', 'bigint' or an enum type.",
        argument_count: 0,
        support: Emitted,
    },
    RightHandSideOfArithmeticOperationMustBeNumberLike => {
        code: 2363,
        key: "The_right_hand_side_of_an_arithmetic_operation_must_be_number_like",
        category: Error,
        message_template: "The right-hand side of an arithmetic operation must be of type 'any', 'number', 'bigint' or an enum type.",
        argument_count: 0,
        support: Emitted,
    },
    ArithmeticOperandMustBeNumberLike => {
        code: 2356,
        key: "An_arithmetic_operand_must_be_number_like",
        category: Error,
        message_template: "An arithmetic operand must be of type 'any', 'number', 'bigint' or an enum type.",
        argument_count: 0,
        support: Emitted,
    },
    OperatorCannotBeAppliedToTypes => {
        code: 2365,
        key: "Operator_0_cannot_be_applied_to_types_1_and_2",
        category: Error,
        message_template: "Operator '{0}' cannot be applied to types '{1}' and '{2}'.",
        argument_count: 3,
        support: Emitted,
    },
    ComparisonAppearsUnintentionalNoOverlap => {
        code: 2367,
        key: "This_comparison_appears_to_be_unintentional_because_the_types_0_and_1_have_no_overlap",
        category: Error,
        message_template: "This comparison appears to be unintentional because the types '{0}' and '{1}' have no overlap.",
        argument_count: 2,
        support: Emitted,
    },
    DuplicateIdentifier => {
        code: 2300,
        key: "Duplicate_identifier_0",
        category: Error,
        message_template: "Duplicate identifier '{0}'.",
        argument_count: 1,
        support: Emitted,
    },
    FileIsNotAModule => {
        code: 2306,
        key: "File_0_is_not_a_module",
        category: Error,
        message_template: "File '{0}' is not a module.",
        argument_count: 1,
        support: CatalogOnly,
    },
    CannotFindModule => {
        code: 2307,
        key: "Cannot_find_module_0_or_its_corresponding_type_declarations",
        category: Error,
        message_template: "Cannot find module '{0}' or its corresponding type declarations.",
        argument_count: 1,
        support: CatalogOnly,
    },
    GenericTypeRequiresTypeArguments => {
        code: 2314,
        key: "Generic_type_0_requires_1_type_argument_s",
        category: Error,
        message_template: "Generic type '{0}' requires {1} type argument(s).",
        argument_count: 2,
        support: CatalogOnly,
    },
    TypeIsNotGeneric => {
        code: 2315,
        key: "Type_0_is_not_generic",
        category: Error,
        message_template: "Type '{0}' is not generic.",
        argument_count: 1,
        support: CatalogOnly,
    },
    TypeDoesNotSatisfyConstraint => {
        code: 2344,
        key: "Type_0_does_not_satisfy_the_constraint_1",
        category: Error,
        message_template: "Type '{0}' does not satisfy the constraint '{1}'.",
        argument_count: 2,
        support: CatalogOnly,
    },
    ThisExpressionIsNotConstructable => {
        code: 2351,
        key: "This_expression_is_not_constructable",
        category: Error,
        message_template: "This expression is not constructable.",
        argument_count: 0,
        support: CatalogOnly,
    },
    ConversionMayBeAMistake => {
        code: 2352,
        key: "Conversion_of_type_0_to_type_1_may_be_a_mistake",
        category: Error,
        message_template: "Conversion of type '{0}' to type '{1}' may be a mistake because neither type sufficiently overlaps with the other. If this was intentional, convert the expression to 'unknown' first.",
        argument_count: 2,
        support: CatalogOnly,
    },
    ObjectLiteralMayOnlySpecifyKnownProperties => {
        code: 2353,
        key: "Object_literal_may_only_specify_known_properties",
        category: Error,
        message_template: "Object literal may only specify known properties, and '{0}' does not exist in type '{1}'.",
        argument_count: 2,
        support: Emitted,
    },
    FunctionMustReturnAValue => {
        code: 2355,
        key: "A_function_whose_declared_type_is_neither_undefined_void_nor_any_must_return_a_value",
        category: Error,
        message_template: "A function whose declared type is neither 'undefined', 'void', nor 'any' must return a value.",
        argument_count: 0,
        support: Emitted,
    },
    FunctionLacksEndingReturnStatement => {
        code: 2366,
        key: "Function_lacks_ending_return_statement_and_return_type_does_not_include_undefined",
        category: Error,
        message_template: "Function lacks ending return statement and return type does not include 'undefined'.",
        argument_count: 0,
        support: Emitted,
    },
    DuplicateFunctionImplementation => {
        code: 2393,
        key: "Duplicate_function_implementation",
        category: Error,
        message_template: "Duplicate function implementation.",
        argument_count: 0,
        support: Emitted,
    },
    OverloadSignatureNotCompatibleWithImplementationSignature => {
        code: 2394,
        key: "This_overload_signature_is_not_compatible_with_its_implementation_signature",
        category: Error,
        message_template: "This overload signature is not compatible with its implementation signature.",
        argument_count: 0,
        support: CatalogOnly,
    },
    BlockScopedVariableUsedBeforeItsDeclaration => {
        code: 2448,
        key: "Block_scoped_variable_0_used_before_its_declaration",
        category: Error,
        message_template: "Block-scoped variable '{0}' used before its declaration.",
        argument_count: 1,
        support: Emitted,
    },
    VariableUsedBeforeBeingAssigned => {
        code: 2454,
        key: "Variable_0_is_used_before_being_assigned",
        category: Error,
        message_template: "Variable '{0}' is used before being assigned.",
        argument_count: 1,
        support: Emitted,
    },
    PropertyDoesNotExistOnTypeDidYouMean => {
        code: 2551,
        key: "Property_0_does_not_exist_on_type_1_did_you_mean_2",
        category: Error,
        message_template: "Property '{0}' does not exist on type '{1}'. Did you mean '{2}'?",
        argument_count: 3,
        support: CatalogOnly,
    },
    TypeOnlyRefersToATypeButIsBeingUsedAsAValueHere => {
        code: 2693,
        key: "Type_0_only_refers_to_a_type_but_is_being_used_as_a_value_here",
        category: Error,
        message_template: "'{0}' only refers to a type, but is being used as a value here.",
        argument_count: 1,
        support: CatalogOnly,
    },
    PropertyIsMissingInTypeButRequiredInType => {
        code: 2741,
        key: "Property_0_is_missing_in_type_1_but_required_in_type_2",
        category: Error,
        message_template: "Property '{0}' is missing in type '{1}' but required in type '{2}'.",
        argument_count: 3,
        support: Emitted,
    },
    ValueRefersToAValueButIsBeingUsedAsATypeHere => {
        code: 2749,
        key: "Type_0_refers_to_a_value_but_is_being_used_as_a_type_here_did_you_mean_typeof_0",
        category: Error,
        message_template: "'{0}' refers to a value, but is being used as a type here. Did you mean 'typeof {0}'?",
        argument_count: 1,
        support: CatalogOnly,
    },
    ThisKindOfExpressionIsAlwaysTruthy => {
        code: 2872,
        key: "This_kind_of_expression_is_always_truthy",
        category: Error,
        message_template: "This kind of expression is always truthy.",
        argument_count: 0,
        support: Emitted,
    },
    ThisKindOfExpressionIsAlwaysFalsy => {
        code: 2873,
        key: "This_kind_of_expression_is_always_falsy",
        category: Error,
        message_template: "This kind of expression is always falsy.",
        argument_count: 0,
        support: Emitted,
    },
    VariableImplicitlyHasAnyType => {
        code: 7005,
        key: "Variable_0_implicitly_has_an_1_type",
        category: Error,
        message_template: "Variable '{0}' implicitly has an '{1}' type.",
        argument_count: 2,
        support: Emitted,
    },
    RestParameterImplicitlyHasAnyArrayType => {
        code: 7019,
        key: "Rest_parameter_0_implicitly_has_an_any_array_type",
        category: Error,
        message_template: "Rest parameter '{0}' implicitly has an 'any[]' type.",
        argument_count: 1,
        support: CatalogOnly,
    },
    NotAllCodePathsReturnAValue => {
        code: 7030,
        key: "Not_all_code_paths_return_a_value",
        category: Error,
        message_template: "Not all code paths return a value.",
        argument_count: 0,
        support: CatalogOnly,
    },
    BindingElementImplicitlyHasAnyType => {
        code: 7031,
        key: "Binding_element_0_implicitly_has_an_1_type",
        category: Error,
        message_template: "Binding element '{0}' implicitly has an '{1}' type.",
        argument_count: 2,
        support: CatalogOnly,
    },
    VariableImplicitlyHasTypeInSomeLocations => {
        code: 7034,
        key: "Variable_0_implicitly_has_type_1_in_some_locations_where_its_type_cannot_be_determined",
        category: Error,
        message_template: "Variable '{0}' implicitly has type '{1}' in some locations where its type cannot be determined.",
        argument_count: 2,
        support: CatalogOnly,
    },
    ParameterHasANameButNoType => {
        code: 7051,
        key: "Parameter_has_a_name_but_no_type_did_you_mean_0_colon_1",
        category: Error,
        message_template: "Parameter has a name but no type. Did you mean '{0}: {1}'?",
        argument_count: 2,
        support: CatalogOnly,
    },
    ElementImplicitlyHasAnyTypeBecauseTypeHasNoIndexSignature => {
        code: 7052,
        key: "Element_implicitly_has_an_any_type_because_type_0_has_no_index_signature_did_you_mean_to_call_1",
        category: Error,
        message_template: "Element implicitly has an 'any' type because type '{0}' has no index signature. Did you mean to call '{1}'?",
        argument_count: 2,
        support: CatalogOnly,
    },
    ElementImplicitlyHasAnyTypeBecauseExpressionCantBeUsedToIndexType => {
        code: 7053,
        key: "Element_implicitly_has_an_any_type_because_expression_of_type_0_cant_be_used_to_index_type_1",
        category: Error,
        message_template: "Element implicitly has an 'any' type because expression of type '{0}' can't be used to index type '{1}'.",
        argument_count: 2,
        support: CatalogOnly,
    },
    NoIndexSignatureWithParameterOfTypeWasFoundOnType => {
        code: 7054,
        key: "No_index_signature_with_a_parameter_of_type_0_was_found_on_type_1",
        category: Error,
        message_template: "No index signature with a parameter of type '{0}' was found on type '{1}'.",
        argument_count: 2,
        support: CatalogOnly,
    },
    LacksReturnTypeAnnotationImplicitlyHasYieldType => {
        code: 7055,
        key: "Implicitly_has_an_1_yield_type",
        category: Error,
        message_template: "'{0}', which lacks return-type annotation, implicitly has an '{1}' yield type.",
        argument_count: 2,
        support: CatalogOnly,
    },
    InferredTypeExceedsMaximumLength => {
        code: 7056,
        key: "Inferred_type_exceeds_maximum_length",
        category: Error,
        message_template: "The inferred type of this node exceeds the maximum length the compiler will serialize. An explicit type annotation is needed.",
        argument_count: 0,
        support: CatalogOnly,
    },
    YieldExpressionImplicitlyResultsInAnyType => {
        code: 7057,
        key: "Yield_expression_implicitly_results_in_any_type",
        category: Error,
        message_template: "'yield' expression implicitly results in an 'any' type because its containing generator lacks a return-type annotation.",
        argument_count: 0,
        support: CatalogOnly,
    },
    PackageExposesModuleAddDeclareModule => {
        code: 7058,
        key: "Package_exposes_module_add_declare_module",
        category: Error,
        message_template: "If the '{0}' package actually exposes this module, try adding a new declaration (.d.ts) file containing `declare module '{1}';`",
        argument_count: 2,
        support: CatalogOnly,
    },
    ReservedSyntaxUseAsExpressionInMtsOrCts => {
        code: 7059,
        key: "Reserved_syntax_use_as_expression_in_mts_or_cts",
        category: Error,
        message_template: "This syntax is reserved in files with the .mts or .cts extension. Use an `as` expression instead.",
        argument_count: 0,
        support: CatalogOnly,
    },
    ReservedSyntaxAddTrailingCommaOrExplicitConstraint => {
        code: 7060,
        key: "Reserved_syntax_add_trailing_comma_or_explicit_constraint",
        category: Error,
        message_template: "This syntax is reserved in files with the .mts or .cts extension. Add a trailing comma or explicit constraint.",
        argument_count: 0,
        support: CatalogOnly,
    },
    MappedTypeMayNotDeclarePropertiesOrMethods => {
        code: 7061,
        key: "Mapped_type_may_not_declare_properties_or_methods",
        category: Error,
        message_template: "A mapped type may not declare properties or methods.",
        argument_count: 0,
        support: CatalogOnly,
    }
}

pub fn cataloged_typescript_diagnostics() -> &'static [TypeScriptDiagnosticDefinition] {
    TYPE_SCRIPT_DIAGNOSTIC_DEFINITIONS
}

pub fn emitted_typescript_diagnostics()
-> impl Iterator<Item = &'static TypeScriptDiagnosticDefinition> {
    TYPE_SCRIPT_DIAGNOSTIC_DEFINITIONS
        .iter()
        .filter(|definition| definition.support == DiagnosticSupport::Emitted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_contains_ts5112() {
        let definition = TypeScriptDiagnosticKind::TsconfigNotLoaded.definition();
        assert_eq!(definition.code, 5112);
        assert_eq!(definition.argument_count, 0);
        assert_eq!(definition.support, DiagnosticSupport::Emitted);
    }
}
