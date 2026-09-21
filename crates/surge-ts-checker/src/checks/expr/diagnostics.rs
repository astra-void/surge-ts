use super::*;

/// Recursively widens fresh literal types to their base primitive, descending
/// into object properties, array elements, and union members. This matches the
/// type tsc infers for `let`/`var` bindings (e.g. `let o = { a: 1 }` widens to
/// `{ a: number }`).
pub(crate) fn widen_type(ty: &Type) -> Type {
    match ty {
        Type::StringLiteral(_) => Type::String,
        Type::NumberLiteral(_) => Type::Number,
        Type::BooleanLiteral(_) => Type::Boolean,
        // A named interface/type-alias object is not a fresh literal; preserve
        // it (and its alias name) as-is rather than widening its members.
        Type::Object(obj) if obj.alias_name.is_some() => ty.clone(),
        Type::Object(obj) => {
            let mut new_props = surge_ts_types::PropertyMap::default();
            for (k, v) in obj.properties.iter() {
                new_props.insert(
                    k.clone(),
                    surge_ts_types::ObjectProperty {
                        ty: widen_type(&v.ty),
                        optional: v.optional,
                        method: v.method,
                        readonly: false,
                        restriction: v.restriction.clone(),
                    },
                );
            }
            // Widening only touches fresh literals; the index signature, the
            // openness marker and the signatures still describe the value.
            let mut widened = alloc_object_type(
                new_props,
                obj.string_index_type.as_deref().map(widen_type),
            );
            if obj.synthetic_open_index {
                widened = widened.with_open_index_marker();
            }
            if let Some(call_signature) = obj.call_signature() {
                widened = widened.with_call_signature(call_signature.clone());
            }
            if let Some(construct_signature) = obj.construct_signature() {
                widened = widened.with_construct_signature(construct_signature.clone());
            }
            Type::Object(widened)
        }
        Type::Array(inner) => Type::Array(Box::new(widen_type(inner))),
        Type::Union(types) => {
            let widened: Vec<_> = types.types().iter().map(widen_type).collect();
            surge_ts_types::union_type(widened)
        }
        _ => ty.clone(),
    }
}

/// `true` if `ty` is a literal type or a union containing one. tsc keeps the
/// source literal in assignability messages when the target is literal-like.
/// tsc's `typeCouldHaveTopLevelSingletonTypes`: a unit type — a literal, or
/// `null`/`undefined` — at the top level of the target.
fn type_contains_literal(ty: &Type) -> bool {
    match ty {
        Type::StringLiteral(_)
        | Type::NumberLiteral(_)
        | Type::BooleanLiteral(_)
        | Type::Null
        | Type::Undefined => true,
        // An enum is a union of enum literal types.
        Type::Reference(reference) if reference.enum_owner.is_some() => true,
        // tsc's `isLiteralType`: a template literal or string mapping type
        // is one.
        ty if surge_ts_types::is_template_literal_type(ty)
            || surge_ts_types::string_mapping_parts(ty).is_some() =>
        {
            true
        }
        Type::Union(types) => types.types().iter().any(type_contains_literal),
        _ => false,
    }
}

/// Type name for the SOURCE side of an assignment/argument diagnostic, matching
/// tsc: a fresh literal source is widened (`g(1)` to `string` -> `'number'`)
/// unless the target is literal-like, where tsc keeps the literal (`f("b")` to
/// `"a"` -> `'"b"'`).
/// Whether a nominal reference carries a type parameter that no enclosing
/// declaration binds — an alias parameter that leaked out of an instantiation
/// surge could not complete (`Setdown<MergeC<A, B>>` reading as
/// `Partial<C1>`). tsc has the real type there; a member miss on the leaked
/// shape describes the gap, not the source, so it is treated as the sentinel.
pub(crate) fn carries_leaked_type_parameter(ty: &Type, ctx: &CheckerContext) -> bool {
    fn walk(ty: &Type, ctx: &CheckerContext, depth: usize) -> bool {
        if depth > 3 {
            return false;
        }
        match ty {
            Type::TypeParameter(parameter) => !ctx.type_parameter_in_scope(parameter.name.as_ref()),
            Type::Reference(reference) => reference
                .arguments
                .iter()
                .any(|argument| walk(argument, ctx, depth + 1)),
            _ => false,
        }
    }
    walk(ty, ctx, 0)
}

