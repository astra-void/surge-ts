//! Grammar checks whose answer depends on where a node sits: tsc walks each
//! node's parent chain (`getThisContainer`, `checkGrammarBreakOrContinueStatement`,
//! the binder's strict-mode checks). oxc nodes carry no parent pointers, so
//! this walk keeps the ancestor chain itself and each check reads it the way
//! tsc reads `node.Parent`.
//!
//! oxc and tsc disagree on one shape the ports below account for: a class or
//! object-literal method is a `MethodDefinition`/`ObjectProperty` wrapping a
//! `Function`, where tsc has a single `MethodDeclaration`.

use oxc_ast::AstKind;
use oxc_ast::ast::{
    AssignmentTarget, BindingPattern, Class, MethodDefinitionKind,
    ModuleExportName, Program, PropertyKind, SimpleAssignmentTarget, Statement,
    VariableDeclarationKind,
};
use oxc_ast_visit::Visit;
use oxc_span::{GetSpan, Span};

use super::grammar_modifiers::{self as modifiers, ModifierContext, NodeKind, Parent, TypeParameterOwner};
use super::spans::text_span_from_oxc_span;
use crate::{ParsedGrammarDiagnostic, ParsedGrammarDiagnosticKind as Kind};

mod class_emit;
mod members;
mod reflect_collision;

pub(crate) fn collect_context_grammar_diagnostics(
    program: &Program<'_>,
    out: &mut Vec<ParsedGrammarDiagnostic>,
) {
    let mut collector = ContextCollector {
        stack: Vec::new(),
        out,
        source_text: program.source_text,
        external_module: is_external_module(program),
        ambient_depth: 0,
        const_enum_names: unshadowed_const_enum_names(program),
    };
    collector.visit_program(program);
}

/// tsc's `ExternalModuleIndicator` under the default `moduleDetection: auto`:
/// a top-level import or export of any form makes the file a module.
fn is_external_module(program: &Program<'_>) -> bool {
    program.body.iter().any(|statement| {
        matches!(
            statement,
            Statement::ImportDeclaration(_)
                | Statement::ExportAllDeclaration(_)
                | Statement::ExportDefaultDeclaration(_)
                | Statement::ExportNamedDeclaration(_)
                | Statement::TSExportAssignment(_)
                | Statement::TSNamespaceExportDeclaration(_)
        ) || matches!(
            statement,
            Statement::TSImportEqualsDeclaration(declaration)
                if matches!(
                    declaration.module_reference,
                    oxc_ast::ast::TSModuleReference::ExternalModuleReference(_)
                )
        )
    })
}

struct ContextCollector<'a, 'o> {
    stack: Vec<AstKind<'a>>,
    out: &'o mut Vec<ParsedGrammarDiagnostic>,
    source_text: &'a str,
    external_module: bool,
    /// How many enclosing nodes carry tsc's `NodeFlagsAmbient`.
    ambient_depth: usize,
    /// Top-level `const enum` names no other binding in the file reuses, so
    /// an identifier reference to one is the const enum object and nothing
    /// else — what tsc's `isConstEnumObjectType` sees on the expression.
    const_enum_names: Vec<String>,
}

/// The top-level `const enum` names whose every binding in the file is such a
/// declaration.
fn unshadowed_const_enum_names(program: &Program<'_>) -> Vec<String> {
    let enum_names: Vec<&str> = program
        .body
        .iter()
        .filter_map(|statement| {
            let declaration = match statement {
                Statement::ExportNamedDeclaration(export) => export.declaration.as_ref(),
                other => other.as_declaration(),
            };
            match declaration {
                Some(oxc_ast::ast::Declaration::TSEnumDeclaration(declaration)) if declaration.r#const => {
                    Some(declaration.id.name.as_str())
                }
                _ => None,
            }
        })
        .collect();
    if enum_names.is_empty() {
        return Vec::new();
    }
    let mut bindings = BindingNames::default();
    bindings.visit_program(program);
    let mut names: Vec<String> = Vec::new();
    for name in &enum_names {
        let bound = bindings.names.iter().filter(|bound| **bound == *name).count();
        let declared = enum_names.iter().filter(|other| **other == *name).count();
        if bound == declared {
            names.push((*name).to_string());
        }
    }
    names.sort_unstable();
    names.dedup();
    names
}

#[derive(Default)]
struct BindingNames<'a> {
    names: Vec<&'a str>,
}

impl<'a> Visit<'a> for BindingNames<'a> {
    fn visit_binding_identifier(&mut self, identifier: &oxc_ast::ast::BindingIdentifier<'a>) {
        self.names.push(identifier.name.as_str());
    }
}

fn is_ambient_marker(kind: &AstKind<'_>) -> bool {
    match kind {
        AstKind::Function(function) => function.declare,
        AstKind::Class(class) => class.declare,
        AstKind::VariableDeclaration(declaration) => declaration.declare,
        AstKind::TSModuleDeclaration(declaration) => declaration.declare,
        AstKind::TSEnumDeclaration(declaration) => declaration.declare,
        AstKind::TSGlobalDeclaration(_) => true,
        _ => false,
    }
}

fn is_function_like(kind: &AstKind<'_>) -> bool {
    matches!(kind, AstKind::Function(_) | AstKind::ArrowFunctionExpression(_))
}

fn is_iteration_statement(statement: &Statement<'_>, look_in_labeled_statements: bool) -> bool {
    match statement {
        Statement::ForStatement(_)
        | Statement::ForInStatement(_)
        | Statement::ForOfStatement(_)
        | Statement::DoWhileStatement(_)
        | Statement::WhileStatement(_) => true,
        Statement::LabeledStatement(labeled) => {
            look_in_labeled_statements && is_iteration_statement(&labeled.body, true)
        }
        _ => false,
    }
}

fn is_iteration_kind(kind: &AstKind<'_>) -> bool {
    matches!(
        kind,
        AstKind::ForStatement(_)
            | AstKind::ForInStatement(_)
            | AstKind::ForOfStatement(_)
            | AstKind::DoWhileStatement(_)
            | AstKind::WhileStatement(_)
    )
}

/// A member whose computed key holds `span`: tsc's `getThisContainer` skips
/// the member and its class or object literal for a node in that key.
fn computed_key_contains(kind: &AstKind<'_>, span: Span) -> bool {
    let key_span = match kind {
        AstKind::MethodDefinition(member) if member.computed => member.key.span(),
        AstKind::PropertyDefinition(member) if member.computed => member.key.span(),
        AstKind::AccessorProperty(member) if member.computed => member.key.span(),
        AstKind::ObjectProperty(member) if member.computed => member.key.span(),
        _ => return false,
    };
    key_span.start <= span.start && span.end <= key_span.end
}

/// What tsc's `getSuperContainer(node, true)` stops at.
#[derive(Clone, Copy)]
enum SuperContainer {
    /// A class method, accessor, constructor, property, or static block.
    ClassMember { is_constructor: bool },
    ObjectMethod,
    Function,
    Arrow,
    Signature,
}

/// The end of the token starting at `start`: an identifier-like run, or one
/// character.
pub(super) fn first_token_end(text: &str, start: usize) -> usize {
    let rest = &text[start..];
    let word = rest
        .char_indices()
        .find(|(_, ch)| !(ch.is_alphanumeric() || *ch == '_' || *ch == '$'))
        .map_or(rest.len(), |(index, _)| index);
    start + word.max(rest.chars().next().map_or(0, char::len_utf8))
}

/// What tsc's `getThisContainer(node, false, false)` stops at.
#[derive(Clone, Copy)]
enum ThisContainer<'a> {
    /// A class method, accessor, or constructor.
    ClassMethod(&'a oxc_ast::ast::MethodDefinition<'a>),
    ClassProperty { is_static: bool },
    /// A signature member; `in_interface` when its parent is an interface
    /// body or a class body rather than a type literal.
    Signature { in_interface: bool, is_static: bool },
    /// A plain function declaration or expression.
    Function,
    /// An object-literal method or accessor, which tsc models as a method.
    ObjectMethod,
    Other,
}

