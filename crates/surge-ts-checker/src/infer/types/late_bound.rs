//! tsc's `lateBindMember` for the members of a type literal, interface or
//! class: a computed name written as an entity name (`[c0]`, `[N.s1]`) names
//! the member by its key's type when that type is usable as a property name
//! (`isTypeUsableAsPropertyName`). The parser keeps such a member under its
//! written key in brackets; a string or number literal key becomes the literal
//! property, and a unique symbol the `[declaration]` name its element
//! accesses look up.

use std::cell::Cell;
use std::sync::Arc;

use surge_ts_syntax::{ParsedType, ParsedTypeOfType};
use surge_ts_types::{PropertyMap, Type};

use crate::context::CheckerContext;

thread_local! {
    /// A key whose type reads the declaration being bound (`[rI.x]` inside
    /// `interface RI` with `declare const rI: RI<"a">`) must not late-bind
    /// that declaration's members again while it is resolved.
    static LATE_BINDING: Cell<bool> = const { Cell::new(false) };
}

/// Renames the late-bindable members of `properties`. `true` when a key's
/// value could not be resolved yet (a class built while signatures are
/// collected, before the module's values are bound): the shape keeps the
/// written name and must not be cached as final.
pub(crate) fn late_bind_member_names(properties: &mut PropertyMap, ctx: &mut CheckerContext) -> bool {
    if !properties.keys().any(|name| is_late_bindable_name(name)) || LATE_BINDING.with(Cell::get) {
        return false;
    }
    LATE_BINDING.with(|active| active.set(true));
    let mut renamed: Vec<(Arc<str>, Arc<str>)> = Vec::new();
    let mut unresolved = false;
    for name in properties.keys().filter(|name| is_late_bindable_name(name)) {
        match late_bound_name(name, ctx) {
            Ok(Some(bound)) if bound.as_str() != name.as_ref() => renamed.push((name.clone(), bound.into())),
            Ok(_) => {}
            Err(Unresolved) => unresolved = true,
        }
    }
    LATE_BINDING.with(|active| active.set(false));
    if renamed.is_empty() {
        return unresolved;
    }
    let previous = std::mem::take(properties);
    for (name, property) in previous {
        let name = renamed
            .iter()
            .find(|(written, _)| *written == name)
            .map_or(name, |(_, bound)| bound.clone());
        // A name already declared keeps its first declaration, as the
        // symbol it merges into does.
        properties.entry(name).or_insert(property);
    }
    unresolved
}

fn is_late_bindable_name(name: &str) -> bool {
    name.starts_with('[') && name.ends_with(']') && !name.starts_with("[Symbol.")
}

struct Unresolved;

/// The member name a key written as `[path]` binds to, `None` when its type is
/// not usable as a property name.
fn late_bound_name(written: &str, ctx: &mut CheckerContext) -> Result<Option<String>, Unresolved> {
    let path = &written[1..written.len() - 1];
    let mut segments = path.split('.');
    let Some(name) = segments.next().map(str::to_string) else {
        return Ok(None);
    };
    let members: Vec<String> = segments.map(str::to_string).collect();
    if name.is_empty() || members.iter().any(String::is_empty) {
        return Ok(None);
    }
    let query = ParsedType::TypeOf(Arc::new(ParsedTypeOfType {
        name,
        name_span: None,
        members,
        import_specifier: None,
        member_spans: Vec::new(),
        type_arguments: Vec::new(),
        type_arguments_span: None,
    }));
    let checkpoint = ctx.diagnostics().len();
    let key_type = super::map_parsed_type(query, ctx);
    ctx.truncate_diagnostics_releasing_utility_keys(checkpoint);
    if key_type.is_unknown() || matches!(key_type, Type::ErrorType) {
        return Err(Unresolved);
    }
    Ok(match &key_type {
        Type::StringLiteral(value) => Some(value.clone()),
        Type::NumberLiteral(literal) => Some(literal.value.clone()),
        Type::Reference(reference) if reference.enum_base.is_some() => match key_type.peeled() {
            Type::StringLiteral(value) => Some(value),
            Type::NumberLiteral(literal) => Some(literal.value),
            _ => None,
        },
        Type::Reference(reference) => reference
            .unique_symbol_name()
            .map(|declaration| format!("[{declaration}]")),
        _ => None,
    })
}
