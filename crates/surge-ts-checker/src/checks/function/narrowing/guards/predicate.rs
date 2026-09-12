use surge_ts_syntax::ParsedExpression;
use surge_ts_types::{Type, union_type};

/// A user-defined type-predicate guard extracted from a call condition
/// (`isFoo(x)` where `isFoo`'s collected signature returns `param is T`).
pub(crate) struct PredicateGuardInfo {
    /// The base identifier of the argument in the tested parameter's position.
    pub(crate) subject: String,
    /// Property path from `subject` to the tested reference, empty for a bare
    /// identifier (`ts.isStringLiteral(node.moduleSpecifier)` -> `["moduleSpecifier"]`).
    pub(crate) path: Vec<String>,
    /// The predicate's parsed target type (`T` in `param is T`), unresolved.
    pub(crate) predicate_type: surge_ts_syntax::ParsedType,
    /// File the predicate signature was declared in; its module-local type
    /// names resolve under this file's scope (see
    /// [`crate::symbols::FunctionSignatureInfo::declaring_file`]).
    pub(crate) declaring_file: Option<std::sync::Arc<str>>,
    /// Namespace the predicate signature was declared in, when it was published
    /// as a qualified `ns.member`. `node is ImportDeclaration` inside
    /// `declare namespace ts` names `ts.ImportDeclaration`, which only resolves
    /// under that prefix (see
    /// [`crate::symbols::FunctionSignatureInfo::namespace_prefix`]).
    pub(crate) namespace_prefix: Option<std::sync::Arc<str>>,
    /// The callee's signature, kept so a generic predicate (`x is OK<T>`) can
    /// infer `T` from the tested argument at the guard site.
    pub(crate) signature: std::sync::Arc<crate::symbols::FunctionSignatureInfo>,
    /// The call's other arguments, by declared position, so a generic
    /// predicate's remaining type parameters can be inferred from them —
    /// `isMatching(pattern, value)` states its target in terms of the *pattern*.
    /// Empty unless the signature is generic, since nothing else reads it.
    pub(crate) other_arguments: Vec<(usize, ParsedExpression)>,
    /// Position of the tested parameter in `signature.parameter_types`.
    pub(crate) parameter_index: usize,
}

/// Extracts a user-defined type-predicate guard from a call condition. The
/// callee's collected signature must declare a non-asserts `param is T`
/// predicate over a named value parameter, the call must not carry explicit
/// type arguments, and the argument in the tested position must be a bare
/// identifier. A *generic* predicate is kept: its type parameters are inferred
/// from the tested argument's own type when the guard is resolved.
pub(crate) fn parse_type_predicate_condition(
    condition: &ParsedExpression,
    signature_of: &mut dyn FnMut(
        &str,
    )
        -> Option<std::sync::Arc<crate::symbols::FunctionSignatureInfo>>,
) -> Option<PredicateGuardInfo> {
    // A guard reached through a namespace (`ts.isImportDeclaration(node)`) parses
    // as a property call; its signature is registered under the qualified
    // `<alias>.<member>` key.
    let qualified_callee;
    let (callee_name, type_arguments, arguments) = match condition {
        ParsedExpression::Call {
            callee_name,
            type_arguments,
            arguments,
            ..
        } => (callee_name.as_str(), type_arguments, arguments),
        ParsedExpression::PropertyCall {
            object,
            property_name,
            type_arguments,
            arguments,
            ..
        } => {
            let ParsedExpression::Identifier { name, .. } = object.as_ref() else {
                return None;
            };
            qualified_callee = format!("{name}.{property_name}");
            (qualified_callee.as_str(), type_arguments, arguments)
        }
        _ => return None,
    };
    if !type_arguments.is_empty() {
        return None;
    }
    let signature = signature_of(callee_name)?;
    // An overload group keeps one declaration's parsed signature, which need not
    // be the one that declared the predicate; the fold records that overload
    // alongside. `arguments.get(index)` below still decides whether this call
    // reaches the predicate's parameter, so a call of a *different* overload
    // narrows nothing.
    let signature = match &signature.return_type {
        Some(surge_ts_syntax::ParsedType::Predicate(_)) => signature,
        _ => signature.predicate_overload.clone()?,
    };
    let Some(surge_ts_syntax::ParsedType::Predicate(predicate)) = &signature.return_type else {
        return None;
    };
    if predicate.asserts || predicate.parameter_name == "this" {
        return None;
    }
    let predicate_type = predicate.ty.clone()?;
    let index = signature
        .parameter_names
        .iter()
        .position(|name| name.as_deref() == Some(predicate.parameter_name.as_str()))?;
    let (subject, path) = super::super::reference_path(&arguments.get(index)?.expression)?;
    let other_arguments = if signature.type_parameters.is_empty() {
        Vec::new()
    } else {
        arguments
            .iter()
            .enumerate()
            .filter(|(position, _)| *position != index)
            .map(|(position, argument)| (position, argument.expression.clone()))
            .collect()
    };
    Some(PredicateGuardInfo {
        subject,
        path,
        other_arguments,
        predicate_type,
        declaring_file: signature.declaring_file.clone(),
        namespace_prefix: signature.namespace_prefix.clone(),
        parameter_index: index,
        signature,
    })
}

/// Narrows a value tested by a `param is T` predicate. In the true branch a
/// union keeps the members assignable to `T`; when none are but `T` itself fits
/// a member (`string | undefined` guarded by `x is "a" | "b"`), the value *is*
/// `T`. The false branch removes the members assignable to `T`. A non-union
/// subject narrows to `T` in the true branch when `T` is a subtype of it.
/// Returns `None` when the guard proves nothing new (or would empty the type —
/// stay conservative rather than model `never`).
pub(crate) fn narrow_by_predicate(
    ty: &Type,
    predicate: &Type,
    keep_matching: bool,
) -> Option<Type> {
    let peeled = ty.peeled();
    if let Type::Union(union) = &peeled {
        let members = union.types();
        if keep_matching {
            let matching: Vec<Type> = members
                .iter()
                .filter(|member| surge_ts_types::is_assignable_to(member, predicate))
                .cloned()
                .collect();
            if !matching.is_empty() {
                if matching.len() == members.len() {
                    return None;
                }
                return Some(union_type(matching));
            }
            if members
                .iter()
                .any(|member| surge_ts_types::is_assignable_to(predicate, member))
            {
                return Some(predicate.clone());
            }
            return None;
        }
        let remaining: Vec<Type> = members
            .iter()
            .filter(|member| !surge_ts_types::is_assignable_to(member, predicate))
            .cloned()
            .collect();
        if remaining.is_empty() || remaining.len() == members.len() {
            return None;
        }
        return Some(union_type(remaining));
    }
    // A non-union subject narrows to the predicate whenever the predicate is a
    // subtype of it, which is tsc's rule. Requiring the subject *not* to be
    // assignable to the predicate as well is stricter than tsc and silently
    // depended on that direction failing: an `unknown` subject narrowed because
    // nothing is assignable to it, while `{}` did not, because an
    // index-signature-only target such as `Record<string, unknown>` accepts any
    // object. `any` and the degradation sentinel are excluded — both are
    // assignable in every direction, so narrowing them would invent a type for
    // a subject whose real one was never reconstructed. The genuine `unknown`
    // keyword is not excluded: narrowing it is the case that already works.
    if keep_matching
        && !matches!(peeled, Type::Any | Type::Unknown | Type::TypeParameter(_))
        && peeled != *predicate
        && surge_ts_types::is_assignable_to(predicate, &peeled)
    {
        return Some(predicate.clone());
    }
    None
}
