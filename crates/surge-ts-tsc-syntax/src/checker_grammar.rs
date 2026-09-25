//! Grammar errors tsc's checker reports on a tree its parser accepted, for the
//! constructs whose recovery only the parser port reproduces: a variable
//! declaration list with a trailing comma or no declarations at all
//! (`checkGrammarVariableDeclarationList`), and members written beside a
//! mapped type's (`checkGrammarMappedType`, `checkGrammarProperty`).

use crate::Diagnostic;
use crate::ast::{NodeId, NodeList};
use crate::kind::Kind;
use crate::messages as diagnostics;
use crate::parser::ParsedFile;
use crate::scanner;

pub(crate) fn checker_grammar_diagnostics(file: &ParsedFile, text: &str) -> Vec<Diagnostic> {
    let mut found = Vec::new();
    let mut stack = vec![file.root];
    while let Some(node) = stack.pop() {
        if let Some(list) = checked_declaration_list(file, node) {
            check_declaration_list(file, text, list, &mut found);
        }
        if let Some(first) = member_beside_mapped_type(file, node) {
            let (start, end) = crate::binder::error_range_for_node(file, text, first);
            found.push(Diagnostic {
                start,
                end,
                message: diagnostics::A_mapped_type_may_not_declare_properties_or_methods,
                args: Vec::new(),
            });
        }
        stack.extend(file.children(node));
    }
    found.sort_by_key(|diagnostic| diagnostic.start);
    found
}

/// A declaration list the checker checks: a variable statement's, or a `for`,
/// `for…in` or `for…of` head's.
fn checked_declaration_list(file: &ParsedFile, node: NodeId) -> Option<NodeId> {
    let n = file.node(node);
    let list = match n.kind {
        Kind::VariableStatement => n.children.first().copied().flatten(),
        Kind::ForStatement | Kind::ForInStatement | Kind::ForOfStatement => n.initializer,
        _ => None,
    }?;
    (file.node(list).kind == Kind::VariableDeclarationList).then_some(list)
}

fn check_declaration_list(file: &ParsedFile, text: &str, list: NodeId, found: &mut Vec<Diagnostic>) {
    let Some(declarations) = file.node(list).lists.first().and_then(|list| list.as_ref()) else { return };
    if let Some(comma) = trailing_comma(file, text, declarations) {
        found.push(Diagnostic { start: comma, end: comma + 1, message: diagnostics::Trailing_comma_not_allowed, args: Vec::new() });
        return;
    }
    if declarations.nodes.is_empty() {
        found.push(Diagnostic {
            start: declarations.pos,
            end: declarations.end,
            message: diagnostics::Variable_declaration_list_cannot_be_empty,
            args: Vec::new(),
        });
    }
}

/// `NodeList.HasTrailingComma`: the list's range ends with a comma after its
/// last element.
fn trailing_comma(file: &ParsedFile, text: &str, list: &NodeList) -> Option<usize> {
    let last = *list.nodes.last()?;
    let comma = scanner::skip_trivia(text, file.node(last).end);
    (comma < list.end && text.as_bytes().get(comma) == Some(&b',')).then_some(comma)
}

/// The member tsc reports a mapped type's extra members at: the first of a
/// mapped type's own members, or the first member of a type whose property is
/// named like a mapped type's key (`[K in Keys]: T`).
fn member_beside_mapped_type(file: &ParsedFile, node: NodeId) -> Option<NodeId> {
    let members_slot = match file.node(node).kind {
        Kind::MappedType => {
            return file.node(node).lists.first()?.as_ref()?.nodes.first().copied();
        }
        Kind::TypeLiteral => 0,
        Kind::InterfaceDeclaration | Kind::ClassDeclaration | Kind::ClassExpression => 1,
        _ => return None,
    };
    let members = file.node(node).lists.get(members_slot)?.as_ref()?;
    let names_key = |member: NodeId| {
        let member = file.node(member);
        matches!(member.kind, Kind::PropertySignature | Kind::PropertyDeclaration)
            && member.name.is_some_and(|name| {
                let name = file.node(name);
                name.kind == Kind::ComputedPropertyName
                    && name.expression.is_some_and(|expression| {
                        let expression = file.node(expression);
                        expression.kind == Kind::BinaryExpression && expression.op == Kind::InKeyword
                    })
            })
    };
    members.nodes.iter().any(|&member| names_key(member)).then(|| members.nodes[0])
}
