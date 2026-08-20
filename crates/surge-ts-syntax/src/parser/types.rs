use oxc_ast::ast::{
    Expression, PropertyKey, TSConditionalType, TSIndexedAccessType, TSIntersectionType, TSLiteral,
    TSLiteralType, TSMappedType, TSMappedTypeModifierOperator, TSMethodSignature,
    TSMethodSignatureKind, TSPropertySignature, TSSignature, TSTupleElement, TSTupleType, TSType,
    TSTypeAliasDeclaration, TSTypeLiteral, TSTypeName, TSTypeOperator, TSTypeOperatorOperator,
    TSTypeParameter, TSTypeParameterDeclaration, TSTypeParameterInstantiation, TSTypeQuery,
    TSTypeQueryExprName, TSTypeReference, TSUnionType,
};
use oxc_span::GetSpan;

use crate::{
    ParsedConditionalType, ParsedFunctionType, ParsedIndexedAccessType, ParsedMappedType,
    ParsedNamedType, ParsedObjectType, ParsedObjectTypeProperty, ParsedPredicateType,
    ParsedTemplateLiteralType, ParsedType, ParsedTypeAliasDeclaration, ParsedTypeOfType,
    ParsedTypeParameter,
};

use super::function_types::{
    parse_constructor_type, parse_function_type, parse_function_type_parameter,
    parse_function_type_rest_parameter,
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
        TSType::TSSymbolKeyword(_) => Some(ParsedType::Symbol),
        TSType::TSBigIntKeyword(_) => Some(ParsedType::BigInt),
        TSType::TSVoidKeyword(_) => Some(ParsedType::Void),
        TSType::TSAnyKeyword(_) => Some(ParsedType::Any),
        TSType::TSUnknownKeyword(_) => Some(ParsedType::UnknownKeyword),
        // `intrinsic`-bodied lib aliases (`Uppercase`, `BuiltinIteratorReturn`,
        // `NoInfer`, …) are compiler built-ins with no user-modellable body.
        // Lower to `Unknown` so the alias resolves instead of dropping to TS2304;
        // the alias's nominal/generic surface is preserved by its declaration.
        TSType::TSIntrinsicKeyword(_) => Some(ParsedType::Unknown),
        TSType::TSNeverKeyword(_) => Some(ParsedType::Never),
        TSType::TSLiteralType(literal_type) => Some(parse_literal_type(literal_type)),
        TSType::TSTypeLiteral(type_literal) => Some(parse_type_literal(type_literal)),
        TSType::TSArrayType(array_type) => parse_type(&array_type.element_type)
            .map(|ty| ParsedType::Array(std::sync::Arc::new(ty))),
        TSType::TSTupleType(tuple_type) => parse_tuple_type(tuple_type),
        TSType::TSFunctionType(function_type) => parse_function_type(function_type)
            .map(|function| ParsedType::Function(std::sync::Arc::new(function))),
        // A constructor type (`new (args) => T`) is lowered to a callable
        // signature; surge does not distinguish newability, and keeping it parsed
        // prevents a union member like `JSXElementConstructor`'s `new (...) => …`
        // from collapsing the whole alias to `Unknown`.
        TSType::TSConstructorType(constructor_type) => parse_constructor_type(constructor_type)
            .map(|function| ParsedType::Function(std::sync::Arc::new(function))),
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
        // `void`. The payload (tested parameter + target type) is preserved so
        // guard narrowing can consume calls of the predicate.
        TSType::TSTypePredicate(predicate) => {
            let parameter_name = match &predicate.parameter_name {
                oxc_ast::ast::TSTypePredicateName::Identifier(identifier) => {
                    identifier.name.to_string()
                }
                oxc_ast::ast::TSTypePredicateName::This(_) => "this".to_string(),
            };
            Some(ParsedType::Predicate(std::sync::Arc::new(
                ParsedPredicateType {
                    parameter_name,
                    ty: predicate
                        .type_annotation
                        .as_ref()
                        .and_then(|annotation| parse_type(&annotation.type_annotation)),
                    asserts: predicate.asserts,
                },
            )))
        }
        // `infer X` in a conditional `extends` clause. Carrying the name (rather
        // than dropping to `None`) keeps the enclosing conditional alive; the
        // resolver treats the capture as a permissive hole.
        TSType::TSInferType(infer_type) => Some(ParsedType::Infer(
            infer_type.type_parameter.name.name.to_string(),
        )),
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

    Some(ParsedType::TemplateLiteral(std::sync::Arc::new(ParsedTemplateLiteralType {
        quasis,
        interpolations,
        span: Some(text_span_from_oxc_span(template_literal.span)),
    })))
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

    Some(ParsedType::Conditional(std::sync::Arc::new(ParsedConditionalType {
        check_type: Box::new(check_type),
        extends_type: Box::new(extends_type),
        true_type: Box::new(true_type),
        false_type: Box::new(false_type),
        span: Some(text_span_from_oxc_span(conditional_type.span)),
    })))
}

