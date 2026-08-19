//! Property access, index access, calls, and their optional-chaining variants.

use super::*;

use std::time::Instant;

use surge_ts_syntax::{ParsedExpression, TextSpan};
use surge_ts_types::{Type, is_assignable_to, union_type};

use crate::context::CheckerContext;
use crate::modules::{PROMISE_LIKE_VALUE_PROPERTY, promise_like_type};
use crate::program::{record_program_timing, record_property_lookup};
use crate::symbols::SymbolTable;

use crate::infer::InferredExpression;

pub(crate) fn infer_index_access(
    object_name: &str,
    object_span: &Option<TextSpan>,
    index: &ParsedExpression,
    index_span: &Option<TextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let Some(symbol) = symbols.get(object_name) else {
        return InferredExpression::UnresolvedIdentifier {
            name: object_name.to_string(),
            span: *object_span,
        };
    };

    match &symbol.ty {
        Type::Any => InferredExpression::Known(Type::Any),
        Type::Unknown | Type::GenuineUnknown => InferredExpression::Unknown,
        Type::Union(union_type) => {
            let mut result_types = vec![];
            for ty in union_type.types() {
                if *ty == Type::Undefined {
                    result_types.push(ty.clone());
                    continue;
                }
                match ty {
                    Type::Tuple(elements) => {
                        let res =
                            infer_tuple_index_access(elements, index, index_span, symbols, ctx);
                        if let InferredExpression::Known(ty) = res {
                            result_types.push(ty);
                        } else {
                            return InferredExpression::Unknown;
                        }
                    }
                    Type::Array(element_type) => {
                        let element_type = element_type.as_ref();
                        if element_type.is_unknown() {
                            return InferredExpression::Unknown;
                        }

                        let index_type = match infer_expression(index, symbols, ctx) {
                            InferredExpression::Known(ty) => ty,
                            _ => return InferredExpression::Unknown,
                        };

                        if !surge_ts_types::is_assignable_to(&index_type, &Type::Number) {
                            return InferredExpression::Unknown;
                        }
                        result_types.push(element_type.clone());
                    }
                    _ => return InferredExpression::Unknown,
                }
            }
            InferredExpression::Known(surge_ts_types::union_type(result_types))
        }
        Type::Tuple(elements) => {
            infer_tuple_index_access(elements, index, index_span, symbols, ctx)
        }
        Type::Array(element_type) => {
            let element_type = element_type.as_ref();
            if element_type.is_unknown() {
                return InferredExpression::Unknown;
            }

            let index_type = match infer_expression(index, symbols, ctx) {
                InferredExpression::Known(ty) => ty,
                InferredExpression::UnresolvedIdentifier { .. }
                | InferredExpression::MissingProperty { .. }
                | InferredExpression::Unknown => return InferredExpression::Unknown,
            };

            if !is_assignable_to(&index_type, &Type::Number) {
                let _ = index_span;
                return InferredExpression::Unknown;
            }

            InferredExpression::Known(element_type.clone())
        }
        Type::Object(_)
        | Type::Function(_)
        | Type::String
        | Type::Number
        | Type::Boolean
        | Type::BigInt
        | Type::Symbol
        | Type::Void
        | Type::Never
        | Type::StringLiteral(_)
        | Type::NumberLiteral(_)
        | Type::BooleanLiteral(_)
        | Type::Reference(_)
        | Type::Undefined => InferredExpression::Unknown,
    }
}

pub(crate) fn infer_tuple_index_access(
    elements: &[Type],
    index: &ParsedExpression,
    index_span: &Option<TextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let index_type = match infer_expression(index, symbols, ctx) {
        InferredExpression::Known(ty) => ty,
        InferredExpression::UnresolvedIdentifier { .. }
        | InferredExpression::MissingProperty { .. }
        | InferredExpression::Unknown => return InferredExpression::Unknown,
    };

    if let Some(index_value) = tuple_index_value(&index_type) {
        return elements
            .get(index_value)
            .cloned()
            .map(InferredExpression::Known)
            .unwrap_or(InferredExpression::Unknown);
    }

    if is_assignable_to(&index_type, &Type::Number) {
        return InferredExpression::Known(union_type(elements.to_vec()));
    }

    let _ = index_span;
    InferredExpression::Unknown
}

