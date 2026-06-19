use oxc_ast::ast::{
    PropertyKey, TSConditionalType, TSIndexedAccessType, TSIntersectionType, TSLiteral,
    TSLiteralType, TSMappedType, TSMappedTypeModifierOperator, TSMethodSignature,
    TSMethodSignatureKind, TSPropertySignature, TSSignature, TSTupleElement, TSTupleType, TSType,
    TSTypeAliasDeclaration, TSTypeLiteral, TSTypeName, TSTypeOperator, TSTypeOperatorOperator,
    TSTypeParameter, TSTypeParameterDeclaration, TSTypeParameterInstantiation, TSTypeQuery,
    TSTypeQueryExprName, TSTypeReference, TSUnionType,
};
use oxc_span::GetSpan;

use crate::{
    ParsedConditionalType, ParsedFunctionType, ParsedIndexedAccessType, ParsedMappedType,
    ParsedNamedType, ParsedObjectType, ParsedObjectTypeProperty, ParsedTemplateLiteralType,
    ParsedType, ParsedTypeAliasDeclaration, ParsedTypeOfType, ParsedTypeParameter,
};

use super::function_types::{
    parse_function_type, parse_function_type_parameter, parse_function_type_rest_parameter,
};
use super::spans::text_span_from_oxc_span;

pub(crate) fn parse_type_annotation(
    type_annotation: &oxc_ast::ast::TSTypeAnnotation<'_>,
) -> Option<ParsedType> {
    parse_type(&type_annotation.type_annotation)
}

pub(crate) fn parse_type(type_annotation: &TSType<'_>) -> Option<ParsedType> {
    match type_annotation {
        TSType::TSStringKeyword(_) => Some(ParsedType::String),
        TSType::TSNumberKeyword(_) => Some(ParsedType::Number),
        TSType::TSBooleanKeyword(_) => Some(ParsedType::Boolean),
        TSType::TSUndefinedKeyword(_) => Some(ParsedType::Undefined),
        TSType::TSNullKeyword(_) => Some(ParsedType::Undefined),
        TSType::TSObjectKeyword(_) => Some(ParsedType::Unknown),
        // `symbol` and `bigint` have no modelled representation; degrade to
        // `Unknown` rather than dropping the annotation, which would poison any
        // call/construct signature that mentions them (e.g. `SymbolConstructor`).
        TSType::TSSymbolKeyword(_) => Some(ParsedType::Unknown),
        TSType::TSBigIntKeyword(_) => Some(ParsedType::Unknown),
        TSType::TSVoidKeyword(_) => Some(ParsedType::Void),
        TSType::TSAnyKeyword(_) => Some(ParsedType::Any),
        TSType::TSUnknownKeyword(_) => Some(ParsedType::Unknown),
        TSType::TSNeverKeyword(_) => Some(ParsedType::Never),
        TSType::TSLiteralType(literal_type) => Some(parse_literal_type(literal_type)),
        TSType::TSTypeLiteral(type_literal) => Some(parse_type_literal(type_literal)),
        TSType::TSArrayType(array_type) => {
            parse_type(&array_type.element_type).map(|ty| ParsedType::Array(Box::new(ty)))
        }
        TSType::TSTupleType(tuple_type) => parse_tuple_type(tuple_type),
        TSType::TSFunctionType(function_type) => {
            parse_function_type(function_type).map(ParsedType::Function)
        }
        TSType::TSParenthesizedType(parenthesized_type) => {
            parse_type(&parenthesized_type.type_annotation)
        }
        TSType::TSUnionType(union_type) => Some(parse_union_type(union_type)),
        TSType::TSIntersectionType(intersection_type) => {
            Some(parse_intersection_type(intersection_type))
        }
        TSType::TSTypeReference(type_reference) => parse_type_reference(type_reference),
        TSType::TSTypeQuery(type_query) => parse_type_query(type_query),
        TSType::TSTypeOperatorType(type_operator) => parse_type_operator(type_operator),
        TSType::TSIndexedAccessType(indexed_access) => parse_indexed_access_type(indexed_access),
        TSType::TSMappedType(mapped_type) => parse_mapped_type(mapped_type),
        TSType::TSConditionalType(conditional_type) => parse_conditional_type(conditional_type),
        TSType::TSTemplateLiteralType(template_literal) => {
            parse_template_literal_type(template_literal)
        }
        // Polymorphic `this` (common in lib builder methods like `Map.set`,
        // `Array.prototype` chaining) is not modelled; fall back to `any` so the
        // member still parses rather than being dropped, keeping the surrounding
        // declaration intact and cascade-free.
        TSType::TSThisType(_) => Some(ParsedType::Any),
        // A type predicate (`x is T`) evaluates to `boolean` at the value level;
        // an assertion predicate (`asserts x`, `asserts x is T`) evaluates to
        // `void`. Lowering to those keeps the surrounding signature intact (e.g.
        // `ArrayConstructor.isArray`) instead of dropping the whole member.
        TSType::TSTypePredicate(predicate) => Some(if predicate.asserts {
            ParsedType::Void
        } else {
            ParsedType::Boolean
        }),
        // `infer X` in a conditional `extends` clause. Carrying the name (rather
        // than dropping to `None`) keeps the enclosing conditional alive; the
        // resolver treats the capture as a permissive hole.
        TSType::TSInferType(infer_type) => {
            Some(ParsedType::Infer(infer_type.type_parameter.name.name.to_string()))
        }
        _ => None,
    }
}

