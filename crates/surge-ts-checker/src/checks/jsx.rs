use std::collections::BTreeMap;
use std::sync::Arc;

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{
    ParsedExpression, ParsedJsxAttribute, ParsedJsxAttributeValueKind, ParsedJsxChild,
    ParsedJsxTag, ParsedNamedType, ParsedType, TextSpan as SyntaxTextSpan,
};
use surge_ts_types::{
    FunctionType, ObjectProperty, ObjectType, PropertyMap, Type, is_assignable_to, union_type,
};

use super::expected::{ExpectedTypeDiagnostic, evaluate_expression_with_expected_type};
use super::expr::{evaluate_expression, source_display_name};
use crate::context::CheckerContext;
use crate::infer::{InferredExpression, map_parsed_type};
use crate::metrics::alloc_object_type;
use crate::spans::diagnostic_with_syntax_span;
use crate::symbols::{SymbolTable, TypeDeclarationScope};

/// Where a file's JSX looks up `IntrinsicElements`, `Element` and the other
/// `JSX` members (tsc's `getJsxNamespaceAt`).
#[derive(Clone, Debug)]
pub(crate) enum JsxNamespace {
    /// The members resolve as `<prefix>.<member>` from the file's own scope.
    Qualified(Arc<str>),
    /// The members resolve as `<prefix>.<member>` inside the scope of the
    /// declaration file that declares them. The automatic runtime reaches its
    /// namespace through the `jsx-runtime` import tsc synthesizes, which surge
    /// does not load, so the declaring module is located instead.
    Declared {
        scope: Arc<TypeDeclarationScope>,
        prefix: Arc<str>,
    },
    /// tsc resolves a namespace whose members surge cannot see from this file:
    /// a UMD global factory namespace read from a module.
    Unmodelled,
    /// No JSX namespace at all: every member is tsc's error type.
    Missing,
}

/// A member of the JSX namespace (tsc's `getJsxType`).
enum JsxMember {
    Found(Type),
    /// tsc's error type: the namespace does not declare the member.
    Missing,
    Unmodelled,
}

/// tsc's `getJsxNamespace`: the root of the factory a tag compiles to — the
/// file's `@jsx` pragma, else `jsxFactory`, else `reactNamespace`, else
/// `React`. A fragment compiles to the fragment factory, so it reads `@jsxFrag`,
/// else `jsxFragmentFactory`, and the default namespace (never the file's
/// `@jsx` pragma) when neither is set.
fn jsx_factory_namespace_name(fragment: bool, ctx: &CheckerContext) -> String {
    let names = &ctx.options.jsx_factory_names;
    let default_namespace = || match names.factory.as_deref().filter(|factory| !factory.is_empty()) {
        Some(factory) => {
            surge_ts_syntax::jsx_entity_root(factory).unwrap_or_else(|| "React".to_string())
        }
        None => names
            .react_namespace
            .clone()
            .filter(|namespace| !namespace.is_empty())
            .unwrap_or_else(|| "React".to_string()),
    };
    if fragment {
        return ctx
            .jsx_factory_uses
            .fragment_pragma
            .clone()
            .or_else(|| {
                names
                    .fragment_factory
                    .as_deref()
                    .and_then(surge_ts_syntax::jsx_entity_root)
            })
            .unwrap_or_else(default_namespace);
    }
    ctx.jsx_factory_uses
        .factory_pragma
        .clone()
        .unwrap_or_else(default_namespace)
}

