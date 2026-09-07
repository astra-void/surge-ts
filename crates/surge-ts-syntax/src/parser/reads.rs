//! Collects every identifier name read within a function body — value positions
//! and type positions alike — walking the full oxc AST. Because it runs over the
//! original AST (not the lossy `Parsed*` tree), it sees reads inside spreads,
//! `for-in` loops, object methods, nested functions, and type annotations — the
//! over-approximation that backs FP-free unused-binding diagnostics (TS6133,
//! TS6196).
//!
//! A body's set includes every nested body's, so asking for each body
//! separately re-walked every identifier once per enclosing function — and
//! allocated a `String` for it each time. [`with_body_read_index`] computes the
//! whole file in one walk instead, folding each body's set into its parent, and
//! [`collect_function_body_reads`] serves from that.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use oxc_ast::ast::{
    AssignmentExpression, AssignmentOperator, AssignmentTarget, FunctionBody, IdentifierReference,
    Program, TSTypeName,
};
use oxc_ast_visit::Visit;

/// A plain `x = value` *writes* `x`; it does not read it, so a binding that is
/// only ever assigned stays unused (tsc reports TS6133 for it). Compound forms
/// (`x += 1`) and every member target (`o.p = v` reads `o`) keep the default
/// walk.
fn is_plain_identifier_write(assignment: &AssignmentExpression<'_>) -> bool {
    assignment.operator == AssignmentOperator::Assign
        && matches!(
            assignment.left,
            AssignmentTarget::AssignmentTargetIdentifier(_)
        )
}

/// The name a type reference resolves through: `A` in `A`, and in `A.B.C`.
/// Type-position names count as reads too — they are what makes a body-local
/// `type`/`interface` used (TS6196), and a value named in `typeof x` is read by
/// tsc even though it never appears in value position.
fn type_name_root<'a>(name: &TSTypeName<'a>) -> Option<&'a str> {
    let mut current = name;
    loop {
        match current {
            TSTypeName::IdentifierReference(identifier) => return Some(identifier.name.as_str()),
            TSTypeName::QualifiedName(qualified) => current = &qualified.left,
            TSTypeName::ThisExpression(_) => return None,
        }
    }
}

fn sorted_names(names: &HashSet<&str>) -> Vec<String> {
    let mut names: Vec<String> = names.iter().map(|name| (*name).to_string()).collect();
    names.sort_unstable();
    names
}

/// Single-body walk, used when the index has no entry for a body.
#[derive(Default)]
struct ReadCollector<'a> {
    names: HashSet<&'a str>,
}

impl<'a> Visit<'a> for ReadCollector<'a> {
    fn visit_identifier_reference(&mut self, reference: &IdentifierReference<'a>) {
        self.names.insert(reference.name.as_str());
    }

    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
        if is_plain_identifier_write(assignment) {
            self.visit_expression(&assignment.right);
            return;
        }
        oxc_ast_visit::walk::walk_assignment_expression(self, assignment);
    }

    fn visit_ts_type_name(&mut self, name: &TSTypeName<'a>) {
        if let Some(root) = type_name_root(name) {
            self.names.insert(root);
        }
    }
}

/// Builds every body's read set in one pass. Each identifier is recorded into
/// the innermost enclosing body's accumulator; when that body closes, its set is
/// stored and then folded into its parent — which is what makes a body's set
/// contain its nested bodies' without walking them again.
struct IndexBuilder<'a> {
    /// One accumulator per open function body, with the program-level one at the
    /// bottom. Never empty.
    scopes: Vec<HashSet<&'a str>>,
    by_body: HashMap<usize, Vec<String>>,
}

impl<'a> IndexBuilder<'a> {
    fn new() -> Self {
        Self {
            scopes: vec![HashSet::new()],
            by_body: HashMap::new(),
        }
    }

    fn record(&mut self, name: &'a str) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name);
        }
    }
}

impl<'a> Visit<'a> for IndexBuilder<'a> {
    fn visit_identifier_reference(&mut self, reference: &IdentifierReference<'a>) {
        self.record(reference.name.as_str());
    }

    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
        if is_plain_identifier_write(assignment) {
            self.visit_expression(&assignment.right);
            return;
        }
        oxc_ast_visit::walk::walk_assignment_expression(self, assignment);
    }

    fn visit_ts_type_name(&mut self, name: &TSTypeName<'a>) {
        if let Some(root) = type_name_root(name) {
            self.record(root);
        }
    }

    fn visit_function_body(&mut self, body: &FunctionBody<'a>) {
        self.scopes.push(HashSet::new());
        oxc_ast_visit::walk::walk_function_body(self, body);
        let own = self.scopes.pop().expect("pushed immediately above");
        self.by_body.insert(body_key(body), sorted_names(&own));
        // A nested function's *signature* (parameter defaults, annotations) is
        // visited before its body opens, so it belongs to the enclosing scope —
        // matching what a standalone walk of the enclosing body would collect.
        if let Some(parent) = self.scopes.last_mut() {
            parent.extend(own);
        }
    }
}

