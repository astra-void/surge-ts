//! `private`/`protected` member access (tsc's `checkPropertyAccessibility`).

use std::cell::RefCell;
use std::sync::Arc;

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{
    ParsedAccessorSide, ParsedClassDeclaration, ParsedExpression, ParsedMemberAccessibility,
    ParsedType, ParsedTypeParameter, TextSpan as SyntaxTextSpan,
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

fn display_name(info: &InterfaceInfo) -> String {
    info.declared_name.as_deref().unwrap_or(&info.name).to_string()
}

/// A class and every class it extends.
fn class_lineage(class: &InterfaceInfo, ctx: &CheckerContext) -> Vec<ClassIdentity> {
    let mut lineage: Vec<ClassIdentity> = identity(class).into_iter().collect();
    let mut current = class.clone();
    for _ in 0..MAX_HERITAGE_DEPTH {
        let Some(base) = base_interface(&current, ctx) else {
            break;
        };
        lineage.extend(identity(&base));
        current = base;
    }
    lineage
}

/// The class named by the `this` parameter of the function being checked
/// (tsc's `getEnclosingClassFromThisParameter`), which reaches the protected
/// instance members of the classes it extends.
#[derive(Clone)]
pub(crate) struct ThisParameterClass {
    lineage: Vec<ClassIdentity>,
    name: String,
}

thread_local! {
    static THIS_PARAMETER_CLASS: RefCell<Option<ThisParameterClass>> = const { RefCell::new(None) };
}

/// Makes `class` the `this`-parameter class until dropped. A function binds its
/// own `this`, so every function body enters one, `None` when it has no `this`
/// parameter; an arrow keeps its enclosing function's.
pub(crate) struct ThisParameterClassScope(Option<ThisParameterClass>);

impl ThisParameterClassScope {
    pub(crate) fn enter(class: Option<ThisParameterClass>) -> Self {
        Self(THIS_PARAMETER_CLASS.with(|current| current.replace(class)))
    }
}

impl Drop for ThisParameterClassScope {
    fn drop(&mut self) {
        let outer = self.0.take();
        THIS_PARAMETER_CLASS.with(|current| *current.borrow_mut() = outer);
    }
}

/// The class or interface a written `this` parameter names, the constraint's
/// when it names one of the function's type parameters.
pub(crate) fn this_parameter_class(
    written: Option<&ParsedType>,
    type_parameters: &[ParsedTypeParameter],
    ctx: &mut CheckerContext,
) -> Option<ThisParameterClass> {
    let mut written = written?;
    if let ParsedType::Named(named) = written
        && named.type_arguments.is_empty()
        && let Some(parameter) = type_parameters
            .iter()
            .find(|parameter| parameter.name == named.name)
    {
        written = parameter.constraint.as_ref()?;
    }
    // The annotation reports its own errors where the signature is resolved.
    let checkpoint = ctx.diagnostics().len();
    let ty = crate::checks::function::with_type_parameter_scope(type_parameters, ctx, |ctx| {
        crate::infer::map_parsed_type(written.clone(), ctx)
    });
    ctx.truncate_diagnostics_releasing_utility_keys(checkpoint);
    let class = instance_class(&ty, ctx)?;
    Some(ThisParameterClass {
        lineage: class_lineage(&class, ctx),
        name: display_name(&class),
    })
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
    let mut current = match ctx.lookup_type_declaration(&class.name) {
        Some(TypeDeclarationInfo::Interface(info)) => info.clone(),
        _ => {
            lineage.push((ctx.file_name_arc(), span.start));
            return lineage;
        }
    };
    // A class merged with a same-named interface is one declaration, and a
    // member read resolves its owner to the merged record, which carries the
    // first fragment's name — the class's own span would never match it.
    lineage.push(
        identity(&current)
            .filter(|(file, _)| **file == *ctx.file_name)
            .unwrap_or_else(|| (ctx.file_name_arc(), span.start)),
    );
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
    if let Type::Reference(_) = receiver_type {
        return instance_class(receiver_type, ctx).map(|class| (class, false));
    }
    if let Some(class) = static_receiver_class(object, symbols, ctx) {
        return Some((class, true));
    }
    instance_class(receiver_type, ctx).map(|class| (class, false))
}

/// The class (or interface) whose instance `ty` is.
fn instance_class(ty: &Type, ctx: &CheckerContext) -> Option<InterfaceInfo> {
    if let Type::Reference(reference) = ty {
        let mut parts = reference.id.split('\u{0}');
        let file = parts.next()?;
        let name = parts.next_back()?;
        let TypeDeclarationInfo::Interface(info) = ctx.lookup_type_declaration(name)? else {
            return None;
        };
        // A different declaration that merely shares the name is not the
        // receiver's class.
        return (info.file_name.as_ref() == file).then(|| info.clone());
    }
    // Narrowing one of an instance's members materializes the instance as an
    // object that keeps only the class's name.
    // An instantiation (a generic class's own `C<T>` inside its body) keeps
    // its arguments in the name.
    if let Type::Object(object) = ty
        && let Some(name) = object.alias_name.as_deref()
        && let Some(TypeDeclarationInfo::Interface(info)) =
            ctx.lookup_type_declaration(name.split('<').next().unwrap_or(name))
    {
        return Some(info.clone());
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

/// tsc's `checkPropertyAccessibilityAtLocation` for a named member read: a
/// `private` member is reachable only inside the class that declares it
/// (TS2341), a `protected` one from a class deriving from it (TS2445) and only
/// through an instance of that class (TS2446).
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
    let receiver_type = surge_ts_types::remove_nullish(declared.unwrap_or(receiver_type));
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
    let diagnostic = match accessibility {
        ParsedMemberAccessibility::Private => {
            if ctx
                .enclosing_classes
                .iter()
                .any(|lineage| lineage.first() == Some(&declaring_identity))
            {
                return;
            }
            Diagnostic::ts2341(member, display_name(&declaring), ctx.file_name.clone())
        }
        ParsedMemberAccessibility::Protected => {
            // A `super` access reaches every protected member of the base.
            if matches!(object, ParsedExpression::Identifier { name, .. } if name == "super") {
                return;
            }
            let Some((enclosing, enclosing_name)) =
                protected_access_class(&declaring_identity, is_static, ctx)
            else {
                let diagnostic =
                    Diagnostic::ts2445(member, display_name(&declaring), ctx.file_name.clone());
                ctx.push(diagnostic_with_syntax_span(diagnostic, member_span));
                return;
            };
            if is_static || class_lineage(&class, ctx).contains(&enclosing) {
                return;
            }
            Diagnostic::ts2446(member, enclosing_name, receiver_type.name(), ctx.file_name.clone())
        }
    };
    ctx.push(diagnostic_with_syntax_span(diagnostic, member_span));
}

/// The class a protected member is reached from: the innermost enclosing class
/// that derives from the member's declaring class, or else — for an instance
/// member — the class the enclosing function's `this` parameter names.
fn protected_access_class(
    declaring: &ClassIdentity,
    is_static: bool,
    ctx: &CheckerContext,
) -> Option<(ClassIdentity, String)> {
    if let Some(lineage) = ctx
        .enclosing_classes
        .iter()
        .rev()
        .find(|lineage| lineage.contains(declaring))
    {
        let enclosing = lineage.first()?.clone();
        let name = enclosing_class_name(&enclosing, ctx);
        return Some((enclosing, name));
    }
    if is_static {
        return None;
    }
    THIS_PARAMETER_CLASS.with(|current| {
        let current = current.borrow();
        let class = current.as_ref().filter(|class| class.lineage.contains(declaring))?;
        Some((class.lineage.first()?.clone(), class.name.clone()))
    })
}

fn enclosing_class_name(enclosing: &ClassIdentity, ctx: &CheckerContext) -> String {
    let members = &ctx.enclosing_class_members;
    members
        .iter()
        .rev()
        .find_map(|class| {
            let info = instance_class(&class.instance_type, ctx)?;
            (identity(&info).as_ref() == Some(enclosing)).then(|| display_name(&info))
        })
        .or_else(|| members.last().map(|class| class.class_name.clone()))
        .unwrap_or_default()
}
