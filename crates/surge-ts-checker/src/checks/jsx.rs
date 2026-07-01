use std::collections::BTreeMap;

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{
    ParsedExpression, ParsedJsxAttribute, ParsedJsxAttributeValueKind, ParsedJsxChild,
    ParsedNamedType, ParsedType, TextSpan as SyntaxTextSpan,
};
use surge_ts_types::{
    FunctionType, ObjectProperty, ObjectType, PropertyMap, Type, is_assignable_to, union_type,
};

use super::expected::{ExpectedTypeDiagnostic, evaluate_expression_with_expected_type};
use super::expr::{evaluate_expression, source_display_name};
use crate::arena::alloc_object_type;
use crate::context::CheckerContext;
use crate::infer::{InferredExpression, map_parsed_type};
use crate::spans::diagnostic_with_syntax_span;
use crate::symbols::SymbolTable;

/// Checks a JSX element: resolves the tag to an intrinsic element or function
/// component, lowers attributes into a props object, and reports missing,
/// excess, and mistyped props plus basic `children` mismatches. Attribute and
/// child expressions are always evaluated for ordinary diagnostics (e.g. an
/// unresolved name in `{expr}`), even when the tag itself does not resolve, so a
/// missing component never cascades into a prop-checking storm.
#[allow(clippy::too_many_arguments)]
pub(crate) fn check_jsx_element(
    tag_name: &str,
    tag_name_span: Option<SyntaxTextSpan>,
    component_name: Option<&str>,
    component_span: Option<SyntaxTextSpan>,
    element_span: Option<SyntaxTextSpan>,
    attributes: &[ParsedJsxAttribute],
    children: &[ParsedJsxChild],
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) {
    let props_type = resolve_props_type(
        tag_name,
        tag_name_span,
        component_name,
        component_span,
        element_span,
        fallback_span,
        symbols,
        ctx,
    );

    let props_object = match props_type.as_ref().map(Type::peeled) {
        Some(Type::Object(object)) => Some(object),
        _ => None,
    };

    check_attributes(
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

    if let Some(object) = props_object.as_ref() {
        check_missing_required_props(
            object,
            attributes,
            children_provided,
            tag_name_span.or(element_span),
            fallback_span,
            ctx,
        );
    }
}

/// Resolves the props/attributes type the element is checked against, or `None`
/// when no check should run (unresolved component, a non-object/`any` component
/// type, or an intrinsic element with no `JSX.IntrinsicElements` declaration).
/// Emits TS2304 for an unresolved component and TS2339 for an unknown intrinsic
/// tag, matching tsc.
#[allow(clippy::too_many_arguments)]
fn resolve_props_type(
    tag_name: &str,
    tag_name_span: Option<SyntaxTextSpan>,
    component_name: Option<&str>,
    component_span: Option<SyntaxTextSpan>,
    element_span: Option<SyntaxTextSpan>,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    match component_name {
        Some(name) => {
            let component_expr = build_component_value_expression(
                name,
                tag_name,
                component_span.or(tag_name_span),
                tag_name_span,
            );
            let inferred = evaluate_expression(
                &component_expr,
                component_span.or(fallback_span),
                symbols,
                ctx,
            );
            match inferred {
                InferredExpression::Known(component_type) => component_props_type(&component_type),
                _ => None,
            }
        }
        None => resolve_intrinsic_props_type(tag_name, element_span.or(fallback_span), ctx),
    }
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
            .unwrap_or_else(|| Type::Object(alloc_object_type(PropertyMap::new(), None))),
    )
}

/// Looks up `<tag>` in `JSX.IntrinsicElements`. Returns the element's attribute
/// type, or `None` when there is no `JSX.IntrinsicElements` (conservative
/// fallback). Emits TS2339 for a tag absent from a declared `JSX.IntrinsicElements`.
fn resolve_intrinsic_props_type(
    tag_name: &str,
    element_span: Option<SyntaxTextSpan>,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    const INTRINSIC_ELEMENTS: &str = "JSX.IntrinsicElements";

    if ctx.lookup_type_declaration(INTRINSIC_ELEMENTS).is_none() {
        return None;
    }

    let named = ParsedType::Named(ParsedNamedType {
        name: INTRINSIC_ELEMENTS.to_string(),
        span: None,
        type_arguments: Vec::new(),
    });
    let intrinsic_type = map_parsed_type(named, ctx).peeled();
    let Type::Object(object) = &intrinsic_type else {
        return None;
    };

    if let Some(property_type) = object.get_property_type(tag_name) {
        return Some(property_type.clone());
    }

    if object.allows_string_index_access() {
        return object.string_index_type.as_deref().cloned();
    }

    ctx.push(diagnostic_with_syntax_span(
        Diagnostic::ts2339(tag_name, INTRINSIC_ELEMENTS, ctx.file_name.clone()),
        element_span,
    ));
    None
}