/// Builds the diagnostic for a missing property access. When the object is a
/// class instance whose static side declares the property, tsc emits TS2576
/// ("Did you mean to access the static member ...") instead of the plain TS2339.
pub(crate) fn missing_property_diagnostic(
    property_name: &str,
    object_type: &Type,
    symbols: &SymbolTable,
    file_name: String,
) -> Diagnostic {
    let object_type_name = object_type.name();
    if let Some(class_name) =
        static_member_owner_for_missing_instance_property(property_name, object_type, symbols)
    {
        return Diagnostic::ts2576(
            property_name,
            &object_type_name,
            format!("{class_name}.{property_name}"),
            file_name,
        );
    }

    // tsc asks this before looking for a misspelling (`at` is no typo of `map`).
    if let Some(lib) = lib_feature_of_missing_member(object_type, property_name) {
        return Diagnostic::ts2550(property_name, &object_type_name, lib, file_name);
    }
    if let Some(suggestion) = property_spelling_suggestion(property_name, object_type) {
        return Diagnostic::ts2551(property_name, &object_type_name, suggestion, file_name);
    }
    Diagnostic::ts2339(property_name, &object_type_name, file_name)
}

/// tsc's `getScriptTargetFeatures`, for the receivers surge answers from its
/// own tables: the lib that first declares `member` on an array or a string.
/// A member listed here that a receiver lacks is a lib the project did not
/// ask for, which tsc says (TS2550) instead of calling the member unknown.
pub(crate) fn lib_feature_of_missing_member(receiver: &Type, member: &str) -> Option<&'static str> {
    const ARRAY: &[(&str, &[&str])] = &[
        ("es2015", &["find", "findIndex", "fill", "copyWithin", "entries", "keys", "values"]),
        ("es2016", &["includes"]),
        ("es2019", &["flat", "flatMap"]),
        ("es2022", &["at"]),
        ("es2023", &["findLast", "findLastIndex", "toReversed", "toSorted", "toSpliced", "with"]),
    ];
    const STRING: &[(&str, &[&str])] = &[
        (
            "es2015",
            &[
                "codePointAt", "includes", "endsWith", "normalize", "repeat", "startsWith", "anchor",
                "big", "blink", "bold", "fixed", "fontcolor", "fontsize", "italics", "link", "small",
                "strike", "sub", "sup",
            ],
        ),
        ("es2017", &["padStart", "padEnd"]),
        ("es2019", &["trimStart", "trimEnd", "trimLeft", "trimRight"]),
        ("es2020", &["matchAll"]),
        ("es2021", &["replaceAll"]),
        ("es2022", &["at"]),
        ("esnext", &["isWellFormed", "toWellFormed"]),
    ];
    let features = match receiver {
        Type::Array(_) | Type::Tuple(_) => ARRAY,
        Type::String | Type::StringLiteral(_) => STRING,
        _ => return None,
    };
    features
        .iter()
        .find(|(_, members)| members.contains(&member))
        .map(|(lib, _)| *lib)
}

/// `String.prototype` members in lib declaration order, which breaks ties
/// between equally close suggestions.
const STRING_MEMBERS: &[&str] = &[
    "toString", "charAt", "charCodeAt", "concat", "indexOf", "lastIndexOf", "localeCompare",
    "match", "replace", "search", "slice", "split", "substring", "toLowerCase",
    "toLocaleLowerCase", "toUpperCase", "toLocaleUpperCase", "trim", "length", "substr",
    "valueOf", "codePointAt", "includes", "endsWith", "normalize", "repeat", "startsWith",
    "padStart", "padEnd", "trimEnd", "trimStart", "trimLeft", "trimRight", "matchAll",
    "replaceAll", "at",
];

