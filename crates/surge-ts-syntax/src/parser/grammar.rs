//! Grammar-level diagnostics, collected in one walk of the oxc AST.
//!
//! These are the checks that need no types at all — a `const` with no
//! initializer, a duplicate object-literal key, an overload group with no
//! implementation, a member with no annotation. The lossy `Parsed*` tree drops
//! most of what they look at (an unannotated type member is not even kept), so
//! they run here, against the real AST, and the checker turns each entry into a
//! diagnostic.

use oxc_ast::ast::{
    AssignmentTarget, Class, ClassElement, Declaration, ExportDefaultDeclarationKind, Expression,
    ForInStatement, ForOfStatement, FormalParameters, Function, FunctionBody, MethodDefinitionKind,
    MethodDefinitionType, ModuleExportName, ObjectExpression, ObjectPropertyKind, Program,
    PropertyDefinitionType, PropertyKey, PropertyKind, Statement, TSGlobalDeclaration,
    TSEnumMemberName, TSMethodSignature, TSMethodSignatureKind, TSModuleDeclaration,
    TSModuleDeclarationBody,
    TSPropertySignature, TSSignature, VariableDeclaration, VariableDeclarationKind,
};
use oxc_ast_visit::Visit;
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::UnaryOperator;

use super::spans::text_span_from_oxc_span;
use crate::{ParsedGrammarDiagnostic, ParsedGrammarDiagnosticKind as Kind};

pub(crate) fn collect_grammar_diagnostics(program: &Program<'_>) -> Vec<ParsedGrammarDiagnostic> {
    let mut collector = GrammarCollector::default();
    collector.visit_program(program);
    collector.diagnostics
}

#[derive(Default)]
struct GrammarCollector {
    diagnostics: Vec<ParsedGrammarDiagnostic>,
    /// Whether the file is strict-mode code, which a few rules are specific to.
    /// An ES module always is; a script only with an explicit `"use strict"`.
    strict_mode: bool,
    /// `declare namespace`/`declare module` nesting. An ambient container makes
    /// a bodyless declaration legal, so the implementation-missing checks stay
    /// quiet inside one.
    ambient_depth: usize,
    /// The file's text, which modifier-order checks read: the AST keeps which
    /// modifiers a member has, not the order they were written in.
    source_text: String,
}

impl GrammarCollector {
    /// tsc's `checkReturnTypeAnnotation` for an `async` signature: the written
    /// return type must be a reference to the global `Promise`. A written type
    /// reference is left alone, since an alias may resolve to `Promise`.
    fn check_async_return_type(&mut self, return_type: Option<&oxc_ast::ast::TSTypeAnnotation<'_>>) {
        let Some(annotation) = return_type else {
            return;
        };
        let written = &annotation.type_annotation;
        if matches!(
            written,
            oxc_ast::ast::TSType::TSTypeReference(_)
                | oxc_ast::ast::TSType::TSTypeQuery(_)
                | oxc_ast::ast::TSType::TSImportType(_)
                | oxc_ast::ast::TSType::TSIndexedAccessType(_)
                | oxc_ast::ast::TSType::TSConditionalType(_)
                | oxc_ast::ast::TSType::TSParenthesizedType(_)
        ) {
            return;
        }
        let span = written.span();
        let text = self
            .source_text
            .get(span.start as usize..span.end as usize)
            .unwrap_or_default()
            .to_string();
        self.push(Kind::AsyncReturnTypeNotPromise, span, Some(&text));
    }

    fn push(&mut self, kind: Kind, span: Span, name: Option<&str>) {
        self.diagnostics.push(ParsedGrammarDiagnostic {
            kind,
            span: text_span_from_oxc_span(span),
            name: name.map(str::to_string),
        });
    }

    /// tsc's `checkTruthinessOfType` diagnostic, which depends only on syntax.
    fn check_truthiness(&mut self, expression: &Expression<'_>) {
        match syntactic_truthiness(expression) {
            Some(true) => self.push(Kind::AlwaysTruthyExpression, expression.span(), None),
            Some(false) => self.push(Kind::AlwaysFalsyExpression, expression.span(), None),
            None => {}
        }
    }

    fn is_ambient(&self) -> bool {
        self.ambient_depth > 0
    }

    /// `export default` appearing more than once in the module: tsc reports
    /// every one of them, not just the later.
    fn check_default_exports(&mut self, statements: &[Statement<'_>]) {
        let mut defaults: Vec<(Span, bool)> = Vec::new();

        for statement in statements {
            match statement {
                Statement::ExportDefaultDeclaration(export) => {
                    let (span, is_declaration) = match &export.declaration {
                        ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
                            (function.id.as_ref().map_or(export.span, |id| id.span), true)
                        }
                        ExportDefaultDeclarationKind::ClassDeclaration(class) => {
                            (class.id.as_ref().map_or(export.span, |id| id.span), true)
                        }
                        ExportDefaultDeclarationKind::Identifier(identifier) => {
                            (identifier.span, false)
                        }
                        _ => (export.span, false),
                    };
                    defaults.push((span, is_declaration));
                }
                Statement::ExportNamedDeclaration(export) => {
                    for specifier in &export.specifiers {
                        if export_name(&specifier.exported) == "default" {
                            defaults.push((specifier.exported.span(), false));
                        }
                    }
                }
                _ => {}
            }
        }

        if defaults.len() < 2 {
            return;
        }
        // Two default *declarations* are an attempted merge, which tsc reports
        // with the redeclaration diagnostics (TS2323 and friends) instead.
        if defaults
            .iter()
            .filter(|(_, is_declaration)| *is_declaration)
            .count()
            >= 2
        {
            return;
        }
        for (span, _) in defaults {
            self.push(Kind::MultipleDefaultExports, span, None);
        }
    }

