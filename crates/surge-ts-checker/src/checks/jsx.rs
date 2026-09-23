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

/// tsc's `getJsxElementTypeAt`: a JSX element evaluates to the namespace's
/// `Element`, and to tsc's error type when the namespace declares none — a
/// fragment to `any` then (`checkJsxFragment`). `None` when surge cannot see
/// the namespace's `Element`.
pub(crate) fn jsx_expression_type(fragment: bool, ctx: &mut CheckerContext) -> Option<Type> {
    match jsx_namespace_member("Element", ctx) {
        JsxMember::Found(element) if !is_unmodelled_member(&element) => Some(element),
        JsxMember::Missing => Some(if fragment { Type::Any } else { Type::ErrorType }),
        JsxMember::Found(_) | JsxMember::Unmodelled => None,
    }
}

/// One candidate's props as tsc's JSX signature resolution relates the
/// attributes to them.
struct JsxProps {
    target: Type,
    /// The intersection's constituents, the namespace's intrinsic attributes
    /// first, when the props are joined with them.
    constituents: Vec<Type>,
    /// tsc's `reportErrorResults` names only the failing constituent, not the
    /// whole intersection, when that holds the namespace's intrinsic
    /// attributes and the namespace declares both kinds of them.
    reports_constituent: bool,
}

impl JsxProps {
    fn plain(target: Type) -> Self {
        JsxProps {
            target,
            constituents: Vec::new(),
            reports_constituent: false,
        }
    }
}

/// What a tag's attributes are checked against.
enum PropsResolution {
    /// tsc's `resolveCall` over the tag's JSX signatures: the props each
    /// candidate relates the attributes to, in declaration order, and the type
    /// that contextually types the attributes.
    Props {
        contextual: Type,
        candidates: Vec<JsxProps>,
    },
    /// tsc's error type or a non-component value: nothing to check against,
    /// and no contextual type for a callback attribute — tsc reports its
    /// parameters implicitly `any`.
    Unchecked,
    /// The props exist for tsc but surge could not model them, so an
    /// implicit-any report inside the attributes would describe that gap.
    Unmodelled,
}