const NUMBER_MEMBERS: &[&str] =
    &["toString", "toFixed", "toExponential", "toPrecision", "valueOf", "toLocaleString"];

/// tsc's `getSuggestedSymbolForNonexistentProperty` over the members surge can
/// enumerate for the receiver. A union receiver offers none: tsc suggests from
/// the members every constituent shares, which the reported member type does
/// not say.
pub(crate) fn property_spelling_suggestion(name: &str, object_type: &Type) -> Option<String> {
    // A union has the properties every member has, and tsc suggests among
    // those alone: `u.property1` on `{ property1 } | { property2 }` is a plain
    // TS2339, not a misspelling of the other member's `property2`.
    if let Type::Union(union) = object_type {
        let mut members = union
            .types()
            .iter()
            .filter(|member| !matches!(member, Type::Undefined | Type::Null | Type::Void));
        let first = members.next()?;
        let rest: Vec<&Type> = members.collect();
        let suggestion = property_spelling_suggestion(name, first)?;
        return rest
            .iter()
            .all(|member| member.get_property_access_type(&suggestion).is_some())
            .then_some(suggestion);
    }
    let candidates: Vec<String> = match object_type {
        Type::Reference(_) => match object_type.peeled() {
            Type::Reference(_) => return None,
            peeled => return property_spelling_suggestion(name, &peeled),
        },
        Type::Object(object) => object.properties.keys().map(|key| key.to_string()).collect(),
        Type::Array(_) | Type::Tuple(_) | Type::OpenTuple(_) => surge_ts_types::array_property_names()
            .iter()
            .filter(|member| object_type.get_property_access_type(member).is_some())
            .map(|member| member.to_string())
            .collect(),
        Type::String | Type::StringLiteral(_) => STRING_MEMBERS
            .iter()
            .filter(|member| Type::String.get_property_access_type(member).is_some())
            .map(|member| member.to_string())
            .collect(),
        Type::Number | Type::NumberLiteral(_) => NUMBER_MEMBERS
            .iter()
            .filter(|member| Type::Number.get_property_access_type(member).is_some())
            .map(|member| member.to_string())
            .collect(),
        _ => return None,
    };
    spelling_suggestion(
        name,
        candidates
            .iter()
            .map(String::as_str)
            .filter(|candidate| !candidate.starts_with('[')),
        0,
    )
    .map(str::to_string)
}

/// tsc's `getPropertyTypeForIndexType` for a literal key that neither a member
/// nor an index signature answers: under `noImplicitAny` the whole element
/// access is an implicit `any` (TS7053, or TS2576 for a static-member mixup);
/// without it the access silently reads `any`.
pub(crate) fn report_missing_element(
    key: &str,
    key_type: &Type,
    object_type: &Type,
    key_span: Option<SyntaxTextSpan>,
    access_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) {
    if !ctx.options.no_implicit_any {
        return;
    }
    let object_type_name = object_type.name();
    let file_name = ctx.file_name.clone();
    if static_member_owner_for_missing_instance_property(key, object_type, symbols).is_none()
        && let Some(suggestion) = property_spelling_suggestion(key, object_type)
    {
        ctx.push(diagnostic_with_syntax_span(
            Diagnostic::ts2551(key, &object_type_name, suggestion, file_name),
            key_span.or(access_span),
        ));
        return;
    }
    let diagnostic = match static_member_owner_for_missing_instance_property(key, object_type, symbols)
    {
        Some(class_name) => {
            let written_key = match key_type {
                Type::StringLiteral(_) => format!("\"{key}\""),
                _ => key.to_string(),
            };
            Diagnostic::ts2576(key, &object_type_name, format!("{class_name}[{written_key}]"), file_name)
        }
        None => Diagnostic::ts7053(key_type.name(), &object_type_name, file_name),
    };
    ctx.push(diagnostic_with_syntax_span(diagnostic, access_span));
}