    /// An overload group with no implementation anywhere in its container. The
    /// narrower "implementation is not *immediately* following" half of tsc's
    /// check is deliberately left out: only a group that has no body at all
    /// reports here.
    fn check_function_implementations(&mut self, statements: &[Statement<'_>]) {
        if self.is_ambient() {
            return;
        }

        let mut groups: Vec<(&str, Vec<&Function<'_>>)> = Vec::new();
        for statement in statements {
            let function = match statement {
                Statement::FunctionDeclaration(function) => Some(&**function),
                Statement::ExportNamedDeclaration(export) => match &export.declaration {
                    Some(Declaration::FunctionDeclaration(function)) => Some(&**function),
                    _ => None,
                },
                _ => None,
            };
            let Some(function) = function else {
                continue;
            };
            let Some(id) = function.id.as_ref() else {
                continue;
            };
            match groups
                .iter_mut()
                .find(|(name, _)| *name == id.name.as_str())
            {
                Some((_, declarations)) => declarations.push(function),
                None => groups.push((id.name.as_str(), vec![function])),
            }
        }

        for (_, declarations) in groups {
            if declarations
                .iter()
                .any(|function| function.body.is_some() || function.declare)
            {
                continue;
            }
            let Some(last) = declarations.last() else {
                continue;
            };
            let Some(id) = last.id.as_ref() else {
                continue;
            };
            self.push(Kind::FunctionImplementationMissing, id.span, None);
        }
    }

    /// Everything one class body can say wrong about its own members: an
    /// overload group with no implementation (TS2391/TS2390), two
    /// implementations of one name (TS2393/TS2392), and two members declaring
    /// the same name where neither is an overload of the other (TS2300).
    ///
    /// Static and instance members are separate names, and an `abstract` or
    /// ambient member has no implementation to miss.
    /// tsc's `checkGrammarModifiers` ordering rules for class members: an
    /// accessibility modifier precedes `static`, `override`, `readonly`, `async`
    /// and `abstract`; `static` precedes `override`, `readonly` and `async`;
    /// `override` precedes `readonly` and `async`. The first violation of a
    /// member is reported on the later modifier. A modifier list containing
    /// anything but plain keywords (a decorator, a comment) or repeating one is
    /// left alone, since tsc reports those differently.
    fn check_member_modifier_order(&mut self, class: &Class<'_>) {
        for element in &class.body.body {
            let (element_span, decorators, key_start) = match element {
                ClassElement::MethodDefinition(method) => {
                    (method.span, &method.decorators, method.key.span().start)
                }
                ClassElement::PropertyDefinition(property) => {
                    (property.span, &property.decorators, property.key.span().start)
                }
                ClassElement::AccessorProperty(property) => {
                    (property.span, &property.decorators, property.key.span().start)
                }
                _ => continue,
            };
            let modifiers_start = decorators
                .iter()
                .map(|decorator| decorator.span.end)
                .max()
                .unwrap_or(element_span.start)
                .max(element_span.start);
            let Some(prefix) = self
                .source_text
                .get(modifiers_start as usize..key_start as usize)
            else {
                continue;
            };
            let mut seen: Vec<&str> = Vec::new();
            let mut offset = modifiers_start as usize;
            let mut rest = prefix;
            let mut violation: Option<(&str, &str, usize, usize)> = None;
            loop {
                let trimmed = rest.trim_start();
                offset += rest.len() - trimmed.len();
                if trimmed.is_empty() {
                    break;
                }
                if let Some(comment) = trimmed.strip_prefix("/*") {
                    let Some(end) = comment.find("*/") else {
                        seen.push("\u{0}");
                        break;
                    };
                    let skipped = 2 + end + 2;
                    offset += skipped;
                    rest = &trimmed[skipped..];
                    continue;
                }
                let word_len = trimmed
                    .find(|c: char| c.is_whitespace())
                    .unwrap_or(trimmed.len());
                let word = &trimmed[..word_len];
                if matches!(word, "get" | "set" | "*" | "[") {
                    break;
                }
                let precedence: &[&str] = match word {
                    // Outside an abstract class tsc stops at the misplaced
                    // `abstract` itself, so the order is only judged inside one.
                    "public" | "private" | "protected" if class.r#abstract => {
                        &["override", "static", "accessor", "readonly", "async", "abstract"]
                    }
                    "public" | "private" | "protected" => {
                        &["override", "static", "accessor", "readonly", "async"]
                    }
                    "static" => &["readonly", "async", "accessor", "override"],
                    "override" => &["readonly", "accessor", "async"],
                    "readonly" | "async" | "abstract" | "declare" | "accessor" => &[],
                    _ => {
                        violation = None;
                        seen.clear();
                        seen.push("\u{0}");
                        break;
                    }
                };
                if seen.contains(&word) {
                    seen.push("\u{0}");
                    break;
                }
                if violation.is_none()
                    && let Some(earlier) = precedence.iter().find(|modifier| seen.contains(modifier))
                {
                    violation = Some((word, earlier, offset, word_len));
                }
                seen.push(word);
                offset += word_len;
                rest = &trimmed[word_len..];
            }
            if seen.contains(&"\u{0}") {
                continue;
            }
            if let Some((first, second, start, len)) = violation {
                let pair = format!("{first}\u{0}{second}");
                self.push(
                    Kind::ModifierMustPrecede,
                    Span::new(start as u32, (start + len) as u32),
                    Some(&pair),
                );
            }
        }
    }

    fn check_class_members(&mut self, class: &Class<'_>) {
        let ambient = self.is_ambient() || class.declare;
        let mut groups: Vec<MemberGroup> = Vec::new();
        let mut constructors: Vec<(Span, bool)> = Vec::new();

        for element in &class.body.body {
            let (key, is_static, member) = match element {
                ClassElement::MethodDefinition(method) => {
                    if method.r#type == MethodDefinitionType::TSAbstractMethodDefinition
                        && !class.r#abstract
                    {
                        self.push(Kind::AbstractMethodOutsideAbstractClass, method.span, None);
                    }
                    if method.kind == MethodDefinitionKind::Constructor {
                        if method.value.body.is_none() {
                            self.check_signature_parameters(&method.value.params, true);
                        }
                        constructors.push((method.key.span(), method.value.body.is_some()));
                        continue;
                    }
                    let member = match method.kind {
                        MethodDefinitionKind::Get | MethodDefinitionKind::Set => {
                            MemberKind::Accessor
                        }
                        _ if method.r#type == MethodDefinitionType::TSAbstractMethodDefinition => {
                            MemberKind::AbstractMethod
                        }
                        _ => MemberKind::Method {
                            has_body: method.value.body.is_some(),
                        },
                    };
                    (&method.key, method.r#static, member)
                }
                ClassElement::PropertyDefinition(property) => {
                    if property.r#type == PropertyDefinitionType::TSAbstractPropertyDefinition
                        && !class.r#abstract
                    {
                        // tsc reports on the modifier, which is where the
                        // member's own span starts.
                        self.push(
                            Kind::AbstractPropertyOutsideAbstractClass,
                            property.span,
                            None,
                        );
                    }
                    if ambient
                        && let Some(value) = property.value.as_ref() {
                            self.push(Kind::AmbientInitializer, value.span(), None);
                        }
                    (&property.key, property.r#static, MemberKind::Property)
                }
                _ => continue,
            };

            let Some(name) = property_key_name(key) else {
                continue;
            };
            match groups
                .iter_mut()
                .find(|group| group.name == name && group.is_static == is_static)
            {
                Some(group) => group.members.push((key.span(), member)),
                None => groups.push(MemberGroup {
                    name,
                    is_static,
                    members: vec![(key.span(), member)],
                }),
            }
        }

        for group in &groups {
            self.report_member_group(group, ambient);
        }

        // A property with no annotation and no initializer has an implicit
        // `any` type *unless* the constructor assigns it: tsc infers the
        // declaration's type from that assignment.
        let assigned = constructor_assigned_property_names(class);
        for element in &class.body.body {
            let ClassElement::PropertyDefinition(property) = element else {
                continue;
            };
            if property.type_annotation.is_some() || property.value.is_some() || property.computed {
                continue;
            }
            let Some(name) = property_key_name(&property.key) else {
                continue;
            };
            if assigned.contains(&name) {
                continue;
            }
            self.push(Kind::ImplicitAnyMember, property.key.span(), Some(&name));
        }

        if constructors.len() > 1 {
            let implementations: Vec<Span> = constructors
                .iter()
                .filter(|(_, has_body)| *has_body)
                .map(|(span, _)| *span)
                .collect();
            if implementations.len() > 1 {
                for span in implementations {
                    self.push(Kind::MultipleConstructorImplementations, span, None);
                }
            }
        }
        if !ambient
            && !constructors.is_empty()
            && !constructors.iter().any(|(_, has_body)| *has_body)
            && let Some((span, _)) = constructors.last() {
                self.push(Kind::ConstructorImplementationMissing, *span, None);
            }

        if !ambient && class.super_class.is_some() {
            for element in &class.body.body {
                let ClassElement::MethodDefinition(method) = element else {
                    continue;
                };
                if method.kind != MethodDefinitionKind::Constructor {
                    continue;
                }
                let Some(body) = method.value.body.as_ref() else {
                    continue;
                };
                if !body_calls_super(body) {
                    self.push(Kind::MissingSuperCall, method.key.span(), None);
                }
            }
        }
    }

    /// One name's worth of class or interface members. A group of method
    /// signatures is an overload set, so it reports only about implementations;
    /// as soon as a property is in the group nothing there is an overload of
    /// anything and every member is a duplicate.
    fn report_member_group(&mut self, group: &MemberGroup, ambient: bool) {
        let has_property = group
            .members
            .iter()
            .any(|(_, member)| matches!(member, MemberKind::Property));
        let has_accessor = group
            .members
            .iter()
            .any(|(_, member)| matches!(member, MemberKind::Accessor));

        if group.members.len() > 1 && has_property && !has_accessor {
            let name = group.name.clone();
            for (span, _) in &group.members {
                self.push(Kind::DuplicateMember, *span, Some(&name));
            }
            return;
        }
        if has_property || has_accessor {
            return;
        }

        let implementations: Vec<Span> = group
            .members
            .iter()
            .filter(|(_, member)| matches!(member, MemberKind::Method { has_body: true }))
            .map(|(span, _)| *span)
            .collect();
        if implementations.len() > 1 {
            for span in implementations {
                self.push(Kind::DuplicateImplementation, span, None);
            }
            return;
        }

        let declares_method = group
            .members
            .iter()
            .any(|(_, member)| matches!(member, MemberKind::Method { .. }));
        if ambient || !declares_method || !implementations.is_empty() {
            return;
        }
        if group
            .members
            .iter()
            .any(|(_, member)| matches!(member, MemberKind::AbstractMethod))
        {
            return;
        }
        if let Some((span, _)) = group.members.last() {
            self.push(Kind::FunctionImplementationMissing, *span, None);
        }
    }

    /// An interface reports the duplicate half only: a bodyless method there is
    /// a signature, never a missing implementation.
    fn check_interface_members(&mut self, members: &[TSSignature<'_>]) {
        let mut groups: Vec<MemberGroup> = Vec::new();

        for member in members {
            let (key, kind) = match member {
                TSSignature::TSPropertySignature(property) => (&property.key, MemberKind::Property),
                TSSignature::TSMethodSignature(method) => match method.kind {
                    TSMethodSignatureKind::Method => {
                        (&method.key, MemberKind::Method { has_body: false })
                    }
                    _ => (&method.key, MemberKind::Accessor),
                },
                _ => continue,
            };
            let Some(name) = property_key_name(key) else {
                continue;
            };
            match groups.iter_mut().find(|group| group.name == name) {
                Some(group) => group.members.push((key.span(), kind)),
                None => groups.push(MemberGroup {
                    name,
                    is_static: false,
                    members: vec![(key.span(), kind)],
                }),
            }
        }

        for group in groups {
            let has_property = group
                .members
                .iter()
                .any(|(_, member)| matches!(member, MemberKind::Property));
            let has_accessor = group
                .members
                .iter()
                .any(|(_, member)| matches!(member, MemberKind::Accessor));
            if group.members.len() > 1 && has_property && !has_accessor {
                for (span, _) in &group.members {
                    self.push(Kind::DuplicateMember, *span, Some(&group.name));
                }
            }
        }
    }

    /// The parameter-list grammar, checked wherever a list appears (a
    /// function, a method, a constructor, an arrow, a signature): a required
    /// parameter after an optional one, a parameter written both `?` and with
    /// a default, and a parameter written after the rest.
    fn check_parameter_list(&mut self, parameters: &FormalParameters<'_>) {
        let mut seen_optional = false;

        for parameter in &parameters.items {
            if parameter.optional && parameter.initializer.is_some() {
                self.push(Kind::OptionalParameterWithInitializer, parameter.span, None);
            }
            if parameter.optional {
                seen_optional = true;
            } else if seen_optional && parameter.initializer.is_none() {
                // A *defaulted* parameter does not make the next one illegal —
                // only a `?` one does.
                self.push(Kind::RequiredParameterAfterOptional, parameter.span, None);
            }
        }
    }

    /// A signature has no body to run a default in, and no instance to declare
    /// a parameter property on.
    fn check_signature_parameters(
        &mut self,
        parameters: &FormalParameters<'_>,
        is_constructor: bool,
    ) {
        for parameter in &parameters.items {
            if parameter.initializer.is_some() {
                self.push(
                    Kind::ParameterInitializerOutsideImplementation,
                    parameter.span,
                    None,
                );
            }
            if is_constructor && (parameter.accessibility.is_some() || parameter.readonly) {
                self.push(
                    Kind::ParameterPropertyOutsideImplementation,
                    parameter.span,
                    None,
                );
            }
        }
    }

    fn check_variable_initializers(&mut self, declaration: &VariableDeclaration<'_>) {
        if declaration.declare || self.is_ambient() {
            // An ambient declaration declares a value rather than producing
            // one, so it may not carry an initializer — and its missing
            // initializer is not the `const` grammar error either.
            for declarator in &declaration.declarations {
                if let Some(initializer) = declarator.init.as_ref() {
                    self.push(Kind::AmbientInitializer, initializer.span(), None);
                }
            }
            return;
        }

        if declaration.kind != VariableDeclarationKind::Const {
            return;
        }
        for declarator in &declaration.declarations {
            if declarator.init.is_some() {
                continue;
            }
            self.push(Kind::ConstNotInitialized, declarator.id.span(), None);
        }
    }

    /// tsc reports every property whose name was already written, so
    /// `{ a: 1, a: 2, a: 3 }` reports twice. A duplicate that involves a method
    /// or an accessor is a *different* diagnostic there (TS2300 for methods, a
    /// legal get/set pair for accessors), so only plain properties count.
    fn check_duplicate_properties(&mut self, object: &ObjectExpression<'_>) {
        let mut seen: Vec<String> = Vec::new();
        let mut methods: Vec<(String, Vec<Span>)> = Vec::new();
        let mut accessors: Vec<String> = Vec::new();

        for property in &object.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                continue;
            };
            if property.kind != PropertyKind::Init {
                if let Some(name) = property_key_name(&property.key) {
                    if seen.contains(&name)
                        || methods.iter().any(|(other, _)| *other == name)
                    {
                        self.push(
                            Kind::ObjectLiteralPropertyAndAccessor,
                            property.key.span(),
                            None,
                        );
                    }
                    accessors.push(name);
                }
                continue;
            }
            let Some(name) = property_key_name(&property.key) else {
                continue;
            };
            if name == "__proto__" {
                continue;
            }
            // A name written as both an accessor and something else is its own
            // diagnostic, reported on whichever came later.
            if accessors.contains(&name) {
                self.push(
                    Kind::ObjectLiteralPropertyAndAccessor,
                    property.key.span(),
                    None,
                );
            }
            if property.method {
                match methods.iter_mut().find(|(other, _)| *other == name) {
                    Some((_, spans)) => spans.push(property.span),
                    None => methods.push((name, vec![property.span])),
                }
                continue;
            }
            if seen.contains(&name) {
                self.push(Kind::DuplicateObjectLiteralProperty, property.span, None);
            } else {
                seen.push(name);
            }
        }

        // A repeated *method* is a duplicate identifier to tsc, not a repeated
        // property, and it names both of them. A name written once as a method
        // and once as a property is a third diagnostic pair (TS1119 with
        // TS2300) that is deliberately left alone.
        for (name, spans) in methods {
            if spans.len() < 2 || seen.contains(&name) {
                continue;
            }
            for span in spans {
                self.push(Kind::DuplicateMember, span, Some(&name));
            }
        }
    }

    /// A signature with neither a body nor a written return type has an
    /// implicit `any` return — the overload signatures of a function that *does*
    /// have an implementation included, which is why this does not look at the
    /// group the way the missing-implementation check does.
    fn check_implicit_any_return(&mut self, function: &Function<'_>) {
        if function.body.is_some() || function.return_type.is_some() {
            return;
        }
        let Some(id) = function.id.as_ref() else {
            return;
        };
        self.push(Kind::ImplicitAnyReturn, id.span, Some(id.name.as_str()));
    }
}

struct MemberGroup {
    name: String,
    is_static: bool,
    members: Vec<(Span, MemberKind)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MemberKind {
    Property,
    Accessor,
    AbstractMethod,
    Method { has_body: bool },
}

/// The names a class constructor assigns through `this.<name> = …`, which is
/// where tsc gets the type of a property declared without one.
fn constructor_assigned_property_names(class: &Class<'_>) -> Vec<String> {
    struct ThisAssignmentCollector {
        names: Vec<String>,
    }

    impl<'a> Visit<'a> for ThisAssignmentCollector {
        fn visit_assignment_expression(
            &mut self,
            assignment: &oxc_ast::ast::AssignmentExpression<'a>,
        ) {
            if let AssignmentTarget::StaticMemberExpression(member) = &assignment.left
                && matches!(member.object, Expression::ThisExpression(_)) {
                    self.names.push(member.property.name.to_string());
                }
            oxc_ast_visit::walk::walk_assignment_expression(self, assignment);
        }
    }

    let mut collector = ThisAssignmentCollector { names: Vec::new() };
    for element in &class.body.body {
        let ClassElement::MethodDefinition(method) = element else {
            continue;
        };
        if method.kind != MethodDefinitionKind::Constructor {
            continue;
        }
        if let Some(body) = method.value.body.as_ref() {
            collector.visit_function_body(body);
        }
    }
    collector.names
}

/// Whether a constructor body calls `super(...)` anywhere inside it, including
/// inside a branch or an arrow function — the same places the call counts for
/// tsc.
fn body_calls_super(body: &FunctionBody<'_>) -> bool {
    struct SuperCallFinder {
        found: bool,
    }

    impl<'a> Visit<'a> for SuperCallFinder {
        fn visit_call_expression(&mut self, call: &oxc_ast::ast::CallExpression<'a>) {
            if matches!(call.callee, Expression::Super(_)) {
                self.found = true;
            }
            oxc_ast_visit::walk::walk_call_expression(self, call);
        }
    }

    let mut finder = SuperCallFinder { found: false };
    finder.visit_function_body(body);
    finder.found
}

/// tsc's `isSideEffectFree`: the expressions whose value can be dropped without
/// anything happening. A property access is deliberately *not* one of them — a
/// getter can do anything.
/// tsc's `getSyntacticTruthySemantics`: `Some(true)` for syntax that is always
/// truthy, `Some(false)` for syntax always falsy, `None` when it can be either.
fn syntactic_truthiness(expression: &Expression<'_>) -> Option<bool> {
    match expression.get_inner_expression() {
        // `while (0)` and `while (1)` are idioms, not mistakes. tsc compares the
        // literal's normalized text, so `0.0` and `0x1` are exempt too.
        Expression::NumericLiteral(literal) => {
            (literal.value != 0.0 && literal.value != 1.0).then_some(true)
        }
        Expression::ArrayExpression(_)
        | Expression::ArrowFunctionExpression(_)
        | Expression::BigIntLiteral(_)
        | Expression::ClassExpression(_)
        | Expression::FunctionExpression(_)
        | Expression::JSXElement(_)
        | Expression::ObjectExpression(_)
        | Expression::RegExpLiteral(_) => Some(true),
        Expression::NullLiteral(_) => Some(false),
        Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::Void => Some(false),
        Expression::StringLiteral(literal) => Some(!literal.value.is_empty()),
        Expression::TemplateLiteral(template) if template.expressions.is_empty() => Some(
            template
                .quasis
                .first()
                .is_some_and(|quasi| !quasi.value.raw.is_empty()),
        ),
        Expression::ConditionalExpression(conditional) => {
            let when_true = syntactic_truthiness(&conditional.consequent)?;
            (syntactic_truthiness(&conditional.alternate)? == when_true).then_some(when_true)
        }
        Expression::Identifier(identifier) if identifier.name == "undefined" => Some(false),
        _ => None,
    }
}

/// tsc's `getSyntacticNullishnessSemantics`: `Some(true)` for syntax that is
/// always nullish, `Some(false)` for syntax never nullish, `None` when it can be
/// either. A nested `??` follows its right operand, as TypeScript 7.0 does.
fn syntactic_nullishness(expression: &Expression<'_>) -> Option<bool> {
    match expression.get_inner_expression() {
        Expression::AwaitExpression(_)
        | Expression::CallExpression(_)
        | Expression::ChainExpression(_)
        | Expression::ImportExpression(_)
        | Expression::TaggedTemplateExpression(_)
        | Expression::ComputedMemberExpression(_)
        | Expression::StaticMemberExpression(_)
        | Expression::PrivateFieldExpression(_)
        | Expression::MetaProperty(_)
        | Expression::NewExpression(_)
        | Expression::YieldExpression(_)
        | Expression::ThisExpression(_)
        | Expression::Super(_) => None,
        Expression::LogicalExpression(logical) => match logical.operator {
            oxc_syntax::operator::LogicalOperator::Coalesce => syntactic_nullishness(&logical.right),
            _ => None,
        },
        Expression::AssignmentExpression(assignment) => match assignment.operator {
            oxc_syntax::operator::AssignmentOperator::Assign
            | oxc_syntax::operator::AssignmentOperator::LogicalNullish => {
                syntactic_nullishness(&assignment.right)
            }
            oxc_syntax::operator::AssignmentOperator::LogicalOr
            | oxc_syntax::operator::AssignmentOperator::LogicalAnd => None,
            _ => Some(false),
        },
        Expression::SequenceExpression(sequence) => {
            sequence.expressions.last().and_then(syntactic_nullishness)
        }
        Expression::ConditionalExpression(conditional) => {
            let when_true = syntactic_nullishness(&conditional.consequent)?;
            (syntactic_nullishness(&conditional.alternate)? == when_true).then_some(when_true)
        }
        Expression::NullLiteral(_) => Some(true),
        Expression::Identifier(identifier) => (identifier.name == "undefined").then_some(true),
        _ => Some(false),
    }
}

fn is_side_effect_free(expression: &Expression<'_>) -> bool {
    match expression.get_inner_expression() {
        Expression::Identifier(_)
        | Expression::StringLiteral(_)
        | Expression::RegExpLiteral(_)
        | Expression::TemplateLiteral(_)
        | Expression::TaggedTemplateExpression(_)
        | Expression::NumericLiteral(_)
        | Expression::BigIntLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_)
        | Expression::FunctionExpression(_)
        | Expression::ClassExpression(_)
        | Expression::ArrowFunctionExpression(_)
        | Expression::ArrayExpression(_)
        | Expression::ObjectExpression(_)
        | Expression::JSXElement(_)
        | Expression::JSXFragment(_) => true,
        Expression::ConditionalExpression(conditional) => {
            is_side_effect_free(&conditional.consequent)
                && is_side_effect_free(&conditional.alternate)
        }
        Expression::BinaryExpression(binary) => {
            is_side_effect_free(&binary.left) && is_side_effect_free(&binary.right)
        }
        Expression::UnaryExpression(unary) => matches!(
            unary.operator,
            UnaryOperator::LogicalNot
                | UnaryOperator::UnaryPlus
                | UnaryOperator::UnaryNegation
                | UnaryOperator::BitwiseNot
                | UnaryOperator::Typeof
        ),
        Expression::TSNonNullExpression(_) => true,
        _ => false,
    }
}

fn export_name(name: &ModuleExportName<'_>) -> String {
    match name {
        ModuleExportName::IdentifierName(identifier) => identifier.name.to_string(),
        ModuleExportName::IdentifierReference(identifier) => identifier.name.to_string(),
        ModuleExportName::StringLiteral(literal) => literal.value.to_string(),
    }
}

fn property_key_name(key: &PropertyKey<'_>) -> Option<String> {
    match key {
        PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.to_string()),
        PropertyKey::StringLiteral(literal) => Some(literal.value.to_string()),
        PropertyKey::NumericLiteral(literal) => Some(literal.value.to_string()),
        PropertyKey::PrivateIdentifier(identifier) => Some(format!("#{}", identifier.name)),
        _ => None,
    }
}

