//! tsc's `checkGrammarModifiers`: the modifier keywords written on a
//! declaration, judged in order, stopping at the first one it rejects. oxc
//! drops the modifiers it does not accept from the AST (reporting them as
//! parse errors of its own), so they are read back from the source text
//! between a declaration's start and its keyword or name.
//!
//! The whole decision table is ported so that a code this pass reports is
//! only reported when tsc would reach it; the caller decides which codes it
//! owns.

use oxc_ast::ast::{Program, Statement};
use oxc_span::{GetSpan, Span};

use super::spans::text_span_from_oxc_span;
use crate::{ParsedGrammarDiagnostic, ParsedGrammarDiagnosticKind as Kind};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum ModifierKind {
    Public,
    Private,
    Protected,
    Static,
    Readonly,
    Abstract,
    Override,
    Declare,
    Async,
    Accessor,
    Const,
    Export,
    Default,
    In,
    Out,
}

impl ModifierKind {
    fn parse(word: &str) -> Option<Self> {
        Some(match word {
            "public" => Self::Public,
            "private" => Self::Private,
            "protected" => Self::Protected,
            "static" => Self::Static,
            "readonly" => Self::Readonly,
            "abstract" => Self::Abstract,
            "override" => Self::Override,
            "declare" => Self::Declare,
            "async" => Self::Async,
            "accessor" => Self::Accessor,
            "const" => Self::Const,
            "export" => Self::Export,
            "default" => Self::Default,
            "in" => Self::In,
            "out" => Self::Out,
            _ => return None,
        })
    }