/// Reports the implicit factory reference a JSX tag makes (tsc's
/// `markJsxAliasReferenced`). Under `jsx: react` tsc resolves the factory
/// namespace as a value at every opening element, self-closing element, and
/// opening fragment — reporting at the tag name, or at the `<` of a fragment —
/// so a module that never imports it reports once per tag. A fragment also
/// resolves the file's element factory. `preserve` and `react-native` resolve
/// it without error reporting and the automatic runtime never names it, which
/// is why this is gated on the classic React mode alone.
pub(crate) fn check_jsx_factory_reference(
    fragment: bool,
    location_span: Option<SyntaxTextSpan>,
    fallback_span: Option<SyntaxTextSpan>,
    ctx: &mut CheckerContext,
) {
    if !ctx.options.jsx_classic_react {
        return;
    }

    let span = location_span.or(fallback_span);
    let namespace = jsx_factory_namespace_name(fragment, ctx);
    // `jsxFragmentFactory: "null"` names no binding.
    if !(fragment && namespace == "null") {
        crate::checks::emit_value_position_reference_diagnostic(&namespace, span, ctx);
    }
    if fragment {
        let element_namespace = jsx_factory_namespace_name(false, ctx);
        if element_namespace != namespace {
            crate::checks::emit_value_position_reference_diagnostic(&element_namespace, span, ctx);
        }
    }
}

/// tsc's `getJsxNamespaceAt`, resolved once per file.
pub(crate) fn jsx_namespace(ctx: &mut CheckerContext) -> JsxNamespace {
    if let Some((file_name, namespace)) = &ctx.jsx_namespace
        && file_name.as_ref() == ctx.file_name.as_str()
    {
        return namespace.clone();
    }
    let namespace = resolve_jsx_namespace(ctx);
    ctx.jsx_namespace = Some((Arc::from(ctx.file_name.as_str()), namespace.clone()));
    namespace
}

/// The factory namespace's `JSX` when the factory resolves to a namespace
/// that has one, else the global `JSX`.
fn resolve_jsx_namespace(ctx: &CheckerContext) -> JsxNamespace {
    if ctx.options.jsx_automatic_runtime {
        return automatic_runtime_jsx_namespace(ctx);
    }
    let factory = jsx_factory_namespace_name(false, ctx);
    let qualified = format!("{factory}.JSX");
    if namespace_is_visible(&qualified, ctx) {
        return JsxNamespace::Qualified(qualified.into());
    }
    if global_jsx_namespace_exists(ctx) {
        return JsxNamespace::Qualified("JSX".into());
    }
    // A UMD global resolves as a namespace for tsc, but its members are not
    // reachable from a module here, so whether it has a `JSX` is unknowable.
    if ctx.is_umd_global_value_reference(&factory) {
        return JsxNamespace::Unmodelled;
    }
    JsxNamespace::Missing
}

/// The namespace the automatic runtime's `jsx-runtime` module exports.
/// surge does not load that module, so a visible `JSX` or `React.JSX` stands
/// in for it, and otherwise the declaration file that declares the intrinsic
/// elements.
fn automatic_runtime_jsx_namespace(ctx: &CheckerContext) -> JsxNamespace {
    for prefix in ["JSX", "React.JSX"] {
        if ctx
            .lookup_type_declaration(&format!("{prefix}.IntrinsicElements"))
            .is_some()
        {
            return JsxNamespace::Qualified(prefix.into());
        }
    }
    match &ctx.jsx_intrinsic_elements_declarer {
        Some((table, key)) => JsxNamespace::Declared {
            scope: Arc::new(TypeDeclarationScope::new(vec![table.clone()])),
            prefix: key
                .strip_suffix(".IntrinsicElements")
                .unwrap_or(key)
                .into(),
        },
        None => JsxNamespace::Missing,
    }
}

/// The JSX member names tsc reads; one of them resolving is the cheap proof
/// that a namespace exists before the declaration tables are scanned.
const JSX_MEMBER_NAMES: [&str; 9] = [
    "IntrinsicElements",
    "Element",
    "ElementClass",
    "ElementAttributesProperty",
    "ElementChildrenAttribute",
    "IntrinsicAttributes",
    "IntrinsicClassAttributes",
    "LibraryManagedAttributes",
    "ElementType",
];

