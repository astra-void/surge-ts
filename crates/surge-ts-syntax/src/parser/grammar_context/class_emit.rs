//! Name rules that follow from how a declaration is emitted: tsc's
//! `checkCollisionsForDeclarationName` family (TS2441, TS2725, TS1216), the
//! built-in global conflicts of a script (TS2397), the constructor-scope rule
//! for property initializers (TS2301), and the private-name write rules
//! (TS2803, TS2806). The findings that depend on `module`, `target`,
//! `useDefineForClassFields` or `noEmit` are gated by the checker.

use oxc_ast::AstKind;
use oxc_ast::ast::{
    AssignmentOperator, AssignmentTarget, BindingPattern, Class, ClassElement, Declaration,
    Expression, ForStatementLeft, ImportDeclarationSpecifier, MethodDefinitionKind,
    PrivateFieldExpression, Program, PropertyDefinition, PropertyKey, SimpleAssignmentTarget,
    Statement, TSModuleDeclarationBody, TSModuleDeclarationName, VariableDeclaration,
};
use oxc_ast_visit::Visit;
use oxc_span::{GetSpan, Span};

use super::{ContextCollector, ThisContainer, collect_binding_names};

#[derive(Clone, Copy, PartialEq, Eq)]
enum DeclarationKind {
    /// `var`, `let`, `const` bindings, including destructured names.
    Variable,
    Function,
    Class,
    Enum,
    /// A namespace whose body needs code (tsc's `ModuleInstanceStateInstantiated`).
    Namespace,
    /// A namespace with only types in it.
    TypeOnlyNamespace,
    Interface,
    TypeAlias,
    /// A value import binding.
    Import,
}

impl DeclarationKind {
    /// tsc's `isTypeDeclaration`.
    fn is_type_declaration(self) -> bool {
        matches!(
            self,
            Self::Class | Self::Enum | Self::Interface | Self::TypeAlias
        )
    }

    /// Whether `checkCollisionsForDeclarationName` runs for the declaration.
    fn is_collision_checked(self) -> bool {
        matches!(
            self,
            Self::Variable | Self::Function | Self::Class | Self::Enum | Self::Namespace | Self::Import
        )
    }
}

struct TopLevelName<'s> {
    name: &'s str,
    span: Span,
    kind: DeclarationKind,
    ambient: bool,
}

