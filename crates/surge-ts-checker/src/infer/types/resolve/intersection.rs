use super::*;

use surge_ts_types::{ObjectType, PropertyMap};

use crate::arena::alloc_object_type;

/// Reference-only intersections remain nominal during declaration indexing.
/// Without this companion to dependency-alias deferral, constructing
/// `ComponentProps<...> & RefAttributes<...>` immediately peels both deferred
/// operands and recreates the eager expansion the references were meant to
/// avoid. The escape hatch is for paired before/after profiling only.
fn defer_reference_intersections() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED
        .get_or_init(|| std::env::var("SURGE_EAGER_REFERENCE_INTERSECTIONS").as_deref() != Ok("1"))
}

/// Resolves an intersection `A & B`. Object-like operands are merged into a
/// single object exposing every member's property surface, which lets the
/// existing object machinery (property access, assignability, object-literal
/// checking) handle intersections without a dedicated runtime type. The merged
/// object is tagged via [`with_intersection_marker`] so a missing required
/// property surfaces the outer assignability code tsc reports for intersections.
///
/// Simplification follows the existing `any`/`unknown` policy: `T & any` is
/// `any`, `T & unknown` is `T`. Conflicting properties keep the left operand
/// (full `string & number -> never` reduction is a non-goal). If any operand is
/// unresolved the whole intersection degrades to `Unknown` after the root
/// diagnostic is reported, so reads stay conservative and never cascade.
pub(crate) fn resolve_intersection_type(
    types: Vec<ParsedType>,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let mut resolved_types = Vec::new();
    let mut had_error = false;

    for ty in types {
        let resolved = resolve_parsed_type(ty, ctx, resolving, substitution);
        had_error |= resolved.had_error;
        resolved_types.push(resolved.ty);
    }

    // One failed operand must not erase the others: `ComponentProps<"button"> &
    // VariantProps<…>` with an unmodelled second operand still has a fully usable
    // first operand, and collapsing the whole intersection to `unknown` is what
    // strips contextual typing from every prop that flows through it. Merge the
    // usable members (the merge already drops `unknown` operands) but keep
    // `had_error` — the taint still gates every cache/bail exactly as before, so
    // no degraded shape is interned or re-expanded.
    if had_error {
        if resolved_types.iter().all(Type::is_unknown) {
            return ResolvedType {
                ty: Type::Unknown,
                had_error: true,
            };
        }
        return ResolvedType {
            ty: merge_intersection_members(resolved_types),
            had_error: true,
        };
    }

    ResolvedType {
        ty: merge_intersection_members(resolved_types),
        had_error: false,
    }
}

/// Arity ceiling for distributing a union operand across an intersection. Each
/// arm rebuilds the whole merged property map, so the bound keeps the worst case
/// at 8x a shape that is rare in practice; wider unions keep the single merge.
const MAX_DISTRIBUTED_UNION_ARITY: usize = 8;

