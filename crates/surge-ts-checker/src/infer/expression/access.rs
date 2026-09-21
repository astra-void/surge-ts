//! Property access, index access, calls, and their optional-chaining variants.

use super::*;

use std::time::Instant;

use surge_ts_syntax::{ParsedExpression, TextSpan};
use surge_ts_types::{Type, is_assignable_to, union_type};

use crate::context::CheckerContext;
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
    if let Some(narrowed) = narrowed_element_read_named(object_name, index, symbols) {
        return InferredExpression::Known(narrowed);
    }
    let Some(symbol) = symbols.get(object_name) else {
        return InferredExpression::UnresolvedIdentifier {
            name: object_name.to_string(),
            span: *object_span,
        };
    };

    let receiver_type = match &symbol.ty {
        Type::Reference(_) => match symbol.ty.peeled() {
            peeled @ (Type::Array(_) | Type::Tuple(_)) => peeled,
            _ => symbol.ty.clone(),
        },
        other => other.clone(),
    };
    if let Type::OpenTuple(tuple) = &receiver_type {
        return infer_open_tuple_index_access(tuple, index, index_span, symbols, ctx);
    }
    match &receiver_type {
        Type::Any => InferredExpression::Known(Type::Any),
        Type::Unknown
        | Type::GenuineUnknown
        | Type::ErrorType
        | Type::TypeParameter(_) => InferredExpression::Unknown,
        // Lowered to its element array above.
        Type::OpenTuple(_) => InferredExpression::Unknown,
        Type::Union(union_type) => {
            let mut result_types = vec![];
            for ty in union_type.types() {
                if matches!(ty, Type::Undefined | Type::Null) {
                    result_types.push(Type::Undefined);
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
                        result_types.push(unchecked_index_read(element_type.clone(), ctx));
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

            InferredExpression::Known(unchecked_index_read(element_type.clone(), ctx))
        }
        Type::Function(_)
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
        | Type::Undefined
        | Type::Null => InferredExpression::Unknown,
        Type::Object(_) | Type::Reference(_) => {
            infer_object_element_read(&receiver_type, index, symbols, ctx)
        }
    }
}

fn infer_object_element_read(
    receiver_type: &Type,
    index: &ParsedExpression,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let InferredExpression::Known(index_type) = infer_expression(index, symbols, ctx) else {
        return InferredExpression::Unknown;
    };
    match crate::checks::expr::object_element_read(receiver_type, index, &index_type, symbols, ctx) {
        Some(ty) => InferredExpression::Known(ty),
        None => InferredExpression::Unknown,
    }
}

/// An open tuple (`[number, string, ...boolean[]]`) has fixed positions in
/// front of its rest: `t[0]` is `number`, and anything at or past the rest
/// reads as what the rest and the trailing elements hold.
pub(crate) fn infer_open_tuple_index_access(
    tuple: &surge_ts_types::OpenTupleType,
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

    let past_fixed = || {
        let mut members = vec![tuple.rest.as_ref().clone()];
        members.extend(tuple.trailing.iter().cloned());
        union_type(members)
    };
    if let Some(index_value) = tuple_index_value(&index_type) {
        return InferredExpression::Known(
            tuple
                .leading
                .get(index_value)
                .cloned()
                .unwrap_or_else(past_fixed),
        );
    }

    if is_assignable_to(&index_type, &Type::Number) {
        return InferredExpression::Known(unchecked_index_read(tuple.element_union(), ctx));
    }

    let _ = index_span;
    InferredExpression::Unknown
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
        return InferredExpression::Known(unchecked_index_read(union_type(elements.to_vec()), ctx));
    }

    let _ = index_span;
    InferredExpression::Unknown
}

