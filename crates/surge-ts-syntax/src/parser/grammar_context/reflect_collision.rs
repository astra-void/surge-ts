//! tsc's `checkReflectCollision` (TS2818). Below ES2022 a `super.x` inside a
//! static initializer is emitted through `Reflect`, so every block scope
//! container around it reserves that name. The checker keeps the finding only
//! for `target <= ES2021` and an emitting program.

use oxc_ast::ast::{
    ArrowFunctionExpression, BindingPattern, CatchClause, Class, ComputedMemberExpression,
    ForInStatement, ForOfStatement, ForStatement, FormalParameters, Function, FunctionType,
    ImportDeclaration, ImportDeclarationSpecifier, ImportOrExportKind, Program, PropertyDefinition,
    StaticBlock, StaticMemberExpression, SwitchStatement, TSEnumDeclaration, TSImportEqualsDeclaration,
    TSModuleDeclaration, TSModuleDeclarationName, VariableDeclaration, BlockStatement, Expression,
};
use oxc_ast_visit::{Visit, walk};
use oxc_span::Span;
use oxc_syntax::scope::ScopeFlags;

use super::{ContextCollector, collect_binding_names};

const RESERVED: &str = "Reflect";

#[derive(Clone, Copy)]
enum SuperContainer {
    /// A static property initializer or static block of a class with a base.
    StaticInitializer,
    Other,
}

struct Declared {
    span: Span,
    /// The declaration collides when any of these containers is marked.
    containers: Vec<usize>,
}

struct Collector {
    ambient_depth: usize,
    marked: Vec<bool>,
    /// Open block scope containers, innermost last; `None` is a script's
    /// source file, which tsc never marks.
    containers: Vec<Option<usize>>,
    super_containers: Vec<SuperContainer>,
    class_has_base: Vec<bool>,
    declared: Vec<Declared>,
}

impl Collector {
    fn open_container(&mut self) -> usize {
        self.marked.push(false);
        let id = self.marked.len() - 1;
        self.containers.push(Some(id));
        id
    }

    fn close_container(&mut self) {
        self.containers.pop();
    }

    fn current_container(&self) -> Option<usize> {
        self.containers.last().copied().flatten()
    }

    fn declare(&mut self, name: &str, span: Span) {
        if name != RESERVED || self.ambient_depth > 0 {
            return;
        }
        if let Some(container) = self.current_container() {
            self.declared.push(Declared { span, containers: vec![container] });
        }
    }

    fn declare_pattern(&mut self, pattern: &BindingPattern<'_>) {
        let mut names = Vec::new();
        collect_binding_names(pattern, &mut names);
        for (name, span) in names {
            self.declare(name, span);
        }
    }

    fn declare_parameters(&mut self, parameters: &FormalParameters<'_>) {
        for parameter in &parameters.items {
            self.declare_pattern(&parameter.pattern);
        }
        if let Some(rest) = &parameters.rest {
            self.declare_pattern(&rest.rest.argument);
        }
    }

    fn with_ambient(&mut self, ambient: bool, walk: impl FnOnce(&mut Self)) {
        if ambient {
            self.ambient_depth += 1;
        }
        walk(self);
        if ambient {
            self.ambient_depth -= 1;
        }
    }

    fn reference_super_member(&mut self) {
        if !matches!(self.super_containers.last(), Some(SuperContainer::StaticInitializer)) {
            return;
        }
        for container in self.containers.iter().flatten() {
            self.marked[*container] = true;
        }
    }
}

impl<'a> Visit<'a> for Collector {
    fn visit_static_member_expression(&mut self, expression: &StaticMemberExpression<'a>) {
        if matches!(expression.object, Expression::Super(_)) {
            self.reference_super_member();
        }
        walk::walk_static_member_expression(self, expression);
    }

    fn visit_computed_member_expression(&mut self, expression: &ComputedMemberExpression<'a>) {
        if matches!(expression.object, Expression::Super(_)) {
            self.reference_super_member();
        }
        walk::walk_computed_member_expression(self, expression);
    }