/// Whether `name` resolves as a namespace from the current file: a namespace
/// block surge registered, or a type declared beneath it in any table the
/// file's type lookups consult (an import copies its module's members in under
/// the local name).
fn namespace_is_visible(name: &str, ctx: &CheckerContext) -> bool {
    if ctx.namespace_info(name).is_some() {
        return true;
    }
    if JSX_MEMBER_NAMES
        .iter()
        .any(|member| ctx.lookup_type_declaration(&format!("{name}.{member}")).is_some())
    {
        return true;
    }
    let heads = |key: &Arc<str>| {
        key.strip_prefix(name)
            .is_some_and(|rest| rest.starts_with('.'))
    };
    ctx.type_declarations.iter().any(|(key, _)| heads(key))
        || ctx.type_declaration_scope.as_ref().is_some_and(|scope| {
            scope
                .layers()
                .iter()
                .any(|layer| layer.iter().any(|(key, _)| heads(key)))
        })
        || ctx
            .ambient_global_type_declarations
            .iter()
            .any(|(key, _)| heads(key))
}

/// tsc's JSX global fallback (`getGlobalSymbol(JSX, Namespace)`): only a
/// global `JSX` counts, never one a module declares for itself.
fn global_jsx_namespace_exists(ctx: &CheckerContext) -> bool {
    ctx.namespace_registry.is_global("JSX")
        || JSX_MEMBER_NAMES.iter().any(|member| {
            ctx.ambient_global_type_declarations
                .get(&format!("JSX.{member}"))
                .is_some()
        })
}

/// tsc's `getJsxType`: the declared type of the JSX namespace's `member`.
fn jsx_namespace_member(member: &str, ctx: &mut CheckerContext) -> JsxMember {
    let (prefix, scope) = match jsx_namespace(ctx) {
        JsxNamespace::Qualified(prefix) => (prefix, None),
        JsxNamespace::Declared { scope, prefix } => (prefix, Some(scope)),
        JsxNamespace::Unmodelled => return JsxMember::Unmodelled,
        JsxNamespace::Missing => return JsxMember::Missing,
    };
    let name = format!("{prefix}.{member}");
    let resolve = |ctx: &mut CheckerContext| {
        if ctx.lookup_type_declaration(&name).is_none() {
            return JsxMember::Missing;
        }
        JsxMember::Found(map_parsed_type(
            ParsedType::Named(Arc::new(ParsedNamedType {
                name: name.clone(),
                span: None,
                type_arguments: Vec::new(),
            })),
            ctx,
        ))
    };
    match scope {
        Some(scope) => crate::infer::with_type_declaration_scope(&Some(scope), ctx, resolve),
        None => resolve(ctx),
    }
}

/// What a tag's attributes are checked against.
enum PropsResolution {
    Props(Type),
    /// tsc's error type or a non-component value: nothing to check against,
    /// and no contextual type for a callback attribute — tsc reports its
    /// parameters implicitly `any`.
    Unchecked,
    /// The props exist for tsc but surge could not model them, so an
    /// implicit-any report inside the attributes would describe that gap.
    Unmodelled,
}