/// `object[key]` from the start of the object through the closing bracket.
pub(crate) fn element_access_span(
    object_span: Option<SyntaxTextSpan>,
    key_span: Option<SyntaxTextSpan>,
) -> Option<SyntaxTextSpan> {
    let (object_span, key_span) = (object_span?, key_span?);
    Some(SyntaxTextSpan {
        start: object_span.start,
        end: key_span.end + 1,
    })
}

const OBJECT_PROTOTYPE_MEMBERS: &[&str] = &[
    "constructor",
    "hasOwnProperty",
    "isPrototypeOf",
    "propertyIsEnumerable",
    "toLocaleString",
    "toString",
    "valueOf",
];

/// TS4111 under `noPropertyAccessFromIndexSignature`: a dotted `obj.foo` whose
/// `foo` resolves through a string index signature rather than a declared
/// property must instead be written `obj["foo"]`. No-op unless the flag is set.
pub(super) fn maybe_emit_index_signature_access(
    object: &ParsedExpression,
    property_name: &str,
    property_span: Option<SyntaxTextSpan>,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) {
    if !ctx.options.no_property_access_from_index_signature {
        return;
    }
    // `Object.prototype` members resolve on the apparent type, not through the
    // index signature, so `record.constructor` is a plain property access to tsc.
    if OBJECT_PROTOTYPE_MEMBERS.contains(&property_name) {
        return;
    }
    if let InferredExpression::Known(object_type) = infer_expression(object, symbols, ctx) {
        if object_type.property_only_from_string_index(property_name) {
            ctx.push(diagnostic_with_syntax_span(
                Diagnostic::ts4111(property_name, ctx.file_name.clone()),
                choose_span(property_span, fallback_span),
            ));
        }
    }
}

/// Returns the class name when `object_type` is a class instance (an object
/// tagged with the class name) and the class's static side declares
/// `property_name`, so the access should be reported as a static-member mixup.
fn static_member_owner_for_missing_instance_property(
    property_name: &str,
    object_type: &Type,
    symbols: &SymbolTable,
) -> Option<String> {
    // A class instance type is a nominal reference; peel it to read the class
    // name (its object's `alias_name`) and detect the static-member mixup.
    let object_type = object_type.peeled();
    let Type::Object(instance) = &object_type else {
        return None;
    };
    let class_name = instance.alias_name.as_deref()?;
    let symbol = symbols.get(class_name)?;
    let Type::Object(static_side) = &symbol.ty else {
        return None;
    };
    if static_side.construct_signature().is_some()
        && static_side.get_property(property_name).is_some()
    {
        Some(class_name.to_string())
    } else {
        None
    }
}

/// The target a relation failure is reported against. tsc's `isRelatedTo`
/// relates a definitely non-nullable source to a `T | null | undefined` target
/// as `T` alone, so the message names `T`.
pub(crate) fn reported_relation_target(source: &Type, target: &Type) -> Type {
    let peeled_source;
    let source = match source {
        Type::Reference(_) => {
            peeled_source = source.peeled();
            &peeled_source
        }
        other => other,
    };
    let definitely_non_nullable = matches!(
        source,
        Type::String
            | Type::Number
            | Type::Boolean
            | Type::BigInt
            | Type::Symbol
            | Type::StringLiteral(_)
            | Type::NumberLiteral(_)
            | Type::BooleanLiteral(_)
            | Type::Object(_)
            | Type::Function(_)
            | Type::Array(_)
            | Type::Tuple(_)
            | Type::OpenTuple(_)
    );
    let Type::Union(union) = target else {
        return target.clone();
    };
    if !definitely_non_nullable || union.types().len() > 3 {
        return target.clone();
    }
    let mut non_nullable = union
        .types()
        .iter()
        .filter(|member| !matches!(member, Type::Null | Type::Undefined));
    match (non_nullable.next(), non_nullable.next()) {
        (Some(only), None) => only.clone(),
        _ => target.clone(),
    }
}