pub(crate) fn infer_property_access(
    object: &ParsedExpression,
    _object_span: &Option<TextSpan>,
    property_name: &str,
    property_span: &Option<TextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    record_property_lookup();
    let property_access_start = Instant::now();
    let object_type = match infer_expression(object, symbols, ctx) {
        InferredExpression::Known(ty) => ty,
        InferredExpression::UnresolvedIdentifier { name, span } => {
            return InferredExpression::UnresolvedIdentifier { name, span };
        }
        InferredExpression::MissingProperty { .. } | InferredExpression::Unknown => {
            return InferredExpression::Unknown;
        }
    };

    let result = crate::program::with_dts_expansion_reason(
        crate::program::DtsExpansionReason::PropertyLookup,
        || match &object_type {
            Type::Any => InferredExpression::Known(Type::Any),
            Type::Unknown | Type::GenuineUnknown => InferredExpression::Unknown,
            Type::Union(union_type) => {
                // A sentinel member means part of the receiver is unmodelled, so
                // a miss on any *other* member says nothing about the source —
                // the same no-cascade rule a wholly-sentinel receiver gets.
                if union_type
                    .types()
                    .iter()
                    .any(surge_ts_types::Type::is_unknown)
                {
                    return InferredExpression::Unknown;
                }
                let mut result_types = vec![];
                for ty in union_type.types() {
                    if *ty == Type::Undefined {
                        result_types.push(ty.clone());
                        continue;
                    }
                    match ty.get_property_access_type(property_name) {
                        Some(ty) => result_types.push(ty),
                        None if no_lib_array_member(ty, ctx) => result_types.push(Type::Any),
                        None => {
                            return InferredExpression::MissingProperty {
                                property_name: property_name.to_string(),
                                object_type: ty.clone(),
                                span: *property_span,
                            };
                        }
                    }
                }
                InferredExpression::Known(surge_ts_types::union_type(result_types))
            }
            _ => object_type
                .get_property_access_type(property_name)
                .map(InferredExpression::Known)
                .unwrap_or_else(|| {
                    if no_lib_array_member(&object_type, ctx) {
                        InferredExpression::Known(Type::Any)
                    // A nominal reference that peels to the sentinel is a shape
                    // surge could not reconstruct (a cross-module `Set<string>`
                    // annotation whose lazy environment is gone), not a type
                    // without the member.
                    } else if object_type.peeled().is_unknown() {
                        InferredExpression::Unknown
                    } else {
                        InferredExpression::MissingProperty {
                            property_name: property_name.to_string(),
                            object_type: object_type.clone(),
                            span: *property_span,
                        }
                    }
                }),
        },
    );
    record_program_timing(ctx.timings.as_ref(), |timings| {
        timings.property_access_checking += property_access_start.elapsed()
    });
    result
}

/// The symbol bound to a qualified `ns.member` value key, using the same lookup
/// chain a bare call resolves through.
pub(crate) fn qualified_namespace_member(
    qualified_name: &str,
    symbols: &SymbolTable,
    ctx: &CheckerContext,
) -> Option<std::sync::Arc<crate::symbols::SymbolInfo>> {
    if let Some(symbol) = symbols.get_shared(qualified_name) {
        return Some(symbol);
    }
    if let Some(symbol) = ctx
        .module_value_fallback
        .as_ref()
        .and_then(|fallback| fallback.get_shared(qualified_name))
    {
        return Some(symbol);
    }
    let file_name = ctx.file_name.clone();
    ctx.module_local_values_for_file(&file_name)
        .and_then(|table| table.get_shared(qualified_name))
}