/// Lowers a type-position template literal `` `a${X}b${Y}c` `` into its literal
/// segments plus the interpolated types. The `quasis` are the cooked string
/// pieces (always one more than the interpolation count). If any interpolation
/// uses a construct we cannot model, the whole template degrades to `Unknown`
/// so a reference to it resolves conservatively instead of vanishing and
/// cascading into `TS2304`.
///
/// This is distinct from expression-position template literals (parsed in
/// `parser/expressions.rs`); only `TSTemplateLiteralType` reaches here.
fn parse_template_literal_type(
    template_literal: &oxc_ast::ast::TSTemplateLiteralType<'_>,
) -> Option<ParsedType> {
    let mut quasis = Vec::with_capacity(template_literal.quasis.len());
    for quasi in &template_literal.quasis {
        let text = quasi
            .value
            .cooked
            .as_ref()
            .map(|cooked| cooked.to_string())
            .unwrap_or_else(|| quasi.value.raw.to_string());
        quasis.push(text);
    }

    let mut interpolations = Vec::with_capacity(template_literal.types.len());
    for interpolation in &template_literal.types {
        let Some(parsed) = parse_type(interpolation) else {
            return Some(ParsedType::Unknown);
        };
        interpolations.push(parsed);
    }

    Some(ParsedType::TemplateLiteral(ParsedTemplateLiteralType {
        quasis,
        interpolations,
        span: Some(text_span_from_oxc_span(template_literal.span)),
    }))
}

/// Lowers `Check extends Extends ? True : False`. If any branch contains a
/// construct we do not model yet (e.g. nested `infer`), the whole conditional
/// degrades to `Unknown` so a reference to it resolves conservatively instead of
/// disappearing and cascading into `TS2304`.
fn parse_conditional_type(conditional_type: &TSConditionalType<'_>) -> Option<ParsedType> {
    let (Some(check_type), Some(extends_type), Some(true_type), Some(false_type)) = (
        parse_type(&conditional_type.check_type),
        parse_type(&conditional_type.extends_type),
        parse_type(&conditional_type.true_type),
        parse_type(&conditional_type.false_type),
    ) else {
        return Some(ParsedType::Unknown);
    };

    Some(ParsedType::Conditional(ParsedConditionalType {
        check_type: Box::new(check_type),
        extends_type: Box::new(extends_type),
        true_type: Box::new(true_type),
        false_type: Box::new(false_type),
        span: Some(text_span_from_oxc_span(conditional_type.span)),
    }))
}

fn parse_type_query(type_query: &TSTypeQuery<'_>) -> Option<ParsedType> {
    match &type_query.expr_name {
        TSTypeQueryExprName::IdentifierReference(identifier) => {
            Some(ParsedType::TypeOf(ParsedTypeOfType {
                name: identifier.name.to_string(),
                name_span: Some(text_span_from_oxc_span(identifier.span)),
                members: Vec::new(),
            }))
        }
        TSTypeQueryExprName::QualifiedName(qualified_name) => {
            let mut members = Vec::new();
            let (base, base_span) = flatten_qualified_type_name(qualified_name, &mut members)?;
            Some(ParsedType::TypeOf(ParsedTypeOfType {
                name: base,
                name_span: Some(text_span_from_oxc_span(base_span)),
                members,
            }))
        }
        // `typeof import('foo')` and `typeof this` are not modelled.
        _ => None,
    }
}