/// Identity of a body node. oxc AST nodes live in the parse arena and are never
/// moved, so the address is a stable key for the duration of one parse — which
/// is exactly the lifetime of the index.
fn body_key(body: &FunctionBody<'_>) -> usize {
    std::ptr::from_ref(body) as usize
}

struct BodyReadIndex {
    by_body: HashMap<usize, Vec<String>>,
}

thread_local! {
    static BODY_READ_INDEX: RefCell<Option<BodyReadIndex>> = const { RefCell::new(None) };
}

/// Builds `program`'s per-body read index, installs it for the duration of `f`
/// (the statement conversion, which is what asks for each body's reads), and
/// returns the program-wide read set alongside `f`'s result.
///
/// The program-wide set is the outermost accumulator of the same walk, so it
/// costs nothing beyond it. The previous index is restored on exit (including
/// unwinds) rather than cleared, so a nested parse cannot strand a stale one.
pub(crate) fn with_body_read_index<R>(
    program: &Program<'_>,
    f: impl FnOnce() -> R,
) -> (Vec<String>, R) {
    struct Restore(Option<BodyReadIndex>);
    impl Drop for Restore {
        fn drop(&mut self) {
            BODY_READ_INDEX.with(|index| *index.borrow_mut() = self.0.take());
        }
    }

    let mut builder = IndexBuilder::new();
    builder.visit_program(program);
    let program_scope = builder.scopes.pop().expect("program scope is never popped");
    let program_reads = sorted_names(&program_scope);

    let previous = BODY_READ_INDEX.with(|index| {
        index.borrow_mut().replace(BodyReadIndex {
            by_body: builder.by_body,
        })
    });
    let _restore = Restore(previous);

    (program_reads, f())
}