/// Checks a JSX element the way tsc's `checkJsxOpeningLikeElementOrOpeningFragment`
/// resolves it: the tag to an intrinsic element or a component's signatures,
/// the attributes and children into one attributes object contextually typed by
/// the props, and that object related to each candidate's props. Attribute and
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
    let tag_name_span = tag.name_span.or(fallback_span);
    let has_children = children.iter().any(is_semantic_child);
    // Which attribute the body forms decides what the attributes object is;
    // when surge cannot tell, the element's props cannot be judged.
    let (children_name, children_unmodelled) = match jsx_children_property_name(ctx) {
        ChildrenPropertyName::Name(name) => (Some(name), false),
        ChildrenPropertyName::None => (None, false),
        ChildrenPropertyName::Unmodelled => (None, has_children),
    };
    let resolution = match &tag.expression {
        Some(expression) => resolve_component_props(expression, tag_name_span, symbols, ctx),
        None => intrinsic_element_props(tag_name, tag.span.or(fallback_span), ctx),
    };
    let (contextual, candidates, unmodelled_props) = match resolution {
        PropsResolution::Props { .. } | PropsResolution::Unmodelled if children_unmodelled => {
            (None, Vec::new(), true)
        }
        PropsResolution::Props {
            contextual,
            candidates,
        } => (Some(contextual), candidates, false),
        PropsResolution::Unchecked => (None, Vec::new(), false),
        PropsResolution::Unmodelled => (None, Vec::new(), true),
    };
    let overloaded = candidates.len() > 1;

    // A component whose props type collapsed to the degradation sentinel offers
    // no contextual type for an inline callback attribute, so any implicit-any
    // report inside those attributes would describe surge's modelling gap rather
    // than the source. Suppress it for the element's attributes and children, the
    // same no-cascade rule a sentinel receiver gets elsewhere.
    if unmodelled_props {
        ctx.unmodelled_jsx_props_depth += 1;
    }
    // Overload candidates are tried against the attributes one at a time, so a
    // value is only contextually typed while they are evaluated; the report is
    // the chosen candidate's, or TS2769.
    let value_diagnostic = if overloaded {
        ExpectedTypeDiagnostic::ContextOnly
    } else {
        ExpectedTypeDiagnostic::TypeNotAssignable
    };
    let evaluated = evaluate_attributes(
        attributes,
        contextual.as_ref(),
        value_diagnostic,
        fallback_span,
        symbols,
        ctx,
    );
    let children_attribute =
        evaluate_children(children, &tag.child_spans, fallback_span, symbols, ctx);
    if unmodelled_props {
        ctx.unmodelled_jsx_props_depth -= 1;
    }

    let children_attribute = children_name
        .as_deref()
        .filter(|_| has_children)
        .map(|name| (name, children_attribute));
    match candidates.as_slice() {
        [] => {}
        [props] => {
            for (diagnostic, span) in relate_attributes(
                &evaluated,
                children_attribute.as_ref(),
                props,
                tag_name_span,
                ctx,
            ) {
                ctx.push(diagnostic_with_syntax_span(diagnostic, span));
            }
        }
        [.., last] => {
            let chosen = candidates.iter().any(|props| {
                relate_attributes(
                    &evaluated,
                    children_attribute.as_ref(),
                    props,
                    tag_name_span,
                    ctx,
                )
                .is_empty()
            });
            if !chosen {
                // tsc reports the last candidate's errors, each as "No overload
                // matches this call".
                for (_, span) in relate_attributes(
                    &evaluated,
                    children_attribute.as_ref(),
                    last,
                    tag_name_span,
                    ctx,
                ) {
                    ctx.push(diagnostic_with_syntax_span(
                        Diagnostic::ts2769(ctx.file_name.clone()),
                        span,
                    ));
                }
            }
        }
    }

    if let Some(closing) = &tag.closing {
        let span = closing.span.or(fallback_span);
        match &closing.expression {
            Some(expression) => {
                let _ = evaluate_expression(expression, span, symbols, ctx);
            }
            None => {
                let _ = intrinsic_element_props(tag_name, span, ctx);
            }
        }
    }
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
    let component_type = match evaluate_expression(expression, span, symbols, ctx) {
        InferredExpression::Known(component_type) => component_type,
        // A value surge could not type at all is its own gap: tsc has the
        // component's props and types the callbacks from them.
        InferredExpression::Unknown => return PropsResolution::Unmodelled,
        // An unresolved name or member is tsc's error type. Its
        // attributes get no contextual signature (`getContextualSignature`
        // of `any` is undefined), so tsc *does* report a callback in them
        // as implicit `any` — leave them unsuppressed.
        InferredExpression::UnresolvedIdentifier { .. }
        | InferredExpression::MissingProperty { .. } => return PropsResolution::Unchecked,
    };
    let Some((signatures, construct)) = jsx_signatures(&component_type) else {
        return if component_is_unmodelled(&component_type) {
            PropsResolution::Unmodelled
        } else {
            PropsResolution::Unchecked
        };
    };
    let mut candidates = Vec::with_capacity(signatures.len());
    for signature in &signatures {
        let declared = if construct {
            class_component_props(signature, ctx)
        } else {
            signature
                .parameters()
                .first()
                .cloned()
                .unwrap_or(Type::GenuineUnknown)
        };
        if is_unmodelled_props(&declared) {
            return PropsResolution::Unmodelled;
        }
        candidates.push(with_intrinsic_attributes(
            declared,
            construct.then(|| signature.return_type()),
            ctx,
        ));
    }
    let contextual = match candidates.as_slice() {
        [single] => single.target.clone(),
        _ => union_type(candidates.iter().map(|props| props.target.clone()).collect()),
    };
    PropsResolution::Props {
        contextual,
        candidates,
    }
}

/// A JSX namespace member surge resolved without its members: the degradation
/// sentinel, or an object an unmodelled operand left open.
fn is_unmodelled_member(member: &Type) -> bool {
    match member.peeled() {
        Type::Object(object) => object.synthetic_open_index,
        other => is_unmodelled_props(&other),
    }
}