/// Inference-side twin of `try_qualified_namespace_call`: instantiates the
/// member's return type through its qualified `ns.member` signature. `None` when
/// no such binding exists.
fn infer_qualified_namespace_call(
    object_name: &str,
    property_name: &str,
    property_span: Option<TextSpan>,
    type_arguments: &[surge_ts_syntax::ParsedType],
    arguments: &[surge_ts_syntax::ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<InferredExpression> {
    let qualified_name = format!("{object_name}.{property_name}");
    let symbol = qualified_namespace_member(&qualified_name, symbols, ctx)?;
    // The qualified binding is authoritative once it exists: answering from the
    // namespace object's permissive `any` member instead would bake that into
    // whatever declaration is being resolved.
    let Type::Function(function_type) = &symbol.ty else {
        return Some(InferredExpression::Unknown);
    };
    let function_type = function_type.clone();
    let function_signature = symbol.function_signature.clone();
    Some(InferredExpression::Known(
        crate::checks::call::instantiate_function_return_type_for_call(
            &function_type,
            function_signature.as_deref(),
            type_arguments,
            property_span,
            arguments,
            symbols,
            ctx,
        ),
    ))
}

pub(crate) fn infer_property_call(
    object: &ParsedExpression,
    _object_span: &Option<TextSpan>,
    property_name: &str,
    property_span: &Option<TextSpan>,
    type_arguments: &[surge_ts_syntax::ParsedType],
    arguments: &[surge_ts_syntax::ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    record_property_lookup();
    let property_call_start = Instant::now();
    let object_type = match infer_expression(object, symbols, ctx) {
        InferredExpression::Known(ty) => ty,
        InferredExpression::UnresolvedIdentifier { name, span } => {
            return InferredExpression::UnresolvedIdentifier { name, span };
        }
        InferredExpression::MissingProperty { .. } | InferredExpression::Unknown => {
            return InferredExpression::Unknown;
        }
    };

    if matches!(
        object_type,
        Type::Any | Type::Unknown | Type::GenuineUnknown
    ) && let ParsedExpression::Identifier { name, .. } = object
        && let Some(inferred) = infer_qualified_namespace_call(
            name,
            property_name,
            *property_span,
            type_arguments,
            arguments,
            symbols,
            ctx,
        )
    {
        record_program_timing(ctx.timings.as_ref(), |timings| {
            timings.property_access_checking += property_call_start.elapsed()
        });
        return inferred;
    }

    let result = match &object_type {
        Type::Any => InferredExpression::Known(Type::Any),
        Type::Unknown | Type::GenuineUnknown => InferredExpression::Unknown,
        _ if property_name == "then" && promise_like_value_type(&object_type).is_some() => {
            InferredExpression::Known(promise_like_type(Type::Unknown))
        }
        Type::Array(element_type) if property_name == "find" => {
            InferredExpression::Known(surge_ts_types::union_type(vec![
                element_type.as_ref().clone(),
                Type::Undefined,
            ]))
        }
        Type::Union(union_type) => {
            let mut result_types = vec![];
            for ty in union_type.types() {
                if *ty == Type::Undefined {
                    result_types.push(ty.clone());
                    continue;
                }
                if property_name == "find"
                    && let Type::Array(element_type) = ty
                {
                    result_types.push(surge_ts_types::union_type(vec![
                        element_type.as_ref().clone(),
                        Type::Undefined,
                    ]));
                    continue;
                }
                match ty.get_property_access_type(property_name) {
                    Some(Type::Function(function_type)) => {
                        result_types.push(function_type.return_type().clone());
                    }
                    Some(Type::Any) => result_types.push(Type::Any),
                    Some(_) | None => return InferredExpression::Unknown,
                }
            }
            InferredExpression::Known(surge_ts_types::union_type(result_types))
        }
        _ => match object_type.get_property_access_type(property_name) {
            // A generic namespace member call resolves through its qualified
            // `ns.member` binding, which carries the real signature the namespace
            // object does not — the inference-side twin of the routing in
            // `check_property_call_like`, gated the same way on the permissive
            // member shape so the lookup stays off the ordinary path.
            Some(member_type)
                if crate::checks::call::is_permissive_member_type(&member_type)
                    && matches!(object, ParsedExpression::Identifier { .. }) =>
            {
                let ParsedExpression::Identifier { name, .. } = object else {
                    unreachable!("guarded by the match arm")
                };
                match infer_qualified_namespace_call(
                    name,
                    property_name,
                    *property_span,
                    type_arguments,
                    arguments,
                    symbols,
                    ctx,
                ) {
                    Some(inferred) => inferred,
                    None => match member_type {
                        Type::Function(function_type) => {
                            InferredExpression::Known(function_type.return_type().clone())
                        }
                        _ => InferredExpression::Known(Type::Any),
                    },
                }
            }
            Some(Type::Function(function_type)) => {
                InferredExpression::Known(function_type.return_type().clone())
            }
            Some(Type::Any) => InferredExpression::Known(Type::Any),
            None if no_lib_array_member(&object_type, ctx) => InferredExpression::Known(Type::Any),
            Some(_) | None => InferredExpression::Unknown,
        },
    };
    record_program_timing(ctx.timings.as_ref(), |timings| {
        timings.property_access_checking += property_call_start.elapsed()
    });
    result
}

/// Under `noLib` the array member surface comes from the configured replacement
/// lib (roblox-ts's `Array` adds `size`/`push`/`pop`/… that the standard JS array
/// surface lacks). surge collapses that interface to `Type::Array` for
/// assignability, discarding its member set, so a member the std surface does not
/// know is not a real typo here — stay permissive instead of over-reporting
/// TS2339. Without `noLib` the std array surface is authoritative and a miss is a
/// genuine error.
fn no_lib_array_member(object_type: &Type, ctx: &CheckerContext) -> bool {
    ctx.options.no_lib && matches!(object_type, Type::Array(_))
}

fn promise_like_value_type(ty: &Type) -> Option<Type> {
    let Type::Object(object_type) = ty.peeled() else {
        return None;
    };
    object_type
        .get_property_type(PROMISE_LIKE_VALUE_PROPERTY)
        .cloned()
}

/// Non-optional element access on an arbitrary object expression (`expr[index]`).
/// Mirrors tuple/array indexing without the `| undefined` that optional access
/// adds, so a destructured `const [, setX] = useState()` reads the exact element
/// type (the setter) rather than `setter | undefined`.
pub(crate) fn infer_element_access(
    object: &ParsedExpression,
    _object_span: &Option<TextSpan>,
    index: &ParsedExpression,
    index_span: &Option<TextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let object_type = match infer_expression(object, symbols, ctx) {
        InferredExpression::Known(ty) => ty,
        InferredExpression::UnresolvedIdentifier { name, span } => {
            return InferredExpression::UnresolvedIdentifier { name, span };
        }
        InferredExpression::MissingProperty { .. } | InferredExpression::Unknown => {
            return InferredExpression::Unknown;
        }
    };

    match &object_type {
        Type::Any => InferredExpression::Known(Type::Any),
        Type::Tuple(elements) => {
            infer_tuple_index_access(elements, index, index_span, symbols, ctx)
        }
        Type::Array(element_type) => {
            if element_type.as_ref().is_unknown() {
                return InferredExpression::Unknown;
            }
            match infer_expression(index, symbols, ctx) {
                InferredExpression::Known(Type::NumberLiteral(_))
                | InferredExpression::Known(Type::Number) => {
                    InferredExpression::Known((**element_type).clone())
                }
                _ => InferredExpression::Unknown,
            }
        }
        _ => InferredExpression::Unknown,
    }
}

pub(crate) fn infer_optional_index_access(
    object: &ParsedExpression,
    _object_span: &Option<TextSpan>,
    index: &ParsedExpression,
    index_span: &Option<TextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let object_type = match infer_expression(object, symbols, ctx) {
        InferredExpression::Known(ty) => ty,
        InferredExpression::UnresolvedIdentifier { name, span } => {
            return InferredExpression::UnresolvedIdentifier { name, span };
        }
        InferredExpression::MissingProperty { .. } | InferredExpression::Unknown => {
            return InferredExpression::Unknown;
        }
    };

    let base_type = surge_ts_types::remove_undefined(&object_type);

    match &base_type {
        Type::Any => InferredExpression::Known(Type::Any),
        Type::Unknown | Type::GenuineUnknown => InferredExpression::Unknown,
        Type::Tuple(elements) => {
            let result = infer_tuple_index_access(elements, index, index_span, symbols, ctx);
            match result {
                InferredExpression::Known(ty) => {
                    InferredExpression::Known(union_type(vec![ty, Type::Undefined]))
                }
                _ => result,
            }
        }
        Type::Array(element_type) => {
            let element_type = element_type.as_ref();
            if element_type.is_unknown() {
                return InferredExpression::Unknown;
            }

            let index_type = match infer_expression(index, symbols, ctx) {
                InferredExpression::Known(ty) => ty,
                InferredExpression::UnresolvedIdentifier { .. }
                | InferredExpression::MissingProperty { .. }
                | InferredExpression::Unknown => return InferredExpression::Unknown,
            };

            if let Type::NumberLiteral(_) | Type::Number = index_type {
                InferredExpression::Known(union_type(vec![element_type.clone(), Type::Undefined]))
            } else {
                InferredExpression::Unknown
            }
        }
        _ => InferredExpression::Unknown,
    }
}

pub(crate) fn infer_optional_property_access(
    object: &ParsedExpression,
    _object_span: &Option<TextSpan>,
    property_name: &str,
    property_span: &Option<TextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    record_property_lookup();
    let property_access_start = Instant::now();
    let object_type = match infer_expression(object, symbols, ctx) {
        InferredExpression::Known(ty) => ty,
        InferredExpression::UnresolvedIdentifier { name, span } => {
            return InferredExpression::UnresolvedIdentifier { name, span };
        }
        InferredExpression::MissingProperty { .. } | InferredExpression::Unknown => {
            return InferredExpression::Unknown;
        }
    };

    // `a?.b` only widens to `| undefined` when `a` can actually be nullish. After
    // a guard proves it is not (`if (!a.result) throw;` then `a.result?.mean`),
    // tsc types the access exactly as `a.b`, so an unconditional widen turned
    // guarded arithmetic into TS2362/TS2363.
    let object_is_nullish = optional_chain_can_short_circuit(&object_type);
    let base_type = surge_ts_types::remove_undefined(&object_type);

    let result_type = match base_type {
        Type::Unknown | Type::GenuineUnknown | Type::Any => {
            InferredExpression::Known(base_type.clone())
        }
        Type::Union(ref union_type) => {
            let mut result_types = Vec::new();
            let mut saw_known = false;

            for ty in union_type.types() {
                if *ty == Type::Undefined || ty.is_unknown() {
                    continue;
                }

                saw_known = true;
                match ty.get_property_access_type(property_name) {
                    Some(property_type) => result_types.push(property_type),
                    None => {
                        return InferredExpression::MissingProperty {
                            property_name: property_name.to_string(),
                            object_type: base_type.clone(),
                            span: *property_span,
                        };
                    }
                }
            }

            if !saw_known || result_types.is_empty() {
                InferredExpression::Unknown
            } else {
                InferredExpression::Known(surge_ts_types::union_type(result_types))
            }
        }
        _ => base_type
            .get_property_access_type(property_name)
            .map(InferredExpression::Known)
            .unwrap_or_else(|| InferredExpression::MissingProperty {
                property_name: property_name.to_string(),
                object_type: base_type.clone(),
                span: *property_span,
            }),
    };

    let result = match result_type {
        InferredExpression::Known(ty) if object_is_nullish => {
            InferredExpression::Known(union_type(vec![ty, Type::Undefined]))
        }
        other => other,
    };
    record_program_timing(ctx.timings.as_ref(), |timings| {
        timings.property_access_checking += property_access_start.elapsed()
    });
    result
}

pub(crate) fn infer_new_expression(
    callee: &ParsedExpression,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    if let ParsedExpression::Identifier { name, .. } = callee
        && let Some(result_type) = surge_ts_types::Type::builtin_constructor_result_type(name)
    {
        return InferredExpression::Known(result_type);
    }

    match infer_expression(callee, symbols, ctx) {
        InferredExpression::Known(Type::Function(function_type)) => {
            InferredExpression::Known(function_type.return_type().clone())
        }
        InferredExpression::Known(Type::Object(object))
            if object.construct_signature().is_some() =>
        {
            InferredExpression::Known(
                object
                    .construct_signature()
                    .expect("construct signature present")
                    .return_type()
                    .clone(),
            )
        }
        InferredExpression::Known(Type::Any) => InferredExpression::Known(Type::Any),
        InferredExpression::UnresolvedIdentifier { name, span } => {
            InferredExpression::UnresolvedIdentifier { name, span }
        }
        _ => InferredExpression::Unknown,
    }
}

/// Whether an optional chain over a receiver of this type can short-circuit, and
/// so contributes `undefined` to the access's type. The degradation sentinel and
/// `any` are left alone: their result is already the sentinel/`any`.
fn optional_chain_can_short_circuit(object_type: &Type) -> bool {
    match object_type {
        Type::Undefined | Type::Void | Type::GenuineUnknown => true,
        Type::Union(union) => union
            .types()
            .iter()
            .any(|member| matches!(member, Type::Undefined | Type::Void)),
        _ => false,
    }
}