fn merge_intersection_members(members: Vec<Type>) -> Type {
    if members.iter().any(|ty| matches!(ty, Type::Any)) {
        return Type::Any;
    }

    // A dropped `Type::Unknown` operand is surge's degradation sentinel for an
    // operand it could not model (`ComponentProps<typeof UnmodelledValue> & {…}`),
    // NOT the `unknown` keyword (`GenuineUnknown`). The failed operand may have
    // contributed members we never saw, so a surviving *inline object* surface
    // must stay OPEN — a closed merge would flag every dropped member's use as an
    // excess property. A surviving nominal reference is returned untouched (see
    // the lone-survivor comment below).
    let dropped_unmodelled_operand = members.iter().any(|ty| matches!(ty, Type::Unknown));
    let open_if_unmodelled = |ty: Type| -> Type {
        match ty {
            Type::Object(object)
                if dropped_unmodelled_operand && object.string_index_type.is_none() =>
            {
                let mut object = object.with_open_index_marker();
                object.string_index_type = Some(std::sync::Arc::new(Type::Any));
                Type::Object(object)
            }
            other => other,
        }
    };

    let members: Vec<Type> = members.into_iter().filter(|ty| !ty.is_unknown()).collect();

    // `T & unknown ⇒ T`: with the `unknown` operands dropped, a lone survivor is
    // returned unchanged. Peeling and re-merging it (below) would force a lazy
    // library reference's bounded structural expansion and discard its nominal
    // identity — e.g. `Window & typeof globalThis` would otherwise corrupt the
    // shared `Window` apparent type.
    if members.len() == 1 {
        return open_if_unmodelled(members.into_iter().next().unwrap());
    }

    // `(A | B) & C` is `(A & C) | (B & C)`. The object merge below only reads
    // `Type::Object` operands, so an undistributed union operand contributes
    // nothing: the merged surface keeps only `C`'s properties and every use of an
    // `A`/`B` member is reported as an excess property. Distribution rebuilds the
    // merged property map once per arm, so it is bounded — a wider union, or a
    // second union operand (whose product is quadratic), instead falls through to
    // the single merge marked open, which suppresses the excess-property report
    // for the names surge did not enumerate.
    let mut union_operands = members
        .iter()
        .enumerate()
        .filter_map(|(index, ty)| match ty {
            Type::Union(union) => Some((index, union.types())),
            _ => None,
        });
    let lone_union_operand = match (union_operands.next(), union_operands.next()) {
        (Some(lone), None) => Some(lone),
        _ => None,
    };
    let unenumerated_union_operand = members.iter().any(|ty| matches!(ty, Type::Union(_)));
    if let Some((index, arms)) = lone_union_operand
        && !arms.is_empty()
        && arms.len() <= MAX_DISTRIBUTED_UNION_ARITY
    {
        let arms = arms.to_vec();
        let operand_names: Vec<String> = members.iter().map(Type::name).collect();
        let distributed: Vec<Type> = arms
            .into_iter()
            .map(|arm| {
                let mut operands = members.clone();
                let mut names = operand_names.clone();
                names[index] = arm.name();
                operands[index] = arm;
                merge_intersection_members_now(
                    operands,
                    Some(names.join(" & ")),
                    dropped_unmodelled_operand,
                )
            })
            .collect();
        return surge_ts_types::union_type(distributed);
    }

    let display_name = (!members.is_empty()).then(|| {
        members
            .iter()
            .map(Type::name)
            .collect::<Vec<_>>()
            .join(" & ")
    });

    // An intersection whose operands are all lazy/nominal references
    // (`CheckboxProps & RefAttributes<HTMLButtonElement>`) defers its merge:
    // peeling the operands here forces each library reference's structural
    // expansion at *resolution* time, which is what pulls the React/DOM graph
    // while dependency `.d.ts` export tables are being collected. The merge
    // runs instead when a consumer peels the intersection reference. Operands
    // that already carry structure (inline objects, primitives) keep the eager
    // merge so non-reference intersections are unchanged.
    if defer_reference_intersections()
        && members.len() > 1
        && members.iter().all(|ty| matches!(ty, Type::Reference(_)))
    {
        crate::program::record_program_counter(|c| c.lazy_intersection_create_count += 1);
        let display = display_name.unwrap_or_default();
        // Identity from the operands' module-qualified reference ids, not the
        // display form: same-named types from different modules must not
        // collapse into one nominal intersection.
        let id = members
            .iter()
            .filter_map(|ty| match ty {
                Type::Reference(reference) => Some(reference.id.as_ref()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\u{0}&\u{0}");
        // The operands double as the reference arguments so nominal identity
        // (`same_reference`: id + arguments) distinguishes `A & Ref<X>` from
        // `A & Ref<Y>` — the operand ids alone erase the instantiation.
        return Type::Reference(surge_ts_types::TypeReference::new(
            format!("\u{0}intersection\u{0}{id}"),
            display,
            members.clone(),
            std::sync::Arc::new(LazyIntersectionMerge {
                members,
                dropped_unmodelled_operand,
                memo: std::sync::OnceLock::new(),
            }),
        ));
    }

    merge_intersection_members_now(
        members,
        display_name,
        dropped_unmodelled_operand || unenumerated_union_operand,
    )
}

/// Resolver for a deferred all-reference intersection: the member peel + merge
/// runs on first consumer peel instead of at resolution time.
struct LazyIntersectionMerge {
    members: Vec<Type>,
    dropped_unmodelled_operand: bool,
    memo: std::sync::OnceLock<std::sync::Arc<Type>>,
}

impl surge_ts_types::ResolveReference for LazyIntersectionMerge {
    fn resolve(&self) -> Type {
        (*self.resolve_arc()).clone()
    }

    fn resolve_arc(&self) -> std::sync::Arc<Type> {
        self.memo
            .get_or_init(|| {
                crate::program::record_program_counter(|c| c.lazy_intersection_peel_count += 1);
                let display_name = (!self.members.is_empty()).then(|| {
                    self.members
                        .iter()
                        .map(Type::name)
                        .collect::<Vec<_>>()
                        .join(" & ")
                });
                std::sync::Arc::new(crate::program::with_dts_expansion_reason(
                    crate::program::DtsExpansionReason::IntersectionMerge,
                    || {
                        merge_intersection_members_now(
                            self.members.clone(),
                            display_name,
                            self.dropped_unmodelled_operand,
                        )
                    },
                ))
            })
            .clone()
    }
}

fn merge_intersection_members_now(
    members: Vec<Type>,
    display_name: Option<String>,
    dropped_unmodelled_operand: bool,
) -> Type {
    // Peel reference operands (`StudentBulkImportRow & { … }`) so a named object
    // member contributes its properties to the merged intersection surface.
    let members: Vec<Type> = members.iter().map(Type::peeled).collect();

    // A union operand the caller could not distribute contributes no properties
    // to the merge below, so the result must stay OPEN or every use of one of its
    // members reads as an excess property. The caller's check runs *before* this
    // peel, so it misses a union hidden behind a nominal reference — zod's
    // `ZodIssue = ZodIssueOptionalMessage & { fatal?; message }` merged down to
    // `{ fatal, message }` and made `path` a false TS2353.
    let undistributed_union_operand = members.iter().any(|ty| matches!(ty, Type::Union(_)));
    let dropped_unmodelled_operand = dropped_unmodelled_operand || undistributed_union_operand;

    let open_if_unmodelled = |ty: Type| -> Type {
        match ty {
            Type::Object(object)
                if dropped_unmodelled_operand && object.string_index_type.is_none() =>
            {
                let mut object = object.with_open_index_marker();
                object.string_index_type = Some(std::sync::Arc::new(Type::Any));
                Type::Object(object)
            }
            other => other,
        }
    };

    let object_members: Vec<_> = members
        .iter()
        .filter_map(|ty| match ty {
            Type::Object(object) => Some(object),
            _ => None,
        })
        .collect();

    // Brand idiom: `string & { _?: never }` (and other `Base & {…all-optional…}`
    // shapes, e.g. `LiteralUnion<L, B> = L | (B & { _?: never })`). When every
    // object operand only contributes optional members, the object side is a
    // phantom "brand" and the intersection is structurally just the non-object
    // side — tsc treats `string & {}` as assignable both to and from `string`.
    // Collapsing to the non-object member keeps that bidirectional behavior;
    // falling through to the object-merge below would keep only `{ _?: never }`
    // and wrongly reject `(string & brand) → string`.
    if !object_members.is_empty()
        && object_members
            .iter()
            .all(|object| is_brand_like_object(object))
    {
        let mut non_object = members.iter().filter(|ty| !matches!(ty, Type::Object(_)));
        if let Some(first) = non_object.next() {
            if non_object.next().is_none() {
                return first.clone();
            }
        }
    }

    if !object_members.is_empty() {
        let mut properties: PropertyMap = PropertyMap::default();
        let mut string_index_type: Option<Type> = None;
        // An operand that was itself opened by a nested merge contributes an
        // index the author never wrote; re-merging must not launder it into a
        // declared one, or `noPropertyAccessFromIndexSignature` fires on the
        // outer surface instead.
        let mut string_index_is_synthetic = false;
        // A callable operand (`F & { … }`, or an interface with a call signature)
        // keeps the merged intersection callable. An intersection of *several*
        // function types is TypeScript's overload spelling (execa's
        // `ExecaMethod` is four call signatures intersected), so they fold into
        // one permissive signature rather than the first winning — otherwise
        // every call matching a later overload is a false TS2554.
        let mut call_signature = members
            .iter()
            .filter_map(|ty| match ty {
                Type::Function(function_type) => Some(function_type.clone()),
                _ => None,
            })
            .reduce(|merged, function_type| {
                crate::infer::types::interface::merge_overload_signatures(&merged, &function_type)
            })
            .map(std::sync::Arc::new);
        let mut construct_signature: Option<std::sync::Arc<surge_ts_types::FunctionType>> = None;

        for object in &object_members {
            for (name, property) in object.properties.iter() {
                properties
                    .entry(name.clone())
                    .or_insert_with(|| property.clone());
            }
            if string_index_type.is_none()
                && let Some(index) = object.string_index_type.as_deref()
            {
                string_index_type = Some(index.clone());
                string_index_is_synthetic = object.synthetic_open_index;
            }
            if let Some(object_call_signature) = object.call_signature.as_deref() {
                call_signature = Some(match call_signature.take() {
                    Some(existing) => std::sync::Arc::new(
                        crate::infer::types::interface::merge_overload_signatures(
                            &existing,
                            object_call_signature,
                        ),
                    ),
                    None => std::sync::Arc::new(object_call_signature.clone()),
                });
            }
            if construct_signature.is_none() {
                construct_signature = object.construct_signature.clone();
            }
        }

        let mut merged =
            alloc_object_type(properties, string_index_type).with_intersection_marker();
        if string_index_is_synthetic {
            merged = merged.with_open_index_marker();
        }
        if let Some(display_name) = display_name {
            merged = merged.with_alias_name(display_name);
        }
        merged.call_signature = call_signature;
        merged.construct_signature = construct_signature;
        return open_if_unmodelled(Type::Object(merged));
    }

    // An intersection of *only* function types is TypeScript's overload
    // spelling (execa's `ExecaMethod` intersects four call signatures). Folding
    // them into one permissive signature keeps every overload's arity callable;
    // taking the first made calls matching a later one a false TS2554.
    let mut function_members = members.iter().filter_map(|ty| match ty {
        Type::Function(function_type) => Some(function_type),
        _ => None,
    });
    if let (Some(first), Some(second)) = (function_members.next(), function_members.next()) {
        let merged = function_members.fold(
            crate::infer::types::interface::merge_overload_signatures(first, second),
            |merged, function_type| {
                crate::infer::types::interface::merge_overload_signatures(&merged, function_type)
            },
        );
        return Type::Function(merged);
    }

    // Two distinct literals have no common inhabitant, so their intersection is
    // `never`. Without this the tail was first-operand-wins, which inverts a
    // guard written as `keyof A & keyof B extends never ? … : …` — trpc's
    // `ProtectedIntersection` — because the disjoint key sets reduced to the
    // first operand instead of `never`. Only literal operands participate;
    // full `string & number -> never` reduction stays a non-goal, as above.
    if let Some(reduced) = reduce_disjoint_literals(&members) {
        return reduced;
    }

    match members.into_iter().next() {
        Some(member) => member,
        None => Type::Unknown,
    }
}

/// `Some` when every operand is a literal: `never` if any two differ, otherwise
/// the shared literal. `None` leaves the caller's existing behavior alone.
fn reduce_disjoint_literals(members: &[Type]) -> Option<Type> {
    let mut literals = members.iter().map(|member| match member {
        Type::StringLiteral(_) | Type::NumberLiteral(_) | Type::BooleanLiteral(_) => Some(member),
        _ => None,
    });
    let first = literals.next()??;
    for other in literals {
        let other = other?;
        if other != first {
            return Some(Type::Never);
        }
    }
    Some(first.clone())
}

/// Whether an object contributes no required structure to an intersection — all
/// properties optional, no index signature, no call/construct signature. Such an
/// operand is a phantom "brand" (`{ _?: never }`), so `Base & brand` is
/// structurally just `Base`.
fn is_brand_like_object(object: &ObjectType) -> bool {
    object.string_index_type.is_none()
        && object.call_signature().is_none()
        && object.construct_signature().is_none()
        && object
            .properties
            .values()
            .all(|property| property.is_optional())
}
