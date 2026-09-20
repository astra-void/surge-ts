//! `private`/`protected` member access (tsc's `checkPropertyAccessibility`).

use std::sync::Arc;

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{
    ParsedAccessorSide, ParsedClassDeclaration, ParsedExpression, ParsedMemberAccessibility,
    TextSpan as SyntaxTextSpan,
};
use surge_ts_types::Type;

use crate::context::CheckerContext;
use crate::symbols::{InterfaceInfo, SymbolKind, SymbolTable, TypeDeclarationInfo};

use super::diagnostic_with_syntax_span;

/// A class declaration's identity: its file and the offset of its name. Names
/// alone are not enough — an import can rename a class, and two files can
/// declare the same one.
pub(crate) type ClassIdentity = (Arc<str>, usize);

/// Inheritance chains are short; the bound only guards a malformed cycle.
const MAX_HERITAGE_DEPTH: usize = 32;

fn identity(info: &InterfaceInfo) -> Option<ClassIdentity> {
    Some((info.file_name.clone(), info.name_span?.start))
}

fn base_interface(info: &InterfaceInfo, ctx: &CheckerContext) -> Option<InterfaceInfo> {
    let base = info.body.extends.first()?;
    // The declaration's own scope answers first, but it carries only the layer
    // it was bound in: a base imported into the subclass's file is not in it,
    // and stopping there cut the lineage short — every `super.x` on a
    // `protected` member of a cross-module base read as out of reach.
    let declaration = info
        .resolution_scope
        .as_ref()
        .and_then(|scope| scope.get(&base.name))
        .or_else(|| ctx.lookup_type_declaration(&base.name));
    match declaration? {
        TypeDeclarationInfo::Interface(base) => Some(base.clone()),
        TypeDeclarationInfo::Alias(_) => None,
    }
}

/// The class being entered and every class it extends, pushed for the length
/// of its body so an access inside it can tell what it may reach.
pub(crate) fn enclosing_class_lineage(
    class: &ParsedClassDeclaration,
    ctx: &mut CheckerContext,
) -> Vec<ClassIdentity> {
    let mut lineage = Vec::new();
    let Some(span) = class.name_span else {
        return lineage;
    };
    lineage.push((ctx.file_name_arc(), span.start));
    let mut current = match ctx.lookup_type_declaration(&class.name) {
        Some(TypeDeclarationInfo::Interface(info)) => info.clone(),
        _ => return lineage,
    };
    for _ in 0..MAX_HERITAGE_DEPTH {
        let Some(base) = base_interface(&current, ctx) else {
            break;
        };
        if let Some(identity) = identity(&base) {
            lineage.push(identity);
        }
        current = base;
    }
    lineage
}

/// The class that declares `member` restricted, found by walking up from the
/// receiver's class. A class along the way that declares the member without a
/// modifier makes it public from there on.
fn restricted_member_owner(
    receiver_class: &InterfaceInfo,
    member: &str,
    is_static: bool,
    is_write: bool,
    ctx: &CheckerContext,
) -> Option<(InterfaceInfo, ParsedMemberAccessibility)> {
    // The accessor side the access goes through; the other side's modifier
    // does not apply to it.
    let other_side = if is_write {
        ParsedAccessorSide::Get
    } else {
        ParsedAccessorSide::Set
    };
    let mut current = receiver_class.clone();
    for _ in 0..MAX_HERITAGE_DEPTH {
        if let Some(restricted) = current.body.restricted_members.iter().find(|restricted| {
            restricted.name == member
                && restricted.is_static == is_static
                && restricted.accessor_side != Some(other_side)
        }) {
            let accessibility = restricted.accessibility;
            return Some((current, accessibility));
        }
        if !is_static && current.body.members.iter().any(|declared| declared.name == member) {
            return None;
        }
        current = base_interface(&current, ctx)?;
    }
    None
}