/// tsc's `getModuleInstanceState`, reduced to whether anything in the body
/// needs code.
fn namespace_is_instantiated(statements: &[Statement<'_>]) -> bool {
    statements.iter().any(|statement| match statement {
        Statement::TSInterfaceDeclaration(_) | Statement::TSTypeAliasDeclaration(_) => false,
        Statement::TSEnumDeclaration(declaration) => !declaration.r#const,
        Statement::TSModuleDeclaration(declaration) => module_body_is_instantiated(declaration),
        Statement::ImportDeclaration(declaration) => !declaration.import_kind.is_type(),
        Statement::TSImportEqualsDeclaration(declaration) => !declaration.import_kind.is_type(),
        Statement::ExportNamedDeclaration(export) => match &export.declaration {
            Some(Declaration::TSInterfaceDeclaration(_) | Declaration::TSTypeAliasDeclaration(_)) => false,
            Some(Declaration::TSEnumDeclaration(declaration)) => !declaration.r#const,
            Some(Declaration::TSModuleDeclaration(declaration)) => module_body_is_instantiated(declaration),
            Some(Declaration::TSImportEqualsDeclaration(declaration)) => !declaration.import_kind.is_type(),
            Some(_) => true,
            None => !export.export_kind.is_type(),
        },
        _ => true,
    })
}

fn module_body_is_instantiated(declaration: &oxc_ast::ast::TSModuleDeclaration<'_>) -> bool {
    match &declaration.body {
        Some(TSModuleDeclarationBody::TSModuleBlock(block)) => namespace_is_instantiated(&block.body),
        Some(TSModuleDeclarationBody::TSModuleDeclaration(inner)) => module_body_is_instantiated(inner),
        None => false,
    }
}

fn top_level_names<'s>(statements: &'s [Statement<'_>], out: &mut Vec<TopLevelName<'s>>) {
    for statement in statements {
        match statement {
            Statement::VariableDeclaration(declaration) => variable_names(declaration, out),
            Statement::FunctionDeclaration(function) => {
                if let Some(id) = &function.id {
                    out.push(TopLevelName {
                        name: id.name.as_str(),
                        span: id.span,
                        kind: DeclarationKind::Function,
                        ambient: function.declare,
                    });
                }
            }
            Statement::ClassDeclaration(class) => {
                if let Some(id) = &class.id {
                    out.push(TopLevelName {
                        name: id.name.as_str(),
                        span: id.span,
                        kind: DeclarationKind::Class,
                        ambient: class.declare,
                    });
                }
            }
            Statement::TSEnumDeclaration(declaration) => out.push(TopLevelName {
                name: declaration.id.name.as_str(),
                span: declaration.id.span,
                kind: DeclarationKind::Enum,
                ambient: declaration.declare,
            }),
            Statement::TSInterfaceDeclaration(declaration) => out.push(TopLevelName {
                name: declaration.id.name.as_str(),
                span: declaration.id.span,
                kind: DeclarationKind::Interface,
                ambient: declaration.declare,
            }),
            Statement::TSTypeAliasDeclaration(declaration) => out.push(TopLevelName {
                name: declaration.id.name.as_str(),
                span: declaration.id.span,
                kind: DeclarationKind::TypeAlias,
                ambient: declaration.declare,
            }),
            Statement::TSModuleDeclaration(declaration) => {
                if let TSModuleDeclarationName::Identifier(id) = &declaration.id {
                    let instantiated = module_body_is_instantiated(declaration);
                    out.push(TopLevelName {
                        name: id.name.as_str(),
                        span: id.span,
                        kind: if instantiated {
                            DeclarationKind::Namespace
                        } else {
                            DeclarationKind::TypeOnlyNamespace
                        },
                        ambient: declaration.declare,
                    });
                }
            }
            Statement::ImportDeclaration(declaration) => {
                if declaration.import_kind.is_type() {
                    continue;
                }
                for specifier in declaration.specifiers.iter().flatten() {
                    let (id, type_only) = match specifier {
                        ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                            (&specifier.local, specifier.import_kind.is_type())
                        }
                        ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                            (&specifier.local, false)
                        }
                        ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                            (&specifier.local, false)
                        }
                    };
                    if !type_only {
                        out.push(TopLevelName {
                            name: id.name.as_str(),
                            span: id.span,
                            kind: DeclarationKind::Import,
                            ambient: false,
                        });
                    }
                }
            }
            Statement::ExportNamedDeclaration(export) => {
                if let Some(declaration) = &export.declaration {
                    exported_declaration_names(declaration, out);
                }
            }
            Statement::ExportDefaultDeclaration(export) => {
                use oxc_ast::ast::ExportDefaultDeclarationKind;
                match &export.declaration {
                    ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
                        if let Some(id) = &function.id {
                            out.push(TopLevelName {
                                name: id.name.as_str(),
                                span: id.span,
                                kind: DeclarationKind::Function,
                                ambient: function.declare,
                            });
                        }
                    }
                    ExportDefaultDeclarationKind::ClassDeclaration(class) => {
                        if let Some(id) = &class.id {
                            out.push(TopLevelName {
                                name: id.name.as_str(),
                                span: id.span,
                                kind: DeclarationKind::Class,
                                ambient: class.declare,
                            });
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

fn exported_declaration_names<'s>(declaration: &'s Declaration<'_>, out: &mut Vec<TopLevelName<'s>>) {
    match declaration {
        Declaration::VariableDeclaration(declaration) => variable_names(declaration, out),
        Declaration::FunctionDeclaration(function) => {
            if let Some(id) = &function.id {
                out.push(TopLevelName {
                    name: id.name.as_str(),
                    span: id.span,
                    kind: DeclarationKind::Function,
                    ambient: function.declare,
                });
            }
        }
        Declaration::ClassDeclaration(class) => {
            if let Some(id) = &class.id {
                out.push(TopLevelName {
                    name: id.name.as_str(),
                    span: id.span,
                    kind: DeclarationKind::Class,
                    ambient: class.declare,
                });
            }
        }
        Declaration::TSEnumDeclaration(declaration) => out.push(TopLevelName {
            name: declaration.id.name.as_str(),
            span: declaration.id.span,
            kind: DeclarationKind::Enum,
            ambient: declaration.declare,
        }),
        Declaration::TSInterfaceDeclaration(declaration) => out.push(TopLevelName {
            name: declaration.id.name.as_str(),
            span: declaration.id.span,
            kind: DeclarationKind::Interface,
            ambient: declaration.declare,
        }),
        Declaration::TSTypeAliasDeclaration(declaration) => out.push(TopLevelName {
            name: declaration.id.name.as_str(),
            span: declaration.id.span,
            kind: DeclarationKind::TypeAlias,
            ambient: declaration.declare,
        }),
        Declaration::TSModuleDeclaration(declaration) => {
            if let TSModuleDeclarationName::Identifier(id) = &declaration.id {
                let instantiated = module_body_is_instantiated(declaration);
                out.push(TopLevelName {
                    name: id.name.as_str(),
                    span: id.span,
                    kind: if instantiated {
                        DeclarationKind::Namespace
                    } else {
                        DeclarationKind::TypeOnlyNamespace
                    },
                    ambient: declaration.declare,
                });
            }
        }
        Declaration::TSImportEqualsDeclaration(_) | Declaration::TSGlobalDeclaration(_) => {}
    }
}

fn variable_names<'s>(declaration: &'s VariableDeclaration<'_>, out: &mut Vec<TopLevelName<'s>>) {
    for declarator in &declaration.declarations {
        let mut names = Vec::new();
        collect_binding_names(&declarator.id, &mut names);
        out.extend(names.into_iter().map(|(name, span)| TopLevelName {
            name,
            span,
            kind: DeclarationKind::Variable,
            ambient: declaration.declare,
        }));
    }
}

/// tsc's `checkGrammarForEsModuleMarkerInBindingName`: the name itself, or
/// the first element of a pattern, recursively.
fn es_module_marker_span(pattern: &BindingPattern<'_>) -> Option<Span> {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => {
            (identifier.name == "__esModule").then_some(identifier.span)
        }
        BindingPattern::ObjectPattern(object) => match object.properties.first() {
            Some(property) => es_module_marker_span(&property.value),
            None => object.rest.as_ref().and_then(|rest| es_module_marker_span(&rest.argument)),
        },
        BindingPattern::ArrayPattern(array) => match array.elements.iter().flatten().next() {
            Some(element) => es_module_marker_span(element),
            None => array.rest.as_ref().and_then(|rest| es_module_marker_span(&rest.argument)),
        },
        BindingPattern::AssignmentPattern(assignment) => es_module_marker_span(&assignment.left),
    }
}