/// Checks a JSX element: resolves the tag to an intrinsic element or function
/// component, lowers attributes into a props object, and reports missing,
/// excess, and mistyped props plus basic `children` mismatches. Attribute and
/// child expressions are always evaluated for ordinary diagnostics (e.g. an
/// unresolved name in `{expr}`), even when the tag itself does not resolve, so a
/// missing component never cascades into a prop-checking storm.
pub(crate) fn check_jsx_element(
    tag_name: &str,
    tag: &ParsedJsxTag,
    attributes: &[ParsedJsxAttribute],
    children: &[ParsedJsxChild],
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) {
    let tag_name_span = tag.name_span;
    let resolution = match &tag.expression {
        Some(expression) => {
            resolve_component_props(expression, tag_name_span.or(fallback_span), symbols, ctx)
        }
        None => intrinsic_tag_props(tag_name, tag.span.or(fallback_span), ctx),
    };
    let (props_type, unmodelled) = match resolution {
        PropsResolution::Props(props) => (Some(props), false),
        PropsResolution::Unchecked => (None, false),
        PropsResolution::Unmodelled => (None, true),
    };

    let props_object = match props_type.as_ref().map(Type::peeled) {
        Some(Type::Object(object)) => Some(object),
        _ => None,
    };

    // A component whose props type collapsed to the degradation sentinel offers
    // no contextual type for an inline callback attribute, so any implicit-any
    // report inside those attributes would describe surge's modelling gap rather
    // than the source. Suppress it for the element's attributes and children, the
    // same no-cascade rule a sentinel receiver gets elsewhere.
    // An intrinsic element always declares props in `JSX.IntrinsicElements`, so
    // failing to reduce them to an object is surge's modelling gap rather than
    // the source's — `<form onSubmit={(event) => …}>` lost its handler's
    // contextual type that way and reported the parameter implicit-any.
    let unmodelled_props = unmodelled
        || props_type.as_ref().is_some_and(Type::is_unknown)
        || (tag.expression.is_none() && props_object.is_none());
    if unmodelled_props {
        ctx.unmodelled_jsx_props_depth += 1;
    }

    let spreads = check_attributes(
        attributes,
        props_object.as_ref(),
        fallback_span,
        symbols,
        ctx,
    );

    let children_provided = check_children(
        children,
        props_object.as_ref(),
        tag_name_span,
        fallback_span,
        symbols,
        ctx,
    );

    if unmodelled_props {
        ctx.unmodelled_jsx_props_depth -= 1;
    }

    if let Some(object) = props_object.as_ref() {
        check_missing_required_props(
            object,
            attributes,
            &spreads,
            children_provided,
            tag_name_span.or(tag.span),
            fallback_span,
            ctx,
        );
    }

    if let Some(closing) = &tag.closing {
        let span = closing.span.or(fallback_span);
        match &closing.expression {
            Some(expression) => {
                let _ = evaluate_expression(expression, span, symbols, ctx);
            }
            None => {
                let _ = intrinsic_tag_props(tag_name, span, ctx);
            }
        }
    }
}

/// What `{...expr}` spread attributes contribute to the element's attributes
/// object: the properties of every spread whose type resolved to an object, and
/// whether any spread was opaque (non-object, `any`/`unknown`, or carrying a
/// string index), in which case attribute coverage is unknowable and presence
/// checks must stand down — tsc folds an `any` spread into the whole attributes
/// type, silencing them likewise.
#[derive(Default)]
struct SpreadAttributes {
    properties: BTreeMap<String, ObjectProperty>,
    opaque: bool,
}