/// Props surge could not model: its degradation sentinel, or a bare type
/// parameter nothing instantiated. The `unknown` keyword and tsc's error type
/// are props of their own.
fn is_unmodelled_props(props: &Type) -> bool {
    matches!(props.peeled(), Type::Unknown | Type::TypeParameter(_))
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

/// tsc's `getUninstantiatedJsxSignaturesOfType`: a component's construct
/// signatures when it has any, else its call signatures, each overload a
/// candidate. The flag says which kind they are.
fn jsx_signatures(component_type: &Type) -> Option<(Vec<FunctionType>, bool)> {
    let peeled = component_type.peeled();
    let (signature, construct) = match &peeled {
        Type::Function(function_type) => (function_type.clone(), false),
        Type::Object(object) => match object.construct_signature() {
            Some(construct_signature) => (construct_signature.clone(), true),
            None => (object.call_signature()?.clone(), false),
        },
        _ => return None,
    };
    let mut signatures = Vec::new();
    signature.push_overload_members(&mut signatures);
    Some((signatures, construct))
}

/// tsc's `getJsxPropsTypeFromClassType` before the intrinsic attributes are
/// added: the instance's `ElementAttributesProperty` member, the instance
/// itself when that interface declares no member, and the first constructor
/// parameter when the JSX namespace has no such interface.
fn class_component_props(signature: &FunctionType, ctx: &mut CheckerContext) -> Type {
    let first_parameter = || {
        signature
            .parameters()
            .first()
            .cloned()
            .unwrap_or(Type::GenuineUnknown)
    };
    match jsx_attributes_container_member("ElementAttributesProperty", ctx) {
        ContainerMember::Missing => first_parameter(),
        ContainerMember::Unmodelled => Type::Unknown,
        ContainerMember::Empty => signature.return_type().clone(),
        ContainerMember::Name(name) => match signature.return_type().peeled() {
            Type::Object(instance) => instance
                .get_property(&name)
                .map_or(Type::GenuineUnknown, |property| property.ty.clone()),
            Type::Any => Type::Any,
            other if other.is_unknown() => other,
            _ => Type::GenuineUnknown,
        },
    }
}

/// tsc's `getJsxPropsTypeFromCallSignature` / `getJsxPropsTypeFromClassType`
/// tail: `IntrinsicAttributes & IntrinsicClassAttributes<Instance> & Props`,
/// each part only when the JSX namespace declares it. `any` props stay `any`
/// for a class component.
fn with_intrinsic_attributes(
    props: Type,
    class_instance: Option<&Type>,
    ctx: &mut CheckerContext,
) -> JsxProps {
    if class_instance.is_some() && matches!(props.peeled(), Type::Any) {
        return JsxProps::plain(props);
    }
    let intrinsic_attributes = match jsx_namespace_member("IntrinsicAttributes", ctx) {
        JsxMember::Found(intrinsic_attributes) if !intrinsic_attributes.is_unknown() => {
            Some(intrinsic_attributes)
        }
        _ => None,
    };
    let class_attributes =
        class_instance.and_then(|instance| intrinsic_class_attributes(instance, ctx));
    if intrinsic_attributes.is_none() && class_attributes.is_none() {
        return JsxProps::plain(props);
    }
    let both_declared = intrinsic_attributes.is_some()
        && matches!(
            jsx_namespace_member("IntrinsicClassAttributes", ctx),
            JsxMember::Found(_)
        );
    let intrinsic_parts: Vec<Type> = intrinsic_attributes.into_iter().chain(class_attributes).collect();
    let target = crate::infer::types::merge_intersection_members(
        intrinsic_parts
            .iter()
            .cloned()
            .chain(std::iter::once(props.clone()))
            .collect(),
    );
    let mut constituents = intrinsic_parts;
    match props.peeled() {
        Type::Object(object) if object.is_intersection => match object.intersection_operands.as_deref() {
            Some(operands) if !operands.is_empty() => constituents.extend(operands.iter().cloned()),
            _ => constituents.push(props),
        },
        _ => constituents.push(props),
    }
    JsxProps {
        target,
        constituents,
        reports_constituent: both_declared,
    }
}

/// `IntrinsicClassAttributes<Instance>`, instantiated with the class
/// component's instance type when the interface is generic.
fn intrinsic_class_attributes(instance: &Type, ctx: &mut CheckerContext) -> Option<Type> {
    let (prefix, scope) = match jsx_namespace(ctx) {
        JsxNamespace::Qualified(prefix) => (prefix, None),
        JsxNamespace::Declared { scope, prefix } => (prefix, Some(scope)),
        JsxNamespace::Unmodelled | JsxNamespace::Missing => return None,
    };
    let name = format!("{prefix}.IntrinsicClassAttributes");
    let instance = instance.clone();
    let resolve = |ctx: &mut CheckerContext| {
        let parameter_count = match ctx.lookup_type_declaration(&name)? {
            crate::symbols::TypeDeclarationInfo::Interface(info) => info.body.type_parameters.len(),
            crate::symbols::TypeDeclarationInfo::Alias(info) => info.body.type_parameters.len(),
        };
        // The instance fills the first parameter; tsc defaults the others,
        // which only a single-parameter declaration spares surge from doing.
        let resolved = match parameter_count {
            0 => map_parsed_type(named_type(&name, Vec::new()), ctx),
            1 => {
                let slot = instance.name();
                let mut substitution = crate::infer::TypeParameterSubstitution::new();
                substitution.insert(slot.clone(), instance.clone());
                crate::infer::map_parsed_type_with_substitution(
                    named_type(&name, vec![named_type(&slot, Vec::new())]),
                    ctx,
                    &substitution,
                )
            }
            _ => return None,
        };
        (!resolved.is_unknown()).then_some(resolved)
    };
    match scope {
        Some(scope) => crate::infer::with_type_declaration_scope(&Some(scope), ctx, resolve),
        None => resolve(ctx),
    }
}

fn named_type(name: &str, type_arguments: Vec<ParsedType>) -> ParsedType {
    ParsedType::Named(Arc::new(ParsedNamedType {
        name: name.to_string(),
        span: None,
        type_arguments,
    }))
}

/// What `ElementAttributesProperty` / `ElementChildrenAttribute` name (tsc's
/// `getNameFromJsxElementAttributesContainer`).
enum ContainerMember {
    /// The namespace does not declare the interface, or it declares several
    /// members.
    Missing,
    /// It declares no member.
    Empty,
    Name(String),
    /// surge could not enumerate the interface's members.
    Unmodelled,
}

fn jsx_attributes_container_member(container: &str, ctx: &mut CheckerContext) -> ContainerMember {
    let declared = match jsx_namespace_member(container, ctx) {
        JsxMember::Found(declared) => declared,
        JsxMember::Missing => return ContainerMember::Missing,
        JsxMember::Unmodelled => return ContainerMember::Unmodelled,
    };
    let Type::Object(object) = declared.peeled() else {
        return ContainerMember::Unmodelled;
    };
    if object.synthetic_open_index {
        return ContainerMember::Unmodelled;
    }
    let mut names = object.properties.keys();
    match (names.next(), names.next()) {
        (None, _) => ContainerMember::Empty,
        (Some(name), None) => ContainerMember::Name(name.to_string()),
        (Some(_), Some(_)) => ContainerMember::Missing,
    }
}

/// tsc's `getJsxElementChildrenPropertyName`: fixed to `children` under the
/// automatic runtime, else the `ElementChildrenAttribute` member. Without a
/// name the element's body is not an attribute at all.
enum ChildrenPropertyName {
    Name(String),
    None,
    Unmodelled,
}

fn jsx_children_property_name(ctx: &mut CheckerContext) -> ChildrenPropertyName {
    if ctx.options.jsx_automatic_runtime {
        return ChildrenPropertyName::Name("children".to_string());
    }
    match jsx_attributes_container_member("ElementChildrenAttribute", ctx) {
        ContainerMember::Name(name) => ChildrenPropertyName::Name(name),
        ContainerMember::Missing | ContainerMember::Empty => ChildrenPropertyName::None,
        ContainerMember::Unmodelled => ChildrenPropertyName::Unmodelled,
    }
}

/// tsc's `getIntrinsicTagSymbol` and `getIntrinsicAttributesTypeFromJsxOpeningLikeElement`:
/// the props `IntrinsicElements` declares for `tag_name`, reported at `node_span`
/// as TS2339 when it declares none and as TS7026 when the JSX namespace has no
/// `IntrinsicElements` at all. The attributes relate to those props as they
/// are, but are contextually typed by them joined with `IntrinsicAttributes`
/// (the intrinsic element's fake signature).
fn intrinsic_element_props(
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

    let props = if let Some(property_type) = object.get_property_type(tag_name) {
        property_type.clone()
    } else if object.synthetic_open_index {
        // The index stands for members surge could not enumerate.
        return PropsResolution::Unmodelled;
    } else if let Some(index_type) = object.string_index_type.as_deref() {
        index_type.clone()
    } else {
        ctx.push(diagnostic_with_syntax_span(
            Diagnostic::ts2339(tag_name, "JSX.IntrinsicElements", ctx.file_name.clone()),
            node_span,
        ));
        return PropsResolution::Unchecked;
    };
    if is_unmodelled_props(&props) {
        return PropsResolution::Unmodelled;
    }
    PropsResolution::Props {
        contextual: with_intrinsic_attributes(props.clone(), None, ctx).target,
        candidates: vec![JsxProps::plain(props)],
    }
}

/// tsc's `isHyphenatedJsxName`: `data-*`/`aria-*` style attributes are never
/// excess and never checked against an index signature.
fn is_hyphenated_jsx_name(name: &str) -> bool {
    name.contains('-')
}

/// An explicit attribute's value as the attributes object carries it (tsc's
/// `checkJsxAttribute`): the value's type for a mutable location, `true` for
/// the shorthand, or `None` when surge could not type it.
struct AttributeValue<'a> {
    attribute: &'a ParsedJsxAttribute,
    ty: Option<Type>,
}