impl<'a> Visit<'a> for GrammarCollector {
    fn visit_program(&mut self, program: &Program<'a>) {
        self.source_text = program.source_text.to_string();
        self.strict_mode = program.source_type.is_module()
            || program
                .directives
                .iter()
                .any(|directive| directive.directive == "use strict");
        self.check_default_exports(&program.body);
        self.check_function_implementations(&program.body);
        oxc_ast_visit::walk::walk_program(self, program);
    }

    fn visit_function_body(&mut self, body: &FunctionBody<'a>) {
        self.check_function_implementations(&body.statements);
        oxc_ast_visit::walk::walk_function_body(self, body);
    }

    // An enum member initializer may only name members declared before it: the
    // members are evaluated in order, so a forward reference reads a binding
    // that does not have its value yet (tsc's `checkEnumDeclaration`, TS2651).
    fn visit_ts_enum_declaration(&mut self, declaration: &oxc_ast::ast::TSEnumDeclaration<'a>) {
        let mut declared_so_far: Vec<&str> = Vec::new();
        let member_names: Vec<Option<String>> = declaration
            .body
            .members
            .iter()
            .map(|member| property_key_name_of_enum_member(&member.id))
            .collect();

        for (index, member) in declaration.body.members.iter().enumerate() {
            if let Some(initializer) = &member.initializer {
                let later: Vec<&str> = member_names
                    .iter()
                    .skip(index)
                    .filter_map(|name| name.as_deref())
                    .collect();
                report_enum_forward_references(self, initializer, &later);
            }
            if let Some(name) = member_names[index].as_deref() {
                declared_so_far.push(name);
            }
        }

        oxc_ast_visit::walk::walk_ts_enum_declaration(self, declaration);
    }

