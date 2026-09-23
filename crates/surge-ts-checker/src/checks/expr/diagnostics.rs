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
                        index_slot: v.index_slot,
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
            if obj.non_primitive {
                widened = widened.with_non_primitive_marker();
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
/// tsc's `isUnitType`: a literal, `null` or `undefined`. An enum is a union
/// of enum literal types.
fn is_unit_type(ty: &Type) -> bool {
    match ty {
        Type::StringLiteral(_)
        | Type::NumberLiteral(_)
        | Type::BooleanLiteral(_)
        | Type::Null
        | Type::Undefined => true,
        Type::Reference(reference) => reference.enum_owner.is_some(),
        _ => false,
    }
}

/// tsc's `isLiteralType`, which `reportRelationError` generalizes. An object
/// source is still widened: its fresh literal members print generalized.
fn is_literal_like(ty: &Type) -> bool {
    match ty {
        Type::Boolean | Type::Object(_) | Type::Array(_) | Type::Tuple(_) => true,
        Type::Union(union) => union
            .types()
            .iter()
            .all(|member| is_unit_type(member) || *member == Type::Boolean),
        other => is_unit_type(other),
    }
}

/// tsc's `typeCouldHaveTopLevelSingletonTypes`.
fn could_have_singleton_types(ty: &Type) -> bool {
    match ty {
        Type::Boolean => false,
        Type::Union(union) => union.types().iter().any(could_have_singleton_types),
        // A template literal or string mapping type is not a unit type, but
        // it can hold one.
        ty if surge_ts_types::is_template_literal_type(ty)
            || surge_ts_types::string_mapping_parts(ty).is_some() =>
        {
            true
        }
        other => is_unit_type(other),
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
/// tsc's rule for a member `globalThis` lacks: TS2339 only when the name is a
/// block-scoped global (which never lands on the global object), otherwise
/// TS7017 under `noImplicitAny` and nothing without it — the read is `any`.
/// `None` when the receiver is not `globalThis`.
pub(crate) fn global_this_missing_member(
    property_name: &str,
    object_type: &Type,
    ctx: &CheckerContext,
) -> Option<Option<Diagnostic>> {
    if !matches!(object_type, Type::Reference(reference)
        if &*reference.id == crate::driver::GLOBAL_THIS_REFERENCE_ID)
    {
        return None;
    }
    let block_scoped = ctx.block_scoped_globals.contains(property_name);
    let file_name = ctx.file_name.clone();
    Some(if block_scoped {
        Some(Diagnostic::ts2339(
            property_name,
            "typeof globalThis",
            file_name,
        ))
    } else if ctx.options.no_implicit_any {
        Some(Diagnostic::ts7017("typeof globalThis", file_name))
    } else {
        None
    })
}

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

thread_local! {
    static ELEMENT_WRITE_TARGET: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Evaluates an update operand, which tsc checks as an assignment target: the
/// element access at its top is a write.
pub(crate) fn with_element_write_target<R>(evaluate: impl FnOnce() -> R) -> R {
    ELEMENT_WRITE_TARGET.with(|flag| flag.set(true));
    let result = evaluate();
    ELEMENT_WRITE_TARGET.with(|flag| flag.set(false));
    result
}

/// Whether the element access being evaluated is that write target. Taken on
/// entry, so the receiver and key it evaluates are reads.
pub(crate) fn take_element_write_target() -> bool {
    ELEMENT_WRITE_TARGET.with(|flag| flag.replace(false))
}

/// What tsc reads off an element access itself when it reports a key the
/// receiver lacks.
pub(crate) struct ElementAccessSite {
    /// The receiver as an access path (`c`, `m.prop`), which TS7052's
    /// suggestion is spelled from (`tryGetPropertyAccessOrIdentifierToString`).
    pub(crate) receiver_text: Option<String>,
    /// The receiver is an object literal written in place (or a literal
    /// property of one), whose type is still the object-literal type
    /// (`isObjectLiteralType`).
    pub(crate) receiver_is_object_literal: bool,
    /// The access is an assignment target, whose receiver tsc widens first
    /// and whose likely method is `set`.
    pub(crate) is_write: bool,
}

impl ElementAccessSite {
    pub(crate) fn of(receiver: &ParsedExpression, is_write: bool) -> Self {
        Self {
            receiver_text: access_path_text(receiver),
            receiver_is_object_literal: object_literal_properties(receiver).is_some(),
            is_write,
        }
    }

    pub(crate) fn named(name: &str, is_write: bool) -> Self {
        Self {
            receiver_text: Some(name.to_string()),
            receiver_is_object_literal: false,
            is_write,
        }
    }
}

/// The properties of the object literal `expression` is, directly or as a
/// literal-valued property of one.
fn object_literal_properties(expression: &ParsedExpression) -> Option<&[surge_ts_syntax::ParsedObjectProperty]> {
    match expression {
        ParsedExpression::ObjectLiteral { properties, .. } => Some(properties),
        ParsedExpression::ConstAssertion { expression, .. } => object_literal_properties(expression),
        ParsedExpression::PropertyAccess {
            object,
            property_name,
            ..
        } => {
            let property = object_literal_properties(object)?
                .iter()
                .rev()
                .find(|property| !property.is_spread && property.name == *property_name)?;
            object_literal_properties(&property.value)
        }
        _ => None,
    }
}

fn access_path_text(expression: &ParsedExpression) -> Option<String> {
    match expression {
        ParsedExpression::Identifier { name, .. } => Some(name.clone()),
        ParsedExpression::PropertyAccess {
            object,
            property_name,
            ..
        } => Some(format!("{}.{property_name}", access_path_text(object)?)),
        _ => None,
    }
}

/// tsc's `getSuggestionForNonexistentIndexSignature`: a receiver whose own `get`
/// method takes the key as its first argument was probably meant to be called.
fn index_signature_method_suggestion(
    object_type: &Type,
    key_type: &Type,
    site: &ElementAccessSite,
) -> Option<String> {
    let method = if site.is_write { "set" } else { "get" };
    let Type::Object(object) = object_type.peeled() else {
        return None;
    };
    let property = object.properties.get(method)?;
    let signature = match property.ty.peeled() {
        Type::Function(function) if function.overloads().is_none_or(|members| members.len() <= 1) => function,
        _ => return None,
    };
    let first = signature.parameters().first()?;
    if signature.required_parameter_count() < 1 || !surge_ts_types::is_assignable_to(key_type, first) {
        return None;
    }
    Some(match &site.receiver_text {
        Some(text) => format!("{text}.{method}"),
        None => method.to_string(),
    })
}

/// tsc's `getPropertyTypeForIndexType` for a literal key that neither a member
/// nor an index signature answers: under `noImplicitAny` the whole element
/// access is an implicit `any` (TS7053, or TS2576 for a static-member mixup,
/// TS7052 when a `get` method was likely meant), and on an object literal
/// written in place the key is a missing property (TS2339); without
/// `noImplicitAny` the access silently reads `any`. A block-scoped global read
/// through `globalThis` is a missing property whatever the setting.
#[allow(clippy::too_many_arguments)]
pub(crate) fn report_missing_element(
    key: &str,
    key_type: &Type,
    object_type: &Type,
    key_span: Option<SyntaxTextSpan>,
    access_span: Option<SyntaxTextSpan>,
    site: &ElementAccessSite,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) {
    if matches!(object_type, Type::Reference(reference)
        if &*reference.id == crate::driver::GLOBAL_THIS_REFERENCE_ID)
        && ctx.block_scoped_globals.contains(key)
    {
        let file_name = ctx.file_name.clone();
        ctx.push(diagnostic_with_syntax_span(
            Diagnostic::ts2339(key, "typeof globalThis", file_name),
            access_span,
        ));
        return;
    }
    if !ctx.options.no_implicit_any {
        return;
    }
    let object_type_name = object_type.name();
    let file_name = ctx.file_name.clone();
    if site.receiver_is_object_literal
        && !site.is_write
        && matches!(key_type, Type::StringLiteral(_) | Type::NumberLiteral(_))
    {
        ctx.push(diagnostic_with_syntax_span(
            Diagnostic::ts2339(key, &object_type_name, file_name),
            access_span,
        ));
        return;
    }
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
        None => match index_signature_method_suggestion(object_type, key_type, site) {
            Some(suggestion) => Diagnostic::ts7052(&object_type_name, suggestion, file_name),
            None => Diagnostic::ts7053(key_type.name(), &object_type_name, file_name),
        },
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
        emit_index_signature_access_on(
            &object_type,
            property_name,
            choose_span(property_span, fallback_span),
            ctx,
        );
    }
}

/// [`maybe_emit_index_signature_access`] for a receiver whose type is already
/// known, as a member write has it.
pub(crate) fn emit_index_signature_access_on(
    object_type: &Type,
    property_name: &str,
    span: Option<SyntaxTextSpan>,
    ctx: &mut CheckerContext,
) {
    if !ctx.options.no_property_access_from_index_signature
        || OBJECT_PROTOTYPE_MEMBERS.contains(&property_name)
    {
        return;
    }
    if object_type.property_only_from_string_index(property_name) {
        ctx.push(diagnostic_with_syntax_span(
            Diagnostic::ts4111(property_name, ctx.file_name.clone()),
            span,
        ));
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
    } else if could_have_singleton_types(target)
        || matches!(target, Type::Never)
        || !is_literal_like(source)
    {
        // `reportRelationError` generalizes only a literal source (every member
        // a unit type: `1`, `"a" | "b"`, not `number | "x"`), and only toward a
        // target that could not hold it.
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