/// What a class body declares under a private name.
#[derive(Clone, Copy)]
enum PrivateMember {
    Method,
    /// An accessor pair; `has_getter` is false for a set-only one.
    Accessor { has_getter: bool, is_static: bool },
    Other,
}

fn private_member_of(class: &Class<'_>, name: &str) -> Option<PrivateMember> {
    let mut found = None;
    let mut has_getter = false;
    for element in &class.body.body {
        match element {
            ClassElement::MethodDefinition(method)
                if matches!(&method.key, PropertyKey::PrivateIdentifier(key) if key.name.as_str() == name) =>
            {
                match method.kind {
                    MethodDefinitionKind::Method => found = Some(PrivateMember::Method),
                    MethodDefinitionKind::Get => {
                        has_getter = true;
                        found = Some(PrivateMember::Accessor { has_getter: true, is_static: method.r#static });
                    }
                    MethodDefinitionKind::Set => {
                        found = Some(PrivateMember::Accessor { has_getter, is_static: method.r#static });
                    }
                    MethodDefinitionKind::Constructor => {}
                }
            }
            ClassElement::PropertyDefinition(property)
                if matches!(&property.key, PropertyKey::PrivateIdentifier(key) if key.name.as_str() == name) =>
            {
                found = Some(PrivateMember::Other);
            }
            ClassElement::AccessorProperty(accessor)
                if matches!(&accessor.key, PropertyKey::PrivateIdentifier(key) if key.name.as_str() == name) =>
            {
                found = Some(PrivateMember::Other);
            }
            _ => {}
        }
    }
    match found {
        Some(PrivateMember::Accessor { is_static, .. }) => {
            Some(PrivateMember::Accessor { has_getter, is_static })
        }
        other => other,
    }
}

/// The private-name targets of an assignment, skipping the expressions a
/// pattern carries that are not targets (defaults, computed keys, and the
/// object a member target is read off).
#[derive(Default)]
struct PrivateFieldTargets {
    found: Vec<(String, Span)>,
}

impl<'a> Visit<'a> for PrivateFieldTargets {
    fn visit_expression(&mut self, _: &Expression<'a>) {}
    fn visit_private_field_expression(&mut self, access: &PrivateFieldExpression<'a>) {
        self.found.push((access.field.name.to_string(), access.field.span));
    }
}

/// The constructor's locals a property initializer may not read: parameters,
/// hoisted `var`s, and the body's own block-scoped declarations.
fn constructor_locals(class: &Class<'_>) -> Vec<String> {
    let Some(constructor) = class.body.body.iter().find_map(|element| match element {
        ClassElement::MethodDefinition(method)
            if method.kind == MethodDefinitionKind::Constructor && method.value.body.is_some() =>
        {
            Some(&method.value)
        }
        _ => None,
    }) else {
        return Vec::new();
    };
    let mut locals = Vec::new();
    for parameter in &constructor.params.items {
        let mut names = Vec::new();
        collect_binding_names(&parameter.pattern, &mut names);
        locals.extend(names.into_iter().map(|(name, _)| name.to_string()));
    }
    if let Some(rest) = &constructor.params.rest {
        let mut names = Vec::new();
        collect_binding_names(&rest.rest.argument, &mut names);
        locals.extend(names.into_iter().map(|(name, _)| name.to_string()));
    }
    if let Some(body) = &constructor.body {
        body_scope_names(&body.statements, true, &mut locals);
    }
    locals
}

/// The names a function body or block binds: its direct `let`/`const`/class/
/// function declarations, plus every hoisted `var` when `hoist_var`.
fn body_scope_names(statements: &[Statement<'_>], hoist_var: bool, out: &mut Vec<String>) {
    struct HoistedVars<'o> {
        out: &'o mut Vec<String>,
    }
    impl<'a> Visit<'a> for HoistedVars<'_> {
        fn visit_variable_declaration(&mut self, declaration: &VariableDeclaration<'a>) {
            if declaration.kind.is_var() {
                for declarator in &declaration.declarations {
                    let mut names = Vec::new();
                    collect_binding_names(&declarator.id, &mut names);
                    self.out.extend(names.into_iter().map(|(name, _)| name.to_string()));
                }
            }
        }
        fn visit_function(&mut self, _: &oxc_ast::ast::Function<'a>, _: oxc_syntax::scope::ScopeFlags) {}
        fn visit_arrow_function_expression(&mut self, _: &oxc_ast::ast::ArrowFunctionExpression<'a>) {}
        fn visit_class(&mut self, _: &Class<'a>) {}
    }

    for statement in statements {
        match statement {
            Statement::VariableDeclaration(declaration) if !declaration.kind.is_var() => {
                for declarator in &declaration.declarations {
                    let mut names = Vec::new();
                    collect_binding_names(&declarator.id, &mut names);
                    out.extend(names.into_iter().map(|(name, _)| name.to_string()));
                }
            }
            Statement::FunctionDeclaration(function) => {
                if let Some(id) = &function.id {
                    out.push(id.name.to_string());
                }
            }
            Statement::ClassDeclaration(class) => {
                if let Some(id) = &class.id {
                    out.push(id.name.to_string());
                }
            }
            _ => {}
        }
    }
    if hoist_var {
        let mut hoisted = HoistedVars { out };
        for statement in statements {
            hoisted.visit_statement(statement);
        }
    }
}

/// The reads of a constructor local from a property initializer, walking the
/// scopes between the two the way the binder's name resolution does: a nested
/// function, block, catch clause or loop head that rebinds the name hides the
/// constructor's.
struct ConstructorLocalReads<'l> {
    locals: &'l [String],
    scopes: Vec<Vec<String>>,
    found: Vec<(String, Span)>,
}

impl ConstructorLocalReads<'_> {
    fn with_scope(&mut self, names: Vec<String>, walk: impl FnOnce(&mut Self)) {
        self.scopes.push(names);
        walk(self);
        self.scopes.pop();
    }
}

impl<'a> Visit<'a> for ConstructorLocalReads<'_> {
    fn visit_identifier_reference(&mut self, identifier: &oxc_ast::ast::IdentifierReference<'a>) {
        let name = identifier.name.as_str();
        if self.locals.iter().any(|local| local == name)
            && !self.scopes.iter().any(|scope| scope.iter().any(|bound| bound == name))
        {
            self.found.push((name.to_string(), identifier.span));
        }
    }
    fn visit_function(&mut self, function: &oxc_ast::ast::Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        let mut names = Vec::new();
        if let Some(id) = &function.id {
            names.push(id.name.to_string());
        }
        for parameter in &function.params.items {
            let mut bound = Vec::new();
            collect_binding_names(&parameter.pattern, &mut bound);
            names.extend(bound.into_iter().map(|(name, _)| name.to_string()));
        }
        if let Some(rest) = &function.params.rest {
            let mut bound = Vec::new();
            collect_binding_names(&rest.rest.argument, &mut bound);
            names.extend(bound.into_iter().map(|(name, _)| name.to_string()));
        }
        if let Some(body) = &function.body {
            body_scope_names(&body.statements, true, &mut names);
        }
        self.with_scope(names, |visitor| oxc_ast_visit::walk::walk_function(visitor, function, flags));
    }
    fn visit_arrow_function_expression(&mut self, arrow: &oxc_ast::ast::ArrowFunctionExpression<'a>) {
        let mut names = Vec::new();
        for parameter in &arrow.params.items {
            let mut bound = Vec::new();
            collect_binding_names(&parameter.pattern, &mut bound);
            names.extend(bound.into_iter().map(|(name, _)| name.to_string()));
        }
        if let Some(rest) = &arrow.params.rest {
            let mut bound = Vec::new();
            collect_binding_names(&rest.rest.argument, &mut bound);
            names.extend(bound.into_iter().map(|(name, _)| name.to_string()));
        }
        body_scope_names(&arrow.body.statements, true, &mut names);
        self.with_scope(names, |visitor| oxc_ast_visit::walk::walk_arrow_function_expression(visitor, arrow));
    }
    fn visit_block_statement(&mut self, block: &oxc_ast::ast::BlockStatement<'a>) {
        let mut names = Vec::new();
        body_scope_names(&block.body, false, &mut names);
        self.with_scope(names, |visitor| oxc_ast_visit::walk::walk_block_statement(visitor, block));
    }
    fn visit_catch_clause(&mut self, clause: &oxc_ast::ast::CatchClause<'a>) {
        let mut names = Vec::new();
        if let Some(parameter) = &clause.param {
            let mut bound = Vec::new();
            collect_binding_names(&parameter.pattern, &mut bound);
            names.extend(bound.into_iter().map(|(name, _)| name.to_string()));
        }
        self.with_scope(names, |visitor| oxc_ast_visit::walk::walk_catch_clause(visitor, clause));
    }
    fn visit_for_statement(&mut self, statement: &oxc_ast::ast::ForStatement<'a>) {
        let mut names = Vec::new();
        if let Some(oxc_ast::ast::ForStatementInit::VariableDeclaration(declaration)) = &statement.init
            && !declaration.kind.is_var()
        {
            for declarator in &declaration.declarations {
                let mut bound = Vec::new();
                collect_binding_names(&declarator.id, &mut bound);
                names.extend(bound.into_iter().map(|(name, _)| name.to_string()));
            }
        }
        self.with_scope(names, |visitor| oxc_ast_visit::walk::walk_for_statement(visitor, statement));
    }
    fn visit_for_in_statement(&mut self, statement: &oxc_ast::ast::ForInStatement<'a>) {
        let names = for_left_names(&statement.left);
        self.with_scope(names, |visitor| oxc_ast_visit::walk::walk_for_in_statement(visitor, statement));
    }
    fn visit_for_of_statement(&mut self, statement: &oxc_ast::ast::ForOfStatement<'a>) {
        let names = for_left_names(&statement.left);
        self.with_scope(names, |visitor| oxc_ast_visit::walk::walk_for_of_statement(visitor, statement));
    }
    fn visit_class(&mut self, _: &Class<'a>) {}
    fn visit_ts_type_annotation(&mut self, _: &oxc_ast::ast::TSTypeAnnotation<'a>) {}
    fn visit_ts_type(&mut self, _: &oxc_ast::ast::TSType<'a>) {}
}

fn for_left_names(left: &ForStatementLeft<'_>) -> Vec<String> {
    let mut names = Vec::new();
    if let ForStatementLeft::VariableDeclaration(declaration) = left
        && !declaration.kind.is_var()
    {
        for declarator in &declaration.declarations {
            let mut bound = Vec::new();
            collect_binding_names(&declarator.id, &mut bound);
            names.extend(bound.into_iter().map(|(name, _)| name.to_string()));
        }
    }
    names
}

impl<'a> ContextCollector<'a, '_> {
    /// tsc's `checkClassNameCollisionWithObject`: a class named `Object` in a
    /// file emitted as CommonJS. The checker supplies the module kind.
    pub(super) fn check_object_class_name(&mut self, class: &Class<'_>) {
        if let Some(id) = &class.id
            && id.name == "Object"
            && !class.declare
            && self.ambient_depth == 0
        {
            self.push(2725, id.span, &[]);
        }
    }

    /// The names a source file's top-level declarations bind. In a module,
    /// `require`, `exports` and a non-class `Object` collide with the CommonJS
    /// wrapper (TS2441, gated on the file's emit format). In a script they
    /// join the globals, where `undefined` and `globalThis` are already taken
    /// (TS2397).
    pub(super) fn check_top_level_names(&mut self, program: &Program<'_>) {
        let mut names = Vec::new();
        top_level_names(&program.body, &mut names);
        if self.external_module {
            for declared in names {
                if declared.ambient || !declared.kind.is_collision_checked() {
                    continue;
                }
                let reserved = match declared.name {
                    "require" | "exports" => true,
                    "Object" => declared.kind != DeclarationKind::Class,
                    _ => false,
                };
                if reserved {
                    self.push(2441, declared.span, &[declared.name, declared.name]);
                }
            }
        } else {
            self.check_global_declaration_names(&names, true);
        }
    }

    /// `declare global` contributes to the globals from inside a module, where
    /// `undefined` is already declared.
    pub(super) fn check_global_augmentation_names(&mut self, statements: &[Statement<'_>]) {
        let mut names = Vec::new();
        top_level_names(statements, &mut names);
        self.check_global_declaration_names(&names, false);
    }

    /// tsc's `addUndefinedToGlobalsOrErrorOnRedeclaration` (any non-type
    /// declaration of `undefined`) and, for a script's own locals, the
    /// `globalThis` conflict `initializeChecker` reports on every declaration.
    fn check_global_declaration_names(&mut self, names: &[TopLevelName<'_>], script_locals: bool) {
        for declared in names {
            let conflicts = match declared.name {
                "undefined" => !declared.kind.is_type_declaration(),
                "globalThis" => script_locals,
                _ => false,
            };
            if conflicts {
                self.push(2397, declared.span, &[declared.name]);
            }
        }
    }

    /// tsc's `checkTypeNameIsReserved` for `import x = N.T`: a predefined
    /// type keyword may not alias a type (TS2438). Whether the target has a
    /// type meaning is resolved by the checker, which receives the entity
    /// name as the second argument.
    pub(super) fn check_import_alias_name(&mut self, declaration: &oxc_ast::ast::TSImportEqualsDeclaration<'_>) {
        if !matches!(
            declaration.id.name.as_str(),
            "any" | "unknown" | "never" | "number" | "bigint" | "boolean" | "string" | "symbol" | "void"
                | "object" | "undefined"
        ) {
            return;
        }
        let reference = match &declaration.module_reference {
            oxc_ast::ast::TSModuleReference::ExternalModuleReference(_) => return,
            reference => reference.span(),
        };
        let entity = &self.source_text[reference.start as usize..reference.end as usize];
        self.push(2438, declaration.id.span, &[declaration.id.name.as_str(), entity]);
    }

    /// tsc's `checkGrammarForEsModuleMarkerInBindingName`: an exported
    /// variable named `__esModule` in a file emitted as CommonJS (TS1216,
    /// gated on the emit format).
    pub(super) fn check_es_module_marker(&mut self, declaration: &VariableDeclaration<'_>) {
        if declaration.declare
            || self.ambient_depth != 0
            || !matches!(self.stack.last(), Some(AstKind::ExportNamedDeclaration(_)))
        {
            return;
        }
        for declarator in &declaration.declarations {
            if let Some(span) = es_module_marker_span(&declarator.id) {
                self.push(1216, span, &[]);
            }
        }
    }

    /// tsc's `lookupSymbolForPrivateIdentifierDeclaration`: the innermost
    /// enclosing class that declares the private name.
    fn lexically_scoped_private_member(&self, name: &str) -> Option<(&'a Class<'a>, PrivateMember)> {
        self.stack.iter().rev().find_map(|kind| match kind {
            AstKind::Class(class) => private_member_of(class, name).map(|member| (*class, member)),
            _ => None,
        })
    }

    /// TS2803: a private method is not writable, whatever the receiver.
    fn report_private_method_writes(&mut self, targets: PrivateFieldTargets) {
        for (name, span) in targets.found {
            if let Some((_, PrivateMember::Method)) = self.lexically_scoped_private_member(&name) {
                self.push(2803, span, &[&format!("#{name}")]);
            }
        }
    }

    pub(super) fn check_private_method_assignment(&mut self, target: &AssignmentTarget<'a>) {
        let mut targets = PrivateFieldTargets::default();
        targets.visit_assignment_target(target);
        self.report_private_method_writes(targets);
    }

    pub(super) fn check_private_method_update(&mut self, target: &SimpleAssignmentTarget<'a>) {
        let mut targets = PrivateFieldTargets::default();
        targets.visit_simple_assignment_target(target);
        self.report_private_method_writes(targets);
    }

    pub(super) fn check_private_method_for_target(&mut self, left: &ForStatementLeft<'a>) {
        let mut targets = PrivateFieldTargets::default();
        targets.visit_for_statement_left(left);
        self.report_private_method_writes(targets);
    }

    /// TS2806: reading `this.#x` through a set-only private accessor. Only a
    /// `this` receiver is decided here — its type is the enclosing class's —
    /// and a plain `=` to it is the one write that needs no getter.
    pub(super) fn check_private_setter_read(&mut self, access: &PrivateFieldExpression<'a>) {
        if !matches!(access.object, Expression::ThisExpression(_)) {
            return;
        }
        let definite = match self.stack.last() {
            Some(AstKind::AssignmentExpression(assignment)) => {
                assignment.operator == AssignmentOperator::Assign
                    && matches!(&assignment.left, AssignmentTarget::PrivateFieldExpression(left) if left.span == access.span)
            }
            Some(
                AstKind::ArrayAssignmentTarget(_)
                | AstKind::ObjectAssignmentTarget(_)
                | AstKind::AssignmentTargetWithDefault(_)
                | AstKind::AssignmentTargetPropertyProperty(_),
            ) => true,
            _ => false,
        };
        if definite {
            return;
        }
        let Some((declaring, PrivateMember::Accessor { has_getter: false, is_static })) =
            self.lexically_scoped_private_member(&access.field.name)
        else {
            return;
        };
        let container_is_static = match self.this_container(access.span) {
            ThisContainer::ClassMethod(method) => method.r#static,
            ThisContainer::ClassProperty { is_static } => is_static,
            _ => return,
        };
        // `this` belongs to the innermost class; an outer class's accessor is
        // not on its type.
        let innermost = self.stack.iter().rev().find_map(|kind| match kind {
            AstKind::Class(class) => Some(*class),
            _ => None,
        });
        if innermost.is_some_and(|class| std::ptr::eq(class, declaring)) && container_is_static == is_static {
            self.push(2806, access.span, &[]);
        }
    }

    /// TS2301: an instance property initializer that reads a name the
    /// constructor binds. The emitted initializer runs inside the constructor,
    /// where that local would shadow what the source meant. Gated by the
    /// checker on `GetEmitStandardClassFields`.
    pub(super) fn check_property_initializer_constructor_locals(&mut self, property: &PropertyDefinition<'a>) {
        if property.r#static {
            return;
        }
        let Some(value) = &property.value else {
            return;
        };
        let Some(AstKind::Class(class)) = self.stack.iter().rev().nth(1) else {
            return;
        };
        let locals = constructor_locals(class);
        if locals.is_empty() {
            return;
        }
        let mut reads = ConstructorLocalReads {
            locals: &locals,
            scopes: Vec::new(),
            found: Vec::new(),
        };
        reads.visit_expression(value);
        if reads.found.is_empty() {
            return;
        }
        let property_name = match &property.key {
            PropertyKey::PrivateIdentifier(key) => format!("#{}", key.name),
            key if property.computed => {
                let span = key.span();
                format!("[{}]", &self.source_text[span.start as usize..span.end as usize])
            }
            key => key
                .static_name()
                .map(|name| name.into_owned())
                .unwrap_or_default(),
        };
        for (name, span) in reads.found {
            self.push(2301, span, &[&property_name, &name]);
        }
    }
}