/// The props a value tag's attributes are checked against (tsc's
/// `resolveJsxOpeningLikeElement` for a non-intrinsic tag). Evaluating the tag
/// reports what an unresolved name or member reports anywhere else.
fn resolve_component_props(
    expression: &ParsedExpression,
    span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> PropsResolution {
    match evaluate_expression(expression, span, symbols, ctx) {
        InferredExpression::Known(component_type) => match component_props_type(&component_type) {
            Some(props) => PropsResolution::Props(props),
            None if component_is_unmodelled(&component_type) => PropsResolution::Unmodelled,
            None => PropsResolution::Unchecked,
        },
        // A value surge could not type at all is its own gap: tsc has the
        // component's props and types the callbacks from them.
        InferredExpression::Unknown => PropsResolution::Unmodelled,
        // An unresolved name or member is tsc's error type. Its
        // attributes get no contextual signature (`getContextualSignature`
        // of `any` is undefined), so tsc *does* report a callback in them
        // as implicit `any` — leave them unsuppressed.
        InferredExpression::UnresolvedIdentifier { .. }
        | InferredExpression::MissingProperty { .. } => PropsResolution::Unchecked,
    }
}

/// Whether a component that yields no props type is surge's modelling gap
/// rather than the source's. Only surge's own sentinel and a bare type parameter
/// qualify — tRPC's `stream.Provider`, read off a `createHydrationStreamProvider<…>()`
/// surge could not type, where tsc has the props and types every callback in
/// them. `any` and tsc's error type do not: a callback attribute there has no
/// contextual signature in tsc either, and it reports the parameter (the
/// unresolved `Textarea` in tRPC's `next-sse-chat`).
fn component_is_unmodelled(component_type: &Type) -> bool {
    matches!(
        component_type.peeled(),
        Type::Unknown | Type::TypeParameter(_)
    )
}

/// The props type for a component value: the first parameter of its call (or, for
/// a class component, construct) signature, or an empty object for a zero-parameter
/// component. A `forwardRef`/`memo` component is a callable object rather than a
/// bare function, so its call signature is consulted too. Non-callable values
/// (including `any`) yield `None` so no prop check runs.
fn component_props_type(component_type: &Type) -> Option<Type> {
    // `const Foo: FC<Props> = …` types the component as a nominal reference; peel
    // it to reach the underlying signature and its props parameter.
    let peeled = component_type.peeled();
    let signature: &FunctionType = match &peeled {
        Type::Function(function_type) => function_type,
        Type::Object(object) => object
            .call_signature()
            .or_else(|| object.construct_signature())?,
        _ => return None,
    };

    Some(
        signature
            .parameters()
            .first()
            .cloned()
            .unwrap_or_else(|| Type::Object(alloc_object_type(PropertyMap::default(), None))),
    )
}

/// tsc's `getIntrinsicTagSymbol` and `getIntrinsicAttributesTypeFromJsxOpeningLikeElement`:
/// the props `IntrinsicElements` declares for `tag_name`, reported at `node_span`
/// as TS2339 when it declares none and as TS7026 when the JSX namespace has no
/// `IntrinsicElements` at all.
fn intrinsic_tag_props(
    tag_name: &str,
    node_span: Option<SyntaxTextSpan>,
    ctx: &mut CheckerContext,
) -> PropsResolution {
    let intrinsic_elements = match jsx_namespace_member("IntrinsicElements", ctx) {
        JsxMember::Found(intrinsic_elements) => intrinsic_elements,
        JsxMember::Missing => {
            if ctx.options.no_implicit_any {
                ctx.push(diagnostic_with_syntax_span(
                    Diagnostic::ts7026("IntrinsicElements", ctx.file_name.clone()),
                    node_span,
                ));
            }
            return PropsResolution::Unchecked;
        }
        JsxMember::Unmodelled => return PropsResolution::Unmodelled,
    };
    let Type::Object(object) = intrinsic_elements.peeled() else {
        return PropsResolution::Unmodelled;
    };

    if let Some(property_type) = object.get_property_type(tag_name) {
        return PropsResolution::Props(property_type.clone());
    }
    if object.synthetic_open_index {
        // The index stands for members surge could not enumerate.
        return PropsResolution::Unmodelled;
    }
    if let Some(index_type) = object.string_index_type.as_deref() {
        return PropsResolution::Props(index_type.clone());
    }

    ctx.push(diagnostic_with_syntax_span(
        Diagnostic::ts2339(tag_name, "JSX.IntrinsicElements", ctx.file_name.clone()),
        node_span,
    ));
    PropsResolution::Unchecked
}

/// Evaluates each attribute (so inner expression diagnostics are preserved) and,
/// when a props type is known, reports type mismatches on known props and the
/// first excess prop.
fn check_attributes(
    attributes: &[ParsedJsxAttribute],
    props_object: Option<&ObjectType>,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> SpreadAttributes {
    let mut first_excess: Option<Option<SyntaxTextSpan>> = None;
    let mut attribute_types: BTreeMap<String, Type> = BTreeMap::new();
    let mut spreads = SpreadAttributes::default();

    for attribute in attributes {
        // `{...spread}` attributes carry no name; evaluate the argument and
        // record what it contributes so presence checks see spread members.
        if attribute.name.is_empty() {
            if let Some(value) = &attribute.value {
                let inferred = evaluate_expression(
                    value,
                    attribute.value_span.or(fallback_span),
                    symbols,
                    ctx,
                );
                match inferred {
                    InferredExpression::Known(ty) => match ty.peeled() {
                        Type::Object(object) => {
                            if object.allows_string_index_access() {
                                spreads.opaque = true;
                            }
                            for (name, property) in object.properties.iter() {
                                spreads
                                    .properties
                                    .insert(name.to_string(), property.clone());
                            }
                        }
                        _ => spreads.opaque = true,
                    },
                    _ => spreads.opaque = true,
                }
            }
            continue;
        }

        let expected_property =
            props_object.and_then(|object| object.get_property(&attribute.name));
        let contextual_type = expected_property.map(optional_aware_property_type);

        let attribute_type = infer_attribute_type(
            attribute,
            contextual_type.as_ref(),
            fallback_span,
            symbols,
            ctx,
        );

        if let Some(attribute_type) = &attribute_type {
            attribute_types.insert(attribute.name.clone(), attribute_type.clone());
        }

        let Some(object) = props_object else {
            continue;
        };

        match expected_property {
            Some(property) => {
                if let Some(attribute_type) = &attribute_type {
                    check_known_prop(attribute, attribute_type, property, fallback_span, ctx);
                }
            }
            None => {
                if let Some(index_type) = object.string_index_type.as_deref() {
                    if let Some(attribute_type) = &attribute_type {
                        let property = ObjectProperty::required(index_type.clone());
                        check_known_prop(attribute, attribute_type, &property, fallback_span, ctx);
                    }
                } else if attribute.name.contains('-')
                    || matches!(attribute.name.as_str(), "key" | "ref")
                {
                    // tsc never excess-checks hyphenated JSX attribute names
                    // (`data-slot`, `aria-*` — isKnownProperty), on components
                    // and intrinsics alike. `key` and `ref` are reserved by the
                    // JSX runtime and are removed from the props check the same
                    // way, so `<Item key={id} … />` is not an excess property.
                } else if first_excess.is_none() {
                    first_excess = Some(attribute.name_span);
                }
            }
        }
    }

    // An opaque spread folds the whole attributes object into `any` in tsc, so
    // an unmatched named attribute no longer reports there either.
    if let (Some(object), Some(excess_span), false) = (props_object, first_excess, spreads.opaque) {
        let source = source_object_name(&attribute_types);
        let target = Type::Object(object.clone()).name();
        ctx.push(diagnostic_with_syntax_span(
            Diagnostic::ts2322(&source, &target, ctx.file_name.clone()),
            excess_span.or(fallback_span),
        ));
    }

    spreads
}

/// Infers the type of a single attribute value: `string` for a string literal,
/// `true` for boolean shorthand, otherwise the contextually-typed expression.
/// Returns `None` when the value carries nothing checkable or did not resolve.
fn infer_attribute_type(
    attribute: &ParsedJsxAttribute,
    contextual_type: Option<&Type>,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    match &attribute.value_kind {
        ParsedJsxAttributeValueKind::StringLiteral(value) => {
            Some(Type::StringLiteral(value.clone()))
        }
        ParsedJsxAttributeValueKind::BooleanShorthand => Some(Type::BooleanLiteral(true)),
        ParsedJsxAttributeValueKind::Expression => {
            let value = attribute.value.as_ref()?;
            let inferred = evaluate_expression_with_expected_type(
                value,
                attribute.value_span.or(fallback_span),
                contextual_type,
                ExpectedTypeDiagnostic::TypeNotAssignable,
                symbols,
                ctx,
            );
            match inferred {
                InferredExpression::Known(ty) if !ty.is_unknown() => Some(ty),
                _ => None,
            }
        }
    }
}

/// An optional prop's type includes `undefined` without
/// `exactOptionalPropertyTypes`, both as the contextual type for its value and
/// as the target it is compared against.
fn optional_aware_property_type(property: &ObjectProperty) -> Type {
    if property.is_optional() {
        union_type(vec![property.ty.clone(), Type::Undefined])
    } else {
        property.ty.clone()
    }
}

/// Reports TS2322 when a known prop's value is not assignable to its declared
/// type, pointing at the attribute name (tsc's span for JSX prop mismatches).
fn check_known_prop(
    attribute: &ParsedJsxAttribute,
    attribute_type: &Type,
    property: &ObjectProperty,
    fallback_span: Option<SyntaxTextSpan>,
    ctx: &mut CheckerContext,
) {
    let expected_type = optional_aware_property_type(property);

    if attribute_type.is_unknown() {
        return;
    }

    if is_assignable_to(attribute_type, &expected_type) {
        return;
    }

    // tsc checks an attribute value as a mutable location
    // (`getWidenedLiteralLikeTypeForContextualType`): `disabled="yes"` against
    // `boolean` relates `string`, not the literal.
    let checked = crate::checks::function::widen_unit_return_type(
        attribute_type.clone(),
        Some(&expected_type),
    );
    let (source, target) = crate::checks::expr::disambiguated_pair(
        &checked,
        source_display_name(&checked, &expected_type),
        &expected_type,
        expected_type.name(),
        &ctx.file_name,
    );
    ctx.push(diagnostic_with_syntax_span(
        crate::checks::expr::type_not_assignable_diagnostic(
            attribute_type,
            &expected_type,
            &source,
            &target,
            ctx.file_name.clone(),
        ),
        attribute.name_span.or(fallback_span),
    ));
}

/// Reports TS2741 for the first required prop that is neither passed as an
/// attribute, contributed by a `{...spread}` whose type resolved, nor (for
/// `children`) supplied as element children. An opaque spread may contribute
/// anything, so the check stands down entirely.
fn check_missing_required_props(
    props_object: &ObjectType,
    attributes: &[ParsedJsxAttribute],
    spreads: &SpreadAttributes,
    children_provided: bool,
    tag_span: Option<SyntaxTextSpan>,
    fallback_span: Option<SyntaxTextSpan>,
    ctx: &mut CheckerContext,
) {
    if spreads.opaque {
        return;
    }

    let missing = props_object.required_properties().find(|(name, _)| {
        if name.as_ref() == "children" && children_provided {
            return false;
        }
        if spreads.properties.contains_key(name.as_ref()) {
            return false;
        }
        !attributes
            .iter()
            .any(|attribute| attribute.name == name.as_ref())
    });

    let Some((property_name, _)) = missing else {
        return;
    };

    let present = present_attribute_object_name(attributes, spreads);
    let target = Type::Object(props_object.clone()).name();
    ctx.push(diagnostic_with_syntax_span(
        Diagnostic::ts2741(property_name, &present, &target, ctx.file_name.clone()),
        tag_span.or(fallback_span),
    ));
}

/// Evaluates child expressions for ordinary diagnostics and, when the props type
/// declares `children`, checks a single child against the `children` type
/// (TS2745). Returns whether any content children were provided.
fn check_children(
    children: &[ParsedJsxChild],
    props_object: Option<&ObjectType>,
    tag_name_span: Option<SyntaxTextSpan>,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> bool {
    let mut content_children: Vec<ChildContent> = Vec::new();

    for child in children {
        match child {
            ParsedJsxChild::Text => content_children.push(ChildContent::Text),
            ParsedJsxChild::Expression { expression, span } => {
                if let Some(expression) = expression {
                    // The element's own props say nothing about a callback
                    // nested inside a child expression: `{items.map((item) =>
                    // …)}` takes its parameter types from `map`, not from this
                    // tag, and tsc reports it whatever the tag resolved to.
                    // Only a *render prop* — a function written directly as the
                    // child — is contextually typed by `props.children`, so the
                    // suppression is kept for that one shape.
                    // The suppression is cleared outright, not decremented:
                    // a child expression sits outside the props context of
                    // *every* enclosing element, and a nested tag raises the
                    // depth again, so stepping down by one left it elevated.
                    let release = !matches!(expression, ParsedExpression::ArrowFunction(_));
                    let saved_depth = ctx.unmodelled_jsx_props_depth;
                    if release {
                        ctx.unmodelled_jsx_props_depth = 0;
                    }
                    let inferred =
                        evaluate_expression(expression, span.or(fallback_span), symbols, ctx);
                    ctx.unmodelled_jsx_props_depth = saved_depth;
                    let child_type = match inferred {
                        InferredExpression::Known(ty) if !ty.is_unknown() => Some(ty),
                        _ => None,
                    };
                    content_children.push(ChildContent::Expression(child_type));
                }
            }
            ParsedJsxChild::Element(element) => {
                let _ = evaluate_expression(element, fallback_span, symbols, ctx);
                content_children.push(ChildContent::Element);
            }
        }
    }

    let children_provided = !content_children.is_empty();

    let Some(object) = props_object else {
        return children_provided;
    };
    let Some(children_property) = object.get_property("children") else {
        return children_provided;
    };
    if content_children.len() != 1 {
        return children_provided;
    }

    let expected = &children_property.ty;
    if type_contains_unknown_or_any(expected) {
        return children_provided;
    }

    if let ChildContent::Expression(Some(child_type)) = &content_children[0] {
        if !type_contains_unknown_or_any(child_type) && !is_assignable_to(child_type, expected) {
            ctx.push(diagnostic_with_syntax_span(
                Diagnostic::ts2745("children", expected.name(), ctx.file_name.clone()),
                tag_name_span.or(fallback_span),
            ));
        }
    }

    children_provided
}

enum ChildContent {
    Text,
    Expression(Option<Type>),
    Element,
}

/// The object-type name tsc shows for the attributes actually provided, used as
/// the source type in excess-prop (TS2322) diagnostics.
fn source_object_name(attribute_types: &BTreeMap<String, Type>) -> String {
    // tsc renders the excess-prop source with widened literals (`href: string`,
    // not `href: "/x"`), even though assignability keeps the literal.
    let properties = attribute_types
        .iter()
        .map(|(name, ty)| {
            let display_ty = match ty {
                Type::StringLiteral(_) => Type::String,
                Type::NumberLiteral(_) => Type::Number,
                Type::BooleanLiteral(_) => Type::Boolean,
                other => other.clone(),
            };
            (name.as_str().into(), ObjectProperty::required(display_ty))
        })
        .collect::<PropertyMap>();
    Type::Object(alloc_object_type(properties, None)).name()
}

/// The `'{1}'` argument of TS2741: the object type of the attributes already
/// present, including members contributed by resolved spreads (tsc renders the
/// spread source's members with their declared types), or `{}` when none are.
fn present_attribute_object_name(
    attributes: &[ParsedJsxAttribute],
    spreads: &SpreadAttributes,
) -> String {
    let properties = spreads
        .properties
        .iter()
        .map(|(name, property)| (name.as_str().into(), property.clone()))
        .chain(
            attributes
                .iter()
                .filter(|attribute| !attribute.name.is_empty())
                .map(|attribute| {
                    (
                        attribute.name.as_str().into(),
                        ObjectProperty::required(Type::Unknown),
                    )
                }),
        )
        .collect::<PropertyMap>();

    if properties.is_empty() {
        return "{}".to_string();
    }

    Type::Object(alloc_object_type(properties, None)).name()
}

fn type_contains_unknown_or_any(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Unknown | Type::GenuineUnknown | Type::TypeParameter(_) | Type::Any
    ) || matches!(ty, Type::Union(union) if union.types().iter().any(type_contains_unknown_or_any))
}