    // `delete x` on a direct binding reference is a strict-mode syntax error
    // (tsc's `checkStrictModeDeleteExpression`, binder.go:1409). `delete o.p`,
    // the legal form, is checked by the checker's own operand rules.
    fn visit_unary_expression(&mut self, unary: &oxc_ast::ast::UnaryExpression<'a>) {
        if unary.operator == UnaryOperator::LogicalNot {
            self.check_truthiness(&unary.argument);
        }
        if self.strict_mode
            && unary.operator == oxc_syntax::operator::UnaryOperator::Delete
            && let Expression::Identifier(identifier) = &unary.argument
        {
            self.push(Kind::DeleteOnIdentifierInStrictMode, identifier.span, None);
        }
        oxc_ast_visit::walk::walk_unary_expression(self, unary);
    }

    // `declare global { … }` is its own node, and everything inside it is
    // ambient: a bodyless declaration there is a declaration, not a missing
    // implementation.
    fn visit_ts_global_declaration(&mut self, declaration: &TSGlobalDeclaration<'a>) {
        self.ambient_depth += 1;
        oxc_ast_visit::walk::walk_ts_global_declaration(self, declaration);
        self.ambient_depth -= 1;
    }

    fn visit_ts_module_declaration(&mut self, declaration: &TSModuleDeclaration<'a>) {
        let ambient = declaration.declare || self.is_ambient();
        if ambient {
            self.ambient_depth += 1;
        }
        if let Some(TSModuleDeclarationBody::TSModuleBlock(block)) = declaration.body.as_ref() {
            self.check_function_implementations(&block.body);
        }
        oxc_ast_visit::walk::walk_ts_module_declaration(self, declaration);
        if ambient {
            self.ambient_depth -= 1;
        }
    }