    pub(super) fn text(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
            Self::Protected => "protected",
            Self::Static => "static",
            Self::Readonly => "readonly",
            Self::Abstract => "abstract",
            Self::Override => "override",
            Self::Declare => "declare",
            Self::Async => "async",
            Self::Accessor => "accessor",
            Self::Const => "const",
            Self::Export => "export",
            Self::Default => "default",
            Self::In => "in",
            Self::Out => "out",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Modifier {
    pub kind: ModifierKind,
    pub span: Span,
}

/// The modifier keywords written from `start` on, in order, up to the first
/// word that is not one (the declaration keyword, a name, `get`, `*`, `[`).
/// `const` counts only when `const_is_modifier`, since on a variable
/// statement it is the declaration keyword. A decorator in the range makes
/// the list unreliable, and `None` is returned.
pub(super) fn scan_modifiers(
    source_text: &str,
    start: u32,
    end: u32,
    const_is_modifier: bool,
) -> Option<Vec<Modifier>> {
    let text = source_text.get(start as usize..end as usize)?;
    let mut modifiers = Vec::new();
    let mut offset = 0usize;
    loop {
        let rest = &text[offset..];
        let trimmed = rest.trim_start();
        offset += rest.len() - trimmed.len();
        if trimmed.is_empty() {
            break;
        }
        if let Some(comment) = trimmed.strip_prefix("//") {
            let line_len = comment.find('\n').map_or(comment.len(), |at| at + 1);
            offset += 2 + line_len;
            continue;
        }
        if let Some(comment) = trimmed.strip_prefix("/*") {
            let Some(close) = comment.find("*/") else { break };
            offset += 2 + close + 2;
            continue;
        }
        if trimmed.starts_with('@') {
            return None;
        }
        let word_len = trimmed
            .find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$'))
            .unwrap_or(trimmed.len());
        if word_len == 0 {
            break;
        }
        let word = &trimmed[..word_len];
        let Some(kind) = ModifierKind::parse(word) else { break };
        if kind == ModifierKind::Const && !const_is_modifier {
            break;
        }
        let word_start = start + offset as u32;
        modifiers.push(Modifier { kind, span: Span::new(word_start, word_start + word_len as u32) });
        offset += word_len;
    }
    Some(modifiers)
}

/// The tsc node kinds `checkGrammarModifiers` distinguishes, for the nodes
/// this pass scans.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum NodeKind {
    ClassDeclaration,
    FunctionDeclaration,
    VariableStatement,
    InterfaceDeclaration,
    TypeAliasDeclaration,
    EnumDeclaration,
    ModuleDeclaration,
    PropertyDeclaration,
    MethodDeclaration,
    GetAccessor,
    SetAccessor,
    Constructor,
    TypeParameter,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Parent {
    SourceFile,
    /// A namespace or module body; `namespace` when the enclosing declaration
    /// is a non-ambient module (an identifier-named one).
    ModuleBlock { namespace: bool },
    Class { is_declaration: bool, is_abstract: bool },
    Other,
}

/// What owns a type parameter list.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum TypeParameterOwner {
    FunctionLike,
    Class,
    Interface,
    TypeAlias,
    Other,
}

pub(super) struct ModifierContext {
    pub node: NodeKind,
    pub parent: Parent,
    /// tsc's `node.Parent.Flags & NodeFlagsAmbient`.
    pub parent_ambient: bool,
    pub name_is_private: bool,
    pub type_parameter_owner: TypeParameterOwner,
}

pub(super) struct ModifierError {
    pub code: u32,
    pub span: Span,
    pub args: Vec<&'static str>,
}

fn error(code: u32, span: Span, args: &[&'static str]) -> Option<ModifierError> {
    Some(ModifierError { code, span, args: args.to_vec() })
}

const FLAG_EXPORT: u32 = 1 << 0;
const FLAG_AMBIENT: u32 = 1 << 1;
const FLAG_PUBLIC: u32 = 1 << 2;
const FLAG_PRIVATE: u32 = 1 << 3;
const FLAG_PROTECTED: u32 = 1 << 4;
const FLAG_STATIC: u32 = 1 << 5;
const FLAG_READONLY: u32 = 1 << 6;
const FLAG_ACCESSOR: u32 = 1 << 7;
const FLAG_ABSTRACT: u32 = 1 << 8;
const FLAG_ASYNC: u32 = 1 << 9;
const FLAG_DEFAULT: u32 = 1 << 10;
const FLAG_OVERRIDE: u32 = 1 << 11;
const FLAG_IN: u32 = 1 << 12;
const FLAG_OUT: u32 = 1 << 13;
const FLAG_ACCESSIBILITY: u32 = FLAG_PUBLIC | FLAG_PRIVATE | FLAG_PROTECTED;

fn accessibility_flag(kind: ModifierKind) -> u32 {
    match kind {
        ModifierKind::Public => FLAG_PUBLIC,
        ModifierKind::Private => FLAG_PRIVATE,
        _ => FLAG_PROTECTED,
    }
}

fn is_class_member(node: NodeKind) -> bool {
    matches!(
        node,
        NodeKind::PropertyDeclaration
            | NodeKind::MethodDeclaration
            | NodeKind::GetAccessor
            | NodeKind::SetAccessor
            | NodeKind::Constructor
    )
}

/// tsc's `findFirstIllegalModifier`: a declaration that is not a module
/// element takes no modifiers at all (TS1184), except the ones that are part
/// of its own syntax.
fn first_illegal_modifier(modifiers: &[Modifier], ctx: &ModifierContext) -> Option<Modifier> {
    if is_class_member(ctx.node)
        || matches!(ctx.node, NodeKind::ModuleDeclaration | NodeKind::TypeParameter)
    {
        return None;
    }
    if matches!(ctx.parent, Parent::SourceFile | Parent::ModuleBlock { .. }) {
        return None;
    }
    let allowed = match ctx.node {
        NodeKind::FunctionDeclaration => Some(ModifierKind::Async),
        NodeKind::ClassDeclaration => Some(ModifierKind::Abstract),
        NodeKind::EnumDeclaration => Some(ModifierKind::Const),
        _ => None,
    };
    modifiers.iter().find(|modifier| Some(modifier.kind) != allowed).copied()
}

/// The first modifier error tsc reports on a declaration, if any. Codes
/// other pass or oxc already report come back too, so the caller can tell
/// that its own code would not have been reached.
pub(super) fn first_modifier_error(
    modifiers: &[Modifier],
    ctx: &ModifierContext,
) -> Option<ModifierError> {
    if modifiers.is_empty() {
        return None;
    }
    if let Some(modifier) = first_illegal_modifier(modifiers, ctx) {
        return error(1184, modifier.span, &[]);
    }
    let in_class = matches!(ctx.parent, Parent::Class { .. });
    let at_module_level = matches!(ctx.parent, Parent::SourceFile | Parent::ModuleBlock { .. });
    let mut flags = 0u32;
    let mut last_async: Option<Modifier> = None;
    let mut last_static: Option<Modifier> = None;
    let mut last_override: Option<Modifier> = None;
    for &modifier in modifiers {
        let span = modifier.span;
        let text = modifier.kind.text();
        if !matches!(modifier.kind, ModifierKind::In | ModifierKind::Out | ModifierKind::Const)
            && ctx.node == NodeKind::TypeParameter
        {
            return error(1273, span, &[text]);
        }
        match modifier.kind {
            ModifierKind::Const => {
                if !matches!(ctx.node, NodeKind::EnumDeclaration | NodeKind::TypeParameter) {
                    return error(1248, span, &["const"]);
                }
                if ctx.node == NodeKind::TypeParameter
                    && !matches!(
                        ctx.type_parameter_owner,
                        TypeParameterOwner::FunctionLike | TypeParameterOwner::Class
                    )
                {
                    return error(1277, span, &["const"]);
                }
            }
            ModifierKind::Override => {
                if flags & FLAG_OVERRIDE != 0 {
                    return error(1030, span, &["override"]);
                } else if flags & FLAG_AMBIENT != 0 {
                    return error(1243, span, &["override", "declare"]);
                } else if flags & FLAG_READONLY != 0 {
                    return error(1029, span, &["override", "readonly"]);
                } else if flags & FLAG_ACCESSOR != 0 {
                    return error(1029, span, &["override", "accessor"]);
                } else if flags & FLAG_ASYNC != 0 {
                    return error(1029, span, &["override", "async"]);
                }
                flags |= FLAG_OVERRIDE;
                last_override = Some(modifier);
            }
            ModifierKind::Public | ModifierKind::Protected | ModifierKind::Private => {
                if flags & FLAG_ACCESSIBILITY != 0 {
                    return error(1028, span, &[]);
                } else if flags & FLAG_OVERRIDE != 0 {
                    return error(1029, span, &[text, "override"]);
                } else if flags & FLAG_STATIC != 0 {
                    return error(1029, span, &[text, "static"]);
                } else if flags & FLAG_ACCESSOR != 0 {
                    return error(1029, span, &[text, "accessor"]);
                } else if flags & FLAG_READONLY != 0 {
                    return error(1029, span, &[text, "readonly"]);
                } else if flags & FLAG_ASYNC != 0 {
                    return error(1029, span, &[text, "async"]);
                } else if at_module_level {
                    return error(1044, span, &[text]);
                } else if flags & FLAG_ABSTRACT != 0 {
                    if modifier.kind == ModifierKind::Private {
                        return error(1243, span, &[text, "abstract"]);
                    }
                    return error(1029, span, &[text, "abstract"]);
                } else if ctx.name_is_private && is_class_member(ctx.node) {
                    return error(18010, span, &[]);
                }
                flags |= accessibility_flag(modifier.kind);
            }
            ModifierKind::Static => {
                if flags & FLAG_STATIC != 0 {
                    return error(1030, span, &["static"]);
                } else if flags & FLAG_READONLY != 0 {
                    return error(1029, span, &["static", "readonly"]);
                } else if flags & FLAG_ASYNC != 0 {
                    return error(1029, span, &["static", "async"]);
                } else if flags & FLAG_ACCESSOR != 0 {
                    return error(1029, span, &["static", "accessor"]);
                } else if at_module_level {
                    return error(1044, span, &["static"]);
                } else if flags & FLAG_ABSTRACT != 0 {
                    return error(1243, span, &["static", "abstract"]);
                } else if flags & FLAG_OVERRIDE != 0 {
                    return error(1029, span, &["static", "override"]);
                }
                flags |= FLAG_STATIC;
                last_static = Some(modifier);
            }
            ModifierKind::Accessor => {
                if flags & FLAG_ACCESSOR != 0 {
                    return error(1030, span, &["accessor"]);
                } else if flags & FLAG_READONLY != 0 {
                    return error(1243, span, &["accessor", "readonly"]);
                } else if flags & FLAG_AMBIENT != 0 {
                    return error(1243, span, &["accessor", "declare"]);
                } else if ctx.node != NodeKind::PropertyDeclaration {
                    return error(1275, span, &[]);
                }
                flags |= FLAG_ACCESSOR;
            }
            ModifierKind::Readonly => {
                if flags & FLAG_READONLY != 0 {
                    return error(1030, span, &["readonly"]);
                } else if ctx.node != NodeKind::PropertyDeclaration {
                    return error(1024, span, &[]);
                } else if flags & FLAG_ACCESSOR != 0 {
                    return error(1243, span, &["readonly", "accessor"]);
                }
                flags |= FLAG_READONLY;
            }
            ModifierKind::Export => {
                if flags & FLAG_EXPORT != 0 {
                    return error(1030, span, &["export"]);
                } else if flags & FLAG_AMBIENT != 0 {
                    return error(1029, span, &["export", "declare"]);
                } else if flags & FLAG_ABSTRACT != 0 {
                    return error(1029, span, &["export", "abstract"]);
                } else if flags & FLAG_ASYNC != 0 {
                    return error(1029, span, &["export", "async"]);
                } else if in_class {
                    return error(1031, span, &["export"]);
                }
                flags |= FLAG_EXPORT;
            }
            ModifierKind::Default => {
                if matches!(ctx.parent, Parent::ModuleBlock { namespace: true }) {
                    return error(1319, span, &[]);
                } else if flags & FLAG_EXPORT == 0 {
                    return error(1029, span, &["export", "default"]);
                }
                flags |= FLAG_DEFAULT;
            }
            ModifierKind::Declare => {
                if flags & FLAG_AMBIENT != 0 {
                    return error(1030, span, &["declare"]);
                } else if flags & FLAG_ASYNC != 0 {
                    return error(1040, span, &["async"]);
                } else if flags & FLAG_OVERRIDE != 0 {
                    return error(1040, span, &["override"]);
                } else if in_class && ctx.node != NodeKind::PropertyDeclaration {
                    return error(1031, span, &["declare"]);
                } else if ctx.parent_ambient && matches!(ctx.parent, Parent::ModuleBlock { .. }) {
                    return error(1038, span, &[]);
                } else if ctx.name_is_private && is_class_member(ctx.node) {
                    return error(18019, span, &["declare"]);
                } else if flags & FLAG_ACCESSOR != 0 {
                    return error(1243, span, &["declare", "accessor"]);
                }
                flags |= FLAG_AMBIENT;
            }
            ModifierKind::Abstract => {
                if flags & FLAG_ABSTRACT != 0 {
                    return error(1030, span, &["abstract"]);
                }
                if ctx.node != NodeKind::ClassDeclaration {
                    if !matches!(
                        ctx.node,
                        NodeKind::MethodDeclaration
                            | NodeKind::PropertyDeclaration
                            | NodeKind::GetAccessor
                            | NodeKind::SetAccessor
                    ) {
                        return error(1242, span, &[]);
                    }
                    if !matches!(ctx.parent, Parent::Class { is_declaration: true, is_abstract: true }) {
                        let code = if ctx.node == NodeKind::PropertyDeclaration { 1253 } else { 1244 };
                        return error(code, span, &[]);
                    }
                    if flags & FLAG_STATIC != 0 {
                        return error(1243, span, &["static", "abstract"]);
                    }
                    if flags & FLAG_PRIVATE != 0 {
                        return error(1243, span, &["private", "abstract"]);
                    }
                    if flags & FLAG_ASYNC != 0
                        && let Some(last_async) = last_async
                    {
                        return error(1243, last_async.span, &["async", "abstract"]);
                    }
                    if flags & FLAG_OVERRIDE != 0 {
                        return error(1029, span, &["abstract", "override"]);
                    }
                    if flags & FLAG_ACCESSOR != 0 {
                        return error(1029, span, &["abstract", "accessor"]);
                    }
                }
                if ctx.name_is_private {
                    return error(18019, span, &["abstract"]);
                }
                flags |= FLAG_ABSTRACT;
            }
            ModifierKind::Async => {
                if flags & FLAG_ASYNC != 0 {
                    return error(1030, span, &["async"]);
                } else if flags & FLAG_AMBIENT != 0 || ctx.parent_ambient {
                    return error(1040, span, &["async"]);
                }
                if flags & FLAG_ABSTRACT != 0 {
                    return error(1243, span, &["async", "abstract"]);
                }
                flags |= FLAG_ASYNC;
                last_async = Some(modifier);
            }
            ModifierKind::In | ModifierKind::Out => {
                let (flag, text) = if modifier.kind == ModifierKind::In {
                    (FLAG_IN, "in")
                } else {
                    (FLAG_OUT, "out")
                };
                if ctx.node != NodeKind::TypeParameter
                    || !matches!(
                        ctx.type_parameter_owner,
                        TypeParameterOwner::Interface
                            | TypeParameterOwner::Class
                            | TypeParameterOwner::TypeAlias
                    )
                {
                    return error(1274, span, &[text]);
                }
                if flags & flag != 0 {
                    return error(1030, span, &[text]);
                }
                if flag == FLAG_IN && flags & FLAG_OUT != 0 {
                    return error(1029, span, &["in", "out"]);
                }
                flags |= flag;
            }
        }
    }

    if ctx.node == NodeKind::Constructor {
        if flags & FLAG_STATIC != 0 {
            return error(1089, last_static?.span, &["static"]);
        }
        if flags & FLAG_OVERRIDE != 0 {
            return error(1089, last_override?.span, &["override"]);
        }
        if flags & FLAG_ASYNC != 0 {
            return error(1089, last_async?.span, &["async"]);
        }
        return None;
    }
    if flags & FLAG_ASYNC != 0
        && !matches!(ctx.node, NodeKind::MethodDeclaration | NodeKind::FunctionDeclaration)
    {
        return error(1042, last_async?.span, &["async"]);
    }
    None
}

/// tsc's `checkGrammarTopLevelElementsForRequiredDeclareModifier`: in a
/// declaration file every top-level declaration other than an interface, a
/// type alias, an import or an export needs `declare` or `export` — TS1046
/// on the first token of the first one that lacks it, once per file.
pub(crate) fn collect_declaration_file_diagnostics(
    program: &Program<'_>,
    out: &mut Vec<ParsedGrammarDiagnostic>,
) {
    let offending = program.body.iter().find(|statement| {
        matches!(
            statement,
            Statement::FunctionDeclaration(declaration) if !declaration.declare
        ) || matches!(statement, Statement::ClassDeclaration(declaration) if !declaration.declare)
            || matches!(statement, Statement::VariableDeclaration(declaration) if !declaration.declare)
            || matches!(statement, Statement::TSEnumDeclaration(declaration) if !declaration.declare)
            || matches!(statement, Statement::TSModuleDeclaration(declaration) if !declaration.declare)
            || matches!(statement, Statement::TSGlobalDeclaration(declaration) if !declaration.declare)
    });
    if let Some(statement) = offending {
        let start = statement.span().start;
        let end = super::grammar_context::first_token_end(program.source_text, start as usize) as u32;
        out.push(ParsedGrammarDiagnostic {
            kind: Kind::Ts(1046),
            span: text_span_from_oxc_span(Span::new(start, end)),
            name: None,
        });
    }
}