/// tsc's `createJsxAttributesTypeFromAttributesProperty`, as far as the checks
/// read it.
#[derive(Default)]
struct EvaluatedAttributes<'a> {
    explicit: Vec<AttributeValue<'a>>,
    /// What the `{...spread}` attributes contribute, a later one overriding an
    /// earlier one.
    spread: PropertyMap,
    /// A spread of `any`: tsc types the whole attributes object `any`.
    any_spread: bool,
    /// A spread whose members surge cannot enumerate: the attributes object
    /// may have any property, so none can be reported missing.
    opaque_spread: bool,
}

/// The children attribute tsc synthesizes from the element's body.
struct ChildrenAttribute {
    ty: Option<Type>,
    /// The lone semantic child: where a mismatch against the children's type
    /// is reported and what it was.
    single: Option<(ChildKind, Option<SyntaxTextSpan>)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ChildKind {
    Text,
    Expression,
    Element,
}

fn evaluate_attributes<'a>(
    attributes: &'a [ParsedJsxAttribute],
    contextual: Option<&Type>,
    value_diagnostic: ExpectedTypeDiagnostic,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> EvaluatedAttributes<'a> {
    let mut evaluated = EvaluatedAttributes::default();
    for attribute in attributes {
        // `{...spread}` attributes carry no name.
        if attribute.name.is_empty() {
            let Some(value) = &attribute.value else {
                continue;
            };
            let spread =
                evaluate_expression(value, attribute.value_span.or(fallback_span), symbols, ctx);
            match spread {
                InferredExpression::Known(ty) => match ty.peeled() {
                    Type::Any | Type::ErrorType => evaluated.any_spread = true,
                    Type::Object(object) => {
                        if object.allows_string_index_access() {
                            evaluated.opaque_spread = true;
                        }
                        for (name, property) in object.properties.iter() {
                            evaluated.spread.shift_remove(name);
                            evaluated.spread.insert(name.clone(), property.clone());
                        }
                    }
                    _ => evaluated.opaque_spread = true,
                },
                _ => evaluated.opaque_spread = true,
            }
            continue;
        }

        let contextual_type =
            contextual.and_then(|contextual| attribute_contextual_type(contextual, &attribute.name));
        let ty = attribute_value_type(
            attribute,
            contextual_type.as_ref(),
            value_diagnostic,
            fallback_span,
            symbols,
            ctx,
        );
        evaluated.explicit.push(AttributeValue { attribute, ty });
    }
    evaluated
}