    fn visit_class(&mut self, class: &Class<'a>) {
        self.check_class_members(class);
        self.check_member_modifier_order(class);
        oxc_ast_visit::walk::walk_class(self, class);
    }

    fn visit_ts_interface_declaration(
        &mut self,
        declaration: &oxc_ast::ast::TSInterfaceDeclaration<'a>,
    ) {
        self.check_interface_members(&declaration.body.body);
        oxc_ast_visit::walk::walk_ts_interface_declaration(self, declaration);
    }

    fn visit_ts_type_literal(&mut self, literal: &oxc_ast::ast::TSTypeLiteral<'a>) {
        self.check_interface_members(&literal.members);
        oxc_ast_visit::walk::walk_ts_type_literal(self, literal);
    }

    fn visit_if_statement(&mut self, statement: &oxc_ast::ast::IfStatement<'a>) {
        self.check_truthiness(&statement.test);
        oxc_ast_visit::walk::walk_if_statement(self, statement);
    }

    fn visit_while_statement(&mut self, statement: &oxc_ast::ast::WhileStatement<'a>) {
        self.check_truthiness(&statement.test);
        oxc_ast_visit::walk::walk_while_statement(self, statement);
    }

    fn visit_do_while_statement(&mut self, statement: &oxc_ast::ast::DoWhileStatement<'a>) {
        self.check_truthiness(&statement.test);
        oxc_ast_visit::walk::walk_do_while_statement(self, statement);
    }