/// Flattens a left-nested `TSQualifiedName` (`A.B.C`) into its leftmost
/// identifier (base) plus the trailing member segments in source order. Returns
/// `None` if the leftmost element is not a plain identifier (e.g. `this.x`),
/// which we do not model.
fn flatten_qualified_type_name(
    qualified_name: &oxc_ast::ast::TSQualifiedName<'_>,
    members: &mut Vec<String>,
) -> Option<(String, oxc_span::Span)> {
    let base = match &qualified_name.left {
        TSTypeName::IdentifierReference(identifier) => {
            (identifier.name.to_string(), identifier.span)
        }
        TSTypeName::QualifiedName(inner) => flatten_qualified_type_name(inner, members)?,
        TSTypeName::ThisExpression(_) => return None,
    };
    members.push(qualified_name.right.name.to_string());
    Some(base)
}

fn parse_type_operator(type_operator: &TSTypeOperator<'_>) -> Option<ParsedType> {
    match type_operator.operator {
        TSTypeOperatorOperator::Keyof => {
            parse_type(&type_operator.type_annotation).map(|ty| ParsedType::KeyOf(Box::new(ty)))
        }
        // `readonly T[]` / `readonly [A, B]` are not distinguished from their
        // mutable forms here; lowering to the inner array/tuple keeps the
        // annotation intact instead of dropping it (which would cascade into
        // `TS7006`/`TS7031` on the annotated binding). `unique symbol` has no
        // modelled representation, so it degrades to `Unknown`.
        TSTypeOperatorOperator::Readonly => parse_type(&type_operator.type_annotation),
        TSTypeOperatorOperator::Unique => Some(ParsedType::Unknown),
    }
}

fn parse_indexed_access_type(indexed_access: &TSIndexedAccessType<'_>) -> Option<ParsedType> {
    let object_type = parse_type(&indexed_access.object_type)?;
    let index_type = parse_type(&indexed_access.index_type)?;

    Some(ParsedType::IndexedAccess(ParsedIndexedAccessType {
        object_type: Box::new(object_type),
        index_type: Box::new(index_type),
        span: Some(text_span_from_oxc_span(indexed_access.span)),
    }))
}

fn parse_mapped_type(mapped_type: &TSMappedType<'_>) -> Option<ParsedType> {
    if mapped_type.readonly.is_some() || mapped_type.name_type.is_some() {
        return Some(ParsedType::Unknown);
    }

    let optional = match mapped_type.optional {
        Some(TSMappedTypeModifierOperator::True) => true,
        Some(_) => return Some(ParsedType::Unknown), // unsupported +? or -?
        None => false,
    };

    let constraint = parse_type(&mapped_type.constraint)?;
    let value_type = match &mapped_type.type_annotation {
        Some(t) => parse_type(t)?,
        None => ParsedType::Any, // Though typically TS requires a type, fall back to Any or Unknown
    };

    Some(ParsedType::Mapped(ParsedMappedType {
        key_name: mapped_type.key.name.to_string(),
        key_span: Some(text_span_from_oxc_span(mapped_type.key.span)),
        constraint: Box::new(constraint),
        value_type: Box::new(value_type),
        optional,
        span: Some(text_span_from_oxc_span(mapped_type.span)),
    }))
}

fn parse_type_reference(type_reference: &TSTypeReference<'_>) -> Option<ParsedType> {
    let (name, span) = flatten_type_name(&type_reference.type_name)?;

    let type_arguments = match type_reference.type_arguments.as_deref() {
        Some(type_arguments) => parse_type_arguments(type_arguments)?,
        None => Vec::new(),
    };

    Some(ParsedType::Named(ParsedNamedType {
        name,
        span: Some(span),
        type_arguments,
    }))
}