pub(crate) fn source_display_name(source: &Type, target: &Type) -> String {
    // tsc does not generalize the source when the target is `never`, though
    // an object literal's own property types are already widened.
    if matches!(target, Type::Never) && matches!(source, Type::Object(_)) {
        widen_type(source).name()
    } else if type_contains_literal(target) || matches!(target, Type::Never) {
        source.name()
    } else if let Some(enum_name) = enum_base_display(source) {
        enum_name
    } else {
        widen_type(source).name()
    }
}

/// `getBaseTypeOfLiteralType` for enum literals: an enum member type (or a
/// union of one enum's members) generalizes to the enum itself.
fn enum_base_display(source: &Type) -> Option<String> {
    let member_base = |ty: &Type| -> Option<(std::sync::Arc<str>, String)> {
        let Type::Reference(reference) = ty else {
            return None;
        };
        let owner = reference.enum_owner.clone()?;
        let display = reference.display.to_string();
        let base = if *reference.id == *owner {
            display
        } else {
            display.rsplit_once('.').map(|(base, _)| base.to_string())?
        };
        Some((owner, base))
    };
    match source {
        Type::Reference(_) => member_base(source).map(|(_, base)| base),
        Type::Union(union) => {
            let mut members = union.types().iter().map(member_base);
            let (owner, base) = members.next()??;
            members
                .all(|member| member.is_some_and(|(other, _)| other == owner))
                .then_some(base)
        }
        _ => None,
    }
}

/// The declaring file of a named type, recovered from the nominal id both
/// object aliases and lazy references carry (`<file>\0<name>`).
fn declaring_file_of(ty: &Type) -> Option<String> {
    let id = match ty {
        Type::Object(object) => object.alias_id.as_deref()?,
        Type::Reference(reference) => reference.id.as_ref(),
        _ => return None,
    };
    let (file, _) = id.split_once('\0')?;
    (!file.is_empty()).then(|| file.to_string())
}

/// tsc disambiguates two same-named types by qualifying the non-local one with
/// the module it came from (`import("/abs/path/to/store").Store`), so a message
/// never reads `'Store' is not assignable to type 'Store'`. The local side keeps
/// its bare name.
pub(crate) fn disambiguated_type_name(ty: &Type, rendered: &str, current_file: &str) -> String {
    let Some(file) = declaring_file_of(ty) else {
        return rendered.to_string();
    };
    if file == current_file {
        return rendered.to_string();
    }
    let module = [".d.ts", ".d.mts", ".d.cts", ".tsx", ".ts", ".mts", ".cts"]
        .iter()
        .find_map(|extension| file.strip_suffix(extension))
        .unwrap_or(&file);
    format!("import(\"{module}\").{rendered}")
}

/// Renders both sides of an assignability diagnostic, qualifying whichever is
/// non-local when the two would otherwise print the same name.
pub(crate) fn disambiguated_pair(
    source: &Type,
    source_name: String,
    target: &Type,
    target_name: String,
    current_file: &str,
) -> (String, String) {
    if source_name != target_name {
        return (source_name, target_name);
    }
    (
        disambiguated_type_name(source, &source_name, current_file),
        disambiguated_type_name(target, &target_name, current_file),
    )
}

/// Type name for an operand of an operator diagnostic (TS2365/TS2367), matching
/// tsc, which always widens fresh literal operands for display (e.g.
/// `1 === "string"` -> `'number'` and `'string'`).
pub(crate) fn operand_display_name(ty: &Type) -> String {
    widen_type(ty).name()
}