/// The names read anywhere in `body`, sorted and deduplicated.
///
/// Served from the index the enclosing parse installed; each body is asked for
/// once, so the entry is taken rather than cloned. A miss walks the body
/// directly, which keeps the index a pure optimization: a body the pre-pass did
/// not reach — a declaration file's, where no index is built — still gets
/// exactly the set it would have without one.
pub(crate) fn collect_function_body_reads(body: &FunctionBody<'_>) -> Vec<String> {
    let indexed = BODY_READ_INDEX.with(|index| {
        index
            .borrow_mut()
            .as_mut()
            .and_then(|index| index.by_body.remove(&body_key(body)))
    });
    if let Some(reads) = indexed {
        return reads;
    }

    let mut collector = ReadCollector::default();
    collector.visit_function_body(body);
    sorted_names(&collector.names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxc_allocator::Allocator;
    use oxc_parser::Parser;
    use oxc_span::SourceType;
    use std::marker::PhantomData;

    /// Walks every body and compares the indexed set against what a standalone
    /// walk of that same body produces. The index exists only to avoid repeating
    /// those walks, so equality with them *is* the correctness condition.
    struct Differ<'a> {
        index: HashMap<usize, Vec<String>>,
        mismatches: Vec<String>,
        bodies: usize,
        marker: PhantomData<&'a ()>,
    }

    impl<'a> Visit<'a> for Differ<'a> {
        fn visit_function_body(&mut self, body: &FunctionBody<'a>) {
            let mut collector = ReadCollector::default();
            collector.visit_function_body(body);
            let direct = sorted_names(&collector.names);

            match self.index.get(&body_key(body)) {
                Some(indexed) if *indexed == direct => {}
                Some(indexed) => self.mismatches.push(format!(
                    "indexed {indexed:?} != direct {direct:?}"
                )),
                None => self
                    .mismatches
                    .push(format!("no index entry for body reading {direct:?}")),
            }
            self.bodies += 1;

            oxc_ast_visit::walk::walk_function_body(self, body);
        }
    }

    #[track_caller]
    fn assert_index_matches_direct_walk(source: &str, expected_bodies: usize) {
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, source, SourceType::ts()).parse();
        assert!(parsed.errors.is_empty(), "fixture must parse: {:?}", parsed.errors);

        let mut builder = IndexBuilder::new();
        builder.visit_program(&parsed.program);

        let mut differ = Differ {
            index: builder.by_body,
            mismatches: Vec::new(),
            bodies: 0,
            marker: PhantomData,
        };
        differ.visit_program(&parsed.program);

        assert!(differ.mismatches.is_empty(), "{:#?}", differ.mismatches);
        assert_eq!(
            differ.bodies, expected_bodies,
            "fixture should exercise {expected_bodies} bodies"
        );
    }

    #[test]
    fn nested_functions_match_a_standalone_walk() {
        assert_index_matches_direct_walk(
            "function outer(a) {\n\
             \x20 function middle(b) {\n\
             \x20   function inner(c) { return a + b + c; }\n\
             \x20   return inner;\n\
             \x20 }\n\
             \x20 return middle;\n\
             }\n",
            3,
        );
    }

    #[test]
    fn arrows_and_object_methods_match_a_standalone_walk() {
        assert_index_matches_direct_walk(
            "function host(seed) {\n\
             \x20 const bag = {\n\
             \x20   method() { return seed; },\n\
             \x20   arrow: (x) => x + seed,\n\
             \x20 };\n\
             \x20 return [...Object.values(bag)];\n\
             }\n",
            3,
        );
    }

    #[test]
    fn type_positions_and_typeof_match_a_standalone_walk() {
        assert_index_matches_direct_walk(
            "function host() {\n\
             \x20 type Local = Record<string, number>;\n\
             \x20 const value = 1;\n\
             \x20 function inner(): typeof value { return value; }\n\
             \x20 const held: Local = {};\n\
             \x20 return { inner, held };\n\
             }\n",
            2,
        );
    }

    /// A plain `x = value` writes rather than reads, and the index must drop it
    /// on exactly the same terms a standalone walk does — at every nesting level.
    #[test]
    fn plain_writes_are_skipped_at_every_level() {
        assert_index_matches_direct_walk(
            "function host(a, b) {\n\
             \x20 a = 1;\n\
             \x20 function inner(c) { c = 2; b += 1; return c; }\n\
             \x20 return inner;\n\
             }\n",
            2,
        );
    }

    /// A nested function's parameter defaults and annotations are read by the
    /// *enclosing* body, not by the nested one — the walk order the fold relies
    /// on.
    #[test]
    fn nested_signature_reads_belong_to_the_enclosing_body() {
        let allocator = Allocator::default();
        let source = "function host(fallback: Seed) {\n\
             \x20 function inner(value = fallback) { return value; }\n\
             \x20 return inner;\n\
             }\n";
        let parsed = Parser::new(&allocator, source, SourceType::ts()).parse();

        let mut builder = IndexBuilder::new();
        builder.visit_program(&parsed.program);
        let mut sets: Vec<Vec<String>> = builder.by_body.into_values().collect();
        sets.sort_by_key(Vec::len);

        let inner = &sets[0];
        let outer = &sets[1];
        assert!(
            !inner.iter().any(|name| name == "fallback"),
            "the default belongs to the enclosing body, got {inner:?}"
        );
        assert!(
            outer.iter().any(|name| name == "fallback"),
            "the enclosing body reads the default, got {outer:?}"
        );
    }

    #[test]
    fn program_reads_match_a_whole_program_walk() {
        let allocator = Allocator::default();
        let source = "const top = 1;\n\
             function host(a) {\n\
             \x20 const inner = (b) => b + top + a;\n\
             \x20 return inner;\n\
             }\n\
             type Alias = Record<string, typeof top>;\n";
        let parsed = Parser::new(&allocator, source, SourceType::ts()).parse();

        let mut collector = ReadCollector::default();
        collector.visit_program(&parsed.program);
        let direct = sorted_names(&collector.names);

        let (program_reads, ()) = with_body_read_index(&parsed.program, || ());
        assert_eq!(program_reads, direct);
    }

    /// A body the index never saw still answers, so installing the index can
    /// only save work — never change it.
    #[test]
    fn an_unindexed_body_falls_back_to_walking() {
        let allocator = Allocator::default();
        let source = "function host(a) { return a + outside; }\n";
        let parsed = Parser::new(&allocator, source, SourceType::ts()).parse();

        struct First<'a> {
            reads: Option<Vec<String>>,
            marker: PhantomData<&'a ()>,
        }
        impl<'a> Visit<'a> for First<'a> {
            fn visit_function_body(&mut self, body: &FunctionBody<'a>) {
                if self.reads.is_none() {
                    self.reads = Some(collect_function_body_reads(body));
                }
            }
        }

        let mut first = First {
            reads: None,
            marker: PhantomData,
        };
        first.visit_program(&parsed.program);

        assert_eq!(
            first.reads.expect("one body"),
            vec!["a".to_string(), "outside".to_string()]
        );
    }
}
