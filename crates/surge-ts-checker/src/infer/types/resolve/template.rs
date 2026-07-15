use super::*;

use surge_ts_syntax::ParsedTemplateLiteralType;

/// Maximum number of string-literal members a finite template expansion may
/// produce. Beyond this we fall back to broad `string` rather than materialise a
/// huge union (a defensive bound; real fixtures stay tiny).
const TEMPLATE_LITERAL_EXPANSION_LIMIT: usize = 10_000;

/// Evaluates a narrow subset of template literal types.
///
/// When every interpolation resolves to a finite set of string/number/boolean
/// literal members, the template expands to the cartesian product of its parts
/// as a deduped string-literal union (e.g. `` `/${"a"|"b"}/${"c"}` `` becomes
/// `"/a/c" | "/b/c"`). If any interpolation is a broad primitive (`string`,
/// `number`, …) or otherwise unresolved, the whole template degrades to broad
/// `string` so callers stay conservative and never cascade. This means a broad
/// template like `` `id:${string}` `` accepts any string — tsc is stricter, but
/// the mismatch is silent rather than a false positive.
pub(crate) fn resolve_template_literal_type(
    template: ParsedTemplateLiteralType,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let ParsedTemplateLiteralType {
        quasis,
        interpolations,
        ..
    } = template;

    // The head is always present; interpolation i is followed by quasi i + 1.
    let mut combinations: Vec<String> = vec![quasis.first().cloned().unwrap_or_default()];
    let mut had_error = false;

    for (index, interpolation) in interpolations.into_iter().enumerate() {
        let resolved = resolve_parsed_type(interpolation, ctx, resolving, substitution);
        had_error |= resolved.had_error;

        let Some(parts) = finite_literal_strings(&resolved.ty) else {
            // Broad or unresolved interpolation: degrade to `string`.
            return ResolvedType {
                ty: Type::String,
                had_error,
            };
        };

        let suffix = quasis.get(index + 1).cloned().unwrap_or_default();
        if combinations.len().saturating_mul(parts.len()) > TEMPLATE_LITERAL_EXPANSION_LIMIT {
            return ResolvedType {
                ty: Type::String,
                had_error,
            };
        }

        let mut next = Vec::with_capacity(combinations.len() * parts.len());
        for prefix in &combinations {
            for part in &parts {
                next.push(format!("{prefix}{part}{suffix}"));
            }
        }
        combinations = next;
    }

    let members: Vec<Type> = combinations.into_iter().map(Type::StringLiteral).collect();

    ResolvedType {
        ty: union_type(members),
        had_error,
    }
}

/// Returns the finite set of literal string renderings for `ty` if it is a
/// string/number/boolean literal (or a union of such literals), or `None` if it
/// is a broad primitive or anything else that cannot be enumerated. Rendering
/// matches how each literal participates in a template literal: numbers by their
/// literal text and booleans as `true`/`false`.
fn finite_literal_strings(ty: &Type) -> Option<Vec<String>> {
    match ty {
        Type::StringLiteral(value) => Some(vec![value.clone()]),
        Type::NumberLiteral(value) => Some(vec![value.value.clone()]),
        Type::BooleanLiteral(value) => Some(vec![value.to_string()]),
        Type::Union(union) => {
            let mut parts = Vec::new();
            for member in union.types().iter() {
                parts.extend(finite_literal_strings(member)?);
            }
            Some(parts)
        }
        _ => None,
    }
}
