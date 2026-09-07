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

/// Nesting bound for union distribution. Each arm's merge peels its operands,
/// and peeling a deferred intersection reference re-enters the merge — a
/// self-referential shape recursed until the stack gave out once *every* union
/// operand distributed rather than only a lone one. Beyond this depth the merge
/// falls through to the single open form, which is what it did for these shapes
/// before distribution reached them at all.
const MAX_DISTRIBUTION_DEPTH: u32 = 2;

thread_local! {
    static DISTRIBUTION_DEPTH: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

struct DistributionDepth;

impl DistributionDepth {
    fn enter() -> Option<Self> {
        DISTRIBUTION_DEPTH.with(|depth| {
            if depth.get() >= MAX_DISTRIBUTION_DEPTH {
                return None;
            }
            depth.set(depth.get() + 1);
            Some(Self)
        })
    }
}

impl Drop for DistributionDepth {
    fn drop(&mut self) {
        DISTRIBUTION_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
    }
}

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
    // `Type::Void` counts as a loss too: `PromiseLike<T>` is modelled as its
    // awaited `T`, so `PromiseLike<void> & { pull(): void }` reaches here as
    // `void & {…}` with the promise's own `then` gone. Leaving the object
    // surface closed reported `then` as an excess property on every value
    // written against such a type.
    let dropped_unmodelled_operand = members
        .iter()
        .any(|ty| matches!(ty, Type::Unknown | Type::Void));
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
        let survivor = members.into_iter().next().unwrap();
        // A NAMED lone survivor must become open too — `T & Other` where `T`'s
        // binding degraded leaves `Other` alone, and a closed `Other` reports
        // every member that lived on `T`'s constraint as missing. Peeling it
        // here would force the reference's structural expansion at resolution
        // time (the hazard the comment above describes, and the one the
        // `any`-member degradation counter pins), so defer instead: the merge
        // below peels and opens on first consumer peel.
        if dropped_unmodelled_operand
            && let Type::Reference(reference) = &survivor
        {
            crate::program::record_program_counter(|c| c.lazy_intersection_create_count += 1);
            let display = survivor.name();
            let id = format!("\u{0}intersection-open\u{0}{}", reference.id.as_ref());
            let members = vec![survivor.clone()];
            return Type::Reference(surge_ts_types::TypeReference::new(
                id,
                display,
                members.clone(),
                std::sync::Arc::new(LazyIntersectionMerge {
                    members,
                    dropped_unmodelled_operand: true,
                    memo: std::sync::OnceLock::new(),
                }),
            ));
        }
        return open_if_unmodelled(survivor);
    }

    // `(A | B) & C` is `(A & C) | (B & C)`. The object merge below only reads
    // `Type::Object` operands, so an undistributed union operand contributes
    // nothing: the merged surface keeps only `C`'s properties and every use of an
    // `A`/`B` member is reported as an excess property.
    //
    // Every union operand distributes, not just a lone one — tRPC's server-side
    // helper options are `(queryClient | queryClientConfig) & (external |
    // internal)`, two unions, and keeping only the first one's members made
    // every `router`/`ctx`/`client` read a false TS2339. The bound is on the
    // *product*: each arm rebuilds the whole merged property map, so the cap
    // holds the worst case at `MAX_DISTRIBUTED_UNION_ARITY` merges however the
    // operands split. Anything wider falls through to the single merge marked
    // open, which suppresses the excess-property report for the names surge did
    // not enumerate.
    let union_operands: Vec<(usize, Vec<Type>)> = members
        .iter()
        .enumerate()
        .filter_map(|(index, ty)| match ty {
            Type::Union(union) => Some((index, union.types().to_vec())),
            _ => None,
        })
        .collect();
    let unenumerated_union_operand = !union_operands.is_empty();
    let product = union_operands
        .iter()
        .try_fold(1usize, |product, (_, arms)| {
            (!arms.is_empty()).then(|| product.saturating_mul(arms.len()))
        });
    if let Some(product) = product
        && !union_operands.is_empty()
        && product <= MAX_DISTRIBUTED_UNION_ARITY
        && let Some(_guard) = DistributionDepth::enter()
    {
        let operand_names: Vec<String> = members.iter().map(Type::name).collect();
        let mut distributed = Vec::with_capacity(product);
        for selection in 0..product {
            let mut operands = members.clone();
            let mut names = operand_names.clone();
            let mut remaining = selection;
            for (index, arms) in &union_operands {
                let arm = &arms[remaining % arms.len()];
                remaining /= arms.len();
                names[*index] = arm.name();
                operands[*index] = arm.clone();
            }
            distributed.push(merge_intersection_members_now(
                operands,
                Some(names.join(" & ")),
                dropped_unmodelled_operand,
            ));
        }
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

    // An object operand excludes `null`/`undefined`, so `undefined & {}` is
    // `never`. That reduction is what makes `NonNullable<T>` — defined as
    // `T & {}` — actually drop the nullish arms once the union is distributed;
    // without it the brand-collapse below handed `undefined` back and every
    // `NonNullable<…>` kept its `| undefined`.
    //
    // `void` is deliberately excluded even though tsc reduces it the same way:
    // surge models `PromiseLike<void>` as its awaited `void`, so
    // `PromiseLike<void> & { pull(): void }` reaches here as `void & {…}` and
    // reducing it to `never` would strip the contextual type off every method
    // written against it. Skipped, too, when an operand was dropped as
    // unmodelled: the surviving object may not be the whole story.
    if !dropped_unmodelled_operand
        && !object_members.is_empty()
        && members.iter().any(|ty| matches!(ty, Type::Undefined))
    {
        return Type::Never;
    }

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
                // A property declared by more than one operand is the
                // *intersection* of what they declare, not the first one:
                // `Row & { identity: Code }` narrows `identity` from
                // `Code | NameOnly` down to `Code`. Keeping the first made every
                // type-predicate narrowing through an intersection read the
                // unnarrowed union.
                match properties.get(name) {
                    Some(existing) => {
                        let merged_property = surge_ts_types::ObjectProperty {
                            ty: merge_intersection_members(vec![
                                existing.ty.clone(),
                                property.ty.clone(),
                            ]),
                            optional: existing.is_optional() && property.is_optional(),
                            method: existing.method,
                        };
                        properties.insert(name.clone(), merged_property);
                    }
                    None => {
                        properties.insert(name.clone(), property.clone());
                    }
                }
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

/// The set of literals an operand admits, when the operand is a scalar literal,
/// a union of scalar literals, or the primitive that spells one of their
/// domains. `None` for anything else, which leaves the caller's existing
/// behavior alone.
enum LiteralDomain<'a> {
    /// Every literal of one primitive domain (`string`, `number`, …).
    Primitive(&'a Type),
    Members(Vec<&'a Type>),
}

fn literal_primitive_domain(literal: &Type) -> Option<Type> {
    match literal {
        Type::StringLiteral(_) => Some(Type::String),
        Type::NumberLiteral(_) => Some(Type::Number),
        Type::BooleanLiteral(_) => Some(Type::Boolean),
        _ => None,
    }
}

fn literal_domain(ty: &Type) -> Option<LiteralDomain<'_>> {
    match ty {
        Type::String | Type::Number | Type::Boolean => Some(LiteralDomain::Primitive(ty)),
        Type::StringLiteral(_) | Type::NumberLiteral(_) | Type::BooleanLiteral(_) => {
            Some(LiteralDomain::Members(vec![ty]))
        }
        Type::Union(union) => {
            let mut members = Vec::with_capacity(union.types().len());
            for member in union.types() {
                match member {
                    Type::StringLiteral(_) | Type::NumberLiteral(_) | Type::BooleanLiteral(_) => {
                        members.push(member);
                    }
                    _ => return None,
                }
            }
            (!members.is_empty()).then_some(LiteralDomain::Members(members))
        }
        _ => None,
    }
}

/// `Some` when every operand is literal-like: the set intersection of their
/// admitted literals, `never` when that is empty. Without this a `keyof A &
/// keyof B` guard — trpc's `ProtectedIntersection`, whose disjoint key sets must
/// reduce to `never` — took the first operand and inverted the conditional. Two
/// union operands never reach the distribution path above (its product is
/// quadratic), so the reduction has to happen here.
fn reduce_disjoint_literals(members: &[Type]) -> Option<Type> {
    let mut domains = members.iter().map(literal_domain);
    let mut reduced = domains.next()??;

    for domain in domains {
        reduced = match (reduced, domain?) {
            (LiteralDomain::Primitive(left), LiteralDomain::Primitive(right)) => {
                if left == right {
                    LiteralDomain::Primitive(left)
                } else {
                    return Some(Type::Never);
                }
            }
            (LiteralDomain::Primitive(primitive), LiteralDomain::Members(members))
            | (LiteralDomain::Members(members), LiteralDomain::Primitive(primitive)) => {
                LiteralDomain::Members(
                    members
                        .into_iter()
                        .filter(|member| {
                            literal_primitive_domain(member).as_ref() == Some(primitive)
                        })
                        .collect(),
                )
            }
            (LiteralDomain::Members(left), LiteralDomain::Members(right)) => LiteralDomain::Members(
                left.into_iter()
                    .filter(|member| right.contains(member))
                    .collect(),
            ),
        };
        if matches!(&reduced, LiteralDomain::Members(members) if members.is_empty()) {
            return Some(Type::Never);
        }
    }

    Some(match reduced {
        LiteralDomain::Primitive(primitive) => primitive.clone(),
        LiteralDomain::Members(members) => {
            surge_ts_types::union_type(members.into_iter().cloned().collect())
        }
    })
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