fn parse_type_query(type_query: &TSTypeQuery<'_>) -> Option<ParsedType> {
    match &type_query.expr_name {
        TSTypeQueryExprName::IdentifierReference(identifier) => {
            Some(ParsedType::TypeOf(std::sync::Arc::new(ParsedTypeOfType {
                name: identifier.name.to_string(),
                name_span: Some(text_span_from_oxc_span(identifier.span)),
                members: Vec::new(),
            })))
        }
        TSTypeQueryExprName::QualifiedName(qualified_name) => {
            let mut members = Vec::new();
            let (base, base_span) = flatten_qualified_type_name(qualified_name, &mut members)?;
            Some(ParsedType::TypeOf(std::sync::Arc::new(ParsedTypeOfType {
                name: base,
                name_span: Some(text_span_from_oxc_span(base_span)),
                members,
            })))
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
        TSTypeOperatorOperator::Keyof => parse_type(&type_operator.type_annotation)
            .map(|ty| ParsedType::KeyOf(std::sync::Arc::new(ty))),
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

    Some(ParsedType::IndexedAccess(std::sync::Arc::new(ParsedIndexedAccessType {
        object_type: Box::new(object_type),
        index_type: Box::new(index_type),
        span: Some(text_span_from_oxc_span(indexed_access.span)),
    })))
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

    Some(ParsedType::Mapped(std::sync::Arc::new(ParsedMappedType {
        key_name: mapped_type.key.name.to_string(),
        key_span: Some(text_span_from_oxc_span(mapped_type.key.span)),
        constraint: Box::new(constraint),
        value_type: Box::new(value_type),
        optional,
        span: Some(text_span_from_oxc_span(mapped_type.span)),
    })))
}

fn parse_type_reference(type_reference: &TSTypeReference<'_>) -> Option<ParsedType> {
    let (name, span) = flatten_type_name(&type_reference.type_name)?;

    let type_arguments = match type_reference.type_arguments.as_deref() {
        Some(type_arguments) => parse_type_arguments(type_arguments)?,
        None => Vec::new(),
    };

    Some(ParsedType::Named(std::sync::Arc::new(ParsedNamedType {
        name,
        span: Some(span),
        type_arguments,
    })))
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

/// Flatten a heritage clause expression (`extends NS.Member`) into the same
/// dotted name [`flatten_type_name`] produces for type references, so a
/// qualified base resolves through the namespace-member key instead of being
/// dropped. Computed members and non-identifier heads have no such key.
pub(super) fn flatten_heritage_expression(
    expression: &Expression<'_>,
) -> Option<(String, crate::TextSpan)> {
    match expression {
        Expression::Identifier(identifier) => Some((
            identifier.name.to_string(),
            text_span_from_oxc_span(identifier.span),
        )),
        Expression::StaticMemberExpression(member) => {
            let (object_name, object_span) = flatten_heritage_expression(&member.object)?;
            Some((
                format!("{}.{}", object_name, member.property.name),
                object_span,
            ))
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

    ParsedType::Union(std::sync::Arc::new(types))
}

fn parse_intersection_type(intersection_type: &TSIntersectionType<'_>) -> ParsedType {
    let mut types = Vec::new();

    for ty in &intersection_type.types {
        let Some(parsed_type) = parse_type(ty) else {
            return ParsedType::Unknown;
        };

        types.push(parsed_type);
    }

    ParsedType::Intersection(std::sync::Arc::new(types))
}

/// Tuple labels are display-only, so named members lower to their element type.
/// Optional and rest members change tuple arity, which the fixed-length tuple
/// model cannot express; degrade those tuples to `Unknown` so an alias like
/// `type Args = [msg: string, extra?: any]` still resolves instead of dropping
/// to TS2304 at every use site.
fn parse_tuple_type(tuple_type: &TSTupleType<'_>) -> Option<ParsedType> {
    let mut elements = Vec::new();

    for element in &tuple_type.element_types {
        match element {
            TSTupleElement::TSNamedTupleMember(member) => {
                if member.optional || matches!(member.element_type, TSTupleElement::TSRestType(_)) {
                    return Some(homogeneous_variadic_tuple(tuple_type));
                }
                let Some(inner) = member.element_type.as_ts_type() else {
                    return Some(ParsedType::Unknown);
                };
                let Some(parsed_element) = parse_type(inner) else {
                    return None;
                };

                elements.push(parsed_element);
            }
            TSTupleElement::TSOptionalType(_) | TSTupleElement::TSRestType(_) => {
                return Some(homogeneous_variadic_tuple(tuple_type));
            }
            _ => {
                let Some(parsed_element) = parse_type(element.as_ts_type()?) else {
                    return None;
                };

                elements.push(parsed_element);
            }
        }
    }

    Some(ParsedType::Tuple(std::sync::Arc::new(elements)))
}

/// A variadic tuple whose fixed and rest elements are all the *same* type
/// (`[T, ...T[]]`, the non-empty-array idiom every enum/tuple builder uses)
/// carries no more information than `T[]` beyond a minimum length surge does
/// not model, so it lowers to the array. A heterogeneous one (`[string,
/// ...number[]]`) would lose its element types that way and stays degraded.
fn homogeneous_variadic_tuple(tuple_type: &TSTupleType<'_>) -> ParsedType {
    // `[...T]` spreads a tuple *type parameter*: it is `T` itself, not `T[]`.
    if let [TSTupleElement::TSRestType(rest)] = tuple_type.element_types.as_slice()
        && !matches!(rest.type_annotation, TSType::TSArrayType(_))
    {
        return parse_type(&rest.type_annotation).unwrap_or(ParsedType::Unknown);
    }

    let mut element_type: Option<ParsedType> = None;

    for element in &tuple_type.element_types {
        let inner = match element {
            // A rest element only contributes an element type when it is written
            // `...T[]`; `...T` spreads a whole tuple whose element types this
            // shape cannot express.
            TSTupleElement::TSNamedTupleMember(member) => match &member.element_type {
                TSTupleElement::TSRestType(rest) => match &rest.type_annotation {
                    TSType::TSArrayType(array) => Some(&array.element_type),
                    _ => return ParsedType::Unknown,
                },
                other => other.as_ts_type(),
            },
            TSTupleElement::TSRestType(rest) => match &rest.type_annotation {
                TSType::TSArrayType(array) => Some(&array.element_type),
                _ => return ParsedType::Unknown,
            },
            TSTupleElement::TSOptionalType(optional) => Some(&optional.type_annotation),
            other => other.as_ts_type(),
        };
        let Some(inner) = inner else {
            return ParsedType::Unknown;
        };
        let Some(parsed) = parse_type(inner) else {
            return ParsedType::Unknown;
        };
        match &element_type {
            Some(existing) if !same_tuple_element_type(existing, &parsed) => {
                return ParsedType::Unknown;
            }
            Some(_) => {}
            None => element_type = Some(parsed),
        }
    }

    match element_type {
        Some(element_type) => ParsedType::Array(std::sync::Arc::new(element_type)),
        None => ParsedType::Unknown,
    }
}

/// Structural equality for tuple elements that ignores name spans: the `T` of
/// `[T, ...T[]]` is the same type written twice, and `ParsedNamedType`'s derived
/// equality would separate them on position alone.
fn same_tuple_element_type(left: &ParsedType, right: &ParsedType) -> bool {
    match (left, right) {
        (ParsedType::Named(left), ParsedType::Named(right)) => {
            left.name == right.name
                && left.type_arguments.len() == right.type_arguments.len()
                && left
                    .type_arguments
                    .iter()
                    .zip(right.type_arguments.iter())
                    .all(|(left, right)| same_tuple_element_type(left, right))
        }
        (ParsedType::Array(left), ParsedType::Array(right)) => same_tuple_element_type(left, right),
        _ => left == right,
    }
}

fn parse_type_literal(type_literal: &TSTypeLiteral<'_>) -> ParsedType {
    let mut properties = Vec::new();
    let mut string_index_type: Option<Box<ParsedType>> = None;
    let mut call_signature: Option<Box<ParsedFunctionType>> = None;
    let getters = getter_accessor_names(&type_literal.members);

    for member in &type_literal.members {
        if is_shadowed_setter(member, &getters) {
            continue;
        }

        let property = match member {
            TSSignature::TSPropertySignature(property_signature) => {
                parse_type_property_signature(property_signature)
            }
            TSSignature::TSMethodSignature(method_signature) => {
                parse_type_method_signature(method_signature)
            }
            TSSignature::TSCallSignatureDeclaration(signature) => {
                // Same-shaped overloads fold into one permissive signature
                // (max parameters, everything past a shorter overload's arity
                // optional), mirroring the interface overload merge — a callable
                // type literal like expect-type's `_ExpectTypeOf` declares both
                // `<T>(actual: T): …` and `<T>(): …`, and keeping only the
                // first made every zero-argument call a false TS2554.
                if let Some(parsed) = parse_call_signature(signature) {
                    call_signature = Some(match call_signature.take() {
                        Some(existing) => {
                            Box::new(merge_parsed_call_signatures(&existing, &parsed))
                        }
                        None => Box::new(parsed),
                    });
                }
                continue;
            }
            TSSignature::TSIndexSignature(index_signature) => {
                // The last index signature wins, matching the interface path.
                if let Some(value_type) = parse_index_signature_value_type(index_signature) {
                    string_index_type = Some(Box::new(value_type));
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

    ParsedType::Object(std::sync::Arc::new(ParsedObjectType {
        properties,
        string_index_type,
        call_signature,
    }))
}

/// Lowers a bare call signature (`(value?: any): number`) into a
/// [`ParsedFunctionType`], shared by interface and object-type-literal parsing.
/// Folds two call-signature overloads into one permissive signature: the longer
/// parameter list wins, a position typed differently across overloads widens to
/// `any`, and a position absent (or optional) in either overload is optional.
/// The return type is taken from the first overload. This mirrors the checker's
/// `merge_overload_signatures`, applied at parse time because a type literal
/// stores a single call signature.
pub(crate) fn merge_parsed_call_signatures(
    a: &ParsedFunctionType,
    b: &ParsedFunctionType,
) -> ParsedFunctionType {
    let (longer, shorter) = if a.parameters.len() >= b.parameters.len() {
        (a, b)
    } else {
        (b, a)
    };

    // The merged arity floor is the smaller overload's required count: a call
    // that satisfies either overload must pass (`<E>(v: E): true` merged with
    // `<E>(): true` requires nothing).
    let required = |signature: &ParsedFunctionType| {
        signature
            .parameters
            .iter()
            .filter(|parameter| !parameter.is_this)
            .take_while(|parameter| !parameter.optional && !parameter.rest)
            .count()
    };
    let min_required = required(a).min(required(b));

    let parameters = longer
        .parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            let mut merged = parameter.clone();
            if let Some(other) = shorter.parameters.get(index)
                && other.ty != merged.ty
            {
                merged.ty = ParsedType::Any;
            }
            if index >= min_required && !merged.rest {
                merged.optional = true;
            }
            merged
        })
        .collect();

    // Return type and type parameters must come from the SAME overload as the
    // parameter list: mixing them leaves the return type referencing the other
    // overload's type-parameter names (ky's `json` mixes `JsonType` and
    // `Schema`), which surfaces as a false TS2304.
    ParsedFunctionType {
        parameters,
        return_type: longer.return_type.clone(),
        type_parameters: longer.type_parameters.clone(),
    }
}

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

pub(crate) fn parse_construct_signature(
    signature: &oxc_ast::ast::TSConstructSignatureDeclaration<'_>,
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

/// Names declared with a `get` accessor. A `set` accessor for the same name is
/// dropped so the read type wins, which is what tsc reports for a pair whose
/// getter and setter types differ.
pub(crate) fn getter_accessor_names<'a>(
    members: &'a [TSSignature<'a>],
) -> std::collections::HashSet<&'a str> {
    members
        .iter()
        .filter_map(|member| match member {
            TSSignature::TSMethodSignature(signature)
                if signature.kind == TSMethodSignatureKind::Get =>
            {
                match &signature.key {
                    PropertyKey::StaticIdentifier(key) => Some(key.name.as_str()),
                    _ => None,
                }
            }
            _ => None,
        })
        .collect()
}

pub(crate) fn is_shadowed_setter(
    member: &TSSignature<'_>,
    getters: &std::collections::HashSet<&str>,
) -> bool {
    match member {
        TSSignature::TSMethodSignature(signature)
            if signature.kind == TSMethodSignatureKind::Set =>
        {
            matches!(
                &signature.key,
                PropertyKey::StaticIdentifier(key) if getters.contains(key.name.as_str())
            )
        }
        _ => false,
    }
}

/// Lowers a method signature (`foo(arg: A): R`) into a property whose type is a
/// [`ParsedType::Function`], so method calls reuse the existing function-type property
/// checking. Shared by interface and object-type-literal parsing.
pub(crate) fn parse_type_method_signature(
    method_signature: &TSMethodSignature<'_>,
) -> Option<ParsedObjectTypeProperty> {
    if method_signature.computed {
        return None;
    }

    let PropertyKey::StaticIdentifier(key) = &method_signature.key else {
        return None;
    };

    // A `get`/`set` accessor lowers to a plain property: the getter's return
    // type, or the setter's parameter type when only a setter is declared. The
    // pair is written with differing types across the DOM lib
    // (`get location(): Location; set location(href: string)`), so dropping
    // them left `window.location` unresolved.
    if method_signature.kind != TSMethodSignatureKind::Method {
        let accessor_type = match method_signature.kind {
            TSMethodSignatureKind::Get => method_signature
                .return_type
                .as_ref()
                .and_then(|annotation| parse_type_annotation(annotation.as_ref()))?,
            _ => method_signature
                .params
                .items
                .first()
                .and_then(|parameter| parameter.type_annotation.as_ref())
                .and_then(|annotation| parse_type_annotation(annotation))?,
        };

        return Some(ParsedObjectTypeProperty {
            name: key.name.to_string(),
            name_span: Some(text_span_from_oxc_span(key.span)),
            optional: false,
            is_method: false,
            ty: accessor_type,
        });
    }

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
        ty: ParsedType::Function(std::sync::Arc::new(ParsedFunctionType {
            parameters,
            return_type: Box::new(return_type),
            type_parameters: parse_type_parameters(method_signature.type_parameters.as_deref()),
        })),
        optional: method_signature.optional,
        is_method: true,
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

    // A numeric key is a property name like any other: numeric-key tables
    // (`{ 0: 1; 1: 0 }[B]`, the shape Prisma's generated `Not<B>` uses) must not
    // drop the member — losing one collapses the whole type literal to
    // `unknown`, which then reports the index as invalid. Quoted string keys stay
    // unsupported: admitting them resolves shapes whose members surge models
    // incompletely. Re-measured 2026-08-19 it is a wash (zod +1, trpc -1) — the
    // `"~standard"` members it unblocks sit on interfaces whose `Parameters<…>`
    // chain degrades to `{}` regardless.
    let (name, key_span) = match &property_signature.key {
        PropertyKey::StaticIdentifier(key) => (key.name.to_string(), key.span),
        PropertyKey::NumericLiteral(literal) => (literal.raw_str().to_string(), literal.span),
        PropertyKey::StringLiteral(literal) => (literal.value.to_string(), literal.span),
        _ => return None,
    };

    let type_annotation = property_signature
        .type_annotation
        .as_ref()
        .and_then(|annotation| parse_type_annotation(annotation))?;

    Some(ParsedObjectTypeProperty {
        name,
        name_span: Some(text_span_from_oxc_span(key_span)),
        ty: type_annotation,
        optional: property_signature.optional,
        is_method: false,
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