    fn visit_variable_declaration(&mut self, declaration: &VariableDeclaration<'a>) {
        self.with_ambient(declaration.declare, |this| {
            for declarator in &declaration.declarations {
                this.declare_pattern(&declarator.id);
            }
            walk::walk_variable_declaration(this, declaration);
        });
    }

    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        let ambient = function.declare || function.r#type == FunctionType::TSDeclareFunction;
        self.with_ambient(ambient, |this| {
            let is_expression = function.r#type == FunctionType::FunctionExpression;
            if !is_expression && let Some(id) = &function.id {
                this.declare(&id.name, id.span);
            }
            let container = this.open_container();
            if is_expression
                && let Some(id) = &function.id
                && id.name == RESERVED
                && this.ambient_depth == 0
            {
                this.declared.push(Declared { span: id.span, containers: vec![container] });
            }
            if function.body.is_some() {
                this.declare_parameters(&function.params);
            }
            this.super_containers.push(SuperContainer::Other);
            walk::walk_function(this, function, flags);
            this.super_containers.pop();
            this.close_container();
        });
    }

    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        self.open_container();
        self.declare_parameters(&arrow.params);
        walk::walk_arrow_function_expression(self, arrow);
        self.close_container();
    }

    fn visit_class(&mut self, class: &Class<'a>) {
        self.with_ambient(class.declare, |this| {
            let is_declaration = class.is_declaration();
            if is_declaration && let Some(id) = &class.id {
                this.declare(&id.name, id.span);
            }
            let first_member_container = this.marked.len();
            this.class_has_base.push(class.super_class.is_some());
            walk::walk_class(this, class);
            this.class_has_base.pop();
            // A class expression's name is scoped to its own members; every
            // container its walk opened lies inside one of them.
            if !is_declaration
                && let Some(id) = &class.id
                && id.name == RESERVED
                && this.ambient_depth == 0
            {
                let members = (first_member_container..this.marked.len()).collect();
                this.declared.push(Declared { span: id.span, containers: members });
            }
        });
    }

    fn visit_property_definition(&mut self, property: &PropertyDefinition<'a>) {
        self.visit_property_key(&property.key);
        let Some(value) = &property.value else {
            return;
        };
        self.open_container();
        let has_base = self.class_has_base.last().copied().unwrap_or(false);
        self.super_containers.push(if property.r#static && has_base {
            SuperContainer::StaticInitializer
        } else {
            SuperContainer::Other
        });
        self.visit_expression(value);
        self.super_containers.pop();
        self.close_container();
    }

    fn visit_static_block(&mut self, block: &StaticBlock<'a>) {
        self.open_container();
        let has_base = self.class_has_base.last().copied().unwrap_or(false);
        self.super_containers.push(if has_base {
            SuperContainer::StaticInitializer
        } else {
            SuperContainer::Other
        });
        walk::walk_static_block(self, block);
        self.super_containers.pop();
        self.close_container();
    }

    fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
        self.open_container();
        walk::walk_block_statement(self, block);
        self.close_container();
    }

    fn visit_switch_statement(&mut self, statement: &SwitchStatement<'a>) {
        self.visit_expression(&statement.discriminant);
        self.open_container();
        for case in &statement.cases {
            self.visit_switch_case(case);
        }
        self.close_container();
    }

    fn visit_catch_clause(&mut self, clause: &CatchClause<'a>) {
        self.open_container();
        walk::walk_catch_clause(self, clause);
        self.close_container();
    }

    fn visit_for_statement(&mut self, statement: &ForStatement<'a>) {
        self.open_container();
        walk::walk_for_statement(self, statement);
        self.close_container();
    }

    fn visit_for_in_statement(&mut self, statement: &ForInStatement<'a>) {
        self.open_container();
        walk::walk_for_in_statement(self, statement);
        self.close_container();
    }

    fn visit_for_of_statement(&mut self, statement: &ForOfStatement<'a>) {
        self.open_container();
        walk::walk_for_of_statement(self, statement);
        self.close_container();
    }

    fn visit_ts_module_declaration(&mut self, declaration: &TSModuleDeclaration<'a>) {
        self.with_ambient(declaration.declare, |this| {
            if let TSModuleDeclarationName::Identifier(id) = &declaration.id {
                this.declare(&id.name, id.span);
            }
            this.open_container();
            walk::walk_ts_module_declaration(this, declaration);
            this.close_container();
        });
    }

    fn visit_ts_global_declaration(&mut self, declaration: &oxc_ast::ast::TSGlobalDeclaration<'a>) {
        self.with_ambient(true, |this| walk::walk_ts_global_declaration(this, declaration));
    }

    fn visit_ts_enum_declaration(&mut self, declaration: &TSEnumDeclaration<'a>) {
        self.with_ambient(declaration.declare, |this| {
            this.declare(&declaration.id.name, declaration.id.span);
            walk::walk_ts_enum_declaration(this, declaration);
        });
    }

    fn visit_import_declaration(&mut self, declaration: &ImportDeclaration<'a>) {
        if declaration.import_kind == ImportOrExportKind::Type {
            return;
        }
        for specifier in declaration.specifiers.iter().flatten() {
            let local = match specifier {
                ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                    if specifier.import_kind == ImportOrExportKind::Type {
                        continue;
                    }
                    &specifier.local
                }
                ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => &specifier.local,
                ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => &specifier.local,
            };
            self.declare(&local.name, local.span);
        }
    }

    fn visit_ts_import_equals_declaration(&mut self, declaration: &TSImportEqualsDeclaration<'a>) {
        if declaration.import_kind != ImportOrExportKind::Type {
            self.declare(&declaration.id.name, declaration.id.span);
        }
    }
}

impl ContextCollector<'_, '_> {
    pub(super) fn check_reflect_collisions(&mut self, program: &Program<'_>) {
        let mut collector = Collector {
            ambient_depth: usize::from(program.source_type.is_typescript_definition()),
            marked: Vec::new(),
            containers: Vec::new(),
            super_containers: Vec::new(),
            class_has_base: Vec::new(),
            declared: Vec::new(),
        };
        if self.external_module {
            collector.open_container();
        } else {
            collector.containers.push(None);
        }
        walk::walk_program(&mut collector, program);
        for declared in &collector.declared {
            if declared.containers.iter().any(|container| collector.marked[*container]) {
                self.push(2818, declared.span, &[RESERVED, RESERVED]);
            }
        }
    }
}
