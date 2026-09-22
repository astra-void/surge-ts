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
use crate::{ParenthesizedExpressionSpan, ParsedGrammarDiagnostic, ParsedGrammarDiagnosticKind as Kind};

pub(crate) fn collect_grammar_diagnostics(
    program: &Program<'_>,
) -> (Vec<ParsedGrammarDiagnostic>, Vec<ParenthesizedExpressionSpan>) {
    let mut collector = GrammarCollector::default();
    collector.visit_program(program);
    let mut parenthesized = collector.parenthesized_expressions;
    parenthesized.sort_unstable_by_key(|span| (span.inner.start, span.inner.end));
    (collector.diagnostics, parenthesized)
}

#[derive(Default)]
struct GrammarCollector {
    diagnostics: Vec<ParsedGrammarDiagnostic>,
    parenthesized_expressions: Vec<ParenthesizedExpressionSpan>,
    /// Whether the file is strict-mode code, which a few rules are specific to.
    /// An ES module always is; a script only with an explicit `"use strict"`.
    strict_mode: bool,
    /// Whether each enclosing function is `async`, innermost last. Empty at the
    /// top level, where a module may `await`.
    function_async: Vec<bool>,
    /// `declare namespace`/`declare module` nesting. An ambient container makes
    /// a bodyless declaration legal, so the implementation-missing checks stay
    /// quiet inside one.
    ambient_depth: usize,
    /// The file's text, which modifier-order checks read: the AST keeps which
    /// modifiers a member has, not the order they were written in.
    source_text: String,
    /// What each module-level binding contributes to an enum initializer that
    /// names it; see [`EnumConstant`].
    top_level_constants: std::collections::HashMap<String, EnumConstant>,
    /// Module-level enum declarations, which alone may consult
    /// `top_level_constants` (a nested one could see a shadowing binding).
    top_level_enums: std::collections::HashSet<(u32, u32)>,
}

/// What tsc's enum constant evaluation can say about an initializer without
/// resolving names it cannot see: an imported binding or another enum's member
/// may well be constant, so those are `Unknown` and never reported.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum EnumConstant {
    Number,
    String,
    NonConstant,
    Unknown,
}

fn enum_constant(
    expression: &Expression<'_>,
    members: &std::collections::HashMap<String, EnumConstant>,
    top_level: Option<&std::collections::HashMap<String, EnumConstant>>,
) -> EnumConstant {
    use oxc_syntax::operator::BinaryOperator;
    match expression {
        Expression::NumericLiteral(_) => EnumConstant::Number,
        Expression::StringLiteral(_) => EnumConstant::String,
        Expression::TemplateLiteral(template) if template.expressions.is_empty() => {
            EnumConstant::String
        }
        Expression::TemplateLiteral(_) => EnumConstant::Unknown,
        Expression::ParenthesizedExpression(parenthesized) => {
            enum_constant(&parenthesized.expression, members, top_level)
        }
        Expression::UnaryExpression(unary)
            if matches!(
                unary.operator,
                UnaryOperator::UnaryPlus | UnaryOperator::UnaryNegation | UnaryOperator::BitwiseNot
            ) =>
        {
            match enum_constant(&unary.argument, members, top_level) {
                EnumConstant::Number => EnumConstant::Number,
                EnumConstant::Unknown => EnumConstant::Unknown,
                _ => EnumConstant::NonConstant,
            }
        }
        Expression::BinaryExpression(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Addition
                    | BinaryOperator::Subtraction
                    | BinaryOperator::Multiplication
                    | BinaryOperator::Division
                    | BinaryOperator::Remainder
                    | BinaryOperator::Exponential
                    | BinaryOperator::ShiftLeft
                    | BinaryOperator::ShiftRight
                    | BinaryOperator::ShiftRightZeroFill
                    | BinaryOperator::BitwiseOR
                    | BinaryOperator::BitwiseXOR
                    | BinaryOperator::BitwiseAnd
            ) =>
        {
            let left = enum_constant(&binary.left, members, top_level);
            let right = enum_constant(&binary.right, members, top_level);
            match (left, right) {
                (EnumConstant::NonConstant, _) | (_, EnumConstant::NonConstant) => {
                    EnumConstant::NonConstant
                }
                (EnumConstant::Unknown, _) | (_, EnumConstant::Unknown) => EnumConstant::Unknown,
                (EnumConstant::Number, EnumConstant::Number) => EnumConstant::Number,
                _ if binary.operator == BinaryOperator::Addition => EnumConstant::String,
                _ => EnumConstant::NonConstant,
            }
        }
        Expression::Identifier(identifier) => members
            .get(identifier.name.as_str())
            .or_else(|| top_level.and_then(|constants| constants.get(identifier.name.as_str())))
            .copied()
            .unwrap_or(EnumConstant::Unknown),
        Expression::StaticMemberExpression(_) | Expression::ComputedMemberExpression(_) => {
            EnumConstant::Unknown
        }
        _ => EnumConstant::NonConstant,
    }
}

