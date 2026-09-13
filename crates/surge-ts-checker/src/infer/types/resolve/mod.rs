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
use std::sync::Arc;

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

thread_local! {
    /// `typeof import("spec")` queries on the current resolution stack. A
    /// module whose namespace member is itself annotated through the module
    /// (`export declare const x: typeof import("./self").y`) re-enters the
    /// query while its namespace is being read; the re-entry answers the
    /// sentinel instead of recursing.
    static IMPORT_TYPE_QUERIES_IN_PROGRESS: std::cell::RefCell<Vec<(String, String)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

fn resolve_import_type_query(
    specifier: &str,
    members: &[String],
    ctx: &mut CheckerContext,
) -> ResolvedType {
    let unknown = ResolvedType {
        ty: Type::Unknown,
        had_error: false,
    };
    let file_name = ctx.file_name.clone();
    let key = (file_name.clone(), specifier.to_string());
    let re_entered = IMPORT_TYPE_QUERIES_IN_PROGRESS.with(|stack| {
        let mut stack = stack.borrow_mut();
        if stack.iter().any(|entry| *entry == key) || stack.len() > 16 {
            return true;
        }
        stack.push(key.clone());
        false
    });
    if re_entered {
        return unknown;
    }
    struct Pop;
    impl Drop for Pop {
        fn drop(&mut self) {
            IMPORT_TYPE_QUERIES_IN_PROGRESS.with(|stack| {
                stack.borrow_mut().pop();
            });
        }
    }
    let _pop = Pop;
    let canonical = ctx.canonical_file_name_arc();
    let found = ctx
        .import_type_namespace(&file_name, specifier)
        .or_else(|| ctx.import_type_namespace(&canonical, specifier));
    let Some(mut ty) = found else {
        return unknown;
    };
    for member in members {
        match ty.get_property_access_type(member) {
            Some(member_ty) => ty = member_ty,
            None => return unknown,
        }
    }
    ResolvedType {
        ty,
        had_error: false,
    }
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
            resolve_object_type(object_type.as_ref(), ctx, resolving, substitution)
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
            if let Some(specifier) = &type_of.import_specifier {
                return resolve_import_type_query(specifier, &type_of.members, ctx);
            }
            // A type query reads the value, so a UMD-global name reports here the
            // same way it would in an expression.
            let umd_global = crate::checks::emit_umd_global_reference_diagnostic(
                &type_of.name,
                type_of.name_span,
                ctx,
            );

            // `typeof X` references a value. During type-declaration resolution the
            // file's imported value bindings may not yet be in `ctx.symbols`, so on
            // a miss consult the module's full value table (the same forward-ref
            // fallback used when checking expressions); genuinely-missing names
            // still report TS2304.
            let symbol = ctx
                .symbols
                .get(&type_of.name)
                .cloned()
                .or_else(|| {
                    ctx.signature_parameter_bindings
                        .iter()
                        .rev()
                        .find(|(name, _)| name == &type_of.name)
                        .map(|(_, ty)| crate::symbols::SymbolInfo {
                            ty: ty.clone(),
                            kind: crate::symbols::SymbolKind::Parameter,
                            function_signature: None,
                        })
                })
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
                // A UMD global resolves for tsc even when surge has no value
                // symbol for it: the reference already reported as TS2686, so a
                // TS2304 on top would be a second diagnostic tsc never emits.
                if type_of.name == "globalThis" || umd_global {
                    return ResolvedType {
                        ty: Type::Unknown,
                        had_error: false,
                    };
                }
                // A class reached through `import type { Class }`, or named
                // inside the ambient module that declares it (`http`'s
                // `RequestListener<Request extends typeof IncomingMessage = …>`),
                // has a type declaration but no value symbol in reach; tsc still
                // resolves `typeof Class` there. Stand in a constructor surface
                // over the instance type — enough for `InstanceType<typeof C>`
                // and for a constructor-shaped constraint — kept open so a static
                // member surge cannot see is never reported. Anything else
                // degrades instead of reporting a name the file demonstrably
                // declares.
                if let Some(handle) = ctx.lookup_type_declaration_handle(&type_of.name) {
                    if type_of.members.is_empty()
                        && matches!(handle.get(), crate::symbols::TypeDeclarationInfo::Interface(_))
                    {
                        let instance = resolve_named_type(
                            std::sync::Arc::new(surge_ts_syntax::ParsedNamedType {
                                name: type_of.name.clone(),
                                span: type_of.name_span,
                                type_arguments: Vec::new(),
                            }),
                            ctx,
                            resolving,
                            substitution,
                        );
                        // A tainted expansion is left degraded: standing a
                        // constructor over a shape whose base failed to resolve
                        // was measured to make `Mock<RequestListener>` uncallable
                        // again, because the eager class body it embeds carries
                        // the sentinel into every declared function parameter.
                        if !instance.had_error && !instance.ty.is_unknown() {
                            let mut properties = surge_ts_types::PropertyMap::default();
                            properties.insert(
                                "prototype".into(),
                                surge_ts_types::ObjectProperty::required(instance.ty.clone()),
                            );
                            let constructor = surge_ts_types::FunctionType::new(
                                vec![Type::Any],
                                instance.ty,
                                true,
                                0,
                            );
                            return ResolvedType {
                                ty: Type::Object(
                                    surge_ts_types::ObjectType::new(properties, None)
                                        .with_open_index_marker()
                                        .with_construct_signature(constructor),
                                ),
                                had_error: false,
                            };
                        }
                    }
                    return ResolvedType {
                        ty: Type::Unknown,
                        had_error: true,
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
            // `typeof fn` over a generic declaration keeps the declaration on
            // the function handle: a property typed by it (`vi.fn`) is called
            // through the property path, which otherwise has only the bare
            // resolved signature and cannot bind the type parameters.
            if type_of.members.is_empty()
                && let Some(signature) = symbol.function_signature.as_ref()
                && !signature.type_parameters.is_empty()
                && !signature.overloaded
                && let Type::Function(function) = ty
            {
                let concrete: Arc<crate::symbols::FunctionSignatureInfo> = Arc::clone(signature);
                let declaration: Arc<dyn std::any::Any + Send + Sync> = concrete;
                ty = Type::Function(function.with_declaration(declaration));
            }
            // A base still standing at the degradation sentinel is a value whose
            // type is not known *yet* (a forward reference resolved during an
            // earlier pass), not a value without the member. Reporting a clean
            // `unknown` would let the enclosing declaration intern that answer and
            // pin every later consumer to it — the shape behind a discriminant
            // written as `typeof Codes.invalid_string` losing its literal type.
            if ty.is_unknown() && !type_of.members.is_empty() {
                return ResolvedType {
                    ty: Type::Unknown,
                    had_error: true,
                };
            }
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
            // `keyof {}` is `never`, not "could not model": the empty-interface
            // escape hatch (`T[keyof DO_NOT_USE_…]` in React's `Key` and
            // `ReactNode`) relies on the `never` arm vanishing from its union.
            // Only a cleanly-resolved object with no index signature is genuinely
            // keyless — a degraded resolution lands on a property-less object too,
            // and calling that `never` would turn an unmodelled type into a closed
            // empty shape (`Omit<Unmodelled, K>` ⇒ `{}`).
            let empty_object_is_never = !resolved_inner.had_error
                && matches!(
                    resolved_inner.ty.peeled(),
                    Type::Object(ref object_type)
                        if object_type.properties.is_empty()
                            && object_type.string_index_type.is_none()
                );
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
                        // A degraded operand yields a degraded key set, not a
                        // clean one. Reporting it clean is what lets an
                        // intersection simplify `keyof A & keyof <degraded>`
                        // down to `keyof A` and a conditional then answer
                        // `extends never` from a key set that never existed —
                        // the `ProtectedIntersection` collision branch fires on
                        // a router surge could not model.
                        had_error: resolved_inner.had_error,
                    };
                }
            }

            ResolvedType {
                ty: if keys.is_empty() {
                    if empty_object_is_never {
                        Type::Never
                    } else {
                        Type::Unknown
                    }
                } else if keys.len() == 1 {
                    keys.into_iter().next().unwrap()
                } else {
                    union_type(keys)
                },
                had_error: resolved_inner.had_error,
            }
        }
        ParsedType::Mapped(mapped) => resolve_mapped_type(
            std::sync::Arc::unwrap_or_clone(mapped),
            ctx,
            resolving,
            substitution,
        ),
        ParsedType::IndexedAccess(indexed_access) => {
            resolve_indexed_access_type(
                std::sync::Arc::unwrap_or_clone(indexed_access),
                ctx,
                resolving,
                substitution,
            )
        }
        ParsedType::Conditional(conditional) => {
            resolve_conditional_type(
                std::sync::Arc::unwrap_or_clone(conditional),
                ctx,
                resolving,
                substitution,
            )
        }
        ParsedType::TemplateLiteral(template) => {
            resolve_template_literal_type(
                std::sync::Arc::unwrap_or_clone(template),
                ctx,
                resolving,
                substitution,
            )
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