/// The class a member read goes through, and whether the read is of its static
/// side. An instance is a reference to the class's declaration; the static side
/// is the class binding itself, written by name.
fn receiver_class(
    object: &ParsedExpression,
    receiver_type: &Type,
    symbols: &SymbolTable,
    ctx: &CheckerContext,
) -> Option<(InterfaceInfo, bool)> {
    if let Type::Reference(reference) = receiver_type {
        let mut parts = reference.id.split('\u{0}');
        let file = parts.next()?;
        let name = parts.next_back()?;
        let TypeDeclarationInfo::Interface(info) = ctx.lookup_type_declaration(name)? else {
            return None;
        };
        // A different declaration that merely shares the name is not the
        // receiver's class.
        return (info.file_name.as_ref() == file).then(|| (info.clone(), false));
    }
    if let Some(class) = static_receiver_class(object, symbols, ctx) {
        return Some((class, true));
    }
    // Narrowing one of an instance's members materializes the instance as an
    // object that keeps only the class's name.
    if let Type::Object(object) = receiver_type
        && let Some(name) = object.alias_name.as_deref()
        && let Some(TypeDeclarationInfo::Interface(info)) = ctx.lookup_type_declaration(name)
    {
        return Some((info.clone(), false));
    }
    None
}

/// The class a static read goes through: the class binding itself, written by
/// name. Looked up by name over the type table, so a binding that shadows the
/// class (a parameter or a variable holding some other value) does not denote
/// it.
fn static_receiver_class(
    object: &ParsedExpression,
    symbols: &SymbolTable,
    ctx: &CheckerContext,
) -> Option<InterfaceInfo> {
    let ParsedExpression::Identifier { name, .. } = object else {
        return None;
    };
    if matches!(
        symbols.get(name).map(|symbol| symbol.kind),
        Some(SymbolKind::Parameter | SymbolKind::Let | SymbolKind::Var) | None
    ) {
        return None;
    }
    match ctx.lookup_type_declaration(name)? {
        TypeDeclarationInfo::Interface(info) => Some(info.clone()),
        TypeDeclarationInfo::Alias(_) => None,
    }
}

/// tsc's `checkPropertyAccessibility` for a named member read: a `private`
/// member is reachable only inside the class that declares it (TS2341), a
/// `protected` one inside that class or a class deriving from it (TS2445).
pub(crate) fn check_member_accessibility(
    object: &ParsedExpression,
    receiver_type: &Type,
    member: &str,
    member_span: Option<SyntaxTextSpan>,
    is_write: bool,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) {
    // Accessibility belongs to the declaration, so a receiver narrowed by an
    // earlier check on one of its members (`if (!c.x) return; c.x = …`) is
    // judged by its declared type. `maybe?.x` reads the member off what the
    // optional chain leaves defined.
    let declared = match object {
        ParsedExpression::Identifier { name, .. } => symbols.declared_type(name),
        _ => None,
    };
    let receiver_type = surge_ts_types::remove_undefined(declared.unwrap_or(receiver_type));
    let Some((class, is_static)) = receiver_class(object, &receiver_type, symbols, ctx) else {
        return;
    };
    let Some((declaring, accessibility)) =
        restricted_member_owner(&class, member, is_static, is_write, ctx)
    else {
        return;
    };
    let Some(declaring_identity) = identity(&declaring) else {
        return;
    };
    let accessible = ctx.enclosing_classes.iter().any(|lineage| match accessibility {
        ParsedMemberAccessibility::Private => lineage.first() == Some(&declaring_identity),
        ParsedMemberAccessibility::Protected => lineage.contains(&declaring_identity),
    });
    if accessible {
        return;
    }
    let class_name = declaring
        .declared_name
        .as_deref()
        .unwrap_or(&declaring.name)
        .to_string();
    let diagnostic = match accessibility {
        ParsedMemberAccessibility::Private => {
            Diagnostic::ts2341(member, class_name, ctx.file_name.clone())
        }
        ParsedMemberAccessibility::Protected => {
            Diagnostic::ts2445(member, class_name, ctx.file_name.clone())
        }
    };
    ctx.push(diagnostic_with_syntax_span(diagnostic, member_span));
}