/// tsc's `checkJsxAttribute`: a string-literal value and an `{expression}`
/// are checked for a mutable location, so a literal widens unless the
/// contextual type holds literals of its kind; the shorthand is `true`.
fn attribute_value_type(
    attribute: &ParsedJsxAttribute,
    contextual_type: Option<&Type>,
    value_diagnostic: ExpectedTypeDiagnostic,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    match &attribute.value_kind {
        ParsedJsxAttributeValueKind::StringLiteral(value) => Some(
            crate::checks::function::widen_unit_return_type(
                Type::StringLiteral(value.clone()),
                contextual_type,
            ),
        ),
        ParsedJsxAttributeValueKind::BooleanShorthand => Some(Type::BooleanLiteral(true)),
        ParsedJsxAttributeValueKind::Expression => {
            let value = attribute.value.as_ref()?;
            let inferred = evaluate_expression_with_expected_type(
                value,
                attribute.value_span.or(fallback_span),
                contextual_type,
                value_diagnostic,
                symbols,
                ctx,
            );
            match inferred {
                InferredExpression::Known(ty) if !ty.is_unknown() => Some(
                    crate::checks::function::widen_unit_return_type(ty, contextual_type),
                ),
                _ => None,
            }
        }
    }
}

/// tsc's `getTypeOfPropertyOfContextualType` for an attribute: a member's
/// declared type (with `undefined` when optional) or its index signature's,
/// joined over a union's members. `any` offers none
/// (`getContextualTypeForJsxAttribute`).
fn attribute_contextual_type(contextual: &Type, name: &str) -> Option<Type> {
    match contextual.peeled() {
        Type::Union(union) => {
            let members: Vec<Type> = union
                .types()
                .iter()
                .filter_map(|member| attribute_contextual_type(member, name))
                .collect();
            (!members.is_empty()).then(|| union_type(members))
        }
        Type::Object(object) => match object.get_property(name) {
            Some(property) => Some(optional_aware_property_type(property)),
            None => object
                .applicable_index_type(surge_ts_types::is_numeric_key(name))
                .cloned(),
        },
        _ => None,
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

/// tsc's `discriminateTypeByDiscriminableItems`: each discriminator drops the
/// members whose property does not admit its value, unless none admits it.
/// `None` when nothing is dropped.
fn discriminate_members(members: &[Type], discriminators: &[(String, Type)]) -> Option<Vec<Type>> {
    #[derive(Clone, Copy, PartialEq)]
    enum Include {
        Yes,
        Maybe,
        No,
    }
    let mut include: Vec<Include> = members
        .iter()
        .map(|member| match member.peeled() {
            Type::Object(_) | Type::Function(_) | Type::Array(_) | Type::Tuple(_) => Include::Yes,
            _ => Include::No,
        })
        .collect();
    for (name, value) in discriminators {
        let mut matched = false;
        for (index, member) in members.iter().enumerate() {
            if include[index] == Include::No {
                continue;
            }
            let Some(target) = member_property_type(member, name) else {
                continue;
            };
            let admits = match value {
                Type::Union(value) => value
                    .types()
                    .iter()
                    .any(|constituent| is_assignable_to(constituent, &target)),
                value => is_assignable_to(value, &target),
            };
            if admits {
                matched = true;
            } else {
                include[index] = Include::Maybe;
            }
        }
        for state in include.iter_mut() {
            if *state == Include::Maybe {
                *state = if matched { Include::No } else { Include::Yes };
            }
        }
    }
    if !include.contains(&Include::No) {
        return None;
    }
    let remaining: Vec<Type> = members
        .iter()
        .zip(&include)
        .filter(|(_, state)| **state == Include::Yes)
        .map(|(member, _)| member.clone())
        .collect();
    (!remaining.is_empty()).then_some(remaining)
}

/// A union member's property type as `getTypeOfPropertyOrIndexSignatureOfType`
/// reads it.
fn member_property_type(member: &Type, name: &str) -> Option<Type> {
    let Type::Object(object) = member.peeled() else {
        return None;
    };
    match object.get_property(name) {
        Some(property) => Some(optional_aware_property_type(property)),
        None => object
            .applicable_index_type(surge_ts_types::is_numeric_key(name))
            .cloned(),
    }
}

/// tsc's `isDiscriminantProperty`: the union's members declare the property
/// with differing types, one of them a literal.
fn is_discriminant_property(members: &[Type], name: &str) -> bool {
    let types: Vec<Type> = members
        .iter()
        .filter_map(|member| {
            let Type::Object(object) = member.peeled() else {
                return None;
            };
            object.get_property(name).map(|property| property.ty.clone())
        })
        .collect();
    let Some(first) = types.first() else {
        return false;
    };
    types.iter().any(|ty| ty != first) && types.iter().any(is_literal_type)
}

/// tsc's `isLiteralType`: `boolean`, a unit type, or a union of unit types.
fn is_literal_type(ty: &Type) -> bool {
    let is_unit = |ty: &Type| {
        matches!(
            ty,
            Type::StringLiteral(_)
                | Type::NumberLiteral(_)
                | Type::BooleanLiteral(_)
                | Type::Undefined
                | Type::Null
        )
    };
    match ty {
        Type::Boolean => true,
        Type::Union(union) => union.types().iter().all(is_unit),
        other => is_unit(other),
    }
}

/// tsc's `GetSemanticJsxChildren` entry test: an empty `{}` container is no
/// child. Whitespace-only text never reaches the checker.
fn is_semantic_child(child: &ParsedJsxChild) -> bool {
    !matches!(
        child,
        ParsedJsxChild::Expression {
            expression: None,
            ..
        }
    )
}

/// tsc's `checkJsxChildren`, and the children attribute the children form: the
/// lone child's type, else an array of their union.
fn evaluate_children(
    children: &[ParsedJsxChild],
    child_spans: &[Option<SyntaxTextSpan>],
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> ChildrenAttribute {
    let semantic: Vec<(&ParsedJsxChild, Option<SyntaxTextSpan>)> = children
        .iter()
        .enumerate()
        .filter(|(_, child)| is_semantic_child(child))
        .map(|(index, child)| (child, child_spans.get(index).copied().flatten()))
        .collect();
    let element_type = match semantic
        .iter()
        .any(|(child, _)| matches!(child, ParsedJsxChild::Element(_)))
    {
        true => match jsx_namespace_member("Element", ctx) {
            JsxMember::Found(element) if !element.is_unknown() => Some(element),
            _ => None,
        },
        false => None,
    };

    let mut types: Vec<Option<Type>> = Vec::with_capacity(semantic.len());
    let mut single = None;
    for &(child, child_span) in &semantic {
        let (kind, ty) = match child {
            ParsedJsxChild::Text => (ChildKind::Text, Some(Type::String)),
            ParsedJsxChild::Expression { expression, span } => {
                let Some(expression) = expression else {
                    continue;
                };
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
                let ty = match inferred {
                    InferredExpression::Known(ty) if !ty.is_unknown() => {
                        Some(crate::checks::function::widen_unit_return_type(ty, None))
                    }
                    _ => None,
                };
                (ChildKind::Expression, ty)
            }
            ParsedJsxChild::Element(element) => {
                let _ = evaluate_expression(element, fallback_span, symbols, ctx);
                (ChildKind::Element, element_type.clone())
            }
        };
        if semantic.len() == 1 {
            single = Some((kind, child_span));
        }
        types.push(ty);
    }

    let ty = match types.as_slice() {
        [] => None,
        [single] => single.clone(),
        many => many
            .iter()
            .cloned()
            .collect::<Option<Vec<Type>>>()
            .map(|types| Type::Array(Box::new(union_type(types)))),
    };
    ChildrenAttribute { ty, single }
}

/// tsc's `checkApplicableSignatureForJsxCallLikeElement` relation of the
/// attributes object to one candidate's props, as the diagnostics it reports:
/// every explicit attribute whose value does not fit its prop
/// (`elaborateJsxComponents`), else the children against the children prop,
/// else the first excess attribute, else the missing required props.
fn relate_attributes(
    evaluated: &EvaluatedAttributes<'_>,
    children: Option<&(&str, ChildrenAttribute)>,
    props: &JsxProps,
    tag_name_span: Option<SyntaxTextSpan>,
    ctx: &CheckerContext,
) -> Vec<(Diagnostic, Option<SyntaxTextSpan>)> {
    let target = props.target.peeled();
    if evaluated.any_spread || target.is_unknown() || matches!(target, Type::Any) {
        return Vec::new();
    }

    let mut elaborated = Vec::new();
    for value in &evaluated.explicit {
        let name = value.attribute.name.as_str();
        if is_hyphenated_jsx_name(name) {
            continue;
        }
        let (Some(ty), Some(expected)) = (&value.ty, attribute_target_type(&target, name)) else {
            continue;
        };
        if type_contains_unknown_or_any(&expected) || is_assignable_to(ty, &expected) {
            continue;
        }
        elaborated.push((
            relation_diagnostic(ty, &expected, ctx),
            value.attribute.name_span.or(tag_name_span),
        ));
    }
    if let Some((name, children)) = children
        && let Some(diagnostic) = relate_children(name, children, &target, tag_name_span, ctx)
    {
        elaborated.push(diagnostic);
    }
    if !elaborated.is_empty() {
        return elaborated;
    }

    let source_name = || attributes_object_name(evaluated, children);
    // Only a written attribute makes the attributes object a fresh literal
    // (`createJsxAttributesType`); spreads and children alone are never
    // checked for excess properties.
    if !evaluated.explicit.is_empty() && is_excess_property_check_target(&target) {
        let known_in = excess_check_members(&target, evaluated);
        let excess = evaluated
            .explicit
            .iter()
            .map(|value| (value.attribute.name.as_str(), value.attribute.name_span))
            .chain(children.map(|(name, _)| (*name, None)))
            .find(|(name, _)| {
                !is_hyphenated_jsx_name(name)
                    && !known_in.iter().any(|member| is_known_property(member, name))
            });
        if let Some((_, span)) = excess {
            return vec![(
                Diagnostic::ts2322(source_name(), props.target.name(), ctx.file_name.clone()),
                span.or(tag_name_span),
            )];
        }
    }

    let Type::Object(object) = &target else {
        return Vec::new();
    };
    if evaluated.opaque_spread {
        return Vec::new();
    }
    let present = |name: &str| {
        evaluated
            .explicit
            .iter()
            .any(|value| value.attribute.name == name)
            || evaluated.spread.contains_key(name)
            || children.is_some_and(|(children_name, _)| *children_name == name)
            || surge_ts_types::object_prototype_member_type(name).is_some()
    };
    let missing_from = |object: &ObjectType| -> Vec<String> {
        object
            .required_properties()
            .filter(|(name, _)| !present(name))
            .map(|(name, _)| name.to_string())
            .collect()
    };
    let missing = missing_from(object);
    let target_name = props.target.name();
    let Some(first) = missing.first() else {
        // A hyphenated attribute is never elaborated, but a prop the target
        // declares under its name still has to fit; the whole object is
        // reported then.
        let hyphenated_mismatch = evaluated.explicit.iter().any(|value| {
            is_hyphenated_jsx_name(&value.attribute.name)
                && value.ty.as_ref().is_some_and(|ty| {
                    object.get_property(&value.attribute.name).is_some_and(|property| {
                        let expected = optional_aware_property_type(property);
                        !type_contains_unknown_or_any(&expected) && !is_assignable_to(ty, &expected)
                    })
                })
        });
        if hyphenated_mismatch {
            return vec![(
                Diagnostic::ts2322(source_name(), target_name, ctx.file_name.clone()),
                tag_name_span,
            )];
        }
        return Vec::new();
    };
    if !object.is_intersection {
        return vec![(
            crate::checks::expr::missing_properties_diagnostic(
                first,
                &missing,
                &source_name(),
                &target_name,
                &ctx.file_name,
            ),
            tag_name_span,
        )];
    }
    if props.reports_constituent {
        // The intersection is related one constituent at a time, and the
        // first that fails is what tsc names.
        for constituent in &props.constituents {
            let Type::Object(constituent_object) = constituent.peeled() else {
                continue;
            };
            let missing = missing_from(&constituent_object);
            if let Some(first) = missing.first() {
                return vec![(
                    crate::checks::expr::missing_properties_diagnostic(
                        first,
                        &missing,
                        &source_name(),
                        &constituent.name(),
                        &ctx.file_name,
                    ),
                    tag_name_span,
                )];
            }
        }
    }
    vec![(
        Diagnostic::ts2322(source_name(), target_name, ctx.file_name.clone()),
        tag_name_span,
    )]
}

/// A value's relation failure against `expected`, named the way tsc's
/// `isRelatedTo` reports it: a non-nullable source against `T | undefined`
/// names `T`.
fn relation_diagnostic(source: &Type, expected: &Type, ctx: &CheckerContext) -> Diagnostic {
    let reported = crate::checks::expr::reported_relation_target(source, expected);
    let (source_name, target_name) = crate::checks::expr::disambiguated_pair(
        source,
        source_display_name(source, &reported),
        &reported,
        reported.name(),
        &ctx.file_name,
    );
    crate::checks::expr::type_not_assignable_diagnostic(
        source,
        &reported,
        &source_name,
        &target_name,
        ctx.file_name.clone(),
    )
}

/// The members an attribute has to be known in (tsc's `hasExcessProperties`):
/// a union target is first narrowed by the discriminants the attributes write
/// (`findMatchingDiscriminantType`).
fn excess_check_members(target: &Type, evaluated: &EvaluatedAttributes<'_>) -> Vec<Type> {
    let Type::Union(union) = target else {
        return vec![target.clone()];
    };
    let members = union.types().to_vec();
    let discriminators: Vec<(String, Type)> = evaluated
        .explicit
        .iter()
        .filter_map(|value| Some((value.attribute.name.clone(), value.ty.clone()?)))
        .filter(|(name, _)| is_discriminant_property(&members, name))
        .collect();
    discriminate_members(&members, &discriminators).unwrap_or(members)
}

/// The type an attribute's value relates to (tsc's
/// `getIndexedAccessTypeOrUndefined` on the props): the prop's type, with
/// `undefined` when it is optional, or the applicable index signature's.
fn attribute_target_type(target: &Type, name: &str) -> Option<Type> {
    let Type::Object(object) = target else {
        return None;
    };
    match object.get_property(name) {
        Some(property) => Some(optional_aware_property_type(property)),
        None => object
            .applicable_index_type(surge_ts_types::is_numeric_key(name))
            .cloned(),
    }
}

/// The children branch of tsc's `elaborateJsxComponents`: a lone child that
/// does not fit a children type with a member that is not iterable is
/// reported at the child; against an iterable-only children type it is TS2745
/// at the tag.
fn relate_children(
    name: &str,
    children: &ChildrenAttribute,
    target: &Type,
    tag_name_span: Option<SyntaxTextSpan>,
    ctx: &CheckerContext,
) -> Option<(Diagnostic, Option<SyntaxTextSpan>)> {
    let expected = attribute_target_type(target, name)?;
    let (kind, span) = children.single?;
    let child_type = children.ty.as_ref()?;
    if type_contains_unknown_or_any(&expected)
        || type_contains_unknown_or_any(child_type)
        || is_assignable_to(child_type, &expected)
    {
        return None;
    }
    let iterable_declared = ctx.lookup_type_declaration("Iterable").is_some();
    if !has_non_array_like_member(&expected, iterable_declared) {
        return Some((
            Diagnostic::ts2745(name, expected.name(), ctx.file_name.clone()),
            tag_name_span,
        ));
    }
    // A text child's mismatch is TS2747, which surge does not report.
    if kind == ChildKind::Text {
        return None;
    }
    Some((relation_diagnostic(child_type, &expected, ctx), span.or(tag_name_span)))
}

/// Whether some member of a children type is not array-like: tsc splits the
/// type by assignability to `Iterable<any>` when the lib declares `Iterable`,
/// and by being an array or tuple otherwise.
fn has_non_array_like_member(ty: &Type, iterable_declared: bool) -> bool {
    match ty.peeled() {
        Type::Union(union) => union
            .types()
            .iter()
            .any(|member| has_non_array_like_member(member, iterable_declared)),
        Type::Array(_) | Type::Tuple(_) | Type::OpenTuple(_) => false,
        Type::String | Type::StringLiteral(_) => !iterable_declared,
        Type::Object(object) => {
            !(iterable_declared
                && object
                    .get_property(surge_ts_types::ITERATION_PROTOCOL_MEMBER)
                    .is_some())
        }
        _ => true,
    }
}

/// tsc's `isExcessPropertyCheckTarget`.
fn is_excess_property_check_target(target: &Type) -> bool {
    match target {
        Type::Object(object) => object.alias_name.as_deref() != Some("Object"),
        Type::Union(union) => union
            .types()
            .iter()
            .any(|member| is_excess_property_check_target(&member.peeled())),
        _ => false,
    }
}

/// tsc's `isKnownProperty` for JSX attributes: declared by the props, answered
/// by an index signature, or declared by any member of a union.
fn is_known_property(target: &Type, name: &str) -> bool {
    match target {
        Type::Object(object) => object.contains_property(name),
        Type::Union(union) => union
            .types()
            .iter()
            .any(|member| is_known_property(&member.peeled(), name)),
        _ => true,
    }
}

/// The attributes object as tsc displays it in a relation error: the explicit
/// attributes in order with their values' types, the spreads' members, and
/// the children.
fn attributes_object_name(
    evaluated: &EvaluatedAttributes<'_>,
    children: Option<&(&str, ChildrenAttribute)>,
) -> String {
    let mut properties = evaluated.spread.clone();
    for value in &evaluated.explicit {
        let name: Arc<str> = value.attribute.name.as_str().into();
        properties.shift_remove(&name);
        properties.insert(
            name,
            ObjectProperty::required(value.ty.clone().unwrap_or(Type::Any)),
        );
    }
    if let Some((name, children)) = children {
        properties.insert(
            (*name).into(),
            ObjectProperty::required(children.ty.clone().unwrap_or(Type::Any)),
        );
    }
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