    fn visit_for_statement(&mut self, statement: &oxc_ast::ast::ForStatement<'a>) {
        if let Some(test) = &statement.test {
            self.check_truthiness(test);
        }
        oxc_ast_visit::walk::walk_for_statement(self, statement);
    }

    fn visit_conditional_expression(
        &mut self,
        conditional: &oxc_ast::ast::ConditionalExpression<'a>,
    ) {
        self.check_truthiness(&conditional.test);
        oxc_ast_visit::walk::walk_conditional_expression(self, conditional);
    }

    fn visit_logical_expression(&mut self, logical: &oxc_ast::ast::LogicalExpression<'a>) {
        if logical.operator == oxc_syntax::operator::LogicalOperator::Coalesce {
            let left = logical.left.get_inner_expression();
            match syntactic_nullishness(left) {
                Some(true) => self.push(Kind::AlwaysNullishCoalesceOperand, left.span(), None),
                Some(false) => self.push(Kind::NeverNullishCoalesceOperand, left.span(), None),
                None => {}
            }
        } else {
            self.check_truthiness(&logical.left);
        }
        oxc_ast_visit::walk::walk_logical_expression(self, logical);
    }

    fn visit_sequence_expression(&mut self, sequence: &oxc_ast::ast::SequenceExpression<'a>) {
        // Every operand but the last has its value discarded.
        for expression in sequence
            .expressions
            .iter()
            .take(sequence.expressions.len().saturating_sub(1))
        {
            if is_side_effect_free(expression) {
                self.push(Kind::UnusedCommaOperand, expression.span(), None);
            }
        }
        oxc_ast_visit::walk::walk_sequence_expression(self, sequence);
    }