impl GrammarCollector {
    /// tsc's `computeEnumMemberValues`: a member without an initializer takes
    /// the previous numeric value plus one, so it needs one after a string or
    /// computed member (TS1061), and a `const enum` initializer must be a
    /// constant expression (TS2474). An ambient enum's members are otherwise
    /// free to be computed, but a written initializer must still be constant
    /// (TS1066).
    fn check_enum_member_values(&mut self, declaration: &oxc_ast::ast::TSEnumDeclaration<'_>) {
        let ambient = declaration.declare || self.is_ambient();
        let top_level_constants = std::mem::take(&mut self.top_level_constants);
        let top_level = self
            .top_level_enums
            .contains(&(declaration.span.start, declaration.span.end))
            .then_some(&top_level_constants);
        let mut members = std::collections::HashMap::new();
        let mut previous = EnumConstant::Number;
        for member in &declaration.body.members {
            let value = match &member.initializer {
                Some(initializer) => {
                    let value = enum_constant(initializer, &members, top_level);
                    if value == EnumConstant::NonConstant {
                        if declaration.r#const {
                            self.push(Kind::ConstEnumInitializerNotConstant, initializer.span(), None);
                        } else if ambient {
                            self.push(Kind::AmbientEnumInitializerNotConstant, initializer.span(), None);
                        }
                    }
                    previous = value;
                    value
                }
                None => {
                    if !ambient && matches!(previous, EnumConstant::String | EnumConstant::NonConstant) {
                        self.push(Kind::EnumMemberInitializerRequired, member.id.span(), None);
                    }
                    if previous == EnumConstant::Unknown {
                        EnumConstant::Unknown
                    } else {
                        EnumConstant::Number
                    }
                }
            };
            if let Some(name) = property_key_name_of_enum_member(&member.id) {
                members.insert(name, value);
            }
        }
        self.top_level_constants = top_level_constants;
    }

    /// Every member name declared more than once across the declarations of
    /// one enum is TS2300 at each declaration, named as written.
    fn check_duplicate_enum_members(&mut self, declarations: &[&oxc_ast::ast::TSEnumDeclaration<'_>]) {
        let names: Vec<(String, Span)> = declarations
            .iter()
            .flat_map(|declaration| declaration.body.members.iter())
            .filter_map(|member| {
                property_key_name_of_enum_member(&member.id).map(|name| (name, member.id.span()))
            })
            .collect();
        for (name, span) in &names {
            if names.iter().filter(|(other, _)| other == name).count() < 2 {
                continue;
            }
            let written = self
                .source_text
                .get(span.start as usize..span.end as usize)
                .unwrap_or(name)
                .to_string();
            self.push(Kind::DuplicateMember, *span, Some(&written));
        }
    }

    fn check_top_level_enum_merges(&mut self, statements: &[Statement<'_>]) {
        let mut groups: Vec<(&str, Vec<&oxc_ast::ast::TSEnumDeclaration<'_>>)> = Vec::new();
        for statement in statements {
            let declaration = match statement {
                Statement::ExportNamedDeclaration(export) => export.declaration.as_ref(),
                other => other.as_declaration(),
            };
            let Some(Declaration::TSEnumDeclaration(enumeration)) = declaration else {
                continue;
            };
            let name = enumeration.id.name.as_str();
            match groups.iter_mut().find(|(group, _)| *group == name) {
                Some((_, declarations)) => declarations.push(enumeration),
                None => groups.push((name, vec![enumeration])),
            }
        }
        for (_, declarations) in groups {
            self.check_duplicate_enum_members(&declarations);
        }
    }

    /// tsc's `checkKindsOfPropertyMemberOverrides` for classes whose base is a
    /// class declared at the top of the same file: an instance member may not
    /// change kind between property, accessor and method. A private member on
    /// either side is not an override, and an abstract base property or
    /// accessor may be implemented as either.
    fn check_member_kind_overrides(&mut self, statements: &[Statement<'_>]) {
        #[derive(Clone, Copy, PartialEq)]
        enum MemberKind {
            Property,
            Accessor,
            Method,
        }
        struct Member {
            kind: MemberKind,
            is_private: bool,
            is_abstract: bool,
            span: Span,
        }
        fn instance_members(class: &Class<'_>) -> Vec<(String, Member)> {
            class
                .body
                .body
                .iter()
                .filter_map(|element| {
                    let (key, is_static, member) = match element {
                        ClassElement::MethodDefinition(method) => {
                            let kind = match method.kind {
                                MethodDefinitionKind::Method => MemberKind::Method,
                                MethodDefinitionKind::Get | MethodDefinitionKind::Set => {
                                    MemberKind::Accessor
                                }
                                MethodDefinitionKind::Constructor => return None,
                            };
                            (
                                &method.key,
                                method.r#static,
                                Member {
                                    kind,
                                    is_private: method.accessibility
                                        == Some(oxc_ast::ast::TSAccessibility::Private),
                                    is_abstract: method.r#type
                                        == MethodDefinitionType::TSAbstractMethodDefinition,
                                    span: method.key.span(),
                                },
                            )
                        }
                        ClassElement::PropertyDefinition(property) => (
                            &property.key,
                            property.r#static,
                            Member {
                                kind: MemberKind::Property,
                                is_private: property.accessibility
                                    == Some(oxc_ast::ast::TSAccessibility::Private),
                                is_abstract: property.r#type
                                    == PropertyDefinitionType::TSAbstractPropertyDefinition,
                                span: property.key.span(),
                            },
                        ),
                        _ => return None,
                    };
                    if is_static || matches!(key, PropertyKey::PrivateIdentifier(_)) {
                        return None;
                    }
                    property_key_name(key).map(|name| (name, member))
                })
                .collect()
        }

        let classes: Vec<&Class<'_>> = statements
            .iter()
            .filter_map(|statement| match statement {
                Statement::ClassDeclaration(class) => Some(class.as_ref()),
                Statement::ExportNamedDeclaration(export) => match &export.declaration {
                    Some(Declaration::ClassDeclaration(class)) => Some(class.as_ref()),
                    _ => None,
                },
                Statement::ExportDefaultDeclaration(export) => match &export.declaration {
                    ExportDefaultDeclarationKind::ClassDeclaration(class) => Some(class.as_ref()),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        let class_named = |name: &str| {
            classes
                .iter()
                .copied()
                .find(|class| class.id.as_ref().is_some_and(|id| id.name == name))
        };
        let base_of = |class: &Class<'_>| match &class.super_class {
            Some(Expression::Identifier(base)) if class.super_type_arguments.is_none() => {
                class_named(base.name.as_str())
            }
            _ => None,
        };

        for class in &classes {
            let (Some(derived_name), Some(base)) = (class.id.as_ref(), base_of(class)) else {
                continue;
            };
            let Some(base_name) = base.id.as_ref() else {
                continue;
            };
            for (name, derived) in instance_members(class) {
                let mut ancestor = Some(base);
                let mut found = None;
                let mut depth = 0;
                while let Some(current) = ancestor
                    && depth < 32
                {
                    if let Some((_, member)) =
                        instance_members(current).into_iter().find(|(other, _)| *other == name)
                    {
                        found = Some(member);
                        break;
                    }
                    ancestor = base_of(current);
                    depth += 1;
                }
                let Some(inherited) = found else {
                    continue;
                };
                if inherited.is_private || derived.is_private {
                    continue;
                }
                let kind = match (inherited.kind, derived.kind) {
                    (MemberKind::Accessor, MemberKind::Property) if !inherited.is_abstract => {
                        Kind::PropertyAccessorOverride
                    }
                    (MemberKind::Property, MemberKind::Accessor) if !inherited.is_abstract => {
                        Kind::AccessorPropertyOverride
                    }
                    (MemberKind::Method, MemberKind::Accessor) => Kind::MethodAccessorOverride,
                    (MemberKind::Property, MemberKind::Method) => Kind::PropertyMethodOverride,
                    (MemberKind::Accessor, MemberKind::Method) => Kind::AccessorMethodOverride,
                    _ => continue,
                };
                let names = format!("{name}\u{0}{}\u{0}{}", base_name.name, derived_name.name);
                self.push(kind, derived.span, Some(&names));
            }
        }
    }

    /// tsc's `pushTypeResolution` cycle for module-level type aliases: an
    /// alias whose body reaches itself through positions resolved eagerly
    /// (union and intersection members, `keyof`, indexed access, a conditional's
    /// check and extends types, template literal spans, another alias and its
    /// type arguments) circularly references itself (TS2456). Object members,
    /// signatures, array and tuple elements, conditional branches and the type
    /// arguments of other references are deferred. Only the aliases on the
    /// cycle are reported, not those that merely lead into one.
    fn check_circular_type_aliases(&mut self, statements: &[Statement<'_>]) {
        use oxc_ast::ast::{TSType, TSTypeAliasDeclaration, TSTypeName, TSTypeOperatorOperator};
        let aliases: Vec<&TSTypeAliasDeclaration<'_>> = statements
            .iter()
            .filter_map(|statement| {
                let declaration = match statement {
                    Statement::ExportNamedDeclaration(export) => export.declaration.as_ref(),
                    other => other.as_declaration(),
                };
                match declaration {
                    Some(Declaration::TSTypeAliasDeclaration(alias)) => Some(alias.as_ref()),
                    _ => None,
                }
            })
            .collect();
        let names: Vec<&str> = aliases.iter().map(|alias| alias.id.name.as_str()).collect();

        fn eager_references<'n>(ty: &TSType<'_>, names: &[&'n str], out: &mut Vec<&'n str>) {
            match ty {
                TSType::TSTypeReference(reference) => {
                    let TSTypeName::IdentifierReference(identifier) = &reference.type_name else {
                        return;
                    };
                    if let Some(name) = names.iter().find(|name| **name == identifier.name.as_str()) {
                        out.push(name);
                        if let Some(arguments) = &reference.type_arguments {
                            for argument in &arguments.params {
                                eager_references(argument, names, out);
                            }
                        }
                    }
                }
                TSType::TSUnionType(union) => {
                    for member in &union.types {
                        eager_references(member, names, out);
                    }
                }
                TSType::TSIntersectionType(intersection) => {
                    for member in &intersection.types {
                        eager_references(member, names, out);
                    }
                }
                TSType::TSParenthesizedType(inner) => {
                    eager_references(&inner.type_annotation, names, out);
                }
                TSType::TSTypeOperatorType(operator)
                    if operator.operator == TSTypeOperatorOperator::Keyof =>
                {
                    eager_references(&operator.type_annotation, names, out);
                }
                TSType::TSIndexedAccessType(access) => {
                    eager_references(&access.object_type, names, out);
                    eager_references(&access.index_type, names, out);
                }
                TSType::TSConditionalType(conditional) => {
                    eager_references(&conditional.check_type, names, out);
                    eager_references(&conditional.extends_type, names, out);
                }
                TSType::TSTemplateLiteralType(template) => {
                    for span in &template.types {
                        eager_references(span, names, out);
                    }
                }
                _ => {}
            }
        }

        let edges: Vec<Vec<usize>> = aliases
            .iter()
            .map(|alias| {
                let mut referenced = Vec::new();
                eager_references(&alias.type_annotation, &names, &mut referenced);
                referenced
                    .into_iter()
                    .filter_map(|name| names.iter().position(|other| *other == name))
                    .collect()
            })
            .collect();
        for (index, alias) in aliases.iter().enumerate() {
            // On a cycle exactly when the alias can reach itself.
            let mut visited = vec![false; aliases.len()];
            let mut stack = edges[index].clone();
            let mut on_cycle = false;
            while let Some(next) = stack.pop() {
                if next == index {
                    on_cycle = true;
                    break;
                }
                if std::mem::replace(&mut visited[next], true) {
                    continue;
                }
                stack.extend(edges[next].iter().copied());
            }
            if on_cycle {
                self.push(Kind::CircularTypeAlias, alias.id.span, Some(alias.id.name.as_str()));
            }
        }
    }

    fn collect_top_level_constants(&mut self, statements: &[Statement<'_>]) {
        for statement in statements {
            let declaration = match statement {
                Statement::ExportNamedDeclaration(export) => export.declaration.as_ref(),
                other => other.as_declaration(),
            };
            match declaration {
                Some(Declaration::VariableDeclaration(variable)) => {
                    for declarator in &variable.declarations {
                        let oxc_ast::ast::BindingPattern::BindingIdentifier(identifier) =
                            &declarator.id
                        else {
                            continue;
                        };
                        let value = match &declarator.init {
                            Some(init)
                                if variable.kind == VariableDeclarationKind::Const
                                    && !variable.declare =>
                            {
                                enum_constant(
                                    init,
                                    &std::collections::HashMap::new(),
                                    Some(&self.top_level_constants),
                                )
                            }
                            _ => EnumConstant::NonConstant,
                        };
                        self.top_level_constants.insert(identifier.name.to_string(), value);
                    }
                }
                Some(Declaration::FunctionDeclaration(function)) => {
                    if let Some(id) = &function.id {
                        self.top_level_constants
                            .insert(id.name.to_string(), EnumConstant::NonConstant);
                    }
                }
                Some(Declaration::ClassDeclaration(class)) => {
                    if let Some(id) = &class.id {
                        self.top_level_constants
                            .insert(id.name.to_string(), EnumConstant::NonConstant);
                    }
                }
                Some(Declaration::TSEnumDeclaration(enumeration)) => {
                    self.top_level_enums
                        .insert((enumeration.span.start, enumeration.span.end));
                }
                _ => {}
            }
        }
    }

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

    /// tsc's `checkExportDeclaration` inside a namespace body: `export … from
    /// "m"` is TS1194 on the module name in any namespace, and a local
    /// `export { … }` is TS1194 on the declaration unless the namespace is
    /// ambient.
    fn check_namespace_export_declarations(&mut self, statements: &[Statement<'_>], ambient: bool) {
        for statement in statements {
            match statement {
                Statement::ExportNamedDeclaration(export) if export.declaration.is_none() => {
                    match export.source.as_ref() {
                        Some(source) => {
                            self.push(Kind::ExportDeclarationInNamespace, source.span, None)
                        }
                        None if !ambient => {
                            self.push(Kind::ExportDeclarationInNamespace, export.span, None)
                        }
                        None => {}
                    }
                }
                Statement::ExportAllDeclaration(export) => {
                    self.push(Kind::ExportDeclarationInNamespace, export.source.span, None);
                }
                _ => {}
            }
        }
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

    /// tsc's `checkAccessorDeclaration` for a `get`/`set` pair of one name: both
    /// or neither `abstract` (TS2676), and a getter no less accessible than its
    /// setter (TS2808). Each is reported on both accessors.
    fn check_accessor_pairs(&mut self, class: &Class<'_>) {
        use oxc_ast::ast::TSAccessibility;
        let accessors: Vec<(String, bool, bool, &oxc_ast::ast::MethodDefinition<'_>)> = class
            .body
            .body
            .iter()
            .filter_map(|element| match element {
                ClassElement::MethodDefinition(method)
                    if matches!(method.kind, MethodDefinitionKind::Get | MethodDefinitionKind::Set) =>
                {
                    property_key_name(&method.key).map(|name| {
                        (name, method.r#static, method.kind == MethodDefinitionKind::Get, method.as_ref())
                    })
                }
                _ => None,
            })
            .collect();
        for (name, is_static, is_getter, getter) in &accessors {
            if !is_getter {
                continue;
            }
            let Some((_, _, _, setter)) = accessors
                .iter()
                .find(|(other, other_static, other_getter, _)| {
                    other == name && other_static == is_static && !other_getter
                })
            else {
                continue;
            };
            let abstract_of = |method: &oxc_ast::ast::MethodDefinition<'_>| {
                method.r#type == MethodDefinitionType::TSAbstractMethodDefinition
            };
            if abstract_of(getter) != abstract_of(setter) {
                self.push(Kind::AccessorAbstractMismatch, getter.key.span(), None);
                self.push(Kind::AccessorAbstractMismatch, setter.key.span(), None);
            }
            let less_accessible = match (getter.accessibility, setter.accessibility) {
                (Some(TSAccessibility::Protected), None | Some(TSAccessibility::Public)) => true,
                (Some(TSAccessibility::Private), setter) => setter != Some(TSAccessibility::Private),
                _ => false,
            };
            if less_accessible {
                self.push(Kind::GetAccessorLessAccessible, getter.key.span(), None);
                self.push(Kind::GetAccessorLessAccessible, setter.key.span(), None);
            }
        }
    }

    /// tsc's "A 'get' accessor must return a value" (TS2378): a body with no
    /// `return` at all whose end is reachable. Reachability is approximated by
    /// the body not ending in `throw`.
    fn check_getter_returns(&mut self, name_span: Span, body: Option<&FunctionBody<'_>>) {
        let Some(body) = body else {
            return;
        };
        if self.is_ambient()
            || matches!(body.statements.last(), Some(Statement::ThrowStatement(_)))
            || body_has_return(body)
        {
            return;
        }
        self.push(Kind::GetAccessorWithoutReturn, name_span, None);
    }

    /// tsc's `checkThisBeforeSuper`: `this` or `super.x` read directly in a
    /// derived constructor before `super()` has run (TS17009, TS17011). The
    /// flow is approximated by the leading statements up to the one that calls
    /// `super`, plus that call's own arguments when it is the whole statement;
    /// a reference inside a nested function runs later and is not one.
    fn report_this_before_super(&mut self, body: &FunctionBody<'_>) {
        struct EarlyReferences {
            found: Vec<(Kind, Span)>,
        }

        impl<'a> Visit<'a> for EarlyReferences {
            fn visit_this_expression(&mut self, this: &oxc_ast::ast::ThisExpression) {
                self.found.push((Kind::ThisBeforeSuperCall, this.span));
            }
            fn visit_super(&mut self, _: &oxc_ast::ast::Super) {}
            fn visit_static_member_expression(
                &mut self,
                member: &oxc_ast::ast::StaticMemberExpression<'a>,
            ) {
                if let Expression::Super(super_keyword) = &member.object {
                    self.found.push((Kind::SuperPropertyBeforeSuperCall, super_keyword.span));
                }
                oxc_ast_visit::walk::walk_static_member_expression(self, member);
            }
            fn visit_computed_member_expression(
                &mut self,
                member: &oxc_ast::ast::ComputedMemberExpression<'a>,
            ) {
                if let Expression::Super(super_keyword) = &member.object {
                    self.found.push((Kind::SuperPropertyBeforeSuperCall, super_keyword.span));
                }
                oxc_ast_visit::walk::walk_computed_member_expression(self, member);
            }
            fn visit_function(&mut self, _: &Function<'a>, _: oxc_syntax::scope::ScopeFlags) {}
            fn visit_arrow_function_expression(
                &mut self,
                _: &oxc_ast::ast::ArrowFunctionExpression<'a>,
            ) {
            }
            fn visit_class(&mut self, _: &Class<'a>) {}
        }

        let mut references = EarlyReferences { found: Vec::new() };
        for statement in &body.statements {
            if let Statement::ExpressionStatement(expression) = statement
                && let Expression::CallExpression(call) = &expression.expression
                && matches!(call.callee, Expression::Super(_))
            {
                for argument in &call.arguments {
                    references.visit_argument(argument);
                }
                break;
            }
            let mut finder = SuperCallInStatement { found: false };
            finder.visit_statement(statement);
            if finder.found {
                break;
            }
            references.visit_statement(statement);
        }
        for (kind, span) in references.found {
            self.push(kind, span, None);
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
        self.check_accessor_pairs(class);

        // A property with no annotation and no initializer has an implicit
        // `any` type *unless* the constructor assigns it: tsc infers the
        // declaration's type from that assignment.
        let (assigned_instance, assigned_static) = assigned_property_names(class);
        for element in &class.body.body {
            // An auto-accessor is declared like a property and judged like one.
            let (key, is_typed, is_static) = match element {
                ClassElement::PropertyDefinition(property) => (
                    &property.key,
                    property.type_annotation.is_some() || property.value.is_some() || property.computed,
                    property.r#static,
                ),
                ClassElement::AccessorProperty(property) => (
                    &property.key,
                    property.type_annotation.is_some() || property.value.is_some() || property.computed,
                    property.r#static,
                ),
                _ => continue,
            };
            if is_typed {
                continue;
            }
            let Some(name) = property_key_name(key) else {
                continue;
            };
            let assigned = if is_static { &assigned_static } else { &assigned_instance };
            if assigned.contains(&name) {
                continue;
            }
            self.push(Kind::ImplicitAnyMember, key.span(), Some(&name));
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
                } else {
                    self.report_this_before_super(body);
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

    /// A type-level signature has no body to infer a parameter from, so an
    /// unannotated one is `any` unless an (erroneous) initializer types it.
    fn check_implicit_any_signature_parameters(&mut self, parameters: &FormalParameters<'_>) {
        for parameter in &parameters.items {
            if parameter.type_annotation.is_some() || parameter.initializer.is_some() {
                continue;
            }
            if let oxc_ast::ast::BindingPattern::BindingIdentifier(binding) = &parameter.pattern {
                self.push(
                    Kind::ImplicitAnySignatureParameter,
                    binding.span,
                    Some(binding.name.as_str()),
                );
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
/// The members tsc infers a declared type for from assignments: `this.x = …`
/// (or `this["x"]`, `this[0]`) in the constructor for an instance member, and
/// the same inside a `static {}` block for a static one. Returned as
/// `(instance, static)`.
fn assigned_property_names(class: &Class<'_>) -> (Vec<String>, Vec<String>) {
    struct ThisAssignmentCollector {
        names: Vec<String>,
    }

    impl<'a> Visit<'a> for ThisAssignmentCollector {
        fn visit_assignment_expression(
            &mut self,
            assignment: &oxc_ast::ast::AssignmentExpression<'a>,
        ) {
            match &assignment.left {
                AssignmentTarget::StaticMemberExpression(member)
                    if matches!(member.object, Expression::ThisExpression(_)) =>
                {
                    self.names.push(member.property.name.to_string());
                }
                AssignmentTarget::ComputedMemberExpression(member)
                    if matches!(member.object, Expression::ThisExpression(_)) =>
                {
                    match &member.expression {
                        Expression::StringLiteral(literal) => {
                            self.names.push(literal.value.to_string());
                        }
                        Expression::NumericLiteral(literal) => {
                            self.names.push(literal.value.to_string());
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
            oxc_ast_visit::walk::walk_assignment_expression(self, assignment);
        }
    }

    let mut instance = ThisAssignmentCollector { names: Vec::new() };
    let mut statics = ThisAssignmentCollector { names: Vec::new() };
    for element in &class.body.body {
        match element {
            ClassElement::MethodDefinition(method)
                if method.kind == MethodDefinitionKind::Constructor =>
            {
                if let Some(body) = method.value.body.as_ref() {
                    instance.visit_function_body(body);
                }
            }
            ClassElement::StaticBlock(block) => {
                for statement in &block.body {
                    statics.visit_statement(statement);
                }
            }
            _ => {}
        }
    }
    (instance.names, statics.names)
}

struct SuperCallInStatement {
    found: bool,
}

impl<'a> Visit<'a> for SuperCallInStatement {
    fn visit_call_expression(&mut self, call: &oxc_ast::ast::CallExpression<'a>) {
        if matches!(call.callee, Expression::Super(_)) {
            self.found = true;
        }
        oxc_ast_visit::walk::walk_call_expression(self, call);
    }
}

/// Whether a function body has a `return` of its own, outside any nested
/// function.
fn body_has_return(body: &FunctionBody<'_>) -> bool {
    struct ReturnFinder {
        found: bool,
    }

    impl<'a> Visit<'a> for ReturnFinder {
        fn visit_return_statement(&mut self, _: &oxc_ast::ast::ReturnStatement<'a>) {
            self.found = true;
        }
        fn visit_function(&mut self, _: &Function<'a>, _: oxc_syntax::scope::ScopeFlags) {}
        fn visit_arrow_function_expression(&mut self, _: &oxc_ast::ast::ArrowFunctionExpression<'a>) {}
        fn visit_class(&mut self, _: &Class<'a>) {}
    }

    let mut finder = ReturnFinder { found: false };
    finder.visit_function_body(body);
    finder.found
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
        self.collect_top_level_constants(&program.body);
        self.check_member_kind_overrides(&program.body);
        self.check_circular_type_aliases(&program.body);
        self.check_top_level_enum_merges(&program.body);
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
        self.check_enum_member_values(declaration);
        if !self
            .top_level_enums
            .contains(&(declaration.span.start, declaration.span.end))
        {
            self.check_duplicate_enum_members(std::slice::from_ref(&declaration));
        }
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
            // An ambient external module (`declare module "x"`) may re-export.
            if !matches!(declaration.id, oxc_ast::ast::TSModuleDeclarationName::StringLiteral(_)) {
                self.check_namespace_export_declarations(&block.body, ambient);
            }
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
        self.function_async.push(function.r#async);
        oxc_ast_visit::walk::walk_function(self, function, flags);
        self.function_async.pop();
    }

    fn visit_arrow_function_expression(&mut self, arrow: &oxc_ast::ast::ArrowFunctionExpression<'a>) {
        if arrow.r#async {
            self.check_async_return_type(arrow.return_type.as_deref());
        }
        self.function_async.push(arrow.r#async);
        oxc_ast_visit::walk::walk_arrow_function_expression(self, arrow);
        self.function_async.pop();
    }

    /// tsc's `checkGrammarAwaitOrAwaitUsing`: `await` inside a function that
    /// is not `async` — TS1308.
    fn visit_await_expression(&mut self, expression: &oxc_ast::ast::AwaitExpression<'a>) {
        if self.function_async.last() == Some(&false) {
            let keyword = Span::new(expression.span.start, expression.span.start + 5);
            self.push(Kind::AwaitOutsideAsyncFunction, keyword, None);
        }
        oxc_ast_visit::walk::walk_await_expression(self, expression);
    }

    fn visit_ts_type_parameter_declaration(
        &mut self,
        declaration: &oxc_ast::ast::TSTypeParameterDeclaration<'a>,
    ) {
        let mut seen_default = false;
        for parameter in &declaration.params {
            if parameter.default.is_some() {
                seen_default = true;
            } else if seen_default {
                self.push(Kind::RequiredTypeParameterAfterOptional, parameter.name.span, None);
            }
        }
        oxc_ast_visit::walk::walk_ts_type_parameter_declaration(self, declaration);
    }

    fn visit_formal_parameters(&mut self, parameters: &FormalParameters<'a>) {
        self.check_parameter_list(parameters);
        oxc_ast_visit::walk::walk_formal_parameters(self, parameters);
    }

    fn visit_method_definition(&mut self, method: &oxc_ast::ast::MethodDefinition<'a>) {
        if method.kind == MethodDefinitionKind::Get {
            self.check_getter_returns(method.key.span(), method.value.body.as_deref());
        }
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

    fn visit_ts_call_signature_declaration(
        &mut self,
        signature: &oxc_ast::ast::TSCallSignatureDeclaration<'a>,
    ) {
        if signature.return_type.is_none() {
            self.push(Kind::ImplicitAnyCallReturn, signature.span, None);
        }
        self.check_signature_parameters(&signature.params, false);
        self.check_implicit_any_signature_parameters(&signature.params);
        oxc_ast_visit::walk::walk_ts_call_signature_declaration(self, signature);
    }

    fn visit_ts_construct_signature_declaration(
        &mut self,
        signature: &oxc_ast::ast::TSConstructSignatureDeclaration<'a>,
    ) {
        if signature.return_type.is_none() {
            self.push(Kind::ImplicitAnyConstructReturn, signature.span, None);
        }
        self.check_signature_parameters(&signature.params, false);
        self.check_implicit_any_signature_parameters(&signature.params);
        oxc_ast_visit::walk::walk_ts_construct_signature_declaration(self, signature);
    }

    fn visit_ts_function_type(&mut self, function: &oxc_ast::ast::TSFunctionType<'a>) {
        self.check_implicit_any_signature_parameters(&function.params);
        oxc_ast_visit::walk::walk_ts_function_type(self, function);
    }

    fn visit_ts_constructor_type(&mut self, constructor: &oxc_ast::ast::TSConstructorType<'a>) {
        self.check_implicit_any_signature_parameters(&constructor.params);
        oxc_ast_visit::walk::walk_ts_constructor_type(self, constructor);
    }

    fn visit_ts_method_signature(&mut self, method: &TSMethodSignature<'a>) {
        self.check_signature_parameters(&method.params, false);
        self.check_implicit_any_signature_parameters(&method.params);
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
        for property in &object.properties {
            if let ObjectPropertyKind::ObjectProperty(property) = property
                && property.kind == PropertyKind::Get
                && let Expression::FunctionExpression(function) = &property.value
            {
                self.check_getter_returns(property.key.span(), function.body.as_deref());
            }
        }
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

    // Only the outermost of nested parentheses is recorded, keyed by the
    // expression they wrap: that is the span the lowered tree keeps.
    fn visit_parenthesized_expression(
        &mut self,
        parenthesized: &oxc_ast::ast::ParenthesizedExpression<'a>,
    ) {
        let mut inner = &parenthesized.expression;
        while let Expression::ParenthesizedExpression(nested) = inner {
            inner = &nested.expression;
        }
        self.parenthesized_expressions.push(ParenthesizedExpressionSpan {
            inner: text_span_from_oxc_span(inner.span()),
            outer: text_span_from_oxc_span(parenthesized.span),
        });
        self.visit_expression(inner);
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