impl<'a> ContextCollector<'a, '_> {
    fn push(&mut self, code: u32, span: Span, args: &[&str]) {
        self.out.push(ParsedGrammarDiagnostic {
            kind: Kind::Ts(code),
            span: text_span_from_oxc_span(span),
            name: if args.is_empty() { None } else { Some(args.join("\0")) },
        });
    }

    fn in_class(&self) -> bool {
        self.stack.iter().any(|kind| matches!(kind, AstKind::Class(_)))
    }

    fn this_container(&self, node_span: Span) -> ThisContainer<'a> {
        let mut index = self.stack.len();
        let mut child_span = node_span;
        while index > 0 {
            index -= 1;
            let kind = self.stack[index];
            if computed_key_contains(&kind, child_span) {
                // The member, then its class body/object literal, then the class.
                let skip = match kind {
                    AstKind::ObjectProperty(_) => 1,
                    _ => 2,
                };
                index = index.saturating_sub(skip);
                child_span = self.stack[index].span();
                continue;
            }
            let parent = index.checked_sub(1).map(|parent| self.stack[parent]);
            match kind {
                AstKind::Function(_) => {
                    return match parent {
                        Some(AstKind::MethodDefinition(method)) => ThisContainer::ClassMethod(method),
                        Some(AstKind::ObjectProperty(property))
                            if property.method || property.kind != PropertyKind::Init =>
                        {
                            ThisContainer::ObjectMethod
                        }
                        _ => ThisContainer::Function,
                    };
                }
                AstKind::PropertyDefinition(property) => {
                    return ThisContainer::ClassProperty { is_static: property.r#static };
                }
                AstKind::AccessorProperty(property) => {
                    return ThisContainer::ClassProperty { is_static: property.r#static };
                }
                AstKind::TSPropertySignature(_)
                | AstKind::TSMethodSignature(_)
                | AstKind::TSCallSignatureDeclaration(_)
                | AstKind::TSConstructSignatureDeclaration(_)
                | AstKind::TSIndexSignature(_) => {
                    let is_static = matches!(kind, AstKind::TSIndexSignature(signature) if signature.r#static);
                    return ThisContainer::Signature {
                        in_interface: matches!(
                            parent,
                            Some(AstKind::TSInterfaceBody(_) | AstKind::ClassBody(_))
                        ),
                        is_static,
                    };
                }
                AstKind::TSModuleDeclaration(_)
                | AstKind::TSGlobalDeclaration(_)
                | AstKind::StaticBlock(_)
                | AstKind::TSEnumDeclaration(_)
                | AstKind::Program(_) => return ThisContainer::Other,
                _ => {}
            }
            child_span = kind.span();
        }
        ThisContainer::Other
    }

    /// tsc's `getSuperContainer(node, true)`, as a stack index, starting the
    /// walk below `from`.
    fn super_container(&self, from: usize, node_span: Span) -> Option<(usize, SuperContainer)> {
        let mut index = from;
        let mut child_span = node_span;
        while index > 0 {
            index -= 1;
            let kind = self.stack[index];
            if computed_key_contains(&kind, child_span) {
                child_span = kind.span();
                continue;
            }
            let parent = index.checked_sub(1).map(|parent| self.stack[parent]);
            let container = match kind {
                AstKind::Function(_) => match parent {
                    Some(AstKind::MethodDefinition(method)) => Some(SuperContainer::ClassMember {
                        is_constructor: method.kind == MethodDefinitionKind::Constructor,
                    }),
                    Some(AstKind::ObjectProperty(property))
                        if property.method || property.kind != PropertyKind::Init =>
                    {
                        Some(SuperContainer::ObjectMethod)
                    }
                    _ => Some(SuperContainer::Function),
                },
                AstKind::ArrowFunctionExpression(_) => Some(SuperContainer::Arrow),
                AstKind::PropertyDefinition(_)
                | AstKind::AccessorProperty(_)
                | AstKind::StaticBlock(_) => Some(SuperContainer::ClassMember { is_constructor: false }),
                AstKind::TSPropertySignature(_) | AstKind::TSMethodSignature(_) => {
                    Some(SuperContainer::Signature)
                }
                _ => None,
            };
            if let Some(container) = container {
                return Some((index, container));
            }
            child_span = kind.span();
        }
        None
    }

    /// tsc's `checkSuperExpression` placement errors.
    fn check_super(&mut self, span: Span) {
        let is_call = matches!(
            self.stack.last(),
            Some(AstKind::CallExpression(call)) if call.callee.span() == span
        );
        let mut found = self.super_container(self.stack.len(), span);
        if !is_call {
            while let Some((index, SuperContainer::Arrow)) = found {
                found = self.super_container(index, self.stack[index].span());
            }
        }
        let legal = match found {
            Some((_, SuperContainer::ClassMember { is_constructor })) => !is_call || is_constructor,
            Some((_, SuperContainer::ObjectMethod)) => !is_call,
            _ => false,
        };
        if !legal {
            let container_index = found.map_or(0, |(index, _)| index);
            let mut child_span = span;
            let mut in_computed_name = false;
            for kind in self.stack[container_index..].iter().rev() {
                if computed_key_contains(kind, child_span) {
                    in_computed_name = true;
                    break;
                }
                child_span = kind.span();
            }
            // tsc's remaining TS2338 needs a class or object-literal container
            // that is not a member kind, which no TypeScript source produces.
            let code = if in_computed_name {
                2466
            } else if is_call {
                2337
            } else {
                2660
            };
            self.push(code, span, &[]);
            return;
        }
        if let Some((index, SuperContainer::ClassMember { .. })) = found {
            let class = self.stack[..index].iter().rev().find_map(|kind| match kind {
                AstKind::Class(class) => Some(*class),
                _ => None,
            });
            if class.is_some_and(|class| class.super_class.is_none()) {
                self.push(2335, span, &[]);
            }
        }
    }

    /// The placement half of tsc's `checkThisExpression`.
    fn check_this_expression(&mut self, span: Span) {
        let mut index = self.stack.len();
        let mut child_span = span;
        let mut captured_by_arrow = false;
        while index > 0 {
            index -= 1;
            let kind = self.stack[index];
            if computed_key_contains(&kind, child_span) {
                if matches!(kind, AstKind::ObjectProperty(_)) {
                    index = index.saturating_sub(1);
                    child_span = self.stack[index].span();
                    continue;
                }
                self.push(2465, span, &[]);
                return;
            }
            // tsc's `tryGetThisTypeAtEx` finds no `this` for a namespace or enum
            // body, nor for a function declaration without a `this`
            // parameter — TS2683 under `noImplicitThis`.
            match kind {
                AstKind::TSModuleDeclaration(_) | AstKind::TSGlobalDeclaration(_) => {
                    self.push(2331, span, &[]);
                    self.push(2683, span, &[]);
                    return;
                }
                AstKind::TSEnumDeclaration(_) => {
                    self.push(2332, span, &[]);
                    self.push(2683, span, &[]);
                    return;
                }
                AstKind::Function(function)
                    if function.r#type == oxc_ast::ast::FunctionType::FunctionDeclaration
                        && function.this_param.is_none() =>
                {
                    self.push(2683, span, &[]);
                    return;
                }
                AstKind::Function(function)
                    if function.r#type == oxc_ast::ast::FunctionType::FunctionExpression
                        && function.this_param.is_none()
                        && self.function_expression_has_no_contextual_this(index) =>
                {
                    self.push(2683, span, &[]);
                    return;
                }
                AstKind::Function(_)
                | AstKind::PropertyDefinition(_)
                | AstKind::AccessorProperty(_)
                | AstKind::StaticBlock(_)
                | AstKind::TSPropertySignature(_)
                | AstKind::TSMethodSignature(_)
                | AstKind::TSCallSignatureDeclaration(_)
                | AstKind::TSConstructSignatureDeclaration(_)
                | AstKind::TSIndexSignature(_) => return,
                // `tryGetThisTypeAt` answers a script file's top level with
                // `globalThis`, and an arrow that captured it is TS7041 under
                // `noImplicitThis`; a module's top-level `this` is `undefined`.
                AstKind::Program(_) => {
                    if captured_by_arrow && !self.external_module {
                        self.push(7041, span, &[]);
                    }
                    return;
                }
                AstKind::ArrowFunctionExpression(_) => captured_by_arrow = true,
                _ => {}
            }
            child_span = kind.span();
        }
    }

    /// tsc's `checkTypeParametersNotReferenced`: a type parameter's default may
    /// name only the parameters declared before it — TS2744 on each reference
    /// to itself or a later one. A nested signature, `infer` capture or mapped
    /// type that redeclares the name hides the outer parameter.
    fn check_type_parameter_defaults(
        &mut self,
        declaration: &oxc_ast::ast::TSTypeParameterDeclaration<'a>,
    ) {
        for (index, parameter) in declaration.params.iter().enumerate() {
            let Some(default) = &parameter.default else {
                continue;
            };
            let mut references = LaterTypeParameterReferences {
                later: declaration.params[index..]
                    .iter()
                    .map(|later| later.name.name.as_str())
                    .collect(),
                shadowed: Vec::new(),
                found: Vec::new(),
            };
            references.visit_ts_type(default);
            for span in references.found {
                self.push(2744, span, &[]);
            }
        }
    }

    /// tsc's `checkGrammarBindingElement`: a rest element with a property name
    /// (`{ ...a: b }`) — TS2566 on the name. oxc keeps the element but drops
    /// the `: b` it parsed as a type annotation, so the name is read from the
    /// source text after the element.
    fn check_rest_binding_property_name(&mut self, rest: &oxc_ast::ast::BindingRestElement<'a>) {
        if !matches!(self.stack.last(), Some(AstKind::ObjectPattern(_))) {
            return;
        }
        let text = self.source_text;
        let after = rest.argument.span().end as usize;
        let Some(colon) = text[after..]
            .find(|ch: char| !ch.is_whitespace())
            .map(|offset| after + offset)
        else {
            return;
        };
        if !text[colon..].starts_with(':') {
            return;
        }
        let Some(start) = text[colon + 1..]
            .find(|ch: char| !ch.is_whitespace())
            .map(|offset| colon + 1 + offset)
        else {
            return;
        };
        let end = match text.as_bytes()[start] {
            b'{' | b'[' => matching_bracket_end(text, start),
            _ => first_token_end(text, start),
        };
        if end > start {
            self.push(2566, Span::new(start as u32, end as u32), &[]);
        }
    }

    /// Whether the function expression at stack `index` sits where tsc's
    /// `getContextualThisParameterType` can find nothing: not an object-literal
    /// member or a member assignment (those give `this` the object), and in a
    /// position with no contextual type at all — an unannotated variable, an
    /// IIFE callee, an expression statement, or the `return` of a function
    /// declaration without a return type — reached through parentheses,
    /// conditionals, logical operators, commas and array literals. An argument
    /// or an annotated target might supply a `this:` parameter, so it is left
    /// alone.
    fn function_expression_has_no_contextual_this(&self, index: usize) -> bool {
        let mut child_span = self.stack[index].span();
        let mut position = index;
        while position > 0 {
            position -= 1;
            match self.stack[position] {
                AstKind::ParenthesizedExpression(_)
                | AstKind::ConditionalExpression(_)
                | AstKind::LogicalExpression(_)
                | AstKind::SequenceExpression(_)
                | AstKind::ArrayExpression(_) => {
                    if let AstKind::ConditionalExpression(conditional) = self.stack[position]
                        && conditional.test.span() == child_span
                    {
                        return false;
                    }
                    child_span = self.stack[position].span();
                }
                AstKind::VariableDeclarator(declarator) => {
                    // An annotation written as a function type without a
                    // `this` parameter supplies no `this` either.
                    let untyped = match declarator.type_annotation.as_ref() {
                        None => true,
                        Some(annotation) => matches!(
                            &annotation.type_annotation,
                            oxc_ast::ast::TSType::TSFunctionType(signature) if signature.this_param.is_none()
                        ),
                    };
                    return untyped
                        && declarator.init.as_ref().is_some_and(|init| init.span() == child_span);
                }
                AstKind::CallExpression(call) => return call.callee.span() == child_span,
                AstKind::ExpressionStatement(_) => return true,
                AstKind::ReturnStatement(_) => {
                    return self.stack[..position].iter().rev().find_map(|kind| match kind {
                        AstKind::Function(function) => Some(
                            function.r#type == oxc_ast::ast::FunctionType::FunctionDeclaration
                                && function.return_type.is_none(),
                        ),
                        AstKind::ArrowFunctionExpression(_) => Some(false),
                        _ => None,
                    }) == Some(true);
                }
                _ => return false,
            }
        }
        false
    }

    /// tsc's `checkGrammarStatementInAmbientContext` for a statement directly
    /// in a block: TS1036, once per block.
    fn check_statements_in_ambient_context(&mut self, statements: &[Statement<'_>]) {
        if self.ambient_depth == 0 {
            return;
        }
        let first = statements.iter().find(|statement| {
            matches!(
                statement,
                Statement::BlockStatement(_)
                    | Statement::IfStatement(_)
                    | Statement::DoWhileStatement(_)
                    | Statement::WhileStatement(_)
                    | Statement::ForStatement(_)
                    | Statement::ForInStatement(_)
                    | Statement::ForOfStatement(_)
                    | Statement::BreakStatement(_)
                    | Statement::ContinueStatement(_)
                    | Statement::WithStatement(_)
                    | Statement::SwitchStatement(_)
                    | Statement::LabeledStatement(_)
                    | Statement::ThrowStatement(_)
                    | Statement::TryStatement(_)
                    | Statement::ExpressionStatement(_)
            )
        });
        if let Some(statement) = first {
            let start = statement.span().start;
            let end = first_token_end(self.source_text, start as usize) as u32;
            self.push(1036, Span::new(start, end), &[]);
        }
    }

    /// A `declare` written on a declaration already inside an ambient
    /// namespace body — TS1038.
    fn check_redundant_declare(&mut self, statements: &[Statement<'_>]) {
        if self.ambient_depth == 0 {
            return;
        }
        for statement in statements {
            let declaration = match statement {
                Statement::ExportNamedDeclaration(export) => export.declaration.as_ref(),
                other => other.as_declaration(),
            };
            let declared = match declaration {
                Some(oxc_ast::ast::Declaration::VariableDeclaration(d)) => d.declare,
                Some(oxc_ast::ast::Declaration::FunctionDeclaration(d)) => d.declare,
                Some(oxc_ast::ast::Declaration::ClassDeclaration(d)) => d.declare,
                Some(oxc_ast::ast::Declaration::TSTypeAliasDeclaration(d)) => d.declare,
                Some(oxc_ast::ast::Declaration::TSInterfaceDeclaration(d)) => d.declare,
                Some(oxc_ast::ast::Declaration::TSEnumDeclaration(d)) => d.declare,
                Some(oxc_ast::ast::Declaration::TSModuleDeclaration(d)) => d.declare,
                _ => false,
            };
            if !declared {
                continue;
            }
            let from = statement.span().start as usize;
            if let Some(offset) = self.source_text[from..statement.span().end as usize].find("declare") {
                let start = (from + offset) as u32;
                self.push(1038, Span::new(start, start + 7), &[]);
            }
        }
    }

    /// The syntactic half of tsc's `checkGrammarIndexSignatureParameters`:
    /// a literal or type-parameter key is TS1337, a keyword key that is not
    /// `string`/`number`/`symbol` is TS1268. A key naming an alias is left to
    /// types this pass does not have.
    fn check_index_signature(&mut self, signature: &oxc_ast::ast::TSIndexSignature<'_>) {
        let [parameter] = signature.parameters.as_slice() else {
            return;
        };
        let key = &parameter.type_annotation.type_annotation;
        let name_span = Span::new(parameter.span.start, parameter.span.start + parameter.name.len() as u32);
        let members: Vec<&oxc_ast::ast::TSType<'_>> = match key {
            oxc_ast::ast::TSType::TSUnionType(union) => union.types.iter().collect(),
            other => vec![other],
        };
        let mut invalid = false;
        for member in &members {
            match member {
                oxc_ast::ast::TSType::TSLiteralType(_) => {
                    self.push(1337, name_span, &[]);
                    return;
                }
                oxc_ast::ast::TSType::TSTypeReference(reference) => {
                    if let oxc_ast::ast::TSTypeName::IdentifierReference(name) = &reference.type_name
                        && self.is_type_parameter_in_scope(&name.name)
                    {
                        self.push(1337, name_span, &[]);
                        return;
                    }
                    return;
                }
                oxc_ast::ast::TSType::TSStringKeyword(_)
                | oxc_ast::ast::TSType::TSNumberKeyword(_)
                | oxc_ast::ast::TSType::TSSymbolKeyword(_)
                | oxc_ast::ast::TSType::TSTemplateLiteralType(_) => {}
                oxc_ast::ast::TSType::TSAnyKeyword(_)
                | oxc_ast::ast::TSType::TSUnknownKeyword(_)
                | oxc_ast::ast::TSType::TSBooleanKeyword(_)
                | oxc_ast::ast::TSType::TSBigIntKeyword(_)
                | oxc_ast::ast::TSType::TSObjectKeyword(_)
                | oxc_ast::ast::TSType::TSNeverKeyword(_)
                | oxc_ast::ast::TSType::TSVoidKeyword(_)
                | oxc_ast::ast::TSType::TSNullKeyword(_)
                | oxc_ast::ast::TSType::TSUndefinedKeyword(_) => invalid = true,
                _ => return,
            }
        }
        if invalid {
            self.push(1268, name_span, &[]);
        }
    }

    fn is_type_parameter_in_scope(&self, name: &str) -> bool {
        self.stack.iter().any(|kind| {
            let parameters = match kind {
                AstKind::TSInterfaceDeclaration(d) => d.type_parameters.as_deref(),
                AstKind::TSTypeAliasDeclaration(d) => d.type_parameters.as_deref(),
                AstKind::Class(d) => d.type_parameters.as_deref(),
                AstKind::Function(d) => d.type_parameters.as_deref(),
                AstKind::ArrowFunctionExpression(d) => d.type_parameters.as_deref(),
                AstKind::TSMethodSignature(d) => d.type_parameters.as_deref(),
                _ => None,
            };
            parameters.is_some_and(|parameters| parameters.params.iter().any(|p| p.name.name == name))
        })
    }

    /// tsc's `checkTypeForDuplicateIndexSignatures` for keys written as a
    /// keyword or template literal type, compared by their text.
    fn check_duplicate_index_signatures<'s>(
        &mut self,
        signatures: impl Iterator<Item = &'s oxc_ast::ast::TSIndexSignature<'s>>,
    ) {
        let mut keys: Vec<(bool, String, Span)> = Vec::new();
        for signature in signatures {
            let [parameter] = signature.parameters.as_slice() else {
                continue;
            };
            let key = &parameter.type_annotation.type_annotation;
            let members: Vec<&oxc_ast::ast::TSType<'_>> = match key {
                oxc_ast::ast::TSType::TSUnionType(union) => union.types.iter().collect(),
                other => vec![other],
            };
            for member in members {
                let text = match member {
                    oxc_ast::ast::TSType::TSStringKeyword(_) => "string".to_string(),
                    oxc_ast::ast::TSType::TSNumberKeyword(_) => "number".to_string(),
                    oxc_ast::ast::TSType::TSSymbolKeyword(_) => "symbol".to_string(),
                    oxc_ast::ast::TSType::TSTemplateLiteralType(template) => {
                        let span = template.span;
                        self.source_text[span.start as usize..span.end as usize].to_string()
                    }
                    _ => continue,
                };
                keys.push((signature.r#static, text, signature.span));
            }
        }
        for (is_static, key, span) in &keys {
            if keys.iter().filter(|(s, k, _)| s == is_static && k == key).count() > 1 {
                self.push(2374, *span, &[key]);
            }
        }
    }

    /// The binder's merge conflict for an enum: a function, class, variable,
    /// interface, or type alias of the same name in the same scope — TS2567
    /// at every declaration involved. Namespaces merge with enums; an import
    /// conflict is its own error.
    fn check_enum_merges(&mut self, statements: &[Statement<'_>]) {
        let mut declarations: Vec<(&str, bool, bool, Span)> = Vec::new();
        for statement in statements {
            let (declaration, exported) = match statement {
                Statement::ExportNamedDeclaration(export) => (export.declaration.as_ref(), true),
                other => (other.as_declaration(), false),
            };
            let Some(declaration) = declaration else {
                continue;
            };
            use oxc_ast::ast::Declaration as D;
            match declaration {
                D::TSEnumDeclaration(d) => declarations.push((d.id.name.as_str(), true, exported, d.id.span)),
                D::FunctionDeclaration(d) => {
                    if let Some(id) = &d.id {
                        declarations.push((id.name.as_str(), false, exported, id.span));
                    }
                }
                D::ClassDeclaration(d) => {
                    if let Some(id) = &d.id {
                        declarations.push((id.name.as_str(), false, exported, id.span));
                    }
                }
                D::TSInterfaceDeclaration(d) => declarations.push((d.id.name.as_str(), false, exported, d.id.span)),
                D::TSTypeAliasDeclaration(d) => declarations.push((d.id.name.as_str(), false, exported, d.id.span)),
                D::VariableDeclaration(d) => {
                    for declarator in &d.declarations {
                        if let BindingPattern::BindingIdentifier(id) = &declarator.id {
                            declarations.push((id.name.as_str(), false, exported, id.span));
                        }
                    }
                }
                _ => {}
            }
        }
        let enum_names: Vec<(&str, bool)> = declarations
            .iter()
            .filter(|(_, is_enum, _, _)| *is_enum)
            .map(|(name, _, exported, _)| (*name, *exported))
            .collect();
        for (name, _, exported, span) in &declarations {
            let conflicts = enum_names.iter().any(|(n, e)| n == name && e == exported)
                && declarations
                    .iter()
                    .any(|(n, is_enum, e, _)| n == name && !is_enum && e == exported);
            if conflicts {
                self.push(2567, *span, &[]);
            }
        }
    }

    /// tsc's `checkExportsOnMergedDeclarations` for the declarations that
    /// merge (interfaces, namespaces, classes, enums, and a named default
    /// export of one): a default-exported declaration sharing a space with
    /// any other is TS2652; exported and local declarations sharing one are
    /// TS2395. Each declaration reports at most one, TS2652 first.
    fn check_merged_export_visibility(&mut self, statements: &[Statement<'_>]) {
        const TYPE: u8 = 1;
        const VALUE: u8 = 2;
        const NAMESPACE: u8 = 4;
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum Visibility {
            Local,
            Exported,
            Default,
        }
        let mut declarations: Vec<(&str, Span, Visibility, u8)> = Vec::new();
        for statement in statements {
            use oxc_ast::ast::Declaration as D;
            let (declaration, visibility) = match statement {
                Statement::ExportNamedDeclaration(export) => (export.declaration.as_ref(), Visibility::Exported),
                Statement::ExportDefaultDeclaration(export) => {
                    let entry = match &export.declaration {
                        oxc_ast::ast::ExportDefaultDeclarationKind::ClassDeclaration(d) => {
                            d.id.as_ref().map(|id| (id.name.as_str(), id.span, TYPE | VALUE))
                        }
                        oxc_ast::ast::ExportDefaultDeclarationKind::TSInterfaceDeclaration(d) => {
                            Some((d.id.name.as_str(), d.id.span, TYPE))
                        }
                        _ => None,
                    };
                    if let Some((name, span, spaces)) = entry {
                        declarations.push((name, span, Visibility::Default, spaces));
                    }
                    continue;
                }
                other => (other.as_declaration(), Visibility::Local),
            };
            let entry = match declaration {
                Some(D::TSInterfaceDeclaration(d)) => Some((d.id.name.as_str(), d.id.span, TYPE)),
                Some(D::ClassDeclaration(d)) => d.id.as_ref().map(|id| (id.name.as_str(), id.span, TYPE | VALUE)),
                Some(D::TSEnumDeclaration(d)) => Some((d.id.name.as_str(), d.id.span, TYPE | VALUE)),
                Some(D::TSModuleDeclaration(d)) => match &d.id {
                    oxc_ast::ast::TSModuleDeclarationName::Identifier(id) => {
                        let spaces = if module_is_instantiated(d) { NAMESPACE | VALUE } else { NAMESPACE };
                        Some((id.name.as_str(), id.span, spaces))
                    }
                    _ => None,
                },
                _ => None,
            };
            if let Some((name, span, spaces)) = entry {
                declarations.push((name, span, visibility, spaces));
            }
        }
        for (name, span, _, spaces) in &declarations {
            let (mut exported_spaces, mut local_spaces, mut default_spaces) = (0u8, 0u8, 0u8);
            for (other, _, visibility, other_spaces) in &declarations {
                if other == name {
                    match visibility {
                        Visibility::Local => local_spaces |= other_spaces,
                        Visibility::Exported => exported_spaces |= other_spaces,
                        Visibility::Default => default_spaces |= other_spaces,
                    }
                }
            }
            if spaces & default_spaces & (exported_spaces | local_spaces) != 0 {
                self.push(2652, *span, &[name]);
            } else if spaces & exported_spaces & local_spaces != 0 {
                self.push(2395, *span, &[name]);
            }
        }
    }

    /// tsc's `checkConstEnumAccess`: the const enum object may only be the
    /// object of a property or element access, the right side of an import
    /// or export assignment, a `typeof` query, or an export specifier —
    /// TS2475 anywhere else. A type-position name is not an expression and is
    /// never checked.
    fn check_const_enum_reference(&mut self, identifier: &oxc_ast::ast::IdentifierReference<'_>) {
        if !self.const_enum_names.iter().any(|name| name == identifier.name.as_str()) {
            return;
        }
        let allowed = match self.stack.last() {
            Some(AstKind::StaticMemberExpression(member)) => member.object.span() == identifier.span,
            Some(AstKind::ComputedMemberExpression(member)) => member.object.span() == identifier.span,
            Some(
                AstKind::ExportDefaultDeclaration(_)
                | AstKind::TSExportAssignment(_)
                | AstKind::TSImportEqualsDeclaration(_)
                | AstKind::ExportSpecifier(_)
                | AstKind::TSTypeQuery(_)
                | AstKind::TSTypeReference(_)
                | AstKind::TSQualifiedName(_)
                | AstKind::TSClassImplements(_),
            ) => true,
            _ => false,
        };
        if !allowed {
            self.push(2475, identifier.span, &[]);
        }
    }

    /// tsc's `checkElementAccessExpression` on a const enum object: the index
    /// must be a string literal or an untemplated template — TS2476 at it.
    fn check_const_enum_element_access(&mut self, member: &oxc_ast::ast::ComputedMemberExpression<'_>) {
        let oxc_ast::ast::Expression::Identifier(object) = &member.object else {
            return;
        };
        if !self.const_enum_names.iter().any(|name| name == object.name.as_str()) {
            return;
        }
        let is_string_literal_like = match &member.expression {
            oxc_ast::ast::Expression::StringLiteral(_) => true,
            oxc_ast::ast::Expression::TemplateLiteral(template) => template.expressions.is_empty(),
            _ => false,
        };
        if !is_string_literal_like {
            self.push(2476, member.expression.span(), &[]);
        }
    }

    fn check_merged_declarations(&mut self, statements: &[Statement<'_>]) {
        super::grammar_merges::check_merged_declarations(
            statements,
            self.ambient_depth > 0,
            self.source_text,
            self.out,
        );
    }

    /// tsc's `checkModuleDeclaration`: an ambient module at the top of a
    /// script file may not use a relative name — TS2436. In a module file the
    /// same declaration is an augmentation, which the checker resolves.
    fn check_relative_ambient_module_name(&mut self, module: &oxc_ast::ast::TSModuleDeclaration<'_>) {
        let oxc_ast::ast::TSModuleDeclarationName::StringLiteral(name) = &module.id else {
            return;
        };
        if self.external_module || !matches!(self.stack.last(), Some(AstKind::Program(_))) {
            return;
        }
        if is_external_module_name_relative(name.value.as_str()) {
            self.push(2436, name.span, &[]);
        }
    }

    /// tsc's `checkVarDeclaredNamesNotShadowed`: a `var` hoists past the
    /// block-scoped declaration of the same name in any enclosing scope short
    /// of its own function, namespace, or file, which at run time is a
    /// syntax error — TS2481 at the declarator. A block-scoped declaration in
    /// the hoisting scope itself is the binder's redeclaration error instead.
    fn check_var_declared_name_not_shadowed(&mut self, declarator: &oxc_ast::ast::VariableDeclarator<'_>) {
        if declarator.kind != VariableDeclarationKind::Var {
            return;
        }
        let BindingPattern::BindingIdentifier(id) = &declarator.id else {
            return;
        };
        let name = id.name.as_str();
        for kind in self.stack.iter().rev() {
            let shadowed = match kind {
                AstKind::BlockStatement(block) => statements_declare_block_scoped(&block.body, name),
                AstKind::SwitchStatement(statement) => statement
                    .cases
                    .iter()
                    .any(|case| statements_declare_block_scoped(&case.consequent, name)),
                AstKind::ForStatement(statement) => matches!(
                    &statement.init,
                    Some(oxc_ast::ast::ForStatementInit::VariableDeclaration(declaration))
                        if variable_declaration_declares_block_scoped(declaration, name)
                ),
                AstKind::ForInStatement(statement) => matches!(
                    &statement.left,
                    oxc_ast::ast::ForStatementLeft::VariableDeclaration(declaration)
                        if variable_declaration_declares_block_scoped(declaration, name)
                ),
                AstKind::ForOfStatement(statement) => matches!(
                    &statement.left,
                    oxc_ast::ast::ForStatementLeft::VariableDeclaration(declaration)
                        if variable_declaration_declares_block_scoped(declaration, name)
                ),
                AstKind::FunctionBody(_)
                | AstKind::StaticBlock(_)
                | AstKind::TSModuleBlock(_)
                | AstKind::Program(_) => return,
                _ => false,
            };
            if shadowed {
                self.push(2481, declarator.span, &[name, name]);
                return;
            }
        }
    }

    /// tsc's `checkTypeParameterListsIdentical` over the class and interface
    /// declarations of one name in one scope — TS2428 on each. Constraints are
    /// compared only where both are a keyword, which is identity without types.
    fn check_type_parameter_lists_identical(&mut self, statements: &[Statement<'_>]) {
        type Params<'s> = Option<&'s oxc_ast::ast::TSTypeParameterDeclaration<'s>>;
        let mut declarations: Vec<(&str, Span, Params<'_>)> = Vec::new();
        for statement in statements {
            let declaration = match statement {
                Statement::ExportNamedDeclaration(export) => export.declaration.as_ref(),
                other => other.as_declaration(),
            };
            use oxc_ast::ast::Declaration as D;
            match declaration {
                Some(D::TSInterfaceDeclaration(d)) => {
                    declarations.push((d.id.name.as_str(), d.id.span, d.type_parameters.as_deref()));
                }
                Some(D::ClassDeclaration(d)) => {
                    if let Some(id) = &d.id {
                        declarations.push((id.name.as_str(), id.span, d.type_parameters.as_deref()));
                    }
                }
                _ => {}
            }
        }
        let mut checked: Vec<&str> = Vec::new();
        for (name, _, _) in &declarations {
            if checked.contains(name) {
                continue;
            }
            checked.push(name);
            let group: Vec<&(&str, Span, Params<'_>)> =
                declarations.iter().filter(|(other, _, _)| other == name).collect();
            if group.len() < 2 {
                continue;
            }
            // The merged type's parameters: every name in order of first
            // appearance, the first declaration of each supplying its default
            // and constraint.
            let mut merged: Vec<(&str, bool, Option<&oxc_ast::ast::TSType<'_>>)> = Vec::new();
            for (_, _, params) in &group {
                for param in params.iter().flat_map(|params| params.params.iter()) {
                    let param_name = param.name.name.as_str();
                    match merged.iter_mut().find(|(merged_name, _, _)| *merged_name == param_name) {
                        Some(entry) => {
                            entry.1 |= param.default.is_some();
                            if entry.2.is_none() {
                                entry.2 = param.constraint.as_ref();
                            }
                        }
                        None => merged.push((param_name, param.default.is_some(), param.constraint.as_ref())),
                    }
                }
            }
            let min = merged.iter().take_while(|(_, has_default, _)| !has_default).count();
            let identical = group.iter().all(|(_, _, params)| {
                let params: Vec<&oxc_ast::ast::TSTypeParameter<'_>> =
                    params.iter().flat_map(|params| params.params.iter()).collect();
                params.len() >= min
                    && params.len() <= merged.len()
                    && params.iter().zip(&merged).all(|(param, (merged_name, _, constraint))| {
                        param.name.name == *merged_name
                            && match (param.constraint.as_ref(), constraint) {
                                (Some(own), Some(merged)) => !keywords_differ(own, merged),
                                _ => true,
                            }
                    })
            });
            if !identical {
                for (_, span, _) in &group {
                    self.push(2428, *span, &[name]);
                }
            }
        }
    }

    /// tsc's `checkExternalModuleExports`: `export =` beside another value
    /// export — TS2309 on the assignment. An export whose meaning surge
    /// cannot see (a re-export, an imported name) is taken to be a type.
    fn check_export_assignment_conflicts(&mut self, program: &Program<'_>) {
        let Some(assignment) = program.body.iter().find_map(|statement| match statement {
            Statement::TSExportAssignment(assignment) => Some(assignment),
            _ => None,
        }) else {
            return;
        };
        let local_value = |name: &str| {
            program.body.iter().any(|statement| {
                statement_declares_value(statement, name)
            })
        };
        let exports_value = program.body.iter().any(|statement| match statement {
            Statement::ExportDefaultDeclaration(export) => !matches!(
                export.declaration,
                oxc_ast::ast::ExportDefaultDeclarationKind::TSInterfaceDeclaration(_)
            ),
            Statement::ExportNamedDeclaration(export) => match &export.declaration {
                Some(declaration) => declaration_is_value(declaration),
                None => {
                    export.source.is_none()
                        && export.export_kind.is_value()
                        && export.specifiers.iter().any(|specifier| {
                            specifier.export_kind.is_value() && local_value(&specifier.local.name())
                        })
                }
            },
            _ => false,
        });
        if exports_value {
            self.push(2309, assignment.span, &[]);
        }
    }

    /// tsc's `checkParameter` rest rule — TS2370 — where the annotation alone
    /// settles it: a primitive or literal type is never an array, and a `?`
    /// adds `undefined` under `strictNullChecks`. oxc drops the `?`, so it is
    /// read from the text between the name and the annotation.
    fn check_rest_parameter_type(&mut self, rest: &oxc_ast::ast::FormalParameterRest<'_>) {
        let BindingPattern::BindingIdentifier(name) = &rest.rest.argument else {
            return;
        };
        let Some(annotation) = rest.type_annotation.as_ref() else {
            return;
        };
        use oxc_ast::ast::TSType as T;
        let never_array = matches!(
            annotation.type_annotation,
            T::TSStringKeyword(_)
                | T::TSNumberKeyword(_)
                | T::TSBooleanKeyword(_)
                | T::TSBigIntKeyword(_)
                | T::TSSymbolKeyword(_)
                | T::TSObjectKeyword(_)
                | T::TSUnknownKeyword(_)
                | T::TSVoidKeyword(_)
                | T::TSUndefinedKeyword(_)
                | T::TSNullKeyword(_)
                | T::TSLiteralType(_)
        );
        if never_array {
            self.push(2370, rest.span, &[]);
            return;
        }
        let between = self
            .source_text
            .get(name.span.end as usize..annotation.span.start as usize)
            .unwrap_or("");
        if between.contains('?') {
            self.out.push(ParsedGrammarDiagnostic {
                kind: Kind::TsUnderStrictNullChecks(2370),
                span: text_span_from_oxc_span(rest.span),
                name: None,
            });
        }
    }

    /// The `!` rules of tsc's `checkGrammarVariableDeclaration`: with an
    /// initializer TS1263, without a type TS1264, and anywhere else it is not
    /// a plain variable statement (a loop head, an ambient declaration) TS1255.
    /// oxc reports the first two as well; reporting them here claims its copy.
    fn check_definite_assertion(&mut self, declarator: &oxc_ast::ast::VariableDeclarator<'_>) {
        if !declarator.definite {
            return;
        }
        let in_statement = !matches!(
            self.stack.iter().rev().nth(1),
            Some(AstKind::ForStatement(_) | AstKind::ForInStatement(_) | AstKind::ForOfStatement(_))
        );
        let code = if declarator.init.is_some() {
            1263
        } else if declarator.type_annotation.is_none() {
            1264
        } else if !in_statement || self.ambient_depth > 0 {
            1255
        } else {
            return;
        };
        self.push_at_exclamation(code, declarator.id.span());
    }

    fn push_at_exclamation(&mut self, code: u32, name_span: Span) {
        let after = name_span.end as usize;
        if let Some(offset) = self.source_text[after..].find('!') {
            let start = (after + offset) as u32;
            self.push(code, Span::new(start, start + 1), &[]);
        }
    }

    /// tsc's `checkGrammarForDisallowedBlockScopedVariableStatement`: a
    /// `const`/`using` declaration as the whole body of an `if`, loop, or
    /// label — TS1156. (A `let` there does not survive oxc's parse.)
    fn check_single_statement_declaration(&mut self, declaration: &oxc_ast::ast::VariableDeclaration<'_>) {
        let keyword = match declaration.kind {
            VariableDeclarationKind::Const => "const",
            VariableDeclarationKind::Using => "using",
            VariableDeclarationKind::AwaitUsing => "await using",
            _ => return,
        };
        let is_body = match self.stack.last() {
            Some(AstKind::IfStatement(statement)) => {
                statement.consequent.span() == declaration.span
                    || statement.alternate.as_ref().is_some_and(|alternate| alternate.span() == declaration.span)
            }
            Some(AstKind::WhileStatement(statement)) => statement.body.span() == declaration.span,
            Some(AstKind::DoWhileStatement(statement)) => statement.body.span() == declaration.span,
            Some(AstKind::ForStatement(statement)) => statement.body.span() == declaration.span,
            Some(AstKind::ForInStatement(statement)) => statement.body.span() == declaration.span,
            Some(AstKind::ForOfStatement(statement)) => statement.body.span() == declaration.span,
            Some(AstKind::LabeledStatement(statement)) => statement.body.span() == declaration.span,
            Some(AstKind::WithStatement(statement)) => statement.body.span() == declaration.span,
            _ => false,
        };
        if is_body {
            self.push(1156, declaration.span, &[keyword]);
        }
    }

    /// tsc's `renamedBindingElementsInTypes`: `{ a: string }` in the parameter
    /// of a signature with no body reads as a type annotation but renames `a`
    /// — TS2842 unless the new name is used (only a `typeof` can use it).
    fn check_renamed_binding_in_signature(&mut self, property: &oxc_ast::ast::BindingProperty<'_>) {
        if property.shorthand || property.computed {
            return;
        }
        let BindingPattern::BindingIdentifier(name) = &property.value else {
            return;
        };
        let Some(property_name) = property.key.static_name() else {
            return;
        };
        let mut in_parameter = false;
        for kind in self.stack.iter().rev() {
            match kind {
                AstKind::FormalParameter(_) | AstKind::FormalParameterRest(_) => in_parameter = true,
                AstKind::ObjectPattern(_)
                | AstKind::ArrayPattern(_)
                | AstKind::BindingProperty(_)
                | AstKind::BindingRestElement(_)
                | AstKind::AssignmentPattern(_)
                | AstKind::FormalParameters(_) => {}
                AstKind::Function(function) if in_parameter && function.body.is_none() => {
                    return self.report_unused_renaming(name, &property_name, function.span);
                }
                AstKind::TSFunctionType(signature) if in_parameter => {
                    return self.report_unused_renaming(name, &property_name, signature.span);
                }
                AstKind::TSConstructorType(signature) if in_parameter => {
                    return self.report_unused_renaming(name, &property_name, signature.span);
                }
                AstKind::TSMethodSignature(signature) if in_parameter => {
                    return self.report_unused_renaming(name, &property_name, signature.span);
                }
                AstKind::TSCallSignatureDeclaration(signature) if in_parameter => {
                    return self.report_unused_renaming(name, &property_name, signature.span);
                }
                AstKind::TSConstructSignatureDeclaration(signature) if in_parameter => {
                    return self.report_unused_renaming(name, &property_name, signature.span);
                }
                _ => return,
            }
        }
    }

    fn report_unused_renaming(
        &mut self,
        name: &oxc_ast::ast::BindingIdentifier<'_>,
        property_name: &str,
        signature_span: Span,
    ) {
        let signature = &self.source_text[signature_span.start as usize..signature_span.end as usize];
        let query = format!("typeof {}", name.name);
        let referenced = signature.match_indices(&query).any(|(at, _)| {
            !signature[at + query.len()..]
                .starts_with(|ch: char| ch.is_alphanumeric() || ch == '_' || ch == '$')
        });
        if !referenced {
            self.push(2842, name.span, &[name.name.as_str(), property_name]);
        }
    }

    /// tsc's `onSuccessfullyResolvedSymbol` for a name read by a parameter's
    /// initializer outside any deferred (function) context: the parameter
    /// itself is TS2372, a parameter declared after it TS2373.
    fn check_parameter_initializer_references(&mut self, parameters: &oxc_ast::ast::FormalParameters<'_>) {
        let names: Vec<Vec<(&str, Span)>> = parameters
            .items
            .iter()
            .map(|parameter| {
                let mut names = Vec::new();
                collect_binding_names(&parameter.pattern, &mut names);
                names
            })
            .chain(parameters.rest.iter().map(|rest| {
                let mut names = Vec::new();
                collect_binding_names(&rest.rest.argument, &mut names);
                names
            }))
            .collect();
        for (index, parameter) in parameters.items.iter().enumerate() {
            let (Some(initializer), BindingPattern::BindingIdentifier(own)) =
                (parameter.initializer.as_deref(), &parameter.pattern)
            else {
                continue;
            };
            let mut references = EagerReferences::default();
            references.visit_expression(initializer);
            let is_later =
                |name: &str| names[index + 1..].iter().flatten().any(|(later, _)| *later == name);
            for (name, span) in references.found {
                if name == own.name.as_str() {
                    self.push(2372, span, &[own.name.as_str()]);
                } else if is_later(&name) {
                    self.push(2373, span, &[own.name.as_str(), &name]);
                }
            }
            for (name, span) in references.deferred {
                if is_later(&name) {
                    self.out.push(ParsedGrammarDiagnostic {
                        kind: Kind::LaterParameterReference,
                        span: text_span_from_oxc_span(span),
                        name: None,
                    });
                }
            }
        }
    }

    /// A private name declared on both the static and the instance side —
    /// TS2804 at every declaration of it.
    fn check_private_name_staticness(&mut self, body: &oxc_ast::ast::ClassBody<'_>) {
        use oxc_ast::ast::{ClassElement, PropertyKey};
        let names: Vec<(&str, bool, Span)> = body
            .body
            .iter()
            .filter_map(|element| {
                let (key, is_static) = match element {
                    ClassElement::PropertyDefinition(member) => (&member.key, member.r#static),
                    ClassElement::MethodDefinition(member) => (&member.key, member.r#static),
                    ClassElement::AccessorProperty(member) => (&member.key, member.r#static),
                    _ => return None,
                };
                match key {
                    PropertyKey::PrivateIdentifier(name) => Some((name.name.as_str(), is_static, name.span)),
                    _ => None,
                }
            })
            .collect();
        for (name, _, span) in &names {
            let sides: Vec<bool> = names
                .iter()
                .filter(|(other, _, _)| other == name)
                .map(|(_, is_static, _)| *is_static)
                .collect();
            if sides.contains(&true) && sides.contains(&false) {
                self.push(2804, *span, &[&format!("#{name}")]);
            }
        }
    }

    /// Variables whose own annotations reach each other through `typeof` in a
    /// position tsc resolves eagerly — TS2502 on each one in the cycle. A
    /// member of an object type or a signature is resolved on demand, so a
    /// `typeof` there does not close a cycle.
    fn check_self_referencing_annotations(&mut self, statements: &[Statement<'_>]) {
        let mut variables: Vec<(&str, Span, Vec<&str>)> = Vec::new();
        for statement in statements {
            let declaration = match statement {
                Statement::VariableDeclaration(declaration) => Some(&**declaration),
                Statement::ExportNamedDeclaration(export) => match &export.declaration {
                    Some(oxc_ast::ast::Declaration::VariableDeclaration(declaration)) => Some(&**declaration),
                    _ => None,
                },
                _ => None,
            };
            let Some(declaration) = declaration else {
                continue;
            };
            for declarator in &declaration.declarations {
                let (BindingPattern::BindingIdentifier(id), Some(annotation)) =
                    (&declarator.id, declarator.type_annotation.as_ref())
                else {
                    continue;
                };
                let mut queried = Vec::new();
                eager_type_queries(&annotation.type_annotation, &mut queried);
                if !queried.is_empty() {
                    variables.push((id.name.as_str(), id.span, queried));
                }
            }
        }
        for (name, span, _) in &variables {
            // Reachable from itself through the query edges.
            let mut stack: Vec<&str> = vec![name];
            let mut seen: Vec<&str> = Vec::new();
            let mut cyclic = false;
            while let Some(current) = stack.pop() {
                let Some((_, _, edges)) = variables.iter().find(|(other, _, _)| *other == current) else {
                    continue;
                };
                for edge in edges {
                    if edge == name {
                        cyclic = true;
                    } else if !seen.contains(edge) {
                        seen.push(edge);
                        stack.push(edge);
                    }
                }
            }
            if cyclic {
                self.push(2502, *span, &[name]);
            }
        }
    }

    /// tsc's circular-constraint check for one type parameter list: a
    /// parameter whose constraint is, through other parameters of the list,
    /// itself — TS2313 on the constraint.
    fn check_circular_constraints(&mut self, declaration: &oxc_ast::ast::TSTypeParameterDeclaration<'_>) {
        let bare_constraint = |param: &oxc_ast::ast::TSTypeParameter<'_>| -> Option<(String, Span)> {
            match param.constraint.as_ref()? {
                oxc_ast::ast::TSType::TSTypeReference(reference) if reference.type_arguments.is_none() => {
                    match &reference.type_name {
                        oxc_ast::ast::TSTypeName::IdentifierReference(name) => {
                            Some((name.name.to_string(), reference.span))
                        }
                        _ => None,
                    }
                }
                _ => None,
            }
        };
        let edges: Vec<(String, Option<(String, Span)>)> = declaration
            .params
            .iter()
            .map(|param| (param.name.name.to_string(), bare_constraint(param)))
            .collect();
        for (name, constraint) in &edges {
            let Some((_, span)) = constraint else {
                continue;
            };
            let mut current = name.clone();
            let mut visited: Vec<String> = Vec::new();
            let circular = loop {
                if visited.contains(&current) {
                    // Only a chain that returns to this parameter is its own
                    // cycle; one that runs into another cycle is not.
                    break current == *name;
                }
                visited.push(current.clone());
                match edges.iter().find(|(param, _)| *param == current) {
                    Some((_, Some((next, _)))) => current = next.clone(),
                    _ => break false,
                }
            };
            if circular {
                self.push(2313, *span, &[name]);
            }
        }
    }

    /// tsc's `checkGrammarTypeOperatorNode` for `unique symbol`: only a
    /// `const` variable in a variable statement (TS1332/TS1333/TS1334), a
    /// `static readonly` class property (TS1331), or a `readonly` property
    /// signature (TS1330) may have the type; anywhere else is TS1335.
    fn check_unique_symbol_position(&mut self, operator: &oxc_ast::ast::TSTypeOperator<'_>) {
        if operator.operator != oxc_ast::ast::TSTypeOperatorOperator::Unique {
            return;
        }
        let mut ancestors = self.stack.iter().rev();
        let mut parent = ancestors.next();
        while let Some(AstKind::TSParenthesizedType(_)) = parent {
            parent = ancestors.next();
        }
        let owner = match parent {
            Some(AstKind::TSTypeAnnotation(_)) => ancestors.next(),
            _ => None,
        };
        match owner {
            Some(AstKind::VariableDeclarator(declarator)) => {
                let BindingPattern::BindingIdentifier(id) = &declarator.id else {
                    self.push(1333, operator.span, &[]);
                    return;
                };
                let in_statement = !matches!(
                    ancestors.nth(1),
                    Some(AstKind::ForStatement(_) | AstKind::ForInStatement(_) | AstKind::ForOfStatement(_))
                );
                if !in_statement {
                    self.push(1334, operator.span, &[]);
                } else if declarator.kind != VariableDeclarationKind::Const {
                    self.push(1332, id.span, &[]);
                }
            }
            Some(AstKind::PropertyDefinition(property)) => {
                if !property.r#static || !property.readonly {
                    self.push(1331, property.key.span(), &[]);
                }
            }
            Some(AstKind::TSPropertySignature(property)) => {
                if !property.readonly {
                    self.push(1330, property.key.span(), &[]);
                }
            }
            _ => self.push(1335, operator.span, &[]),
        }
    }

    /// A decorator on a parameter of a plain function — TS1206. (A class
    /// method's parameter decorators depend on `experimentalDecorators`.)
    fn check_parameter_decorator(&mut self, decorator: &oxc_ast::ast::Decorator<'_>) {
        let mut ancestors = self.stack.iter().rev();
        if !matches!(ancestors.next(), Some(AstKind::FormalParameter(_) | AstKind::FormalParameterRest(_))) {
            return;
        }
        ancestors.next();
        let function = ancestors.next();
        let owner = ancestors.next();
        let in_class_member = matches!(owner, Some(AstKind::MethodDefinition(_)));
        if matches!(function, Some(AstKind::Function(_) | AstKind::ArrowFunctionExpression(_))) && !in_class_member {
            self.push(1206, decorator.span, &[]);
        }
    }

    /// `import x = require("m")` inside a namespace — TS1147 on the module
    /// name. An ambient external module (`declare module "m"`) may.
    fn check_import_require_in_namespace(&mut self, declaration: &oxc_ast::ast::TSImportEqualsDeclaration<'_>) {
        let oxc_ast::ast::TSModuleReference::ExternalModuleReference(reference) = &declaration.module_reference else {
            return;
        };
        let in_namespace = self.stack.iter().rev().find_map(|kind| match kind {
            AstKind::TSModuleDeclaration(module) => Some(matches!(
                module.id,
                oxc_ast::ast::TSModuleDeclarationName::Identifier(_)
            )),
            AstKind::TSGlobalDeclaration(_) => Some(false),
            _ => None,
        });
        if in_namespace == Some(true) {
            self.push(1147, reference.expression.span, &[]);
        }
    }

    /// tsc's `isContainedByNamespace`: the statement's container is a
    /// non-ambient module declaration.
    fn is_contained_by_namespace(&self) -> bool {
        let mut ancestors = self.ancestors_as_tsc();
        matches!(ancestors.next(), Some(AstKind::TSModuleBlock(_)))
            && matches!(
                ancestors.next(),
                Some(AstKind::TSModuleDeclaration(module))
                    if matches!(module.id, oxc_ast::ast::TSModuleDeclarationName::Identifier(_))
            )
    }

    /// `assert { … }` in place of `with { … }` — TS2880 on the keyword.
    fn check_import_assertion(&mut self, clause: &oxc_ast::ast::WithClause<'_>) {
        if clause.keyword == oxc_ast::ast::WithClauseKeyword::Assert
            && let Some(start) = self.source_text[..clause.span.end as usize].rfind("assert")
        {
            let start = start as u32;
            self.push(2880, Span::new(start, start + 6), &[]);
        }
    }

    /// Whether the node being entered is a statement of a source file or a
    /// namespace body — where tsc's `checkGrammarModuleElementContext` allows
    /// `export` and `declare`. Anywhere else they are TS1184.
    fn at_module_element_level(&self) -> bool {
        let mut ancestors = self.stack.iter().rev();
        let mut parent = ancestors.next();
        if matches!(parent, Some(AstKind::ExportNamedDeclaration(_))) {
            parent = ancestors.next();
        }
        matches!(parent, None | Some(AstKind::Program(_) | AstKind::TSModuleBlock(_)))
    }

    /// tsc's `checkClassForStaticPropertyNameConflicts`: a static member named
    /// like one of `Function`'s own properties — TS2699, which the checker
    /// keeps only when `useDefineForClassFields` is off.
    fn check_static_function_property_names(&mut self, body: &oxc_ast::ast::ClassBody<'_>) {
        use oxc_ast::ast::ClassElement;
        // tsc names an anonymous class expression by the variable it
        // initializes (`const E = class {}` is `E`).
        let class_name = match self.stack.last() {
            Some(AstKind::Class(class)) => match class.id.as_ref() {
                Some(id) => id.name.to_string(),
                None => match self.stack.iter().rev().nth(1) {
                    Some(AstKind::VariableDeclarator(declarator)) => match &declarator.id {
                        BindingPattern::BindingIdentifier(id) => id.name.to_string(),
                        _ => "(Anonymous class)".to_string(),
                    },
                    _ => "(Anonymous class)".to_string(),
                },
            },
            _ => return,
        };
        for element in &body.body {
            let (key, is_static) = match element {
                ClassElement::PropertyDefinition(member) => (&member.key, member.r#static),
                ClassElement::MethodDefinition(member) => (&member.key, member.r#static),
                ClassElement::AccessorProperty(member) => (&member.key, member.r#static),
                _ => continue,
            };
            if !is_static {
                continue;
            }
            let Some(name) = key.static_name() else {
                continue;
            };
            if matches!(name.as_ref(), "name" | "length" | "caller" | "arguments") {
                self.push(2699, key.span(), &[&name, &class_name]);
            }
        }
    }

    /// `in`/`out` on a type parameter of anything but a class, interface, or
    /// type alias — TS1274, on the first of them.
    fn check_variance_modifier_owner(&mut self, parameter: &oxc_ast::ast::TSTypeParameter<'_>) {
        if !parameter.r#in && !parameter.out {
            return;
        }
        let owner = self.stack.iter().rev().nth(1);
        if let Some(AstKind::TSTypeAliasDeclaration(alias)) = owner {
            if alias_is_not_anonymous(alias) {
                self.push(2637, parameter.span, &[]);
            }
            return;
        }
        if matches!(owner, Some(AstKind::Class(_) | AstKind::TSInterfaceDeclaration(_))) {
            return;
        }
        let text = &self.source_text[parameter.span.start as usize..parameter.name.span.start as usize];
        let (keyword, offset) = match (text.find("in"), text.find("out")) {
            (Some(i), Some(o)) if o < i => ("out", o),
            (Some(i), _) => ("in", i),
            (None, Some(o)) => ("out", o),
            (None, None) => return,
        };
        let start = parameter.span.start + offset as u32;
        self.push(1274, Span::new(start, start + keyword.len() as u32), &[keyword]);
    }

    /// The declaration half of tsc's `checkGrammarForInOrForOfStatement`: one
    /// declaration only (TS1091/TS1188), no initializer (TS1189/TS1190), no
    /// type annotation (TS2404/TS2483) — the first that applies.
    fn check_for_in_or_of_declaration(&mut self, left: &oxc_ast::ast::ForStatementLeft<'_>, is_in: bool) {
        let oxc_ast::ast::ForStatementLeft::VariableDeclaration(declaration) = left else {
            return;
        };
        if self.ambient_depth > 0 {
            return;
        }
        let declarators = &declaration.declarations;
        if declarators.len() > 1 {
            let start = declarators[1].span.start;
            let end = first_token_end(self.source_text, start as usize) as u32;
            self.push(if is_in { 1091 } else { 1188 }, Span::new(start, end), &[]);
            return;
        }
        let Some(first) = declarators.first() else {
            return;
        };
        if first.init.is_some() {
            self.push(if is_in { 1189 } else { 1190 }, first.id.span(), &[]);
        } else if first.type_annotation.is_some() {
            self.push(if is_in { 2404 } else { 2483 }, first.id.span(), &[]);
        }
    }

    /// tsc's `checkGrammarForInvalidDynamicName`: a computed name that is
    /// neither a literal nor an entity name (`a`, `a.b`) cannot be bound —
    /// TS1166 on a class property, TS1169/TS1170 on an interface or type
    /// literal member, reported on the bracketed name.
    fn check_dynamic_property_name(&mut self, code: u32, key: &oxc_ast::ast::PropertyKey<'_>) {
        let Some(expression) = key.as_expression() else {
            return;
        };
        let expression = expression.without_parentheses();
        if is_literal_name(expression) || is_entity_name_expression(expression) {
            return;
        }
        let span = self.member_name_span(key, true);
        self.push(code, span, &[]);
    }

    /// A member name as tsc's `getErrorSpanForNode` reports it: a computed
    /// name includes its brackets, which oxc's key span leaves out.
    fn member_name_span(&self, key: &oxc_ast::ast::PropertyKey<'_>, computed: bool) -> Span {
        let key_span = key.span();
        if !computed {
            return key_span;
        }
        let open = self.source_text[..key_span.start as usize]
            .rfind('[')
            .map_or(key_span.start, |open| open as u32);
        let close = self.source_text[key_span.end as usize..]
            .find(']')
            .map_or(key_span.end, |offset| key_span.end + offset as u32 + 1);
        Span::new(open, close)
    }

    /// tsc's `checkGrammarProperty`: a `[k in T]` property name is a mapped
    /// type written among other members, reported on the parent's first
    /// member (TS7061). Returns whether it was, since tsc then skips the
    /// dynamic-name check.
    fn check_mapped_type_member(&mut self, key: &oxc_ast::ast::PropertyKey<'_>) -> bool {
        use oxc_ast::ast::{ClassElement, Expression, TSSignature};
        let is_in_expression = match key.as_expression() {
            Some(Expression::BinaryExpression(binary)) => {
                binary.operator == oxc_syntax::operator::BinaryOperator::In
            }
            Some(Expression::PrivateInExpression(_)) => true,
            _ => false,
        };
        if !is_in_expression {
            return false;
        }
        let signature_span = |signature: &TSSignature<'_>| match signature {
            TSSignature::TSPropertySignature(member) => self.member_name_span(&member.key, member.computed),
            TSSignature::TSMethodSignature(member) => self.member_name_span(&member.key, member.computed),
            other => other.span(),
        };
        let first_member = match self.stack.last() {
            Some(AstKind::ClassBody(body)) => body.body.first().map(|element| match element {
                ClassElement::MethodDefinition(member) => self.member_name_span(&member.key, member.computed),
                ClassElement::PropertyDefinition(member) => self.member_name_span(&member.key, member.computed),
                ClassElement::AccessorProperty(member) => self.member_name_span(&member.key, member.computed),
                other => other.span(),
            }),
            Some(AstKind::TSInterfaceBody(body)) => body.body.first().map(signature_span),
            Some(AstKind::TSTypeLiteral(literal)) => literal.members.first().map(signature_span),
            _ => None,
        };
        let Some(span) = first_member else {
            return false;
        };
        self.push(7061, span, &[]);
        true
    }

    /// tsc's `checkGrammarBreakOrContinueStatement`.
    fn check_jump(&mut self, span: Span, label: Option<&str>, is_continue: bool) {
        for kind in self.stack.iter().rev() {
            if is_function_like(kind) || matches!(kind, AstKind::StaticBlock(_)) {
                self.push(1107, span, &[]);
                return;
            }
            match kind {
                AstKind::LabeledStatement(labeled)
                    if label.is_some_and(|label| labeled.label.name == label) =>
                {
                    if is_continue && !is_iteration_statement(&labeled.body, true) {
                        self.push(1115, span, &[]);
                    }
                    return;
                }
                AstKind::SwitchStatement(_) if !is_continue && label.is_none() => return,
                _ if label.is_none() && is_iteration_kind(kind) => return,
                _ => {}
            }
        }
        let code = match (label.is_some(), is_continue) {
            (true, false) => 1116,
            (true, true) => 1115,
            (false, false) => 1105,
            (false, true) => 1104,
        };
        self.push(code, span, &[]);
    }

    fn check_labeled_statement(&mut self, labeled: &oxc_ast::ast::LabeledStatement<'_>) {
        let name = labeled.label.name.as_str();
        if self.ambient_depth == 0 {
            for kind in self.stack.iter().rev() {
                if is_function_like(kind) {
                    break;
                }
                if let AstKind::LabeledStatement(outer) = kind
                    && outer.label.name == name
                {
                    self.push(1114, labeled.label.span, &[name]);
                    break;
                }
            }
        }
        // The binder leaves a label no `break`/`continue` targets unreachable
        // and `checkLabeledStatement` reports it — TS7028, an error only under
        // an explicit `allowUnusedLabels: false`, which the checker gates.
        if !label_is_referenced(&labeled.body, name) {
            self.push(7028, labeled.label.span, &[]);
        }
        // The binder's `checkStrictModeLabeledStatement`.
        let is_declaration = matches!(
            labeled.body,
            Statement::VariableDeclaration(_)
                | Statement::FunctionDeclaration(_)
                | Statement::ClassDeclaration(_)
                | Statement::TSInterfaceDeclaration(_)
                | Statement::TSTypeAliasDeclaration(_)
                | Statement::TSEnumDeclaration(_)
                | Statement::TSModuleDeclaration(_)
                | Statement::TSGlobalDeclaration(_)
                | Statement::ImportDeclaration(_)
                | Statement::TSImportEqualsDeclaration(_)
                | Statement::ExportAllDeclaration(_)
                | Statement::ExportDefaultDeclaration(_)
                | Statement::ExportNamedDeclaration(_)
                | Statement::TSExportAssignment(_)
                | Statement::TSNamespaceExportDeclaration(_)
        );
        if is_declaration {
            self.push(1344, labeled.label.span, &[]);
        }
    }

    fn check_with_statement(&mut self, statement: &oxc_ast::ast::WithStatement<'_>) {
        let start = statement.span.start;
        self.push(1101, Span::new(start, start + 4), &[]);
        if self.ambient_depth == 0 {
            // tsc spans `with (…)` up to the statement's full start, which is
            // just past the closing parenthesis.
            let after_object = statement.object.span().end as usize;
            if let Some(offset) = self.source_text[after_object..].find(')') {
                let end = (after_object + offset + 1) as u32;
                self.push(2410, Span::new(start, end), &[]);
            }
        }
    }

    /// The binder's `checkStrictModeEvalOrArguments`.
    fn check_eval_or_arguments(&mut self, name: &str, span: Span) {
        if name != "eval" && name != "arguments" {
            return;
        }
        let code = if self.in_class() {
            1210
        } else if self.external_module {
            1215
        } else {
            1100
        };
        self.push(code, span, &[name]);
    }

    /// Whether a binding identifier sits where the binder checks its name
    /// for `eval`/`arguments`: a variable, catch, or parameter binding, or a
    /// function name. A parameter or function name in an ambient context is
    /// exempt; a destructured binding never is.
    fn is_strict_checked_binding(&self) -> bool {
        let mut in_pattern = false;
        for kind in self.stack.iter().rev() {
            match kind {
                AstKind::ObjectPattern(_)
                | AstKind::ArrayPattern(_)
                | AstKind::BindingProperty(_)
                | AstKind::BindingRestElement(_)
                | AstKind::AssignmentPattern(_) => in_pattern = true,
                AstKind::VariableDeclarator(_) | AstKind::CatchParameter(_) => return true,
                AstKind::FormalParameter(_) | AstKind::FormalParameterRest(_) => {
                    return in_pattern || self.ambient_depth == 0;
                }
                AstKind::Function(_) => return !in_pattern && self.ambient_depth == 0,
                _ => return false,
            }
        }
        false
    }

    /// The binder's `checkContextualIdentifier` for strict-mode reserved words.
    fn check_contextual_identifier(&mut self, name: &str, span: Span) {
        if self.ambient_depth > 0 {
            return;
        }
        if !matches!(
            name,
            "implements"
                | "interface"
                | "let"
                | "package"
                | "private"
                | "protected"
                | "public"
                | "static"
                | "yield"
        ) {
            return;
        }
        // An export specifier's names are identifier names to tsc.
        if matches!(self.stack.last(), Some(AstKind::ExportSpecifier(_))) {
            return;
        }
        let code = if self.in_class() {
            1213
        } else if self.external_module {
            1214
        } else {
            1212
        };
        self.push(code, span, &[name]);
    }

    fn check_reserved_type_name(&mut self, code: u32, name: &oxc_ast::ast::BindingIdentifier<'_>) {
        if matches!(
            name.name.as_str(),
            "any"
                | "unknown"
                | "never"
                | "number"
                | "bigint"
                | "boolean"
                | "string"
                | "symbol"
                | "void"
                | "object"
                | "undefined"
        ) {
            self.push(code, name.span, &[name.name.as_str()]);
        }
    }

    /// tsc's `isInParameterInitializerBeforeContainingFunction`.
    fn is_in_parameter_initializer(&self, node_span: Span) -> bool {
        let mut child_span = node_span;
        let mut in_binding_initializer = false;
        for kind in self.stack.iter().rev() {
            if is_function_like(kind) {
                return false;
            }
            match kind {
                AstKind::FormalParameter(parameter) => {
                    if in_binding_initializer
                        || parameter.initializer.as_ref().is_some_and(|init| init.span() == child_span)
                    {
                        return true;
                    }
                }
                AstKind::AssignmentPattern(pattern) if pattern.right.span() == child_span => {
                    in_binding_initializer = true;
                }
                _ => {}
            }
            child_span = kind.span();
        }
        false
    }

    fn nearest_function_or_static_block(&self) -> Option<AstKind<'a>> {
        self.stack
            .iter()
            .rev()
            .find(|kind| is_function_like(kind) || matches!(kind, AstKind::StaticBlock(_)))
            .copied()
    }

    fn check_yield(&mut self, expression: &oxc_ast::ast::YieldExpression<'_>) {
        let in_generator = matches!(
            self.stack.iter().rev().find(|kind| is_function_like(kind)),
            Some(AstKind::Function(function)) if function.generator
        );
        if !in_generator {
            let start = expression.span.start;
            self.push(1163, Span::new(start, start + 5), &[]);
        }
        if self.is_in_parameter_initializer(expression.span) {
            self.push(2523, expression.span, &[]);
        }
    }

    fn check_await(&mut self, expression: &oxc_ast::ast::AwaitExpression<'_>) {
        if matches!(self.nearest_function_or_static_block(), Some(AstKind::StaticBlock(_))) {
            self.push(18037, expression.span, &[]);
        }
        if self.is_in_parameter_initializer(expression.span) {
            self.push(2524, expression.span, &[]);
        }
    }

    fn check_for_await(&mut self, statement: &oxc_ast::ast::ForOfStatement<'_>) {
        let non_async_function = match self.nearest_function_or_static_block() {
            Some(AstKind::Function(function)) => !function.r#async,
            Some(AstKind::ArrowFunctionExpression(arrow)) => !arrow.r#async,
            _ => false,
        };
        if !non_async_function {
            return;
        }
        let from = statement.span.start as usize + 3;
        if let Some(offset) = self.source_text[from..].find("await") {
            let start = (from + offset) as u32;
            self.push(1103, Span::new(start, start + 5), &[]);
        }
    }

    fn check_new_target(&mut self, meta: &oxc_ast::ast::MetaProperty<'_>) {
        if meta.meta.name != "new" || meta.property.name != "target" {
            return;
        }
        let allowed = match self.this_container(meta.span) {
            ThisContainer::ClassMethod(method) => method.kind == MethodDefinitionKind::Constructor,
            ThisContainer::Function => true,
            _ => false,
        };
        if !allowed {
            self.push(17013, meta.span, &["new.target"]);
        }
    }

    /// tsc's `getThisType` failure — TS2526.
    fn check_this_type(&mut self, span: Span) {
        let allowed = match self.this_container(span) {
            ThisContainer::ClassMethod(method) => {
                !method.r#static
                    && (method.kind != MethodDefinitionKind::Constructor
                        || method.value.body.as_ref().is_some_and(|body| {
                            body.span.start <= span.start && span.end <= body.span.end
                        }))
            }
            ThisContainer::ClassProperty { is_static } => !is_static,
            ThisContainer::Signature { in_interface, is_static } => in_interface && !is_static,
            ThisContainer::Function | ThisContainer::ObjectMethod | ThisContainer::Other => false,
        };
        if !allowed {
            self.push(2526, span, &[]);
        }
    }

    fn check_infer_type(&mut self, span: Span) {
        let mut child_span = span;
        for kind in self.stack.iter().rev() {
            if let AstKind::TSConditionalType(conditional) = kind
                && conditional.extends_type.span() == child_span
            {
                return;
            }
            child_span = kind.span();
        }
        self.push(1338, span, &[]);
    }

    /// tsc's `checkCatchClause`: a block-scoped variable in the catch block
    /// that redeclares the caught binding — TS2492.
    fn check_catch_clause(&mut self, clause: &oxc_ast::ast::CatchClause<'_>) {
        let Some(parameter) = clause.param.as_ref() else {
            return;
        };
        if parameter.type_annotation.is_some() {
            return;
        }
        let mut caught = Vec::new();
        collect_binding_names(&parameter.pattern, &mut caught);
        let mut reported = Vec::new();
        for statement in &clause.body.body {
            let Statement::VariableDeclaration(declaration) = statement else {
                continue;
            };
            if declaration.kind == VariableDeclarationKind::Var {
                continue;
            }
            for declarator in &declaration.declarations {
                let mut names = Vec::new();
                collect_binding_names(&declarator.id, &mut names);
                for (name, span) in names {
                    if caught.iter().any(|(caught, _)| *caught == name)
                        && !reported.contains(&name)
                    {
                        self.push(2492, span, &[name]);
                        reported.push(name);
                    }
                }
            }
        }
    }

    /// tsc's `checkGrammarNameInLetOrConstDeclarations`, reached only when
    /// `checkGrammarVariableDeclaration` found nothing earlier to report.
    fn check_let_name(&mut self, declarator: &oxc_ast::ast::VariableDeclarator<'_>) {
        if declarator.kind == VariableDeclarationKind::Var {
            return;
        }
        let in_for = matches!(
            self.stack.iter().rev().nth(1),
            Some(AstKind::ForInStatement(_) | AstKind::ForOfStatement(_))
        );
        if !in_for && self.ambient_depth == 0 && declarator.init.is_none() {
            let is_pattern = !matches!(declarator.id, BindingPattern::BindingIdentifier(_));
            if is_pattern || declarator.kind != VariableDeclarationKind::Let {
                return;
            }
        }
        let mut names = Vec::new();
        collect_binding_names(&declarator.id, &mut names);
        for (name, span) in names {
            if name == "let" {
                self.push(2480, span, &[]);
            }
        }
    }
}

fn collect_binding_names<'n>(pattern: &'n BindingPattern<'_>, out: &mut Vec<(&'n str, Span)>) {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => {
            out.push((identifier.name.as_str(), identifier.span));
        }
        BindingPattern::ObjectPattern(object) => {
            for property in &object.properties {
                collect_binding_names(&property.value, out);
            }
            if let Some(rest) = &object.rest {
                collect_binding_names(&rest.argument, out);
            }
        }
        BindingPattern::ArrayPattern(array) => {
            for element in array.elements.iter().flatten() {
                collect_binding_names(element, out);
            }
            if let Some(rest) = &array.rest {
                collect_binding_names(&rest.argument, out);
            }
        }
        BindingPattern::AssignmentPattern(assignment) => collect_binding_names(&assignment.left, out),
    }
}

/// The end of the bracketed run starting at `start`, or `start` when it never
/// closes.
fn matching_bracket_end(text: &str, start: usize) -> usize {
    let mut depth = 0usize;
    for (offset, byte) in text.as_bytes()[start..].iter().enumerate() {
        match byte {
            b'{' | b'[' | b'(' => depth += 1,
            b'}' | b']' | b')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return start + offset + 1;
                }
            }
            _ => {}
        }
    }
    start
}

/// Whether a `break` or `continue` under `body` targets `name`, as the
/// binder's active-label list sees it: a nested function or static block
/// starts a fresh list, and a nested label of the same name takes the jumps
/// under it.
fn label_is_referenced(body: &Statement<'_>, name: &str) -> bool {
    struct LabelReferences<'n> {
        name: &'n str,
        found: bool,
    }

    impl<'a> Visit<'a> for LabelReferences<'_> {
        fn visit_break_statement(&mut self, statement: &oxc_ast::ast::BreakStatement<'a>) {
            if statement.label.as_ref().is_some_and(|label| label.name == self.name) {
                self.found = true;
            }
        }

        fn visit_continue_statement(&mut self, statement: &oxc_ast::ast::ContinueStatement<'a>) {
            if statement.label.as_ref().is_some_and(|label| label.name == self.name) {
                self.found = true;
            }
        }

        fn visit_labeled_statement(&mut self, labeled: &oxc_ast::ast::LabeledStatement<'a>) {
            if labeled.label.name != self.name {
                oxc_ast_visit::walk::walk_labeled_statement(self, labeled);
            }
        }

        fn visit_function(&mut self, _: &oxc_ast::ast::Function<'a>, _: oxc_syntax::scope::ScopeFlags) {}

        fn visit_arrow_function_expression(&mut self, _: &oxc_ast::ast::ArrowFunctionExpression<'a>) {}

        fn visit_static_block(&mut self, _: &oxc_ast::ast::StaticBlock<'a>) {}
    }

    let mut references = LabelReferences { name, found: false };
    references.visit_statement(body);
    references.found
}

/// The type references in a type parameter's default that resolve to one of
/// the parameters from `later` (the parameter itself and those after it).
struct LaterTypeParameterReferences<'n> {
    later: Vec<&'n str>,
    /// Names redeclared by an enclosing nested scope, innermost last.
    shadowed: Vec<&'n str>,
    found: Vec<Span>,
}

impl<'a> LaterTypeParameterReferences<'a> {
    fn scoped(&mut self, walk: impl FnOnce(&mut Self)) {
        let depth = self.shadowed.len();
        walk(self);
        self.shadowed.truncate(depth);
    }
}

impl<'a> Visit<'a> for LaterTypeParameterReferences<'a> {
    fn visit_ts_type_reference(&mut self, reference: &oxc_ast::ast::TSTypeReference<'a>) {
        if let oxc_ast::ast::TSTypeName::IdentifierReference(identifier) = &reference.type_name
            && self.later.contains(&identifier.name.as_str())
            && !self.shadowed.contains(&identifier.name.as_str())
        {
            self.found.push(reference.span);
        }
        oxc_ast_visit::walk::walk_ts_type_reference(self, reference);
    }

    // A signature's own parameters and an `infer` capture arrive here; the
    // enclosing node bounds the scope.
    fn visit_ts_type_parameter(&mut self, parameter: &oxc_ast::ast::TSTypeParameter<'a>) {
        self.shadowed.push(parameter.name.name.as_str());
        oxc_ast_visit::walk::walk_ts_type_parameter(self, parameter);
    }

    fn visit_ts_function_type(&mut self, function: &oxc_ast::ast::TSFunctionType<'a>) {
        self.scoped(|this| oxc_ast_visit::walk::walk_ts_function_type(this, function));
    }

    fn visit_ts_constructor_type(&mut self, constructor: &oxc_ast::ast::TSConstructorType<'a>) {
        self.scoped(|this| oxc_ast_visit::walk::walk_ts_constructor_type(this, constructor));
    }

    fn visit_ts_method_signature(&mut self, method: &oxc_ast::ast::TSMethodSignature<'a>) {
        self.scoped(|this| oxc_ast_visit::walk::walk_ts_method_signature(this, method));
    }

    fn visit_ts_call_signature_declaration(
        &mut self,
        signature: &oxc_ast::ast::TSCallSignatureDeclaration<'a>,
    ) {
        self.scoped(|this| oxc_ast_visit::walk::walk_ts_call_signature_declaration(this, signature));
    }

    fn visit_ts_construct_signature_declaration(
        &mut self,
        signature: &oxc_ast::ast::TSConstructSignatureDeclaration<'a>,
    ) {
        self.scoped(|this| {
            oxc_ast_visit::walk::walk_ts_construct_signature_declaration(this, signature);
        });
    }

    // oxc keeps a mapped type's key as a plain binding rather than a type
    // parameter.
    fn visit_ts_mapped_type(&mut self, mapped: &oxc_ast::ast::TSMappedType<'a>) {
        self.scoped(|this| {
            this.shadowed.push(mapped.key.name.as_str());
            oxc_ast_visit::walk::walk_ts_mapped_type(this, mapped);
        });
    }

    // An `infer` capture is in scope for the `extends` clause and the true
    // branch only.
    fn visit_ts_conditional_type(&mut self, conditional: &oxc_ast::ast::TSConditionalType<'a>) {
        self.scoped(|this| {
            this.visit_ts_type(&conditional.check_type);
            this.visit_ts_type(&conditional.extends_type);
            this.visit_ts_type(&conditional.true_type);
        });
        self.visit_ts_type(&conditional.false_type);
    }
}

impl<'a> Visit<'a> for ContextCollector<'a, '_> {
    fn enter_node(&mut self, kind: AstKind<'a>) {
        self.check_declaration_position(&kind);
        self.check_member_placement(&kind);
        match kind {
            AstKind::BreakStatement(statement) => {
                self.check_jump(statement.span, statement.label.as_ref().map(|l| l.name.as_str()), false);
            }
            AstKind::ContinueStatement(statement) => {
                self.check_jump(statement.span, statement.label.as_ref().map(|l| l.name.as_str()), true);
            }
            AstKind::LabeledStatement(statement) => self.check_labeled_statement(statement),
            AstKind::WithStatement(statement) => self.check_with_statement(statement),
            AstKind::BindingRestElement(rest) => self.check_rest_binding_property_name(rest),
            AstKind::BindingIdentifier(identifier) => {
                self.check_contextual_identifier(&identifier.name, identifier.span);
                if self.is_strict_checked_binding() {
                    self.check_eval_or_arguments(&identifier.name, identifier.span);
                }
            }
            AstKind::IdentifierReference(identifier) => {
                self.check_contextual_identifier(&identifier.name, identifier.span);
                self.check_const_enum_reference(identifier);
            }
            AstKind::ComputedMemberExpression(member) => self.check_const_enum_element_access(member),
            AstKind::LabelIdentifier(identifier) => {
                self.check_contextual_identifier(&identifier.name, identifier.span);
            }
            AstKind::AssignmentExpression(assignment) => {
                if let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left {
                    self.check_eval_or_arguments(&identifier.name, identifier.span);
                }
                self.check_private_method_assignment(&assignment.left);
            }
            AstKind::UpdateExpression(update) => {
                if let SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) = &update.argument {
                    self.check_eval_or_arguments(&identifier.name, identifier.span);
                }
                self.check_private_method_update(&update.argument);
            }
            AstKind::PrivateFieldExpression(access) => self.check_private_setter_read(access),
            // Outside a module or ambient module, `declare global` is TS2669
            // and contributes nothing to the globals.
            AstKind::TSGlobalDeclaration(global)
                if self.external_module
                    || self.stack.iter().any(|kind| matches!(kind, AstKind::TSModuleDeclaration(_))) =>
            {
                self.check_global_augmentation_names(&global.body.body);
            }
            AstKind::MetaProperty(meta) => self.check_new_target(meta),
            AstKind::TSThisType(this_type) => self.check_this_type(this_type.span),
            AstKind::TSInferType(infer) => self.check_infer_type(infer.span),
            AstKind::TSInterfaceDeclaration(declaration) => {
                self.check_reserved_type_name(2427, &declaration.id);
            }
            AstKind::TSTypeAliasDeclaration(declaration) => {
                self.check_reserved_type_name(2457, &declaration.id);
            }
            AstKind::TSEnumDeclaration(declaration) => {
                self.check_reserved_type_name(2431, &declaration.id);
            }
            AstKind::Class(class) => {
                check_class_name(self, class);
                self.check_object_class_name(class);
            }
            AstKind::CatchClause(clause) => self.check_catch_clause(clause),
            AstKind::VariableDeclarator(declarator) => {
                self.check_let_name(declarator);
                self.check_definite_assertion(declarator);
                self.check_var_declared_name_not_shadowed(declarator);
            }
            AstKind::TSModuleDeclaration(module) => self.check_relative_ambient_module_name(module),
            AstKind::PropertyDefinition(property) => {
                if property.definite && property.value.is_some() {
                    self.push_at_exclamation(1263, property.key.span());
                }
                if property.computed && !self.check_mapped_type_member(&property.key) {
                    self.check_dynamic_property_name(1166, &property.key);
                }
                self.check_property_initializer_constructor_locals(property);
            }
            AstKind::AccessorProperty(property) if property.computed => {
                self.check_mapped_type_member(&property.key);
            }
            AstKind::TSPropertySignature(signature) if signature.computed => {
                if !self.check_mapped_type_member(&signature.key) {
                    let code = match self.stack.last() {
                        Some(AstKind::TSInterfaceBody(_)) => 1169,
                        _ => 1170,
                    };
                    self.check_dynamic_property_name(code, &signature.key);
                }
            }
            AstKind::VariableDeclaration(declaration) => {
                self.check_es_module_marker(declaration);
                self.check_single_statement_declaration(declaration);
                if declaration.declare && !self.at_module_element_level() {
                    let start = declaration.span.start;
                    self.push(1184, Span::new(start, start + 7), &[]);
                }
            }
            AstKind::YieldExpression(expression) => self.check_yield(expression),
            AstKind::AwaitExpression(expression) => self.check_await(expression),
            AstKind::ForOfStatement(statement) => {
                if statement.r#await {
                    self.check_for_await(statement);
                }
                self.check_for_in_or_of_declaration(&statement.left, false);
                self.check_private_method_for_target(&statement.left);
            }
            AstKind::ForInStatement(statement) => {
                self.check_for_in_or_of_declaration(&statement.left, true);
                self.check_private_method_for_target(&statement.left);
            }
            AstKind::FormalParameterRest(rest) => self.check_rest_parameter_type(rest),
            AstKind::BindingProperty(property) => self.check_renamed_binding_in_signature(property),
            AstKind::FormalParameters(parameters) => self.check_parameter_initializer_references(parameters),
            AstKind::Super(expression) => self.check_super(expression.span),
            AstKind::ThisExpression(expression) => self.check_this_expression(expression.span),
            AstKind::TSIndexSignature(signature) => self.check_index_signature(signature),
            AstKind::TSTypeOperator(operator) => self.check_unique_symbol_position(operator),
            // oxc keeps a static block's span from its first modifier; any
            // text before `static` is one — TS1184 (oxc reports it too, and a
            // grammar TS1184 elsewhere in the file claims oxc's copies).
            AstKind::StaticBlock(block) => {
                let text = &self.source_text[block.span.start as usize..block.span.end as usize];
                if let Some(keyword) = text.find("static")
                    && !text[..keyword].trim().is_empty()
                {
                    let start = block.span.start;
                    let end = first_token_end(self.source_text, start as usize) as u32;
                    self.push(1184, Span::new(start, end), &[]);
                }
            }
            AstKind::MethodDefinition(method) => {
                if method.kind == MethodDefinitionKind::Constructor && method.value.r#async {
                    let from = method.span.start as usize;
                    if let Some(offset) = self.source_text[from..method.key.span().start as usize].find("async") {
                        let start = (from + offset) as u32;
                        self.push(1089, Span::new(start, start + 5), &["async"]);
                    }
                }
            }
            AstKind::TSTypeParameterDeclaration(declaration) => {
                self.check_circular_constraints(declaration);
                self.check_type_parameter_defaults(declaration);
            }
            AstKind::Decorator(decorator) => self.check_parameter_decorator(decorator),
            AstKind::TSImportEqualsDeclaration(declaration) => {
                // tsc's `checkGrammarModuleElementContext` bails first.
                if !self.at_module_element_level() {
                    let span = self.first_token(self.statement_start(declaration.span.start));
                    self.push(1232, span, &[]);
                } else {
                    self.check_import_require_in_namespace(declaration);
                    self.check_import_alias_name(declaration);
                    // Gated on `module` by the checker: ES2015..ESNext cannot emit it.
                    // Inside a namespace it is TS1147 instead, and tsc stops there.
                    if !matches!(self.stack.last(), Some(AstKind::TSModuleBlock(_)))
                        && matches!(
                            declaration.module_reference,
                            oxc_ast::ast::TSModuleReference::ExternalModuleReference(_)
                        )
                        && declaration.import_kind.is_value()
                        && self.ambient_depth == 0
                    {
                        self.push(1202, declaration.span, &[]);
                    }
                }
            }
            AstKind::TSExportAssignment(assignment) => {
                if !self.at_module_element_level() {
                    self.push(1231, self.first_token(assignment.span.start), &[]);
                } else if self.is_contained_by_namespace() {
                    // tsc reports TS1063 here and stops; that code is not emitted.
                } else if self.ambient_depth == 0 {
                    // Gated on `module` and the file's emit format by the checker.
                    self.push(1203, assignment.span, &[]);
                }
            }
            AstKind::TSTypeParameter(parameter) => {
                self.check_reserved_type_name(2368, &parameter.name);
                self.check_variance_modifier_owner(parameter);
            }
            AstKind::ImportDeclaration(declaration) => {
                if !self.at_module_element_level() {
                    self.push(1232, self.first_token(declaration.span.start), &[]);
                } else if let Some(clause) = &declaration.with_clause {
                    self.check_import_assertion(clause);
                }
            }
            AstKind::ExportNamedDeclaration(declaration) => {
                if !self.at_module_element_level() {
                    // A wrapped declaration's `export` is a misplaced modifier;
                    // an export list is a misplaced export declaration.
                    let code = if declaration.declaration.is_some() { 1184 } else { 1233 };
                    let start = declaration.span.start;
                    self.push(code, Span::new(start, start + 6), &[]);
                } else if let Some(clause) = &declaration.with_clause {
                    self.check_import_assertion(clause);
                }
            }
            AstKind::ExportAllDeclaration(declaration) => {
                if !self.at_module_element_level() {
                    let start = declaration.span.start;
                    self.push(1233, Span::new(start, start + 6), &[]);
                } else if let Some(clause) = &declaration.with_clause {
                    self.check_import_assertion(clause);
                }
            }
            AstKind::ClassBody(body) => {
                self.check_private_name_staticness(body);
                self.check_static_function_property_names(body);
                self.check_duplicate_index_signatures(body.body.iter().filter_map(|element| {
                    match element {
                        oxc_ast::ast::ClassElement::TSIndexSignature(signature) => Some(&**signature),
                        _ => None,
                    }
                }));
            }
            AstKind::TSInterfaceBody(body) => self.check_duplicate_index_signatures(signature_index_signatures(&body.body)),
            AstKind::TSTypeLiteral(literal) => self.check_duplicate_index_signatures(signature_index_signatures(&literal.members)),
            AstKind::TSModuleBlock(block) => {
                self.check_statements_in_ambient_context(&block.body);
                self.check_redundant_declare(&block.body);
                self.check_enum_merges(&block.body);
                self.check_self_referencing_annotations(&block.body);
                self.check_type_parameter_lists_identical(&block.body);
                self.check_merged_export_visibility(&block.body);
                self.check_merged_declarations(&block.body);
            }
            AstKind::BlockStatement(block) => {
                self.check_statements_in_ambient_context(&block.body);
                self.check_enum_merges(&block.body);
                self.check_self_referencing_annotations(&block.body);
                self.check_merged_declarations(&block.body);
            }
            AstKind::Program(program) => {
                self.check_enum_merges(&program.body);
                self.check_self_referencing_annotations(&program.body);
                self.check_type_parameter_lists_identical(&program.body);
                if self.external_module {
                    self.check_merged_export_visibility(&program.body);
                    self.check_export_assignment_conflicts(program);
                }
                self.check_top_level_names(program);
                self.check_reflect_collisions(program);
                self.check_merged_declarations(&program.body);
            }
            AstKind::FunctionBody(body) => {
                self.check_enum_merges(&body.statements);
                self.check_self_referencing_annotations(&body.statements);
                self.check_merged_declarations(&body.statements);
            }
            _ => {}
        }
        if is_ambient_marker(&kind) {
            self.ambient_depth += 1;
        }
        self.stack.push(kind);
    }

    fn leave_node(&mut self, kind: AstKind<'a>) {
        self.stack.pop();
        if is_ambient_marker(&kind) {
            self.ambient_depth -= 1;
        }
    }

    // An export specifier's local name is an identifier name to tsc, and so
    // is the name on either side of `as`.
    fn visit_export_specifier(&mut self, _: &oxc_ast::ast::ExportSpecifier<'a>) {}

    fn visit_module_export_name(&mut self, name: &ModuleExportName<'a>) {
        if !matches!(name, ModuleExportName::IdentifierReference(_)) {
            oxc_ast_visit::walk::walk_module_export_name(self, name);
        }
    }
}

/// The modifier codes this pass reports from tsc's `checkGrammarModifiers`
/// port; the rest of its table is oxc's or another check's to report.
const OWNED_MODIFIER_CODES: &[u32] = &[1040, 1042, 1044, 1243, 1277, 1319];

/// Declaration-position and modifier grammar: tsc's
/// `checkGrammarModuleElementContext`, `checkModuleDeclaration`,
/// `checkGrammarModifiers`, `checkModuleAugmentationElement`, and the
/// `super`/`instanceof` type-argument rules.
impl<'a> ContextCollector<'a, '_> {
    fn check_declaration_position(&mut self, kind: &AstKind<'a>) {
        match kind {
            AstKind::Function(function) => {
                if matches!(
                    function.r#type,
                    oxc_ast::ast::FunctionType::FunctionDeclaration
                        | oxc_ast::ast::FunctionType::TSDeclareFunction
                ) {
                    self.check_statement_modifiers(NodeKind::FunctionDeclaration, function.span);
                }
                if let Some(body) = &function.body {
                    self.check_use_strict_parameters(&function.params, body);
                }
            }
            AstKind::ArrowFunctionExpression(arrow) if !arrow.expression => {
                self.check_use_strict_parameters(&arrow.params, &arrow.body);
            }
            AstKind::Class(class) if class.r#type == oxc_ast::ast::ClassType::ClassDeclaration => {
                self.check_statement_modifiers(NodeKind::ClassDeclaration, class.span);
            }
            AstKind::VariableDeclaration(declaration) => {
                let parent = self.stack.iter().rev().find(|kind| {
                    !matches!(kind, AstKind::ExportNamedDeclaration(_))
                });
                if !matches!(
                    parent,
                    Some(
                        AstKind::ForStatement(_)
                            | AstKind::ForInStatement(_)
                            | AstKind::ForOfStatement(_)
                    )
                ) {
                    self.check_statement_modifiers(NodeKind::VariableStatement, declaration.span);
                }
            }
            AstKind::TSInterfaceDeclaration(declaration) => {
                self.check_statement_modifiers(NodeKind::InterfaceDeclaration, declaration.span);
            }
            AstKind::TSTypeAliasDeclaration(declaration) => {
                self.check_statement_modifiers(NodeKind::TypeAliasDeclaration, declaration.span);
            }
            AstKind::TSEnumDeclaration(declaration) => {
                self.check_statement_modifiers(NodeKind::EnumDeclaration, declaration.span);
            }
            AstKind::TSModuleDeclaration(declaration) => self.check_module_declaration(declaration),
            AstKind::TSGlobalDeclaration(declaration) => self.check_global_declaration(declaration),
            AstKind::ExportDefaultDeclaration(declaration) => self.check_export_default(declaration),
            AstKind::PropertyDefinition(property) => {
                let start = decorators_end(&property.decorators, property.span.start);
                self.check_class_member_modifiers(
                    NodeKind::PropertyDeclaration,
                    start,
                    &property.key,
                );
            }
            AstKind::AccessorProperty(property) => {
                let start = decorators_end(&property.decorators, property.span.start);
                self.check_class_member_modifiers(
                    NodeKind::PropertyDeclaration,
                    start,
                    &property.key,
                );
            }
            AstKind::MethodDefinition(method) => {
                let node = match method.kind {
                    MethodDefinitionKind::Constructor => NodeKind::Constructor,
                    MethodDefinitionKind::Method => NodeKind::MethodDeclaration,
                    MethodDefinitionKind::Get => NodeKind::GetAccessor,
                    MethodDefinitionKind::Set => NodeKind::SetAccessor,
                };
                let start = decorators_end(&method.decorators, method.span.start);
                self.check_class_member_modifiers(node, start, &method.key);
            }
            AstKind::ObjectProperty(property) => self.check_object_member_modifiers(property),
            AstKind::TSTypeParameter(parameter) => self.check_type_parameter_modifiers(parameter),
            AstKind::TSInstantiationExpression(expression) => {
                if matches!(expression.expression, oxc_ast::ast::Expression::Super(_)) {
                    self.push(2754, expression.type_arguments.span, &[]);
                }
            }
            AstKind::CallExpression(call) => {
                if let Some(type_arguments) = &call.type_arguments
                    && matches!(call.callee, oxc_ast::ast::Expression::Super(_))
                {
                    self.push(2754, type_arguments.span, &[]);
                }
            }
            AstKind::BinaryExpression(binary) => self.check_instanceof_instantiation(binary),
            _ => {}
        }
    }

    /// The ancestors as tsc sees them: an `export` wrapper is a modifier of
    /// the declaration it wraps, not its parent.
    fn ancestors_as_tsc(&self) -> impl Iterator<Item = AstKind<'a>> + '_ {
        self.stack
            .iter()
            .rev()
            .filter(|kind| !matches!(kind, AstKind::ExportNamedDeclaration(_)))
            .copied()
    }

    /// The span tsc's `grammarErrorOnFirstToken` reports at.
    fn first_token(&self, start: u32) -> Span {
        Span::new(start, first_token_end(self.source_text, start as usize) as u32)
    }

    fn push_modifier_error(&mut self, error: Option<modifiers::ModifierError>) {
        if let Some(error) = error
            && OWNED_MODIFIER_CODES.contains(&error.code)
        {
            self.push(error.code, error.span, &error.args);
        }
    }

    /// Modifiers on a statement-level declaration, read from the statement's
    /// start (an `export` wrapper's, when there is one) up to its keyword.
    fn check_statement_modifiers(&mut self, node: NodeKind, span: Span) {
        let mut ancestors = self.stack.iter().rev();
        let mut parent = ancestors.next();
        let mut start = span.start;
        match parent {
            Some(AstKind::ExportNamedDeclaration(export)) => {
                start = export.span.start;
                parent = ancestors.next();
            }
            Some(AstKind::ExportDefaultDeclaration(export)) => {
                start = export.span.start;
                parent = ancestors.next();
            }
            _ => {}
        }
        let parent = match parent {
            None | Some(AstKind::Program(_)) => Parent::SourceFile,
            Some(AstKind::TSModuleBlock(_)) => Parent::ModuleBlock {
                namespace: matches!(
                    ancestors.next(),
                    Some(AstKind::TSModuleDeclaration(module))
                        if matches!(module.id, oxc_ast::ast::TSModuleDeclarationName::Identifier(_))
                ),
            },
            _ => Parent::Other,
        };
        let Some(modifiers) = modifiers::scan_modifiers(
            self.source_text,
            start,
            span.end,
            node != NodeKind::VariableStatement,
        ) else {
            return;
        };
        let context = ModifierContext {
            node,
            parent,
            parent_ambient: self.ambient_depth > 0,
            name_is_private: false,
            type_parameter_owner: TypeParameterOwner::Other,
        };
        self.push_modifier_error(modifiers::first_modifier_error(&modifiers, &context));
    }

    fn check_class_member_modifiers(
        &mut self,
        node: NodeKind,
        start: u32,
        key: &oxc_ast::ast::PropertyKey<'_>,
    ) {
        let Some(AstKind::Class(class)) = self.stack.iter().rev().nth(1) else {
            return;
        };
        let Some(modifiers) =
            modifiers::scan_modifiers(self.source_text, start, key.span().start, true)
        else {
            return;
        };
        let context = ModifierContext {
            node,
            parent: Parent::Class {
                is_declaration: class.r#type == oxc_ast::ast::ClassType::ClassDeclaration,
                is_abstract: class.r#abstract,
            },
            parent_ambient: self.ambient_depth > 0,
            name_is_private: matches!(key, oxc_ast::ast::PropertyKey::PrivateIdentifier(_)),
            type_parameter_owner: TypeParameterOwner::Other,
        };
        self.push_modifier_error(modifiers::first_modifier_error(&modifiers, &context));
    }

    fn check_type_parameter_modifiers(&mut self, parameter: &oxc_ast::ast::TSTypeParameter<'_>) {
        let Some(modifiers) = modifiers::scan_modifiers(
            self.source_text,
            parameter.span.start,
            parameter.name.span.start,
            true,
        ) else {
            return;
        };
        let owner = match self.stack.iter().rev().nth(1) {
            Some(
                AstKind::Function(_)
                | AstKind::ArrowFunctionExpression(_)
                | AstKind::TSFunctionType(_)
                | AstKind::TSConstructorType(_)
                | AstKind::TSCallSignatureDeclaration(_)
                | AstKind::TSConstructSignatureDeclaration(_)
                | AstKind::TSMethodSignature(_),
            ) => TypeParameterOwner::FunctionLike,
            Some(AstKind::Class(_)) => TypeParameterOwner::Class,
            Some(AstKind::TSInterfaceDeclaration(_)) => TypeParameterOwner::Interface,
            Some(AstKind::TSTypeAliasDeclaration(_)) => TypeParameterOwner::TypeAlias,
            _ => TypeParameterOwner::Other,
        };
        let context = ModifierContext {
            node: NodeKind::TypeParameter,
            parent: Parent::Other,
            parent_ambient: self.ambient_depth > 0,
            name_is_private: false,
            type_parameter_owner: owner,
        };
        self.push_modifier_error(modifiers::first_modifier_error(&modifiers, &context));
    }

    /// tsc's `checkGrammarObjectLiteralExpression`: no modifier belongs on an
    /// object literal member except `async` on a method — TS1042 each; and
    /// `checkGrammarMethod`'s TS1184 on a method carrying anything else.
    fn check_object_member_modifiers(&mut self, property: &oxc_ast::ast::ObjectProperty<'_>) {
        let Some(modifiers) = modifiers::scan_modifiers(
            self.source_text,
            property.span.start,
            property.key.span().start,
            false,
        ) else {
            return;
        };
        for modifier in &modifiers {
            if modifier.kind == modifiers::ModifierKind::Async && property.method {
                continue;
            }
            self.push(1042, modifier.span, &[modifier.kind.text()]);
        }
        if property.method
            && property.kind == PropertyKind::Init
            && let Some(first) = modifiers.first()
            && !(modifiers.len() == 1 && first.kind == modifiers::ModifierKind::Async)
        {
            self.push(1184, first.span, &[]);
        }
    }

    /// Whether `kind` is what tsc calls an ambient module: a string-named
    /// module declaration or a global augmentation.
    fn is_ambient_module(kind: Option<AstKind<'a>>) -> bool {
        match kind {
            Some(AstKind::TSModuleDeclaration(module)) => {
                matches!(module.id, oxc_ast::ast::TSModuleDeclarationName::StringLiteral(_))
            }
            Some(AstKind::TSGlobalDeclaration(_)) => true,
            _ => false,
        }
    }

    /// tsc's `IsExternalModuleAugmentation` for an ambient module being
    /// entered: at the top level of an external module, or directly inside a
    /// top-level ambient module of a script.
    fn is_external_module_augmentation(&self) -> bool {
        let mut ancestors = self.ancestors_as_tsc();
        match ancestors.next() {
            None | Some(AstKind::Program(_)) => self.external_module,
            Some(AstKind::TSModuleBlock(_)) => {
                let grandparent = ancestors.next();
                Self::is_ambient_module(grandparent)
                    && matches!(ancestors.next(), None | Some(AstKind::Program(_)))
                    && !self.external_module
            }
            _ => false,
        }
    }

    /// tsc's `checkModuleDeclaration` for a `module`/`namespace` declaration:
    /// its position (TS1234/TS1235), modifiers, the `module` keyword on a
    /// namespace (TS1540), and where an ambient module may sit (TS2435).
    fn check_module_declaration(&mut self, declaration: &oxc_ast::ast::TSModuleDeclaration<'_>) {
        let (name_span, is_string_name) = match &declaration.id {
            oxc_ast::ast::TSModuleDeclarationName::Identifier(identifier) => (identifier.span, false),
            oxc_ast::ast::TSModuleDeclarationName::StringLiteral(literal) => (literal.span, true),
        };
        if is_string_name {
            self.check_ambient_module_export();
        }
        let parent = self.ancestors_as_tsc().next();
        let in_context = matches!(
            parent,
            None | Some(AstKind::Program(_) | AstKind::TSModuleBlock(_) | AstKind::TSModuleDeclaration(_))
        );
        if !in_context {
            let code = if is_string_name { 1234 } else { 1235 };
            let span = self.first_token(self.statement_start(declaration.span.start));
            self.push(code, span, &[]);
            return;
        }
        self.check_statement_modifiers(NodeKind::ModuleDeclaration, declaration.span);
        if !is_string_name && declaration.kind == oxc_ast::ast::TSModuleDeclarationKind::Module {
            self.push(1540, name_span, &[]);
        }
        if is_string_name && !self.is_external_module_augmentation() {
            let at_script_top_level =
                matches!(parent, None | Some(AstKind::Program(_))) && !self.external_module;
            if !at_script_top_level {
                self.push(2435, name_span, &[]);
            }
        }
    }

    /// tsc's `checkModuleDeclaration` for `global { … }`: TS2670 without
    /// `declare` outside an ambient context, its position (TS1234), and
    /// TS2669 anywhere it does not augment an external module. An
    /// augmentation's body may not import or export (TS2666/TS2667).
    fn check_global_declaration(&mut self, declaration: &oxc_ast::ast::TSGlobalDeclaration<'_>) {
        self.check_ambient_module_export();
        if !declaration.declare && self.ambient_depth == 0 {
            self.push(2670, declaration.global_span, &[]);
        }
        let parent = self.ancestors_as_tsc().next();
        if !matches!(parent, None | Some(AstKind::Program(_) | AstKind::TSModuleBlock(_))) {
            let span = self.first_token(self.statement_start(declaration.span.start));
            self.push(1234, span, &[]);
            return;
        }
        if self.is_external_module_augmentation() {
            self.check_augmentation_elements(&declaration.body.body);
        } else {
            self.push(2669, declaration.global_span, &[]);
        }
    }

    /// The binder's TS2668: `export` on an ambient module or augmentation.
    fn check_ambient_module_export(&mut self) {
        if let Some(AstKind::ExportNamedDeclaration(export)) = self.stack.last() {
            let span = self.first_token(export.span.start);
            self.push(2668, span, &[]);
        }
    }

    /// The start of the statement a declaration belongs to: its `export`
    /// wrapper's, when there is one.
    fn statement_start(&self, start: u32) -> u32 {
        match self.stack.last() {
            Some(AstKind::ExportNamedDeclaration(export)) => export.span.start,
            _ => start,
        }
    }

    /// tsc's `checkModuleAugmentationElement`.
    fn check_augmentation_elements(&mut self, statements: &[Statement<'_>]) {
        for statement in statements {
            let code = match statement {
                Statement::ExportNamedDeclaration(export) if export.declaration.is_none() => 2666,
                Statement::ExportAllDeclaration(_) | Statement::TSExportAssignment(_) => 2666,
                Statement::ExportDefaultDeclaration(export)
                    if !matches!(
                        export.declaration,
                        oxc_ast::ast::ExportDefaultDeclarationKind::FunctionDeclaration(_)
                            | oxc_ast::ast::ExportDefaultDeclarationKind::ClassDeclaration(_)
                            | oxc_ast::ast::ExportDefaultDeclarationKind::TSInterfaceDeclaration(_)
                    ) =>
                {
                    2666
                }
                Statement::ImportDeclaration(_) => 2667,
                Statement::TSImportEqualsDeclaration(import)
                    if matches!(
                        import.module_reference,
                        oxc_ast::ast::TSModuleReference::ExternalModuleReference(_)
                    ) =>
                {
                    2667
                }
                _ => continue,
            };
            let span = self.first_token(statement.span().start);
            self.push(code, span, &[]);
        }
    }

    /// tsc's `checkExportAssignment` for `export default <expression>`, and
    /// `checkGrammarModifiers` for a default-exported declaration: not
    /// inside a function or block (TS1258 / TS1184), and not inside a
    /// namespace (TS1319).
    fn check_export_default(&mut self, declaration: &oxc_ast::ast::ExportDefaultDeclaration<'_>) {
        let is_declaration = matches!(
            declaration.declaration,
            oxc_ast::ast::ExportDefaultDeclarationKind::FunctionDeclaration(_)
                | oxc_ast::ast::ExportDefaultDeclarationKind::ClassDeclaration(_)
                | oxc_ast::ast::ExportDefaultDeclarationKind::TSInterfaceDeclaration(_)
        );
        let mut ancestors = self.stack.iter().rev();
        let parent = ancestors.next();
        if !matches!(parent, None | Some(AstKind::Program(_) | AstKind::TSModuleBlock(_))) {
            let code = if is_declaration { 1184 } else { 1258 };
            let span = self.first_token(declaration.span.start);
            self.push(code, span, &[]);
            return;
        }
        let in_namespace = matches!(parent, Some(AstKind::TSModuleBlock(_)))
            && matches!(
                ancestors.next(),
                Some(AstKind::TSModuleDeclaration(module))
                    if matches!(module.id, oxc_ast::ast::TSModuleDeclarationName::Identifier(_))
            );
        if in_namespace && !is_declaration {
            let span = self.first_token(declaration.span.start);
            self.push(1319, span, &[]);
        }
    }

    /// tsc's `checkExpressionWithTypeArguments`: an instantiation expression
    /// as the right operand of `instanceof` — TS2848.
    fn check_instanceof_instantiation(&mut self, binary: &oxc_ast::ast::BinaryExpression<'_>) {
        if binary.operator != oxc_syntax::operator::BinaryOperator::Instanceof {
            return;
        }
        let mut right = &binary.right;
        while let oxc_ast::ast::Expression::ParenthesizedExpression(inner) = right {
            right = &inner.expression;
        }
        if let oxc_ast::ast::Expression::TSInstantiationExpression(instantiation) = right {
            self.push(2848, instantiation.span, &[]);
        }
    }

    /// tsc's `checkGrammarForUseStrictSimpleParameterList`: a `"use strict"`
    /// prologue in a function whose parameter list is not simple — TS1346
    /// on each such parameter and TS1347 on the directive. tsc skips this
    /// below target ES2016, which this pass does not see.
    fn check_use_strict_parameters(
        &mut self,
        parameters: &oxc_ast::ast::FormalParameters<'_>,
        body: &oxc_ast::ast::FunctionBody<'_>,
    ) {
        let Some(directive) = body.directives.iter().find(|directive| {
            let span = directive.expression.span;
            matches!(
                self.source_text.get(span.start as usize..span.end as usize),
                Some("\"use strict\"" | "'use strict'")
            )
        }) else {
            return;
        };
        let mut non_simple: Vec<Span> = parameters
            .items
            .iter()
            .filter(|parameter| {
                parameter.initializer.is_some()
                    || !matches!(parameter.pattern, BindingPattern::BindingIdentifier(_))
            })
            .map(|parameter| parameter.span)
            .collect();
        if let Some(rest) = &parameters.rest {
            non_simple.push(rest.span);
        }
        if non_simple.is_empty() {
            return;
        }
        for span in non_simple {
            self.push(1346, span, &[]);
        }
        self.push(1347, directive.span, &[]);
    }
}

fn decorators_end(decorators: &[oxc_ast::ast::Decorator<'_>], start: u32) -> u32 {
    decorators
        .iter()
        .map(|decorator| decorator.span.end)
        .max()
        .unwrap_or(start)
        .max(start)
}

fn check_class_name(collector: &mut ContextCollector<'_, '_>, class: &Class<'_>) {
    if let Some(id) = class.id.as_ref() {
        collector.check_reserved_type_name(2414, id);
    }
}

fn signature_index_signatures<'s>(
    members: &'s [oxc_ast::ast::TSSignature<'s>],
) -> impl Iterator<Item = &'s oxc_ast::ast::TSIndexSignature<'s>> {
    members.iter().filter_map(|member| match member {
        oxc_ast::ast::TSSignature::TSIndexSignature(signature) => Some(&**signature),
        _ => None,
    })
}

fn keywords_differ(left: &oxc_ast::ast::TSType<'_>, right: &oxc_ast::ast::TSType<'_>) -> bool {
    fn keyword(ty: &oxc_ast::ast::TSType<'_>) -> Option<&'static str> {
        use oxc_ast::ast::TSType as T;
        Some(match ty {
            T::TSStringKeyword(_) => "string",
            T::TSNumberKeyword(_) => "number",
            T::TSBooleanKeyword(_) => "boolean",
            T::TSBigIntKeyword(_) => "bigint",
            T::TSSymbolKeyword(_) => "symbol",
            T::TSObjectKeyword(_) => "object",
            T::TSAnyKeyword(_) => "any",
            T::TSUnknownKeyword(_) => "unknown",
            T::TSNeverKeyword(_) => "never",
            T::TSVoidKeyword(_) => "void",
            T::TSUndefinedKeyword(_) => "undefined",
            T::TSNullKeyword(_) => "null",
            _ => return None,
        })
    }
    matches!((keyword(left), keyword(right)), (Some(l), Some(r)) if l != r)
}

/// Whether a namespace body declares a value (tsc's `getModuleInstanceState`
/// is not `NonInstantiated`): anything but interfaces, type aliases, and
/// namespaces that are themselves uninstantiated.
fn module_is_instantiated(declaration: &oxc_ast::ast::TSModuleDeclaration<'_>) -> bool {
    match &declaration.body {
        None => false,
        Some(oxc_ast::ast::TSModuleDeclarationBody::TSModuleDeclaration(inner)) => module_is_instantiated(inner),
        Some(oxc_ast::ast::TSModuleDeclarationBody::TSModuleBlock(block)) => block.body.iter().any(|statement| {
            let declaration = match statement {
                Statement::ExportNamedDeclaration(export) => match &export.declaration {
                    Some(declaration) => declaration,
                    None => return false,
                },
                Statement::TSInterfaceDeclaration(_) | Statement::TSTypeAliasDeclaration(_) => return false,
                Statement::TSModuleDeclaration(inner) => return module_is_instantiated(inner),
                _ => return true,
            };
            match declaration {
                oxc_ast::ast::Declaration::TSInterfaceDeclaration(_)
                | oxc_ast::ast::Declaration::TSTypeAliasDeclaration(_) => false,
                oxc_ast::ast::Declaration::TSModuleDeclaration(inner) => module_is_instantiated(inner),
                _ => true,
            }
        }),
    }
}

fn declaration_is_value(declaration: &oxc_ast::ast::Declaration<'_>) -> bool {
    use oxc_ast::ast::Declaration as D;
    match declaration {
        D::VariableDeclaration(_) | D::FunctionDeclaration(_) | D::ClassDeclaration(_) | D::TSEnumDeclaration(_) => true,
        D::TSModuleDeclaration(module) => module_is_instantiated(module),
        _ => false,
    }
}

fn statement_declares_value(statement: &Statement<'_>, name: &str) -> bool {
    let declaration = match statement {
        Statement::ExportNamedDeclaration(export) => export.declaration.as_ref(),
        other => other.as_declaration(),
    };
    let Some(declaration) = declaration else {
        return false;
    };
    use oxc_ast::ast::Declaration as D;
    let declares = match declaration {
        D::VariableDeclaration(d) => d.declarations.iter().any(|declarator| {
            let mut names = Vec::new();
            collect_binding_names(&declarator.id, &mut names);
            names.iter().any(|(declared, _)| *declared == name)
        }),
        D::FunctionDeclaration(d) => d.id.as_ref().is_some_and(|id| id.name == name),
        D::ClassDeclaration(d) => d.id.as_ref().is_some_and(|id| id.name == name),
        D::TSEnumDeclaration(d) => d.id.name == name,
        D::TSModuleDeclaration(d) => {
            matches!(&d.id, oxc_ast::ast::TSModuleDeclarationName::Identifier(id) if id.name == name)
                && module_is_instantiated(d)
        }
        _ => false,
    };
    declares
}

/// The value names an expression reads as it runs. A nested function, arrow,
/// or class body runs later (tsc's `withinDeferredContext`), and a type
/// annotation reads no values.
/// The deferred reads are kept apart: they resolve (a later parameter is in
/// scope by the time a nested function runs) but are not errors.
#[derive(Default)]
struct EagerReferences {
    found: Vec<(String, Span)>,
    deferred: Vec<(String, Span)>,
    deferred_depth: usize,
}

impl<'a> Visit<'a> for EagerReferences {
    fn visit_identifier_reference(&mut self, identifier: &oxc_ast::ast::IdentifierReference<'a>) {
        let entry = (identifier.name.to_string(), identifier.span);
        if self.deferred_depth > 0 {
            self.deferred.push(entry);
        } else {
            self.found.push(entry);
        }
    }
    fn visit_function(&mut self, function: &oxc_ast::ast::Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        self.deferred_depth += 1;
        oxc_ast_visit::walk::walk_function(self, function, flags);
        self.deferred_depth -= 1;
    }
    fn visit_arrow_function_expression(&mut self, arrow: &oxc_ast::ast::ArrowFunctionExpression<'a>) {
        self.deferred_depth += 1;
        oxc_ast_visit::walk::walk_arrow_function_expression(self, arrow);
        self.deferred_depth -= 1;
    }
    fn visit_class(&mut self, _: &Class<'a>) {}
    fn visit_ts_type_annotation(&mut self, _: &oxc_ast::ast::TSTypeAnnotation<'a>) {}
    fn visit_ts_type(&mut self, _: &oxc_ast::ast::TSType<'a>) {}
}

/// The names a type annotation reads with `typeof` while its own type is
/// being resolved: through unions, intersections, arrays, tuples, operators,
/// indexed accesses and type arguments, but not into an object type's
/// members or a signature, which tsc resolves on demand.
fn eager_type_queries<'t>(ty: &'t oxc_ast::ast::TSType<'_>, out: &mut Vec<&'t str>) {
    use oxc_ast::ast::TSType as T;
    match ty {
        T::TSTypeQuery(query) => match &query.expr_name {
            oxc_ast::ast::TSTypeQueryExprName::IdentifierReference(identifier) => {
                out.push(identifier.name.as_str());
            }
            oxc_ast::ast::TSTypeQueryExprName::QualifiedName(qualified) => {
                let mut left = &qualified.left;
                while let oxc_ast::ast::TSTypeName::QualifiedName(inner) = left {
                    left = &inner.left;
                }
                if let oxc_ast::ast::TSTypeName::IdentifierReference(identifier) = left {
                    out.push(identifier.name.as_str());
                }
            }
            _ => {}
        },
        T::TSUnionType(union) => union.types.iter().for_each(|member| eager_type_queries(member, out)),
        T::TSIntersectionType(intersection) => {
            intersection.types.iter().for_each(|member| eager_type_queries(member, out));
        }
        T::TSArrayType(array) => eager_type_queries(&array.element_type, out),
        T::TSParenthesizedType(inner) => eager_type_queries(&inner.type_annotation, out),
        T::TSTypeOperatorType(operator) => eager_type_queries(&operator.type_annotation, out),
        T::TSIndexedAccessType(access) => {
            eager_type_queries(&access.object_type, out);
            eager_type_queries(&access.index_type, out);
        }
        T::TSTupleType(tuple) => {
            for element in &tuple.element_types {
                if let Some(ty) = element.as_ts_type() {
                    eager_type_queries(ty, out);
                }
            }
        }
        T::TSTypeReference(reference) => {
            if let Some(arguments) = &reference.type_arguments {
                arguments.params.iter().for_each(|argument| eager_type_queries(argument, out));
            }
        }
        _ => {}
    }
}

/// tsc's TS2637 condition, `getDeclaredTypeOfSymbol(alias)` being neither
/// anonymous nor mapped, where the alias body alone settles it. A reference to
/// another type is left alone: it may name an object type.
fn alias_is_not_anonymous(alias: &oxc_ast::ast::TSTypeAliasDeclaration<'_>) -> bool {
    use oxc_ast::ast::TSType as T;
    let mut ty = &alias.type_annotation;
    while let T::TSParenthesizedType(inner) = ty {
        ty = &inner.type_annotation;
    }
    match ty {
        T::TSTypeLiteral(_) | T::TSFunctionType(_) | T::TSConstructorType(_) | T::TSMappedType(_) => false,
        T::TSTypeReference(reference) => {
            reference.type_arguments.is_none()
                && matches!(&reference.type_name, oxc_ast::ast::TSTypeName::IdentifierReference(name)
                    if alias.type_parameters.as_ref().is_some_and(|params| {
                        params.params.iter().any(|param| param.name.name == name.name)
                    }))
        }
        T::TSStringKeyword(_)
        | T::TSNumberKeyword(_)
        | T::TSBooleanKeyword(_)
        | T::TSBigIntKeyword(_)
        | T::TSSymbolKeyword(_)
        | T::TSObjectKeyword(_)
        | T::TSAnyKeyword(_)
        | T::TSUnknownKeyword(_)
        | T::TSNeverKeyword(_)
        | T::TSVoidKeyword(_)
        | T::TSUndefinedKeyword(_)
        | T::TSNullKeyword(_)
        | T::TSLiteralType(_)
        | T::TSTemplateLiteralType(_)
        | T::TSUnionType(_)
        | T::TSIntersectionType(_)
        | T::TSArrayType(_)
        | T::TSTupleType(_) => true,
        _ => false,
    }
}

/// A name tsc treats as static: a string or numeric literal, a signed number,
/// or a template without substitutions (`isDynamicName` is false for these).
fn is_literal_name(expression: &oxc_ast::ast::Expression<'_>) -> bool {
    use oxc_ast::ast::Expression as E;
    match expression {
        E::StringLiteral(_) | E::NumericLiteral(_) => true,
        E::TemplateLiteral(template) => template.expressions.is_empty(),
        E::UnaryExpression(unary) => {
            matches!(
                unary.operator,
                oxc_syntax::operator::UnaryOperator::UnaryNegation | oxc_syntax::operator::UnaryOperator::UnaryPlus
            ) && matches!(unary.argument, E::NumericLiteral(_))
        }
        _ => false,
    }
}

/// tsc's `isEntityNameExpression`: an identifier, or a property access on one.
fn is_entity_name_expression(expression: &oxc_ast::ast::Expression<'_>) -> bool {
    use oxc_ast::ast::Expression as E;
    match expression {
        E::Identifier(_) => true,
        E::StaticMemberExpression(member) => is_entity_name_expression(&member.object),
        _ => false,
    }
}

fn statements_declare_block_scoped(statements: &[Statement<'_>], name: &str) -> bool {
    statements.iter().any(|statement| {
        let declaration = match statement {
            Statement::ExportNamedDeclaration(export) => export.declaration.as_ref(),
            other => other.as_declaration(),
        };
        matches!(
            declaration,
            Some(oxc_ast::ast::Declaration::VariableDeclaration(declaration))
                if variable_declaration_declares_block_scoped(declaration, name)
        )
    })
}

fn variable_declaration_declares_block_scoped(
    declaration: &oxc_ast::ast::VariableDeclaration<'_>,
    name: &str,
) -> bool {
    declaration.kind != VariableDeclarationKind::Var
        && declaration.declarations.iter().any(|declarator| {
            declarator
                .id
                .get_binding_identifiers()
                .iter()
                .any(|identifier| identifier.name == name)
        })
}

/// tsc's `IsExternalModuleNameRelative`: `./`, `../`, or a rooted path.
fn is_external_module_name_relative(name: &str) -> bool {
    let relative = name == "." || name == ".." || name.starts_with("./") || name.starts_with("../")
        || name.starts_with(".\\") || name.starts_with("..\\");
    let rooted = name.starts_with('/')
        || name.starts_with('\\')
        || (name.len() >= 2
            && name.as_bytes()[0].is_ascii_alphabetic()
            && name.as_bytes()[1] == b':');
    relative || rooted
}