    fn visit_function(&mut self, function: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        self.check_implicit_any_return(function);
        if function.r#async && !function.generator {
            self.check_async_return_type(function.return_type.as_deref());
        }
        if function.body.is_none() {
            self.check_signature_parameters(&function.params, false);
        }
        oxc_ast_visit::walk::walk_function(self, function, flags);
    }

    fn visit_arrow_function_expression(&mut self, arrow: &oxc_ast::ast::ArrowFunctionExpression<'a>) {
        if arrow.r#async {
            self.check_async_return_type(arrow.return_type.as_deref());
        }
        oxc_ast_visit::walk::walk_arrow_function_expression(self, arrow);
    }

    fn visit_formal_parameters(&mut self, parameters: &FormalParameters<'a>) {
        self.check_parameter_list(parameters);
        oxc_ast_visit::walk::walk_formal_parameters(self, parameters);
    }

    fn visit_method_definition(&mut self, method: &oxc_ast::ast::MethodDefinition<'a>) {
        if method.kind == MethodDefinitionKind::Set {
            if method.value.return_type.is_some() {
                self.push(Kind::SetAccessorReturnType, method.key.span(), None);
            }
            if method.value.params.items.len() != 1 || method.value.params.rest.is_some() {
                self.push(Kind::SetAccessorParameterCount, method.key.span(), None);
            }
        }
        if method.kind == MethodDefinitionKind::Method
            && method.value.body.is_none()
            && method.value.return_type.is_none()
            && !method.computed
            && let Some(name) = property_key_name(&method.key) {
                self.push(Kind::ImplicitAnyReturn, method.key.span(), Some(&name));
            }
        oxc_ast_visit::walk::walk_method_definition(self, method);
    }

    fn visit_ts_property_signature(&mut self, property: &TSPropertySignature<'a>) {
        if property.type_annotation.is_none() && !property.computed
            && let Some(name) = property_key_name(&property.key) {
                self.push(Kind::ImplicitAnyMember, property.key.span(), Some(&name));
            }
        oxc_ast_visit::walk::walk_ts_property_signature(self, property);
    }

    fn visit_ts_method_signature(&mut self, method: &TSMethodSignature<'a>) {
        if method.kind == TSMethodSignatureKind::Method
            && method.return_type.is_none()
            && !method.computed
            && let Some(name) = property_key_name(&method.key) {
                self.push(Kind::ImplicitAnyReturn, method.key.span(), Some(&name));
            }
        oxc_ast_visit::walk::walk_ts_method_signature(self, method);
    }

    fn visit_variable_declaration(&mut self, declaration: &VariableDeclaration<'a>) {
        self.check_variable_initializers(declaration);
        oxc_ast_visit::walk::walk_variable_declaration(self, declaration);
    }

    fn visit_object_expression(&mut self, object: &ObjectExpression<'a>) {
        self.check_duplicate_properties(object);
        oxc_ast_visit::walk::walk_object_expression(self, object);
    }

    // A `for (const x of xs)` binding is initialized by the loop, not by an
    // initializer, so its declaration must not reach the const check.
    fn visit_for_of_statement(&mut self, statement: &ForOfStatement<'a>) {
        self.visit_expression(&statement.right);
        self.visit_statement(&statement.body);
    }

    fn visit_for_in_statement(&mut self, statement: &ForInStatement<'a>) {
        self.visit_expression(&statement.right);
        self.visit_statement(&statement.body);
    }
}