/// Builds the value expression a component tag refers to: an identifier for
/// `<Button />` or a property-access chain for `<UI.Button />`.
fn build_component_value_expression(
    head_name: &str,
    tag_name: &str,
    head_span: Option<SyntaxTextSpan>,
    tag_name_span: Option<SyntaxTextSpan>,
) -> ParsedExpression {
    let mut expression = ParsedExpression::Identifier {
        name: head_name.to_string(),
        span: head_span,
    };

    let mut segments = tag_name.split('.');
    let _head = segments.next();
    let segments: Vec<&str> = segments.collect();
    let last_index = segments.len().saturating_sub(1);
    for (index, segment) in segments.iter().enumerate() {
        let property_span = if index == last_index {
            tag_name_span
        } else {
            None
        };
        expression = ParsedExpression::PropertyAccess {
            object: Box::new(expression),
            object_span: head_span,
            property_name: (*segment).to_string(),
            property_span,
            is_bracketed: false,
        };
    }

    expression
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
) {
    let mut first_excess: Option<Option<SyntaxTextSpan>> = None;
    let mut attribute_types: BTreeMap<String, Type> = BTreeMap::new();

    for attribute in attributes {
        // `{...spread}` attributes carry no name; still evaluate the argument.
        if attribute.name.is_empty() {
            if let Some(value) = &attribute.value {
                let _ = evaluate_expression(
                    value,
                    attribute.value_span.or(fallback_span),
                    symbols,
                    ctx,
                );
            }
            continue;
        }

        let expected_property =
            props_object.and_then(|object| object.get_property(&attribute.name));
        let contextual_type = expected_property.map(|property| property.ty.clone());

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
                } else if first_excess.is_none() {
                    first_excess = Some(attribute.name_span);
                }
            }
        }
    }

    if let (Some(object), Some(excess_span)) = (props_object, first_excess) {
        let source = source_object_name(&attribute_types);
        let target = Type::Object(object.clone()).name();
        ctx.push(diagnostic_with_syntax_span(
            Diagnostic::ts2322(&source, &target, ctx.file_name.clone()),
            excess_span.or(fallback_span),
        ));
    }
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
    match attribute.value_kind {
        ParsedJsxAttributeValueKind::StringLiteral => Some(Type::String),
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

/// Reports TS2322 when a known prop's value is not assignable to its declared
/// type, pointing at the attribute name (tsc's span for JSX prop mismatches).
fn check_known_prop(
    attribute: &ParsedJsxAttribute,
    attribute_type: &Type,
    property: &ObjectProperty,
    fallback_span: Option<SyntaxTextSpan>,
    ctx: &mut CheckerContext,
) {
    let expected_type = if property.is_optional() {
        union_type(vec![property.ty.clone(), Type::Undefined])
    } else {
        property.ty.clone()
    };

    if attribute_type.is_unknown() {
        return;
    }

    if is_assignable_to(attribute_type, &expected_type) {
        return;
    }

    let source = source_display_name(attribute_type, &expected_type);
    let target = expected_type.name();
    ctx.push(diagnostic_with_syntax_span(
        Diagnostic::ts2322(&source, &target, ctx.file_name.clone()),
        attribute.name_span.or(fallback_span),
    ));
}

/// Reports TS2741 for the first required prop that is neither passed as an
/// attribute nor (for `children`) supplied as element children.
fn check_missing_required_props(
    props_object: &ObjectType,
    attributes: &[ParsedJsxAttribute],
    children_provided: bool,
    tag_span: Option<SyntaxTextSpan>,
    fallback_span: Option<SyntaxTextSpan>,
    ctx: &mut CheckerContext,
) {
    let missing = props_object.required_properties().find(|(name, _)| {
        if name.as_str() == "children" && children_provided {
            return false;
        }
        !attributes
            .iter()
            .any(|attribute| attribute.name == name.as_str())
    });

    let Some((property_name, _)) = missing else {
        return;
    };

    let present = present_attribute_object_name(attributes);
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
                    let inferred =
                        evaluate_expression(expression, span.or(fallback_span), symbols, ctx);
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
    let properties = attribute_types
        .iter()
        .map(|(name, ty)| (name.clone(), ObjectProperty::required(ty.clone())))
        .collect::<PropertyMap>();
    Type::Object(alloc_object_type(properties, None)).name()
}

/// The `'{1}'` argument of TS2741: the object type of the attributes already
/// present (tsc renders `{}` when none are).
fn present_attribute_object_name(attributes: &[ParsedJsxAttribute]) -> String {
    let properties = attributes
        .iter()
        .filter(|attribute| !attribute.name.is_empty())
        .map(|attribute| {
            (
                attribute.name.clone(),
                ObjectProperty::required(Type::Unknown),
            )
        })
        .collect::<PropertyMap>();

    if properties.is_empty() {
        return "{}".to_string();
    }

    Type::Object(alloc_object_type(properties, None)).name()
}

fn type_contains_unknown_or_any(ty: &Type) -> bool {
    matches!(ty, Type::Unknown | Type::GenuineUnknown | Type::Any)
        || matches!(ty, Type::Union(union) if union.types().iter().any(type_contains_unknown_or_any))
}
