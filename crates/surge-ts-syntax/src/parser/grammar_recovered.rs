//! Grammar checks for constructs the vendored parser now keeps where upstream
//! oxc stopped parsing the file (see `crates/surge-ts-parser/VENDORED.md`).

use oxc_ast::ast::{
    Class, Function, FunctionType, MethodDefinition, Program, TSGlobalDeclaration,
    TSModuleDeclaration,
};
use oxc_ast_visit::Visit;
use oxc_span::{GetSpan, Span};
use oxc_syntax::scope::ScopeFlags;

use super::spans::text_span_from_oxc_span;
use crate::{ParsedGrammarDiagnostic, ParsedGrammarDiagnosticKind as Kind};

/// `ambient`: the whole file is (a declaration file).
pub(crate) fn collect_recovered_grammar_diagnostics(
    program: &Program<'_>,
    ambient: bool,
    out: &mut Vec<ParsedGrammarDiagnostic>,
) {
    let mut collector = RecoveredCollector { source_text: program.source_text, ambient_depth: usize::from(ambient), out };
    collector.visit_program(program);
}

struct RecoveredCollector<'s, 'o> {
    source_text: &'s str,
    ambient_depth: usize,
    out: &'o mut Vec<ParsedGrammarDiagnostic>,
}

impl RecoveredCollector<'_, '_> {
    /// tsc's `checkGrammarForGenerator`: TS1221 at the `*` of a generator
    /// declared in an ambient context. oxc keeps no span for the `*`, so it is
    /// the last one written before the name.
    fn check_ambient_generator(&mut self, generator: bool, declare: bool, before: Span) {
        if !generator || !(declare || self.ambient_depth > 0) {
            return;
        }
        let Some(head) = self.source_text.get(before.start as usize..before.end as usize) else {
            return;
        };
        if let Some(offset) = head.rfind('*') {
            let start = before.start as usize + offset;
            self.out.push(ParsedGrammarDiagnostic {
                kind: Kind::Ts(1221),
                span: text_span_from_oxc_span(Span::new(start as u32, start as u32 + 1)),
                name: None,
            });
        }
    }

    fn walk_ambient(&mut self, ambient: bool, walk: impl FnOnce(&mut Self)) {
        if ambient {
            self.ambient_depth += 1;
        }
        walk(self);
        if ambient {
            self.ambient_depth -= 1;
        }
    }
}

impl<'a> Visit<'a> for RecoveredCollector<'_, '_> {
    fn visit_ts_module_declaration(&mut self, declaration: &TSModuleDeclaration<'a>) {
        self.walk_ambient(declaration.declare, |this| {
            oxc_ast_visit::walk::walk_ts_module_declaration(this, declaration);
        });
    }

    fn visit_ts_global_declaration(&mut self, declaration: &TSGlobalDeclaration<'a>) {
        self.walk_ambient(declaration.declare, |this| {
            oxc_ast_visit::walk::walk_ts_global_declaration(this, declaration);
        });
    }

    fn visit_class(&mut self, class: &Class<'a>) {
        self.walk_ambient(class.declare, |this| oxc_ast_visit::walk::walk_class(this, class));
    }

    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        if matches!(function.r#type, FunctionType::FunctionDeclaration | FunctionType::TSDeclareFunction)
        {
            let name_start = function.id.as_ref().map_or(function.params.span.start, |id| id.span.start);
            self.check_ambient_generator(
                function.generator,
                function.declare,
                Span::new(function.span.start, name_start),
            );
        }
        oxc_ast_visit::walk::walk_function(self, function, flags);
    }

    fn visit_method_definition(&mut self, method: &MethodDefinition<'a>) {
        self.check_ambient_generator(
            method.value.generator,
            false,
            Span::new(method.span.start, method.key.span().start),
        );
        oxc_ast_visit::walk::walk_method_definition(self, method);
    }
}