/// `E.A` read off an enum's object is the enum member type `E.A`, not the
/// bare value it holds, so it keeps the enum's nominal identity: `F.X` is not
/// an `E` even when both are `0`. Only a member whose `E.A` alias resolves to
/// an enum reference is answered here; anything else falls back to the object.
fn enum_member_value_type(
    object: &ParsedExpression,
    property_name: &str,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let ParsedExpression::Identifier { name, .. } = object else {
        return None;
    };
    let crate::symbols::TypeDeclarationInfo::Alias(alias) = ctx.lookup_type_declaration(name)? else {
        return None;
    };
    if alias.enum_name.as_deref() != Some(name.as_str()) {
        return None;
    }
    // A local value of the same name shadows the enum.
    if symbols.get(name).is_some_and(|symbol| !matches!(symbol.kind, crate::symbols::SymbolKind::Const)) {
        return None;
    }
    let member_name = format!("{name}.{property_name}");
    ctx.lookup_type_declaration(&member_name)?;
    let member = crate::infer::map_parsed_type(
        surge_ts_syntax::ParsedType::Named(std::sync::Arc::new(surge_ts_syntax::ParsedNamedType {
            name: member_name,
            span: None,
            type_arguments: Vec::new(),
        })),
        ctx,
    );
    matches!(&member, Type::Reference(reference) if reference.enum_owner.is_some()).then_some(member)
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
    if let Some(member) = enum_member_value_type(object, property_name, symbols, ctx) {
        return InferredExpression::Known(member);
    }
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
            Type::Unknown | Type::GenuineUnknown | Type::TypeParameter(_) => InferredExpression::Unknown,
            // The check pass reports the receiver; the access itself has no
            // type tsc would continue with.
            Type::Undefined | Type::Null => InferredExpression::Unknown,
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
                // `a?.b.c` keeps the chain's `undefined`; a plain `x.c` on a
                // possibly-`undefined` `x` is an error the check pass reports,
                // and tsc then types the access from the defined part alone.
                let keeps_undefined = object.continues_optional_chain();
                let mut result_types = vec![];
                for ty in union_type.types() {
                    // An optional chain short-circuits a `null` receiver to
                    // `undefined` too.
                    if matches!(ty, Type::Undefined | Type::Null) {
                        if keeps_undefined {
                            result_types.push(Type::Undefined);
                        }
                        continue;
                    }
                    match ty.get_property_access_type(property_name) {
                        Some(property_type) => result_types.push(
                            widen_index_signature_read(ty, property_name, property_type, ctx),
                        ),
                        None if no_lib_array_member(ty, ctx) => result_types.push(Type::Any),
                        // Same rule as the single-receiver arm below: a member
                        // whose reference peels to the sentinel is a shape surge
                        // could not reconstruct, not a type without the member.
                        None if ty.peeled().is_unknown()
                            || crate::checks::expr::carries_leaked_type_parameter(ty, ctx) =>
                        {
                            return InferredExpression::Unknown;
                        }
                        // tsc names the union the property was looked up on,
                        // not the member that happens to lack it.
                        None => {
                            return InferredExpression::MissingProperty {
                                property_name: property_name.to_string(),
                                object_type: object_type.clone(),
                                span: *property_span,
                            };
                        }
                    }
                }
                InferredExpression::Known(surge_ts_types::union_type(result_types))
            }
            _ => (!lib_lacks_builtin_member(&object_type, property_name, ctx))
                .then(|| object_type.get_property_access_type(property_name))
                .flatten()
                .or_else(|| lib_builtin_member_type(&object_type, property_name, ctx))
                .map(|property_type| {
                    InferredExpression::Known(widen_index_signature_read(
                        &object_type,
                        property_name,
                        property_type,
                        ctx,
                    ))
                })
                .unwrap_or_else(|| {
                    if no_lib_array_member(&object_type, ctx) {
                        InferredExpression::Known(Type::Any)
                    // A nominal reference that peels to the sentinel is a shape
                    // surge could not reconstruct (a cross-module `Set<string>`
                    // annotation whose lazy environment is gone), not a type
                    // without the member.
                    } else if object_type.peeled().is_unknown()
                        || crate::checks::expr::carries_leaked_type_parameter(&object_type, ctx)
                    {
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

/// A guard on this exact element access (`if (xs[i])`) recorded a narrowed
/// read under the access's rendered key; see
/// `narrow_element_reference_guards_in_scope`.
pub(crate) fn narrowed_element_read(
    object: &ParsedExpression,
    index: &ParsedExpression,
    symbols: &SymbolTable,
) -> Option<Type> {
    let key = crate::checks::function::element_reference_key(object, index)?;
    symbols.get(&key).map(|symbol| symbol.ty.clone())
}

pub(crate) fn narrowed_element_read_named(
    object_name: &str,
    index: &ParsedExpression,
    symbols: &SymbolTable,
) -> Option<Type> {
    let key =
        crate::checks::function::element_reference_key_named(object_name, &[], index)?;
    symbols.get(&key).map(|symbol| symbol.ty.clone())
}

/// Under `noUncheckedIndexedAccess` a member that exists only through a string
/// index signature reads as `T | undefined`, like the element access it is.
fn widen_index_signature_read(
    receiver: &Type,
    property_name: &str,
    property_type: Type,
    ctx: &CheckerContext,
) -> Type {
    if ctx.options.no_unchecked_indexed_access
        && receiver.reads_unnarrowed_string_index(property_name)
    {
        unchecked_index_read(property_type, ctx)
    } else {
        property_type
    }
}

/// Under `noUncheckedIndexedAccess` an array or index-signature element read is
/// `T | undefined`; a tuple element at a literal index is exact and never
/// widened.
pub(crate) fn unchecked_index_read(element_type: Type, ctx: &CheckerContext) -> Type {
    // `any`/`unknown` absorb `undefined`; the degradation sentinel must stay
    // the sentinel rather than become a union the checks would read as typed.
    if !ctx.options.no_unchecked_indexed_access
        || matches!(
            element_type,
            Type::Any | Type::GenuineUnknown | Type::Unknown | Type::TypeParameter(_)
        )
    {
        element_type
    } else {
        union_type(vec![element_type, Type::Undefined])
    }
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
        Type::Any | Type::Unknown | Type::GenuineUnknown | Type::TypeParameter(_)
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

    // `Promise.all` / `Promise.resolve` are typed by their own rules on the
    // checked path; the inferred path — a destructuring initializer takes it —
    // has to answer the same type, without reporting anything itself.
    if (property_name == "all"
        || (property_name == "resolve" && crate::checks::call::promise_nominal_enabled()))
        && type_arguments.is_empty()
        && !arguments.iter().any(|argument| argument.spread)
        && crate::checks::call::is_promise_all_receiver(&object_type)
    {
        let reported = ctx.diagnostics().len();
        let result = if property_name == "all" {
            (!arguments.is_empty())
                .then(|| crate::checks::call::check_promise_all_call(arguments, None, None, symbols, ctx))
                .flatten()
        } else if arguments.len() <= 1 {
            crate::checks::call::check_promise_resolve_call(arguments, None, symbols, ctx)
        } else {
            None
        };
        ctx.truncate_diagnostics(reported);
        if let Some(result) = result {
            return InferredExpression::Known(result);
        }
    }

    // `Promise<T>` is modelled as its awaited `T`, so a `.then`/`.catch`/`.finally`
    // chained on a promise-returning call lands on the value type, which declares
    // no such member. The checking path answers these from the chain; without the
    // same answer here the inference-only path degrades, and a generic call whose
    // argument is `() => sleep(10).then(() => 'data')` then binds no type
    // argument at all (vitest's `vi.fn`).
    if matches!(property_name, "then" | "catch" | "finally")
        && !matches!(
            object_type,
            Type::Any | Type::Unknown | Type::GenuineUnknown | Type::TypeParameter(_)
        )
        && !crate::checks::call::declares_own_property(&object_type, property_name)
    {
        let chained = if property_name == "then" {
            match arguments.first() {
                Some(callback) => match infer_expression(&callback.expression, symbols, ctx) {
                    InferredExpression::Known(Type::Function(function_type)) => {
                        crate::checks::call::promise_like_awaited_type(function_type.return_type())
                    }
                    InferredExpression::Known(ty) => {
                        crate::checks::call::promise_like_awaited_type(&ty)
                    }
                    _ => Type::Unknown,
                },
                None => Type::Unknown,
            }
        } else {
            object_type.clone()
        };
        record_program_timing(ctx.timings.as_ref(), |timings| {
            timings.property_access_checking += property_call_start.elapsed()
        });
        return InferredExpression::Known(chained);
    }

    let result = match &object_type {
        Type::Any => InferredExpression::Known(Type::Any),
        Type::Unknown | Type::GenuineUnknown | Type::TypeParameter(_) => InferredExpression::Unknown,
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
                        Type::Function(function_type) => InferredExpression::Known(
                            crate::checks::call::select_overload_return_type_for_inferred_call(
                                &function_type,
                                arguments,
                                symbols,
                                ctx,
                            )
                            .unwrap_or_else(|| function_type.return_type().clone()),
                        ),
                        _ => InferredExpression::Known(Type::Any),
                    },
                }
            }
            Some(Type::Any) => InferredExpression::Known(Type::Any),
            None if no_lib_array_member(&object_type, ctx) => InferredExpression::Known(Type::Any),
            // A member typed by a generic call signature answers its return type
            // only once the call's type arguments are bound, and a reference to a
            // callable interface has to be peeled before it looks callable at all.
            Some(Type::Function(function_type))
                if !crate::checks::call::written_call_signature_recovery_enabled() =>
            {
                InferredExpression::Known(function_type.return_type().clone())
            }
            Some(_) if !crate::checks::call::written_call_signature_recovery_enabled() => {
                InferredExpression::Unknown
            }
            Some(member_type) => {
                match crate::checks::call::callable_member_call_return_type(
                    &member_type,
                    type_arguments,
                    *property_span,
                    arguments,
                    symbols,
                    ctx,
                ) {
                    Some(return_type) => InferredExpression::Known(return_type),
                    None => InferredExpression::Unknown,
                }
            }
            None => InferredExpression::Unknown,
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
/// A member of a built-in receiver that surge's own member tables do not list
/// but the configured lib declares on the global interface behind it
/// (`copyWithin` on `Array<T>`, `anchor` on `String`). What the lib says
/// follows `target`/`lib`, which the tables cannot. A member whose type does
/// not resolve, or that is overloaded, still exists, so it reads `any` rather
/// than as missing.
/// Whether surge's own member tables offer `name` on this receiver although
/// the configured lib does not declare it (`xs.at(0)` under `target: es2015`).
/// Only a member tsc ties to a lib version is judged, and only against a lib
/// that declares the global interface at all.
pub(crate) fn lib_lacks_builtin_member(receiver: &Type, name: &str, ctx: &CheckerContext) -> bool {
    if ctx.options.no_lib
        || crate::checks::expr::lib_feature_of_missing_member(receiver, name).is_none()
    {
        return false;
    }
    // Only the receivers surge answers from its own tables; one resolved from
    // the lib (`ObjectConstructor`) already lacks what the lib lacks.
    let interface_name = match receiver {
        Type::Array(_) | Type::Tuple(_) => "Array",
        Type::String | Type::StringLiteral(_) => "String",
        _ => return false,
    };
    match ctx.lookup_type_declaration(interface_name) {
        Some(crate::symbols::TypeDeclarationInfo::Interface(info)) => {
            ctx.is_library_scoped_file(&info.file_name)
                && !info.body.members.is_empty()
                && !info.body.members.iter().any(|member| member.name == name)
        }
        _ => false,
    }
}

pub(crate) fn lib_builtin_member_type(
    receiver: &Type,
    name: &str,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let (interface_name, element) = match receiver {
        Type::Array(element) => ("Array", Some(element.as_ref().clone())),
        Type::Tuple(elements) => ("Array", Some(surge_ts_types::union_type(elements.clone()))),
        Type::String | Type::StringLiteral(_) => ("String", None),
        Type::Number | Type::NumberLiteral(_) => ("Number", None),
        Type::Boolean | Type::BooleanLiteral(_) => ("Boolean", None),
        Type::BigInt => ("BigInt", None),
        Type::Function(_) => ("Function", None),
        _ => return None,
    };
    let (member_type, optional, scope) = match ctx.lookup_type_declaration(interface_name)? {
        crate::symbols::TypeDeclarationInfo::Interface(info) => {
            let mut declared = info.body.members.iter().filter(|member| member.name == name);
            let member = declared.next()?;
            // An overloaded member is not one signature; it exists, and that is
            // all this answers for it.
            if declared.next().is_some() {
                return Some(Type::Any);
            }
            (member.ty.clone(), member.optional, info.resolution_scope.clone())
        }
        crate::symbols::TypeDeclarationInfo::Alias(_) => return None,
    };
    let mut substitution = crate::infer::TypeParameterSubstitution::new();
    if let Some(element) = element {
        substitution.set("T".to_string(), element, false);
    }
    let diagnostics_before = ctx.diagnostics().len();
    let resolved = crate::infer::with_type_declaration_scope(&scope, ctx, |ctx| {
        crate::infer::types::resolve_parsed_type(member_type, ctx, &mut Vec::new(), &substitution)
    });
    ctx.truncate_diagnostics(diagnostics_before);
    let had_error = resolved.had_error();
    let ty = resolved.into_ty();
    Some(if had_error || ty.is_unknown() {
        Type::Any
    } else if optional {
        surge_ts_types::union_type(vec![ty, Type::Undefined])
    } else {
        ty
    })
}

fn no_lib_array_member(object_type: &Type, ctx: &CheckerContext) -> bool {
    ctx.options.no_lib && matches!(object_type, Type::Array(_))
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
    if let Some(narrowed) = narrowed_element_read(object, index, symbols) {
        return InferredExpression::Known(narrowed);
    }
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
                    InferredExpression::Known(unchecked_index_read((**element_type).clone(), ctx))
                }
                _ => InferredExpression::Unknown,
            }
        }
        Type::Object(_) | Type::Reference(_) => {
            infer_object_element_read(&object_type, index, symbols, ctx)
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

    let base_type = surge_ts_types::remove_nullish(&object_type);

    match &base_type {
        Type::Any => InferredExpression::Known(Type::Any),
        Type::Unknown | Type::GenuineUnknown | Type::TypeParameter(_) => InferredExpression::Unknown,
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
    let base_type = surge_ts_types::remove_nullish(&object_type);

    let result_type = match base_type {
        Type::Unknown | Type::GenuineUnknown | Type::TypeParameter(_) | Type::Any => {
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
                    None if no_lib_array_member(ty, ctx) => result_types.push(Type::Any),
                    // Same rule the non-optional access applies: a receiver that
                    // peels to the sentinel, or one still carrying a leaked type
                    // parameter, is a shape surge could not reconstruct, not a
                    // type without the member. Without it `a?.b` reported a
                    // member that `a!.b` resolved.
                    None if ty.peeled().is_unknown()
                        || crate::checks::expr::carries_leaked_type_parameter(ty, ctx) =>
                    {
                        return InferredExpression::Unknown;
                    }
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
            .unwrap_or_else(|| {
                if no_lib_array_member(&base_type, ctx) {
                    InferredExpression::Known(Type::Any)
                } else if base_type.peeled().is_unknown()
                    || crate::checks::expr::carries_leaked_type_parameter(&base_type, ctx)
                {
                    InferredExpression::Unknown
                } else {
                    InferredExpression::MissingProperty {
                        property_name: property_name.to_string(),
                        object_type: base_type.clone(),
                        span: *property_span,
                    }
                }
            }),
    };

    let result = match result_type {
        // `unknown` and `any` absorb the chain's `undefined`, as every union
        // with them does.
        InferredExpression::Known(ty @ (Type::GenuineUnknown | Type::Any)) => {
            InferredExpression::Known(ty)
        }
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
        Type::Undefined | Type::Null | Type::Void | Type::GenuineUnknown => true,
        Type::Union(union) => union
            .types()
            .iter()
            .any(|member| matches!(member, Type::Undefined | Type::Null | Type::Void)),
        _ => false,
    }
}
