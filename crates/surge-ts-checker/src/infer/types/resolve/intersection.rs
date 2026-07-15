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
                let mut object = object;
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

    merge_intersection_members_now(members, display_name, dropped_unmodelled_operand)
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
    let open_if_unmodelled = |ty: Type| -> Type {
        match ty {
            Type::Object(object)
                if dropped_unmodelled_operand && object.string_index_type.is_none() =>
            {
                let mut object = object;
                object.string_index_type = Some(std::sync::Arc::new(Type::Any));
                Type::Object(object)
            }
            other => other,
        }
    };

    // Peel reference operands (`StudentBulkImportRow & { … }`) so a named object
    // member contributes its properties to the merged intersection surface.
    let members: Vec<Type> = members.iter().map(Type::peeled).collect();

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
        let mut properties: PropertyMap = PropertyMap::new();
        let mut string_index_type: Option<Type> = None;
        // A callable operand (`F & { … }`, or an interface with a call signature)
        // keeps the merged intersection callable; the first signature wins, like
        // conflicting properties.
        let mut call_signature = members.iter().find_map(|ty| match ty {
            Type::Function(function_type) => Some(std::sync::Arc::new(function_type.clone())),
            _ => None,
        });
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
            }
            if call_signature.is_none() {
                call_signature = object.call_signature.clone();
            }
            if construct_signature.is_none() {
                construct_signature = object.construct_signature.clone();
            }
        }

        let mut merged =
            alloc_object_type(properties, string_index_type).with_intersection_marker();
        if let Some(display_name) = display_name {
            merged = merged.with_alias_name(display_name);
        }
        merged.call_signature = call_signature;
        merged.construct_signature = construct_signature;
        return open_if_unmodelled(Type::Object(merged));
    }

    match members.into_iter().next() {
        Some(member) => member,
        None => Type::Unknown,
    }
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
