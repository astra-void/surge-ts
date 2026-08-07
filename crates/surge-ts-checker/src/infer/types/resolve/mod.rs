//! Core ParsedType -> Type resolution (tuples, functions, objects, unions, named, mapped).

mod conditional;
mod indexed_access;
mod intersection;
mod mapped;
mod named;
mod structural;
mod substitution;
mod template;

pub(crate) use conditional::*;
pub(crate) use intersection::*;
pub(crate) use mapped::*;
pub(crate) use named::*;
pub(crate) use structural::*;
pub(crate) use substitution::*;
pub(crate) use template::*;

use super::*;

use indexed_access::resolve_indexed_access_type;

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::ParsedType;
use surge_ts_types::{NumberLiteralType, Type, TypeCopyReason, union_type, with_type_copy_reason};

use crate::context::{CheckerContext, DeclarationResolutionKey, convert_span};

thread_local! {
    static TYPE_EXPANSION_DEPTH: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    static TYPE_EXPANSION_STEPS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Per-root ceiling on distributive-conditional members and mapped-type keys
/// processed, the checker's analogue of tsc's TS2589 instantiation limit.
/// Nested distribution multiplies (|A|·|B|·… per nesting level) and inline
/// branch ASTs never re-enter a named-declaration frame, so the `resolving`
/// cycle stack cannot see the blowup; this budget is what bounds it. Well-formed
/// code stays orders of magnitude below the limit (a 10k-key mapped type
/// consumes 10k steps).
const TYPE_EXPANSION_STEP_LIMIT: u64 = 500_000;

/// Marks a conditional/mapped expansion on the stack. The step counter resets
/// when the outermost scope begins, so the budget covers one entire (possibly
/// multiplicative) expansion tree while independent sibling expansions each get
/// a fresh budget.
pub(crate) struct TypeExpansionScope;

impl TypeExpansionScope {
    pub(crate) fn enter() -> Self {
        TYPE_EXPANSION_DEPTH.with(|depth| {
            if depth.get() == 0 {
                TYPE_EXPANSION_STEPS.with(|steps| steps.set(0));
            }
            depth.set(depth.get() + 1);
        });
        Self
    }
}

impl Drop for TypeExpansionScope {
    fn drop(&mut self) {
        TYPE_EXPANSION_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
    }
}

/// Returns false once the current expansion tree has exhausted its budget; the
/// caller degrades to `Type::Unknown` (the checker's "cannot model" sentinel,
/// which assignability treats leniently) instead of expanding further.
pub(crate) fn try_consume_type_expansion_step() -> bool {
    TYPE_EXPANSION_STEPS.with(|steps| {
        let next = steps.get().saturating_add(1);
        steps.set(next);
        next <= TYPE_EXPANSION_STEP_LIMIT
    })
}

pub(crate) fn resolve_parsed_type(
    parsed_type: ParsedType,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    match parsed_type {
        ParsedType::String => ResolvedType {
            ty: Type::String,
            had_error: false,
        },
        ParsedType::Number => ResolvedType {
            ty: Type::Number,
            had_error: false,
        },
        ParsedType::Boolean => ResolvedType {
            ty: Type::Boolean,
            had_error: false,
        },
        ParsedType::BigInt => ResolvedType {
            ty: Type::BigInt,
            had_error: false,
        },
        ParsedType::Symbol => ResolvedType {
            ty: Type::Symbol,
            had_error: false,
        },
        ParsedType::Undefined => ResolvedType {
            ty: Type::Undefined,
            had_error: false,
        },
        ParsedType::Any => ResolvedType {
            ty: Type::Any,
            had_error: false,
        },
        ParsedType::Unknown => ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        },
        ParsedType::UnknownKeyword => ResolvedType {
            ty: Type::GenuineUnknown,
            had_error: false,
        },
        ParsedType::Never => ResolvedType {
            ty: Type::Never,
            had_error: false,
        },
        ParsedType::StringLiteral(value) => ResolvedType {
            ty: Type::StringLiteral(value),
            had_error: false,
        },
        ParsedType::NumberLiteral(value) => ResolvedType {
            ty: Type::NumberLiteral(NumberLiteralType { value }),
            had_error: false,
        },
        ParsedType::BooleanLiteral(value) => ResolvedType {
            ty: Type::BooleanLiteral(value),
            had_error: false,
        },
        ParsedType::Void => ResolvedType {
            ty: Type::Void,
            had_error: false,
        },
        ParsedType::Object(object_type) => {
            resolve_object_type(object_type, ctx, resolving, substitution)
        }
        ParsedType::Array(element_type) => {
            let resolved_element = resolve_parsed_type(
                std::sync::Arc::unwrap_or_clone(element_type),
                ctx,
                resolving,
                substitution,
            );
            ResolvedType {
                ty: Type::Array(Box::new(resolved_element.ty)),
                had_error: resolved_element.had_error,
            }
        }
        ParsedType::Tuple(elements) => resolve_tuple_type(
            std::sync::Arc::unwrap_or_clone(elements),
            ctx,
            resolving,
            substitution,
        ),
        ParsedType::Union(types) => resolve_union_type(
            std::sync::Arc::unwrap_or_clone(types),
            ctx,
            resolving,
            substitution,
        ),
        ParsedType::Intersection(types) => resolve_intersection_type(
            std::sync::Arc::unwrap_or_clone(types),
            ctx,
            resolving,
            substitution,
        ),
        ParsedType::Function(function_type) => {
            resolve_function_type(function_type, ctx, resolving, substitution)
        }
        ParsedType::Named(named_type) => {
            resolve_named_type(named_type, ctx, resolving, substitution)
        }
        ParsedType::TypeOf(type_of) => {
            // `typeof X` references a value. During type-declaration resolution the
            // file's imported value bindings may not yet be in `ctx.symbols`, so on
            // a miss consult the module's full value table (the same forward-ref
            // fallback used when checking expressions); genuinely-missing names
            // still report TS2304.
            let symbol = ctx
                .symbols
                .get(&type_of.name)
                .cloned()
                .or_else(|| ctx.ambient_global_symbols.get(&type_of.name).cloned())
                .or_else(|| {
                    ctx.module_value_fallback
                        .as_ref()
                        .and_then(|table| table.get(&type_of.name).cloned())
                })
                .or_else(|| {
                    // `typeof X` inside an imported declaration's body is resolved
                    // under the declaring file's name (set by `with_file_name`), but
                    // the consumer's value `symbols`/`module_value_fallback` do not
                    // hold that module's locals. Consult the declaring module's own
                    // value table so a cross-module `Alias<typeof localConst>`
                    // resolves instead of falsely reporting TS2304.
                    let file_name = ctx.file_name.clone();
                    ctx.module_local_values_for_file(&file_name)
                        .and_then(|table| table.get(&type_of.name).cloned())
                });

            let Some(symbol) = symbol else {
                // `globalThis` is always a valid built-in, but its value symbol is
                // installed only after every ambient global is collected, so an
                // ambient declaration naming it (`declare var window: Window & typeof
                // globalThis`) resolves it first. Treat the miss as a clean `unknown`
                // (a false TS2304 / `had_error` would otherwise poison the enclosing
                // intersection); the `T & unknown ⇒ T` simplification then keeps
                // `window`/`self` as `Window`.
                if type_of.name == "globalThis" {
                    return ResolvedType {
                        ty: Type::Unknown,
                        had_error: false,
                    };
                }
                let mut diagnostic = Diagnostic::ts2304(&type_of.name, ctx.file_name.clone());
                if let Some(span) = type_of.name_span {
                    diagnostic = diagnostic.with_span(convert_span(span));
                }
                ctx.push(diagnostic);

                return ResolvedType {
                    ty: Type::Unknown,
                    had_error: true,
                };
            };

            // `typeof NS.Root` walks the dotted member path off the base symbol's
            // type. Any segment we cannot model (a non-object base, or a missing
            // property on a namespace whose shape we don't fully reconstruct)
            // degrades to `Unknown` silently rather than emitting a false
            // positive, since the base name itself was resolved.
            let mut ty = symbol.ty;
            for member in &type_of.members {
                match ty.get_property_access_type(member) {
                    Some(member_ty) => ty = member_ty,
                    None => {
                        return ResolvedType {
                            ty: Type::Unknown,
                            had_error: false,
                        };
                    }
                }
            }

            ResolvedType {
                ty,
                had_error: false,
            }
        }
        ParsedType::KeyOf(inner) => {
            let resolved_inner = resolve_parsed_type(
                std::sync::Arc::unwrap_or_clone(inner),
                ctx,
                resolving,
                substitution,
            );
            let mut keys = Vec::new();
            // Peel a nominal reference (`keyof User`) to read the named type's keys.
            match &resolved_inner.ty.peeled() {
                Type::Object(object_type) => {
                    for key in object_type.properties.keys() {
                        keys.push(Type::StringLiteral(key.to_string()));
                    }
                }
                _ => {
                    return ResolvedType {
                        ty: Type::Unknown,
                        had_error: false,
                    };
                }
            }

            ResolvedType {
                ty: if keys.is_empty() {
                    Type::Unknown
                } else if keys.len() == 1 {
                    keys.into_iter().next().unwrap()
                } else {
                    union_type(keys)
                },
                had_error: false,
            }
        }
        ParsedType::Mapped(mapped) => resolve_mapped_type(mapped, ctx, resolving, substitution),
        ParsedType::IndexedAccess(indexed_access) => {
            resolve_indexed_access_type(indexed_access, ctx, resolving, substitution)
        }
        ParsedType::Conditional(conditional) => {
            resolve_conditional_type(conditional, ctx, resolving, substitution)
        }
        ParsedType::TemplateLiteral(template) => {
            resolve_template_literal_type(template, ctx, resolving, substitution)
        }
        // An `infer X` capture resolves to a permissive `any`: with no real
        // inference, the enclosing `extends` pattern (e.g. `Ctor<infer P>`) stays a
        // concrete shape so a non-matching check type correctly falls through to
        // the conditional's false branch, rather than collapsing to `unknown`
        // (which `is_assignable_to` would treat as matching). See
        // `resolve_conditional_type`.
        ParsedType::Infer(_) => ResolvedType {
            ty: Type::Any,
            had_error: false,
        },
        // A predicate annotation types the function's return value: `boolean`
        // for `x is T`, `void` for an assertion signature. The predicate payload
        // itself is consumed by guard narrowing, not by type resolution.
        ParsedType::Predicate(predicate) => ResolvedType {
            ty: if predicate.asserts {
                Type::Void
            } else {
                Type::Boolean
            },
            had_error: false,
        },
    }
}

pub(crate) fn resolve_parsed_type_with_substitution(
    parsed_type: ParsedType,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    with_type_copy_reason(TypeCopyReason::SubstitutionChanged, || {
        resolve_parsed_type(parsed_type, ctx, resolving, substitution)
    })
}
