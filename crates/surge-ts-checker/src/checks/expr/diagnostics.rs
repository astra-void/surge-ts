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
                    },
                );
            }
            Type::Object(alloc_object_type(new_props, None))
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
fn type_contains_literal(ty: &Type) -> bool {
    match ty {
        Type::StringLiteral(_) | Type::NumberLiteral(_) | Type::BooleanLiteral(_) => true,
        Type::Union(types) => types.types().iter().any(type_contains_literal),
        _ => false,
    }
}

/// Type name for the SOURCE side of an assignment/argument diagnostic, matching
/// tsc: a fresh literal source is widened (`g(1)` to `string` -> `'number'`)
/// unless the target is literal-like, where tsc keeps the literal (`f("b")` to
/// `"a"` -> `'"b"'`).
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
        return Diagnostic::ts2576(property_name, &object_type_name, &class_name, file_name);
    }

    Diagnostic::ts2339(property_name, &object_type_name, file_name)
}

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

pub(crate) fn source_display_name(source: &Type, target: &Type) -> String {
    if type_contains_literal(target) {
        source.name()
    } else {
        widen_type(source).name()
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