/// Flatten a (possibly qualified) type name into a dotted string and the span of
/// the head identifier, e.g. `React.ComponentProps` -> `"React.ComponentProps"`.
/// Mirrors how namespace members are registered under qualified keys.
fn flatten_type_name(type_name: &TSTypeName<'_>) -> Option<(String, crate::TextSpan)> {
    match type_name {
        TSTypeName::IdentifierReference(identifier) => Some((
            identifier.name.to_string(),
            text_span_from_oxc_span(identifier.span),
        )),
        TSTypeName::QualifiedName(qualified) => {
            let (left_name, left_span) = flatten_type_name(&qualified.left)?;
            Some((format!("{}.{}", left_name, qualified.right.name), left_span))
        }
        _ => None,
    }
}

fn parse_literal_type(literal_type: &TSLiteralType<'_>) -> ParsedType {
    match &literal_type.literal {
        TSLiteral::StringLiteral(string_literal) => {
            ParsedType::StringLiteral(string_literal.value.to_string())
        }
        TSLiteral::NumericLiteral(numeric_literal) => {
            ParsedType::NumberLiteral(numeric_literal.value.to_string())
        }
        TSLiteral::BooleanLiteral(boolean_literal) => {
            ParsedType::BooleanLiteral(boolean_literal.value)
        }
        TSLiteral::UnaryExpression(_)
        | TSLiteral::BigIntLiteral(_)
        | TSLiteral::TemplateLiteral(_) => ParsedType::Unknown,
    }
}

fn parse_union_type(union_type: &TSUnionType<'_>) -> ParsedType {
    let mut types = Vec::new();

    for ty in &union_type.types {
        let Some(parsed_type) = parse_type(ty) else {
            return ParsedType::Unknown;
        };

        types.push(parsed_type);
    }

    ParsedType::Union(types)
}

fn parse_intersection_type(intersection_type: &TSIntersectionType<'_>) -> ParsedType {
    let mut types = Vec::new();

    for ty in &intersection_type.types {
        let Some(parsed_type) = parse_type(ty) else {
            return ParsedType::Unknown;
        };

        types.push(parsed_type);
    }

    ParsedType::Intersection(types)
}

fn parse_tuple_type(tuple_type: &TSTupleType<'_>) -> Option<ParsedType> {
    let mut elements = Vec::new();

    for element in &tuple_type.element_types {
        match element {
            TSTupleElement::TSNamedTupleMember(_)
            | TSTupleElement::TSOptionalType(_)
            | TSTupleElement::TSRestType(_) => return None,
            _ => {
                let Some(parsed_element) = parse_type(element.as_ts_type()?) else {
                    return None;
                };

                elements.push(parsed_element);
            }
        }
    }

    Some(ParsedType::Tuple(elements))
}

fn parse_type_literal(type_literal: &TSTypeLiteral<'_>) -> ParsedType {
    let mut properties = Vec::new();
    let mut call_signature = None;

    for member in &type_literal.members {
        let property = match member {
            TSSignature::TSPropertySignature(property_signature) => {
                parse_type_property_signature(property_signature)
            }
            TSSignature::TSMethodSignature(method_signature) => {
                parse_type_method_signature(method_signature)
            }
            TSSignature::TSCallSignatureDeclaration(signature) => {
                if call_signature.is_none() {
                    call_signature = parse_call_signature(signature).map(Box::new);
                }
                continue;
            }
            _ => return ParsedType::Unknown,
        };

        let Some(property) = property else {
            return ParsedType::Unknown;
        };

        properties.push(property);
    }

    ParsedType::Object(ParsedObjectType {
        properties,
        call_signature,
    })
}

/// Lowers a bare call signature (`(value?: any): number`) into a
/// [`ParsedFunctionType`], shared by interface and object-type-literal parsing.
pub(crate) fn parse_call_signature(
    signature: &oxc_ast::ast::TSCallSignatureDeclaration<'_>,
) -> Option<ParsedFunctionType> {
    let mut parameters = signature
        .params
        .items
        .iter()
        .map(parse_function_type_parameter)
        .collect::<Option<Vec<_>>>()?;

    if let Some(rest) = signature.params.rest.as_deref() {
        parameters.push(parse_function_type_rest_parameter(rest)?);
    }

    let return_type = signature
        .return_type
        .as_ref()
        .and_then(|annotation| parse_type_annotation(annotation.as_ref()))?;

    Some(ParsedFunctionType {
        parameters,
        return_type: Box::new(return_type),
        type_parameters: parse_type_parameters(signature.type_parameters.as_deref()),
    })
}