fn property_key_name_of_enum_member(name: &TSEnumMemberName<'_>) -> Option<String> {
    match name {
        TSEnumMemberName::Identifier(identifier) => Some(identifier.name.to_string()),
        TSEnumMemberName::String(literal) => Some(literal.value.to_string()),
        _ => None,
    }
}

/// Reports every identifier in `expression` naming one of `later`. The walk
/// covers the expression forms an enum initializer may take — an enum member
/// must be a constant expression, so there are no function bodies to stop at.
fn report_enum_forward_references(
    collector: &mut GrammarCollector,
    expression: &Expression<'_>,
    later: &[&str],
) {
    match expression {
        Expression::Identifier(identifier) => {
            if later.contains(&identifier.name.as_str()) {
                collector.push(Kind::EnumForwardReference, identifier.span, None);
            }
        }
        Expression::BinaryExpression(binary) => {
            report_enum_forward_references(collector, &binary.left, later);
            report_enum_forward_references(collector, &binary.right, later);
        }
        Expression::UnaryExpression(unary) => {
            report_enum_forward_references(collector, &unary.argument, later);
        }
        Expression::ParenthesizedExpression(parenthesized) => {
            report_enum_forward_references(collector, &parenthesized.expression, later);
        }
        Expression::TemplateLiteral(template) => {
            for interpolation in &template.expressions {
                report_enum_forward_references(collector, interpolation, later);
            }
        }
        _ => {}
    }
}