/// Lowers a method signature (`foo(arg: A): R`) into a property whose type is a
/// [`ParsedType::Function`], so method calls reuse the existing function-type property
/// checking. Shared by interface and object-type-literal parsing.
pub(crate) fn parse_type_method_signature(
    method_signature: &TSMethodSignature<'_>,
) -> Option<ParsedObjectTypeProperty> {
    if method_signature.kind != TSMethodSignatureKind::Method || method_signature.computed {
        return None;
    }

    let PropertyKey::StaticIdentifier(key) = &method_signature.key else {
        return None;
    };

    let mut parameters = method_signature
        .params
        .items
        .iter()
        .map(parse_function_type_parameter)
        .collect::<Option<Vec<_>>>()?;

    if let Some(rest) = method_signature.params.rest.as_deref() {
        parameters.push(parse_function_type_rest_parameter(rest)?);
    }

    let return_type = method_signature
        .return_type
        .as_ref()
        .and_then(|annotation| parse_type_annotation(annotation.as_ref()))?;

    Some(ParsedObjectTypeProperty {
        name: key.name.to_string(),
        name_span: Some(text_span_from_oxc_span(key.span)),
        ty: ParsedType::Function(ParsedFunctionType {
            parameters,
            return_type: Box::new(return_type),
            type_parameters: parse_type_parameters(method_signature.type_parameters.as_deref()),
        }),
        optional: method_signature.optional,
    })
}

/// Extract the value type of an index signature (`[key: string]: T` -> `T`).
/// String and number index signatures are not distinguished; both map to the
/// object's string index type.
pub(crate) fn parse_index_signature_value_type(
    index_signature: &oxc_ast::ast::TSIndexSignature<'_>,
) -> Option<ParsedType> {
    parse_type(&index_signature.type_annotation.type_annotation)
}

pub(crate) fn parse_type_parameters(
    type_parameters: Option<&TSTypeParameterDeclaration<'_>>,
) -> Vec<ParsedTypeParameter> {
    type_parameters
        .map(|type_parameters| {
            type_parameters
                .params
                .iter()
                .map(parse_type_parameter)
                .collect()
        })
        .unwrap_or_default()
}

fn parse_type_parameter(type_parameter: &TSTypeParameter<'_>) -> ParsedTypeParameter {
    ParsedTypeParameter {
        name: type_parameter.name.name.to_string(),
        name_span: Some(text_span_from_oxc_span(type_parameter.name.span)),
        constraint: type_parameter.constraint.as_ref().and_then(parse_type),
        default_type: type_parameter.default.as_ref().and_then(parse_type),
        span: Some(text_span_from_oxc_span(type_parameter.span)),
    }
}

pub(crate) fn parse_type_arguments(
    type_arguments: &TSTypeParameterInstantiation<'_>,
) -> Option<Vec<ParsedType>> {
    type_arguments.params.iter().map(parse_type).collect()
}

pub(crate) fn parse_type_property_signature(
    property_signature: &TSPropertySignature<'_>,
) -> Option<ParsedObjectTypeProperty> {
    // `readonly` is not modelled for assignability, but the property must still
    // be present on the type. Standard lib interfaces (DOM `Response.ok`,
    // `URL`, etc.) rely heavily on `readonly` members, so parse them as ordinary
    // properties rather than dropping them.
    if property_signature.computed {
        return None;
    }

    let PropertyKey::StaticIdentifier(key) = &property_signature.key else {
        return None;
    };

    let type_annotation = property_signature
        .type_annotation
        .as_ref()
        .and_then(|annotation| parse_type_annotation(annotation))?;

    Some(ParsedObjectTypeProperty {
        name: key.name.to_string(),
        name_span: Some(text_span_from_oxc_span(key.span)),
        ty: type_annotation,
        optional: property_signature.optional,
    })
}

pub(crate) fn parse_type_alias_declaration(
    declaration: &TSTypeAliasDeclaration<'_>,
) -> Option<ParsedTypeAliasDeclaration> {
    let ty = parse_type(&declaration.type_annotation)?;

    Some(ParsedTypeAliasDeclaration {
        is_declare: declaration.declare,
        name: declaration.id.name.to_string(),
        name_span: Some(text_span_from_oxc_span(declaration.id.span)),
        type_parameters: parse_type_parameters(declaration.type_parameters.as_deref()),
        ty,
        type_span: Some(text_span_from_oxc_span(declaration.type_annotation.span())),
    })
}
