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

mod super_call;
mod this_before_super;

pub(crate) fn collect_grammar_diagnostics(
    program: &Program<'_>,
) -> (Vec<ParsedGrammarDiagnostic>, Vec<ParenthesizedExpressionSpan>) {
    let mut collector = GrammarCollector::default();
    collector.visit_program(program);
    super::grammar_context::collect_context_grammar_diagnostics(program, &mut collector.diagnostics);
    super::reachability::collect_unreachable_code(program, &mut collector.diagnostics);
    super::grammar_recovered::collect_recovered_grammar_diagnostics(program, false, &mut collector.diagnostics);
    let mut parenthesized = collector.parenthesized_expressions;
    parenthesized.sort_unstable_by_key(|span| (span.inner.start, span.inner.end));
    (collector.diagnostics, parenthesized)
}

#[derive(Default)]
struct GrammarCollector {
    diagnostics: Vec<ParsedGrammarDiagnostic>,
    parenthesized_expressions: Vec<ParenthesizedExpressionSpan>,
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
    /// Starts of the `(0, x.f)` sequences called indirectly; see
    /// [`GrammarCollector::note_indirect_call`].
    indirect_call_sequences: std::collections::HashSet<u32>,
    /// Block and namespace nesting; a computed type member name is answered
    /// only outside both, where the file's top-level scope is the one it reads.
    nested_scope_depth: usize,
    /// Class bodies and namespaces enclosing the node: with `function_async`,
    /// what tsc's `IsInTopLevelContext` looks past.
    this_container_depth: usize,
    /// Every name the file binds, with where; a computed type member name any
    /// of them could answer — a value, a type parameter — is left alone.
    binding_names: Vec<(String, Span)>,
    /// The names of type aliases and interfaces, the bindings that do not.
    type_declaration_name_spans: std::collections::HashSet<(u32, u32)>,
}

/// One `get`/`set` accessor of a class, interface, type literal or object
/// literal, as [`GrammarCollector::check_untyped_setters`] reads it.
struct AccessorRecord {
    key: String,
    display: Option<String>,
    is_static: bool,
    is_getter: bool,
    /// A getter's return annotation, or a setter's parameter annotation.
    annotated: bool,
    has_body: bool,
    private: bool,
    span: Span,
}

fn accessor_display_name(key: &PropertyKey<'_>, computed: bool) -> Option<String> {
    if computed {
        return None;
    }
    match key {
        PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.to_string()),
        PropertyKey::PrivateIdentifier(identifier) => Some(format!("#{}", identifier.name)),
        PropertyKey::StringLiteral(literal) => Some(format!("\"{}\"", literal.value)),
        _ => None,
    }
}

/// tsc's `getSetAccessorValueParameter` takes the first parameter whatever its
/// kind, so `set x(...v: T[])` is annotated through its rest parameter.
fn setter_parameter_annotated(parameters: &FormalParameters<'_>) -> bool {
    match parameters.items.first() {
        Some(parameter) => parameter.type_annotation.is_some(),
        None => parameters.rest.as_ref().is_some_and(|rest| rest.type_annotation.is_some()),
    }
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

/// The number a constant initializer evaluates to, where tsc's `evaluate`
/// can be followed without name resolution: literals, arithmetic, and the
/// numeric members declared before it in the same enum (bare or as
/// `Enum.Member`). `None` for anything else, which is never reported.
fn enum_numeric_value(
    expression: &Expression<'_>,
    members: &std::collections::HashMap<String, f64>,
    enum_name: &str,
) -> Option<f64> {
    use oxc_syntax::operator::BinaryOperator;
    match expression {
        Expression::NumericLiteral(literal) => Some(literal.value),
        Expression::ParenthesizedExpression(parenthesized) => {
            enum_numeric_value(&parenthesized.expression, members, enum_name)
        }
        Expression::UnaryExpression(unary) => {
            let value = enum_numeric_value(&unary.argument, members, enum_name)?;
            match unary.operator {
                UnaryOperator::UnaryPlus => Some(value),
                UnaryOperator::UnaryNegation => Some(-value),
                UnaryOperator::BitwiseNot => Some(f64::from(!to_int32(value))),
                _ => None,
            }
        }
        Expression::BinaryExpression(binary) => {
            let left = enum_numeric_value(&binary.left, members, enum_name)?;
            let right = enum_numeric_value(&binary.right, members, enum_name)?;
            Some(match binary.operator {
                BinaryOperator::Addition => left + right,
                BinaryOperator::Subtraction => left - right,
                BinaryOperator::Multiplication => left * right,
                BinaryOperator::Division => left / right,
                BinaryOperator::Remainder => left % right,
                BinaryOperator::Exponential => left.powf(right),
                BinaryOperator::ShiftLeft => f64::from(to_int32(left).wrapping_shl(to_uint32(right) & 31)),
                BinaryOperator::ShiftRight => f64::from(to_int32(left) >> (to_uint32(right) & 31)),
                BinaryOperator::ShiftRightZeroFill => {
                    f64::from(to_uint32(left) >> (to_uint32(right) & 31))
                }
                BinaryOperator::BitwiseOR => f64::from(to_int32(left) | to_int32(right)),
                BinaryOperator::BitwiseXOR => f64::from(to_int32(left) ^ to_int32(right)),
                BinaryOperator::BitwiseAnd => f64::from(to_int32(left) & to_int32(right)),
                _ => return None,
            })
        }
        Expression::Identifier(identifier) => members.get(identifier.name.as_str()).copied(),
        Expression::StaticMemberExpression(member) => match &member.object {
            Expression::Identifier(object) if object.name == enum_name => {
                members.get(member.property.name.as_str()).copied()
            }
            _ => None,
        },
        _ => None,
    }
}

/// ECMAScript `ToInt32`/`ToUint32`.
fn to_uint32(value: f64) -> u32 {
    if !value.is_finite() {
        return 0;
    }
    let truncated = value.trunc();
    let modulo = truncated.rem_euclid(4_294_967_296.0);
    modulo as u32
}

fn to_int32(value: f64) -> i32 {
    to_uint32(value) as i32
}

impl GrammarCollector {
    /// tsc's `isIndirectCall`: `(0, x.f)(…)`, a tagged `(0, x.f)` or
    /// `(0, eval)(…)` discards the `0` to call without a `this`, so the
    /// unused-comma check leaves it alone.
    fn note_indirect_call(&mut self, callee: &Expression<'_>) {
        let Expression::ParenthesizedExpression(parenthesized) = callee else {
            return;
        };
        let Expression::SequenceExpression(sequence) = &parenthesized.expression else {
            return;
        };
        let [first, last] = &sequence.expressions[..] else {
            return;
        };
        let zero = matches!(first, Expression::NumericLiteral(literal) if literal.raw.as_deref() == Some("0"));
        let access = matches!(
            last,
            Expression::StaticMemberExpression(_)
                | Expression::ComputedMemberExpression(_)
                | Expression::PrivateFieldExpression(_)
        ) || matches!(last, Expression::Identifier(identifier) if identifier.name == "eval");
        if zero && access {
            self.indirect_call_sequences.insert(sequence.span.start);
        }
    }

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
        let mut numeric_values: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
        let mut previous = EnumConstant::Number;
        let mut next_auto_value = Some(0.0);
        for member in &declaration.body.members {
            let mut numeric_value = None;
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
                    if matches!(value, EnumConstant::Number | EnumConstant::Unknown) {
                        numeric_value =
                            enum_numeric_value(initializer, &numeric_values, declaration.id.name.as_str());
                        if declaration.r#const
                            && let Some(number) = numeric_value
                            && !number.is_finite()
                        {
                            let code = if number.is_nan() { 2478 } else { 2477 };
                            self.push(Kind::Ts(code), initializer.span(), None);
                        }
                    }
                    previous = value;
                    value
                }
                None => {
                    if !ambient && matches!(previous, EnumConstant::String | EnumConstant::NonConstant) {
                        self.push(Kind::EnumMemberInitializerRequired, member.id.span(), None);
                    }
                    numeric_value = next_auto_value;
                    if previous == EnumConstant::Unknown {
                        EnumConstant::Unknown
                    } else {
                        EnumConstant::Number
                    }
                }
            };
            next_auto_value = numeric_value.map(|number| number + 1.0);
            if let Some(name) = property_key_name_of_enum_member(&member.id) {
                members.insert(name.clone(), value);
                if let Some(number) = numeric_value {
                    numeric_values.insert(name, number);
                }
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
        // First declaration wins for both lookups, as the linear `find`s did.
        let mut class_slots: std::collections::HashMap<&str, usize> =
            std::collections::HashMap::new();
        for (slot, class) in classes.iter().enumerate() {
            if let Some(id) = class.id.as_ref() {
                class_slots.entry(id.name.as_str()).or_insert(slot);
            }
        }
        let base_of = |class: &Class<'_>| match &class.super_class {
            Some(Expression::Identifier(base)) if class.super_type_arguments.is_none() => {
                class_slots.get(base.name.as_str()).copied()
            }
            _ => None,
        };
        let member_lists: Vec<Vec<(String, Member)>> =
            classes.iter().map(|class| instance_members(class)).collect();
        let member_maps: Vec<std::collections::HashMap<&str, &Member>> = member_lists
            .iter()
            .map(|members| {
                let mut map = std::collections::HashMap::new();
                for (name, member) in members {
                    map.entry(name.as_str()).or_insert(member);
                }
                map
            })
            .collect();

        for (slot, class) in classes.iter().enumerate() {
            let (Some(derived_name), Some(base)) = (class.id.as_ref(), base_of(class)) else {
                continue;
            };
            let Some(base_name) = classes[base].id.as_ref() else {
                continue;
            };
            for (name, derived) in &member_lists[slot] {
                let mut ancestor = Some(base);
                let mut found = None;
                let mut depth = 0;
                while let Some(current) = ancestor
                    && depth < 32
                {
                    if let Some(&member) = member_maps[current].get(name.as_str()) {
                        found = Some(member);
                        break;
                    }
                    ancestor = base_of(classes[current]);
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

    /// tsc's `IsInTopLevelContext`: no function, class body or namespace
    /// encloses the node.
    fn in_top_level_context(&self) -> bool {
        self.function_async.is_empty() && self.this_container_depth == 0
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

    /// tsc's `checkFunctionOrConstructorSymbol` for the function declarations
    /// of one statement list.
    fn check_function_implementations(&mut self, statements: &[Statement<'_>]) {
        let siblings: Vec<OverloadSibling> = statements
            .iter()
            .map(|statement| {
                let (function, exported) = match statement {
                    Statement::FunctionDeclaration(function) => (Some(&**function), false),
                    Statement::ExportNamedDeclaration(export) => match &export.declaration {
                        Some(Declaration::FunctionDeclaration(function)) => (Some(&**function), true),
                        _ => (None, false),
                    },
                    _ => (None, false),
                };
                let Some(function) = function else {
                    return OverloadSibling::other();
                };
                let Some(id) = function.id.as_ref() else {
                    return OverloadSibling::other();
                };
                OverloadSibling {
                    kind: SiblingKind::Function,
                    name: Some(id.name.to_string()),
                    name_span: id.span,
                    is_static: false,
                    has_body: function.body.is_some(),
                    ambient: function.declare || self.is_ambient(),
                    exported,
                    accessibility: None,
                    is_abstract: false,
                    optional: false,
                }
            })
            .collect();
        self.check_overload_groups(&siblings);
    }

    /// The overload half of tsc's `checkFunctionOrConstructorSymbolWorker`
    /// over one container's declarations, grouped the way the binder forms
    /// symbols: by name, and for class members by staticness.
    fn check_overload_groups(&mut self, siblings: &[OverloadSibling]) {
        let mut groups: Vec<(&str, bool, Vec<usize>)> = Vec::new();
        for (index, sibling) in siblings.iter().enumerate() {
            let Some(name) = sibling.name.as_deref() else {
                continue;
            };
            if sibling.kind == SiblingKind::Other {
                continue;
            }
            match groups
                .iter_mut()
                .find(|(group, is_static, _)| *group == name && *is_static == sibling.is_static)
            {
                Some((_, _, indices)) => indices.push(index),
                None => groups.push((name, sibling.is_static, vec![index])),
            }
        }
        for (_, _, indices) in groups {
            let mut previous: Option<usize> = None;
            let mut last_non_ambient: Option<usize> = None;
            let mut body_seen = false;
            for &index in &indices {
                let sibling = &siblings[index];
                if sibling.ambient {
                    previous = None;
                }
                if !(sibling.has_body && body_seen)
                    && let Some(previous) = previous
                    && previous + 1 != index
                {
                    self.report_implementation_expected(siblings, previous);
                }
                body_seen |= sibling.has_body;
                previous = Some(index);
                if !sibling.ambient {
                    last_non_ambient = Some(index);
                }
            }
            if let Some(last) = last_non_ambient
                && !siblings[last].has_body
                && !siblings[last].is_abstract
                && !siblings[last].optional
            {
                self.report_implementation_expected(siblings, last);
            }
            if indices.iter().any(|&index| !siblings[index].has_body) {
                self.check_overload_flag_agreement(siblings, &indices);
            }
        }
    }

    /// tsc's `reportImplementationExpectedError`: the declaration right after
    /// a bodyless one decides the message.
    fn report_implementation_expected(&mut self, siblings: &[OverloadSibling], index: usize) {
        let node = &siblings[index];
        if let Some(next) = siblings.get(index + 1)
            && next.kind == node.kind
        {
            if next.name.is_some() && next.name == node.name {
                if node.kind == SiblingKind::Method && node.is_static != next.is_static {
                    let code = if node.is_static { 2387 } else { 2388 };
                    self.push(Kind::Ts(code), next.name_span, None);
                }
                return;
            }
            if next.has_body {
                self.push(Kind::Ts(2389), next.name_span, node.name.as_deref());
                return;
            }
        }
        self.push(Kind::FunctionImplementationMissing, node.name_span, None);
    }

    /// tsc's `checkFlagAgreementBetweenOverloads` and
    /// `checkQuestionTokenAgreementBetweenOverloads`, measured against the
    /// implementation when there is one and the first declaration otherwise.
    fn check_overload_flag_agreement(&mut self, siblings: &[OverloadSibling], indices: &[usize]) {
        let canonical = indices
            .iter()
            .copied()
            .find(|&index| siblings[index].has_body)
            .unwrap_or(indices[0]);
        let canonical = &siblings[canonical];
        for &index in indices {
            let sibling = &siblings[index];
            let code = if sibling.exported != canonical.exported {
                Some(2383)
            } else if sibling.ambient != canonical.ambient {
                Some(2384)
            } else if sibling.accessibility != canonical.accessibility {
                Some(2385)
            } else if sibling.is_abstract != canonical.is_abstract {
                Some(2512)
            } else {
                None
            };
            if let Some(code) = code {
                self.push(Kind::Ts(code), sibling.name_span, None);
            }
        }
        for &index in indices {
            let sibling = &siblings[index];
            if sibling.optional != canonical.optional {
                self.push(Kind::Ts(2386), sibling.name_span, None);
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

    /// tsc's `getTypeOfAccessors` when neither accessor of a name is typed: no
    /// getter return annotation, no getter body to infer from and no setter
    /// parameter annotation leave the property an implicit `any`, reported on
    /// the setter (TS7032) unless it is private in an ambient context. Only
    /// names whose `symbolToString` surge can spell are reported.
    fn check_untyped_setters(&mut self, accessors: &[AccessorRecord], ambient: bool) {
        for setter in accessors.iter().filter(|accessor| !accessor.is_getter) {
            let Some(display) = setter.display.as_deref() else {
                continue;
            };
            if setter.annotated || (setter.private && ambient) {
                continue;
            }
            let getter = accessors.iter().find(|accessor| {
                accessor.is_getter && accessor.key == setter.key && accessor.is_static == setter.is_static
            });
            if getter.is_some_and(|getter| getter.annotated || getter.has_body) {
                continue;
            }
            // A private setter in a declaration file is ambient too, which
            // only the checker knows.
            let payload = if setter.private { format!("{display}\0private") } else { display.to_string() };
            self.push(Kind::Ts(7032), setter.span, Some(&payload));
        }
    }

    /// A member name `[Name]` in a module-level type literal or interface,
    /// which tsc resolves as a value; see [`Kind::ComputedTypeMemberName`].
    fn check_computed_type_member_names(&mut self, members: &[TSSignature<'_>], type_literal: bool) {
        if !self.function_async.is_empty() || self.nested_scope_depth > 0 {
            return;
        }
        for member in members {
            let (key, mapped) = match member {
                TSSignature::TSPropertySignature(property) if property.computed => {
                    (&property.key, type_literal && members.len() == 1)
                }
                TSSignature::TSMethodSignature(method) if method.computed => (&method.key, false),
                _ => continue,
            };
            if let PropertyKey::Identifier(identifier) = key {
                let payload = format!("{}\0{}", identifier.name, u8::from(mapped));
                self.push(Kind::ComputedTypeMemberName, identifier.span, Some(&payload));
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

    /// The class-method overload checks. A name that is also a property or an
    /// accessor is a duplicate, not an overload set, and reports as one.
    fn check_method_overloads(&mut self, class: &Class<'_>) {
        let conflicting: Vec<(String, bool)> = class
            .body
            .body
            .iter()
            .filter_map(|element| match element {
                ClassElement::PropertyDefinition(property) => {
                    property_key_name(&property.key).map(|name| (name, property.r#static))
                }
                ClassElement::MethodDefinition(method)
                    if matches!(method.kind, MethodDefinitionKind::Get | MethodDefinitionKind::Set) =>
                {
                    property_key_name(&method.key).map(|name| (name, method.r#static))
                }
                _ => None,
            })
            .collect();
        let siblings: Vec<OverloadSibling> = class
            .body
            .body
            .iter()
            .map(|element| {
                let ClassElement::MethodDefinition(method) = element else {
                    return OverloadSibling::other();
                };
                if method.kind != MethodDefinitionKind::Method || method.computed {
                    return OverloadSibling::other();
                }
                let Some(name) = property_key_name(&method.key) else {
                    return OverloadSibling::other();
                };
                if conflicting.iter().any(|(other, is_static)| *other == name && *is_static == method.r#static) {
                    return OverloadSibling::other();
                }
                OverloadSibling {
                    kind: SiblingKind::Method,
                    name: Some(name),
                    name_span: method.key.span(),
                    is_static: method.r#static,
                    has_body: method.value.body.is_some(),
                    ambient: false,
                    exported: false,
                    accessibility: method.accessibility.filter(|accessibility| {
                        *accessibility != oxc_ast::ast::TSAccessibility::Public
                    }),
                    is_abstract: method.r#type == MethodDefinitionType::TSAbstractMethodDefinition,
                    optional: method.optional,
                }
            })
            .collect();
        self.check_overload_groups(&siblings);
    }

    /// Everything one class body can say wrong about its own members: a
    /// constructor group with no implementation (TS2390), two implementations
    /// of one name (TS2393/TS2392), two members declaring the same name where
    /// neither is an overload of the other (TS2300), and the method overload
    /// checks of [`Self::check_method_overloads`].
    ///
    /// Static and instance members are separate names, and an `abstract` or
    /// ambient member has no implementation to miss.
    fn check_class_members(&mut self, class: &Class<'_>) {
        let ambient = self.is_ambient() || class.declare;
        let mut groups: Vec<MemberGroup> = Vec::new();
        let mut group_slots: std::collections::HashMap<(String, bool), usize> =
            std::collections::HashMap::new();
        let mut constructors: Vec<(Span, bool, bool)> = Vec::new();

        for element in &class.body.body {
            let (key, is_static, member) = match element {
                ClassElement::MethodDefinition(method) => {
                    let is_abstract =
                        method.r#type == MethodDefinitionType::TSAbstractMethodDefinition;
                    // `abstract constructor` is TS1242, where tsc stops.
                    if is_abstract
                        && !class.r#abstract
                        && method.kind != MethodDefinitionKind::Constructor
                    {
                        self.push(Kind::AbstractMethodOutsideAbstractClass, method.span, None);
                    }
                    if method.kind == MethodDefinitionKind::Constructor {
                        if method.value.body.is_none() {
                            self.check_signature_parameters(&method.value.params);
                        }
                        constructors.push((
                            method.key.span(),
                            method.value.body.is_some(),
                            is_abstract,
                        ));
                        // A parameter property is an instance property
                        // declared where the constructor stands.
                        for parameter in &method.value.params.items {
                            if (parameter.accessibility.is_some() || parameter.readonly || parameter.r#override)
                                && let oxc_ast::ast::BindingPattern::BindingIdentifier(identifier) = &parameter.pattern
                            {
                                add_group_member(
                                    &mut groups,
                                    &mut group_slots,
                                    identifier.name.to_string(),
                                    false,
                                    (identifier.span, MemberKind::Property),
                                );
                            }
                        }
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
                    // A `declare` field is ambient itself. tsc checks the
                    // initializer in `checkGrammarProperty`, which a decorator on
                    // such a field never reaches (`checkGrammarModifiers` fails
                    // first).
                    if (ambient || property.declare)
                        && !(property.declare && !property.decorators.is_empty())
                        && let Some(value) = property.value.as_ref()
                    {
                        self.check_ambient_initializer(
                            value,
                            property.readonly,
                            property.type_annotation.is_some(),
                        );
                    }
                    (&property.key, property.r#static, MemberKind::Property)
                }
                ClassElement::AccessorProperty(property) => (&property.key, property.r#static, MemberKind::Accessor),
                _ => continue,
            };

            let Some(name) = property_key_name(key) else {
                continue;
            };
            add_group_member(&mut groups, &mut group_slots, name, is_static, (key.span(), member));
        }

        for group in &groups {
            self.report_member_group(group);
        }
        if !ambient {
            self.check_method_overloads(class);
        }
        self.check_accessor_pairs(class);
        let accessors: Vec<AccessorRecord> = class
            .body
            .body
            .iter()
            .filter_map(|element| match element {
                ClassElement::MethodDefinition(method)
                    if matches!(method.kind, MethodDefinitionKind::Get | MethodDefinitionKind::Set) =>
                {
                    let is_getter = method.kind == MethodDefinitionKind::Get;
                    Some(AccessorRecord {
                        key: property_key_name(&method.key)?,
                        display: accessor_display_name(&method.key, method.computed),
                        is_static: method.r#static,
                        is_getter,
                        annotated: if is_getter {
                            method.value.return_type.is_some()
                                || super::jsdoc::return_type_at(method.value.span.start).is_some()
                        } else {
                            setter_parameter_annotated(&method.value.params)
                                || method.value.params.items.first().is_some_and(|parameter| {
                                    super::jsdoc::parameter_at(parameter.span.start)
                                        .is_some_and(|jsdoc| jsdoc.ty.is_some())
                                })
                        },
                        has_body: method.value.body.is_some(),
                        private: method.accessibility == Some(oxc_ast::ast::TSAccessibility::Private)
                            || matches!(method.key, PropertyKey::PrivateIdentifier(_)),
                        span: method.key.span(),
                    })
                }
                _ => None,
            })
            .collect();
        self.check_untyped_setters(&accessors, ambient);

        // A property with no annotation and no initializer has an implicit
        // `any` type *unless* the constructor assigns it: tsc infers the
        // declaration's type from that assignment.
        let (assigned_instance, assigned_static) = assigned_property_names(class);
        for element in &class.body.body {
            // An auto-accessor is declared like a property and judged like one.
            let (key, is_typed, is_static, accessibility) = match element {
                ClassElement::PropertyDefinition(property) => (
                    &property.key,
                    property.type_annotation.is_some()
                        || property.value.is_some()
                        || property.computed
                        || super::jsdoc::declared_type_at(property.span.start).is_some(),
                    property.r#static,
                    property.accessibility,
                ),
                ClassElement::AccessorProperty(property) => (
                    &property.key,
                    property.type_annotation.is_some() || property.value.is_some() || property.computed,
                    property.r#static,
                    property.accessibility,
                ),
                _ => continue,
            };
            if is_typed {
                continue;
            }
            // `declarationBelongsToPrivateAmbientMember`: an ambient class's
            // private member reports no implicit `any`.
            if ambient
                && (accessibility == Some(oxc_ast::ast::TSAccessibility::Private)
                    || matches!(key, PropertyKey::PrivateIdentifier(_)))
            {
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
                .filter(|(_, has_body, _)| *has_body)
                .map(|(span, _, _)| *span)
                .collect();
            if implementations.len() > 1 {
                for span in implementations {
                    self.push(Kind::MultipleConstructorImplementations, span, None);
                }
            }
        }
        // tsc exempts an `abstract` last overload from needing an implementation.
        if !ambient
            && !constructors.is_empty()
            && !constructors.iter().any(|(_, has_body, _)| *has_body)
            && let Some((span, _, false)) = constructors.last() {
                self.push(Kind::ConstructorImplementationMissing, *span, None);
            }

        // A class extending `null` has no base constructor to call (TS17005
        // when it does), so tsc does not require the call.
        let extends_null = matches!(
            class.super_class.as_ref().map(Expression::without_parentheses),
            Some(Expression::NullLiteral(_))
        );
        if !ambient && class.super_class.is_some() && !extends_null {
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
                    self.report_this_before_super(&method.value.params, body);
                    self.check_super_call_placement(class, method.key.span(), &method.value.params, body);
                }
            }
        }
    }

    /// One name's worth of class or interface members. A group of method
    /// signatures is an overload set, so it reports only about implementations;
    /// as soon as a property is in the group nothing there is an overload of
    /// anything and every member is a duplicate.
    fn report_member_group(&mut self, group: &MemberGroup) {
        let has_property = group
            .members
            .iter()
            .any(|(_, member)| matches!(member, MemberKind::Property));
        let has_accessor = group
            .members
            .iter()
            .any(|(_, member)| matches!(member, MemberKind::Accessor));
        let has_method = group
            .members
            .iter()
            .any(|(_, member)| matches!(member, MemberKind::Method { .. } | MemberKind::AbstractMethod));

        // Properties and accessors merge into one symbol, so tsc's checker
        // finds these (`checkObjectTypeForDuplicateDeclarations`): a second
        // property, or a property with an accessor, is a duplicate at every
        // member of the name; a get/set pair is not. A method among them is
        // the binder's conflict instead.
        if !has_method {
            let mut first_kind: Option<MemberKind> = None;
            for (_, member) in &group.members {
                match first_kind {
                    None => first_kind = Some(*member),
                    Some(MemberKind::Accessor) if *member == MemberKind::Accessor => {}
                    Some(_) => {
                        let name = group.name.clone();
                        for (span, _) in &group.members {
                            self.push(Kind::DuplicateMember, *span, Some(&name));
                        }
                        return;
                    }
                }
            }
            return;
        }

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

    }

    /// An interface reports the duplicate half only: a bodyless method there is
    /// a signature, never a missing implementation.
    fn check_interface_members(&mut self, members: &[TSSignature<'_>]) {
        let accessors: Vec<AccessorRecord> = members
            .iter()
            .filter_map(|member| match member {
                TSSignature::TSMethodSignature(method) if method.kind != TSMethodSignatureKind::Method => {
                    let is_getter = method.kind == TSMethodSignatureKind::Get;
                    Some(AccessorRecord {
                        key: property_key_name(&method.key)?,
                        display: accessor_display_name(&method.key, method.computed),
                        is_static: false,
                        is_getter,
                        annotated: if is_getter {
                            method.return_type.is_some()
                        } else {
                            setter_parameter_annotated(&method.params)
                        },
                        has_body: false,
                        private: false,
                        span: method.key.span(),
                    })
                }
                _ => None,
            })
            .collect();
        self.check_untyped_setters(&accessors, false);
        let mut groups: Vec<MemberGroup> = Vec::new();
        let mut group_slots: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

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
            match group_slots.get(&name) {
                Some(&slot) => groups[slot].members.push((key.span(), kind)),
                None => {
                    group_slots.insert(name.clone(), groups.len());
                    groups.push(MemberGroup {
                        name,
                        members: vec![(key.span(), kind)],
                    });
                }
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

    /// tsc's `checkAmbientInitializer`: only a `const` (or `readonly` property)
    /// without a type annotation may be initialized in an ambient context, and
    /// only with a constant (TS1254); any other initializer is TS1039.
    fn check_ambient_initializer(&mut self, initializer: &Expression<'_>, const_or_readonly: bool, has_type: bool) {
        if !const_or_readonly || has_type {
            self.push(Kind::AmbientInitializer, initializer.span(), None);
        } else if !is_ambient_constant_initializer(initializer) {
            self.push(Kind::AmbientConstInitializer, initializer.span(), None);
        }
    }

    /// A signature has no body to run a default in.
    fn check_signature_parameters(&mut self, parameters: &FormalParameters<'_>) {
        for parameter in &parameters.items {
            self.check_signature_pattern_initializers(&parameter.pattern);
            if parameter.initializer.is_some() {
                self.push(
                    Kind::ParameterInitializerOutsideImplementation,
                    parameter.span,
                    None,
                );
            }
        }
    }

    /// The elements of a parameter's pattern are checked like the parameter
    /// (`checkVariableLikeDeclaration`), so an initializer there is TS2371 too,
    /// at the element's name — except on a renamed element (`{ key: local }`),
    /// which that check leaves at once.
    fn check_signature_pattern_initializers(&mut self, pattern: &oxc_ast::ast::BindingPattern<'_>) {
        let elements: Vec<(&oxc_ast::ast::BindingPattern<'_>, bool)> = match pattern {
            oxc_ast::ast::BindingPattern::ObjectPattern(object) => {
                object.properties.iter().map(|property| (&property.value, !property.shorthand)).collect()
            }
            oxc_ast::ast::BindingPattern::ArrayPattern(array) => {
                array.elements.iter().flatten().map(|element| (element, false)).collect()
            }
            _ => return,
        };
        for (element, renamed) in elements {
            match element {
                oxc_ast::ast::BindingPattern::AssignmentPattern(assignment) => {
                    if renamed && matches!(assignment.left, oxc_ast::ast::BindingPattern::BindingIdentifier(_)) {
                        continue;
                    }
                    self.check_signature_pattern_initializers(&assignment.left);
                    self.push(Kind::ParameterInitializerOutsideImplementation, assignment.left.span(), None);
                }
                oxc_ast::ast::BindingPattern::BindingIdentifier(_) => {}
                nested => self.check_signature_pattern_initializers(nested),
            }
        }
    }

    fn check_variable_initializers(&mut self, declaration: &VariableDeclaration<'_>) {
        if declaration.declare || self.is_ambient() {
            // An ambient declaration declares a value rather than producing
            // one, so its missing initializer is not the `const` grammar error.
            let const_like = matches!(
                declaration.kind,
                VariableDeclarationKind::Const
                    | VariableDeclarationKind::Using
                    | VariableDeclarationKind::AwaitUsing
            );
            for declarator in &declaration.declarations {
                if let Some(initializer) = declarator.init.as_ref() {
                    self.check_ambient_initializer(
                        initializer,
                        const_like,
                        declarator.type_annotation.is_some(),
                    );
                } else if declarator.type_annotation.is_none()
                    && let oxc_ast::ast::BindingPattern::BindingIdentifier(identifier) = &declarator.id
                {
                    // `widenTypeForVariableLikeDeclaration`: nothing types an
                    // ambient variable written with neither, so it is an
                    // implicit `any`, reported under `noImplicitAny`.
                    let payload = format!("{}\0any", identifier.name.as_str());
                    self.push(Kind::Ts(7005), identifier.span, Some(&payload));
                }
            }
            return;
        }

        // tsc's `checkGrammarVariableDeclaration`: a pattern without an
        // initializer is TS1182 whatever its keyword, ahead of the `const` rule.
        for declarator in &declaration.declarations {
            if declarator.init.is_some() {
                continue;
            }
            if !matches!(declarator.id, oxc_ast::ast::BindingPattern::BindingIdentifier(_)) {
                self.push(Kind::Ts(1182), declarator.id.span(), None);
            } else if declaration.kind == VariableDeclarationKind::Const {
                self.push(Kind::ConstNotInitialized, declarator.id.span(), None);
            }
        }
    }

    /// tsc reports every property whose name was already written, so
    /// `{ a: 1, a: 2, a: 3 }` reports twice. A duplicate that involves a method
    /// or an accessor is a *different* diagnostic there (TS2300 for methods, a
    /// legal get/set pair for accessors), so only plain properties count.
    fn check_duplicate_properties(&mut self, object: &ObjectExpression<'_>) {
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        // Methods report in first-written order, so the spans live in a `Vec`
        // and the map only finds a name's slot.
        let mut methods: Vec<(String, Vec<Span>)> = Vec::new();
        let mut method_slots: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut accessors: std::collections::HashSet<String> = std::collections::HashSet::new();
        // The accessor kinds seen per name (1 = get, 2 = set): a second
        // accessor of a kind already seen is TS1118, after which tsc's
        // `checkGrammarObjectLiteralExpression` stops looking at the literal.
        let mut accessor_kinds: std::collections::HashMap<String, u8> =
            std::collections::HashMap::new();

        for property in &object.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                continue;
            };
            if property.kind != PropertyKind::Init {
                if let Some(name) = property_key_name(&property.key) {
                    let kind = if property.kind == PropertyKind::Get { 1 } else { 2 };
                    match accessor_kinds.get_mut(&name) {
                        Some(seen_kinds) if *seen_kinds == 3 || *seen_kinds == kind => {
                            self.push(Kind::Ts(1118), property.key.span(), None);
                            self.report_duplicate_accessors(object, &name);
                            return;
                        }
                        Some(seen_kinds) => *seen_kinds |= kind,
                        None => {
                            accessor_kinds.insert(name.clone(), kind);
                        }
                    }
                    if seen.contains(&name) || method_slots.contains_key(&name) {
                        self.push(
                            Kind::ObjectLiteralPropertyAndAccessor,
                            property.key.span(),
                            None,
                        );
                    }
                    accessors.insert(name);
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
                match method_slots.get(&name) {
                    Some(&slot) => methods[slot].1.push(property.span),
                    None => {
                        method_slots.insert(name.clone(), methods.len());
                        methods.push((name, vec![property.span]));
                    }
                }
                continue;
            }
            if seen.contains(&name) {
                self.push(Kind::DuplicateObjectLiteralProperty, property.span, None);
            } else {
                seen.insert(name);
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

    /// The binder's duplicate-symbol error for two accessors of one kind in an
    /// object literal: TS2300 at every accessor of that name and kind.
    fn report_duplicate_accessors(&mut self, object: &ObjectExpression<'_>, name: &str) {
        for kind in [PropertyKind::Get, PropertyKind::Set] {
            let spans: Vec<Span> = object
                .properties
                .iter()
                .filter_map(|property| match property {
                    ObjectPropertyKind::ObjectProperty(property)
                        if property.kind == kind
                            && property_key_name(&property.key).as_deref() == Some(name) =>
                    {
                        Some(property.key.span())
                    }
                    _ => None,
                })
                .collect();
            if spans.len() > 1 {
                for span in spans {
                    self.push(Kind::DuplicateMember, span, Some(name));
                }
            }
        }
    }

    /// A type-level signature has no body to infer a parameter from, so an
    /// unannotated one is `any` unless an (erroneous) initializer types it.
    /// `may_name_type` is tsc's `reportImplicitAny` test for a call signature,
    /// method signature or function type, whose bare parameter name may be a
    /// forgotten type (`(string) => void`); the checker resolves the name.
    fn check_implicit_any_signature_parameters(
        &mut self,
        parameters: &FormalParameters<'_>,
        may_name_type: bool,
    ) {
        for (index, parameter) in parameters.items.iter().enumerate() {
            if parameter.type_annotation.is_some() || parameter.initializer.is_some() {
                continue;
            }
            let binding = match &parameter.pattern {
                oxc_ast::ast::BindingPattern::BindingIdentifier(binding) => binding,
                pattern => {
                    self.report_implicit_any_binding_elements(pattern);
                    continue;
                }
            };
            if may_name_type {
                self.push(
                    Kind::NamedSignatureParameterWithoutType,
                    binding.span,
                    Some(&format!("{}\0arg{index}\0", binding.name)),
                );
            } else {
                self.push(
                    Kind::ImplicitAnySignatureParameter,
                    binding.span,
                    Some(binding.name.as_str()),
                );
            }
        }
        if may_name_type
            && let Some(rest) = parameters.rest.as_deref()
            && rest.type_annotation.is_none()
            && let oxc_ast::ast::BindingPattern::BindingIdentifier(binding) = &rest.rest.argument
        {
            self.push(
                Kind::NamedSignatureParameterWithoutType,
                rest.span,
                Some(&format!("{}\0arg{}\0[]", binding.name, parameters.items.len())),
            );
        }
    }

    /// tsc's `getTypeFromBindingPattern` with errors reported: an element
    /// with neither an initializer nor a pattern of its own is an implicit
    /// `any` (TS7031, which the checker keeps under `noImplicitAny`). An
    /// object rest, a computed name that is no literal, and an array pattern
    /// with nothing but a rest imply nothing to report.
    fn report_implicit_any_binding_elements(&mut self, pattern: &oxc_ast::ast::BindingPattern<'_>) {
        match pattern {
            oxc_ast::ast::BindingPattern::ObjectPattern(object) => {
                for property in &object.properties {
                    // A renamed element is left to the unused-renaming check.
                    let renamed = !property.shorthand
                        && matches!(property.value, oxc_ast::ast::BindingPattern::BindingIdentifier(_));
                    if renamed
                        || property.computed
                            && !matches!(property.key, PropertyKey::StringLiteral(_) | PropertyKey::NumericLiteral(_))
                    {
                        continue;
                    }
                    self.report_implicit_any_binding_element(&property.value);
                }
            }
            oxc_ast::ast::BindingPattern::ArrayPattern(array) => {
                if array.elements.is_empty() {
                    return;
                }
                for element in array.elements.iter().flatten() {
                    self.report_implicit_any_binding_element(element);
                }
                if let Some(rest) = &array.rest {
                    self.report_implicit_any_binding_element(&rest.argument);
                }
            }
            oxc_ast::ast::BindingPattern::BindingIdentifier(_) | oxc_ast::ast::BindingPattern::AssignmentPattern(_) => {}
        }
    }

    fn report_implicit_any_binding_element(&mut self, element: &oxc_ast::ast::BindingPattern<'_>) {
        match element {
            oxc_ast::ast::BindingPattern::BindingIdentifier(identifier) => {
                self.push(Kind::Ts(7031), identifier.span, Some(&format!("{}\0any", identifier.name)));
            }
            oxc_ast::ast::BindingPattern::AssignmentPattern(_) => {}
            nested => self.report_implicit_any_binding_elements(nested),
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum SiblingKind {
    Function,
    Method,
    Other,
}

/// One entry of a container's declaration list, as the overload checks see
/// it; everything that is not a function or method is `Other`, kept so that
/// adjacency is measured against the real next sibling.
struct OverloadSibling {
    kind: SiblingKind,
    name: Option<String>,
    name_span: Span,
    is_static: bool,
    has_body: bool,
    ambient: bool,
    exported: bool,
    accessibility: Option<oxc_ast::ast::TSAccessibility>,
    is_abstract: bool,
    optional: bool,
}

impl OverloadSibling {
    fn other() -> Self {
        Self {
            kind: SiblingKind::Other,
            name: None,
            name_span: Span::default(),
            is_static: false,
            has_body: false,
            ambient: false,
            exported: false,
            accessibility: None,
            is_abstract: false,
            optional: false,
        }
    }
}

struct MemberGroup {
    name: String,
    members: Vec<(Span, MemberKind)>,
}

fn add_group_member(
    groups: &mut Vec<MemberGroup>,
    slots: &mut std::collections::HashMap<(String, bool), usize>,
    name: String,
    is_static: bool,
    member: (Span, MemberKind),
) {
    match slots.get(&(name.clone(), is_static)) {
        Some(&slot) => groups[slot].members.push(member),
        None => {
            slots.insert((name.clone(), is_static), groups.len());
            groups.push(MemberGroup {
                name,
                members: vec![member],
            });
        }
    }
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
                            self.names.push(super::number_text::js_number_to_string(literal.value));
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

/// tsc's `isSideEffectFree`, which looks through parentheses only: an `as`
/// or `satisfies` operand counts as having effects.
fn is_side_effect_free(expression: &Expression<'_>) -> bool {
    match expression.without_parentheses() {
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
        | Expression::JSXElement(_) => true,
        Expression::ConditionalExpression(conditional) => {
            is_side_effect_free(&conditional.consequent)
                && is_side_effect_free(&conditional.alternate)
        }
        // tsc's binary expressions include the logical and comma operators.
        Expression::BinaryExpression(binary) => {
            is_side_effect_free(&binary.left) && is_side_effect_free(&binary.right)
        }
        Expression::LogicalExpression(logical) => {
            is_side_effect_free(&logical.left) && is_side_effect_free(&logical.right)
        }
        Expression::SequenceExpression(sequence) => sequence.expressions.iter().all(is_side_effect_free),
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
        PropertyKey::NumericLiteral(literal) => Some(super::number_text::js_number_to_string(literal.value)),
        PropertyKey::PrivateIdentifier(identifier) => Some(format!("#{}", identifier.name)),
        _ => None,
    }
}

impl<'a> Visit<'a> for GrammarCollector {
    fn visit_program(&mut self, program: &Program<'a>) {
        self.source_text = program.source_text.to_string();
        self.collect_top_level_constants(&program.body);
        self.check_member_kind_overrides(&program.body);
        self.check_circular_type_aliases(&program.body);
        self.check_top_level_enum_merges(&program.body);
        self.check_default_exports(&program.body);
        self.check_function_implementations(&program.body);
        oxc_ast_visit::walk::walk_program(self, program);
        let bound: std::collections::HashSet<&str> = self
            .binding_names
            .iter()
            .filter(|(_, span)| {
                !self
                    .type_declaration_name_spans
                    .contains(&(span.start, span.end))
            })
            .map(|(name, _)| name.as_str())
            .collect();
        let findings = std::mem::take(&mut self.diagnostics);
        self.diagnostics = findings
            .into_iter()
            .filter(|finding| {
                finding.kind != Kind::ComputedTypeMemberName
                    || finding
                        .name
                        .as_deref()
                        .and_then(|payload| payload.split('\0').next())
                        .is_some_and(|name| !bound.contains(name))
            })
            .collect();
    }

    fn visit_binding_identifier(&mut self, identifier: &oxc_ast::ast::BindingIdentifier<'a>) {
        self.binding_names.push((identifier.name.to_string(), identifier.span));
    }

    fn visit_ts_type_alias_declaration(
        &mut self,
        declaration: &oxc_ast::ast::TSTypeAliasDeclaration<'a>,
    ) {
        self.type_declaration_name_spans
            .insert((declaration.id.span.start, declaration.id.span.end));
        oxc_ast_visit::walk::walk_ts_type_alias_declaration(self, declaration);
    }

    fn visit_block_statement(&mut self, block: &oxc_ast::ast::BlockStatement<'a>) {
        self.nested_scope_depth += 1;
        oxc_ast_visit::walk::walk_block_statement(self, block);
        self.nested_scope_depth -= 1;
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
    // (tsc's `checkStrictModeDeleteExpression`, binder.go:1409), and tsgo
    // checks every file as strict-mode code. `delete o.p`,
    // the legal form, is checked by the checker's own operand rules.
    fn visit_unary_expression(&mut self, unary: &oxc_ast::ast::UnaryExpression<'a>) {
        if unary.operator == UnaryOperator::LogicalNot {
            self.check_truthiness(&unary.argument);
        }
        if unary.operator == oxc_syntax::operator::UnaryOperator::Delete
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
        self.nested_scope_depth += 1;
        self.ambient_depth += 1;
        oxc_ast_visit::walk::walk_ts_global_declaration(self, declaration);
        self.ambient_depth -= 1;
        self.nested_scope_depth -= 1;
    }

    fn visit_ts_module_declaration(&mut self, declaration: &TSModuleDeclaration<'a>) {
        let ambient = declaration.declare || self.is_ambient();
        self.nested_scope_depth += 1;
        self.this_container_depth += 1;
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
        self.this_container_depth -= 1;
        self.nested_scope_depth -= 1;
    }

    fn visit_class(&mut self, class: &Class<'a>) {
        self.check_class_members(class);
        oxc_ast_visit::walk::walk_class(self, class);
    }

    fn visit_ts_interface_declaration(
        &mut self,
        declaration: &oxc_ast::ast::TSInterfaceDeclaration<'a>,
    ) {
        self.type_declaration_name_spans
            .insert((declaration.id.span.start, declaration.id.span.end));
        self.check_interface_members(&declaration.body.body);
        self.check_computed_type_member_names(&declaration.body.body, false);
        oxc_ast_visit::walk::walk_ts_interface_declaration(self, declaration);
    }

    fn visit_ts_type_literal(&mut self, literal: &oxc_ast::ast::TSTypeLiteral<'a>) {
        self.check_interface_members(&literal.members);
        self.check_computed_type_member_names(&literal.members, true);
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

    fn visit_call_expression(&mut self, call: &oxc_ast::ast::CallExpression<'a>) {
        self.note_indirect_call(&call.callee);
        oxc_ast_visit::walk::walk_call_expression(self, call);
    }

    fn visit_tagged_template_expression(
        &mut self,
        expression: &oxc_ast::ast::TaggedTemplateExpression<'a>,
    ) {
        self.note_indirect_call(&expression.tag);
        oxc_ast_visit::walk::walk_tagged_template_expression(self, expression);
    }

    fn visit_sequence_expression(&mut self, sequence: &oxc_ast::ast::SequenceExpression<'a>) {
        // Every operand but the last has its value discarded.
        let indirect_call = self.indirect_call_sequences.contains(&sequence.span.start);
        for expression in sequence
            .expressions
            .iter()
            .take(sequence.expressions.len().saturating_sub(1))
        {
            if !indirect_call && is_side_effect_free(expression) {
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
            self.check_signature_parameters(&function.params);
        }
        self.function_async.push(function.r#async);
        oxc_ast_visit::walk::walk_function(self, function, flags);
        self.function_async.pop();
    }

    fn visit_arrow_function_expression(&mut self, arrow: &oxc_ast::ast::ArrowFunctionExpression<'a>) {
        if arrow.r#async {
            self.check_async_return_type(arrow.return_type.as_deref());
        }
        // `checkGrammarArrowFunction`: `<T>() => …` reads as a JSX tag in a
        // `.mts`/`.cts` file; the checker keeps it for those extensions.
        if let Some(type_parameters) = &arrow.type_parameters
            && let [parameter] = type_parameters.params.as_slice()
            && parameter.constraint.is_none()
            && !self.source_text[parameter.span.end as usize..type_parameters.span.end as usize].contains(',')
        {
            self.push(Kind::Ts(7060), parameter.span, None);
        }
        self.function_async.push(arrow.r#async);
        oxc_ast_visit::walk::walk_arrow_function_expression(self, arrow);
        self.function_async.pop();
    }

    /// tsc's `checkGrammarAwaitOrAwaitUsing`: `await` inside a function that
    /// is not `async` — TS1308.
    fn visit_await_expression(&mut self, expression: &oxc_ast::ast::AwaitExpression<'a>) {
        let keyword = Span::new(expression.span.start, expression.span.start + 5);
        if self.function_async.last() == Some(&false) {
            self.push(Kind::AwaitOutsideAsyncFunction, keyword, None);
        } else if self.in_top_level_context() {
            // `checkGrammarAwaitOrAwaitUsing` under a node module kind; the
            // checker keeps it for a CommonJS-format file.
            self.push(Kind::Ts(1309), keyword, None);
        }
        oxc_ast_visit::walk::walk_await_expression(self, expression);
    }

    fn visit_class_body(&mut self, body: &oxc_ast::ast::ClassBody<'a>) {
        self.this_container_depth += 1;
        oxc_ast_visit::walk::walk_class_body(self, body);
        self.this_container_depth -= 1;
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
        if method.kind == MethodDefinitionKind::Set && method.value.return_type.is_some() {
            self.push(Kind::SetAccessorReturnType, method.key.span(), None);
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
        self.check_signature_parameters(&signature.params);
        self.check_implicit_any_signature_parameters(&signature.params, true);
        oxc_ast_visit::walk::walk_ts_call_signature_declaration(self, signature);
    }

    fn visit_ts_construct_signature_declaration(
        &mut self,
        signature: &oxc_ast::ast::TSConstructSignatureDeclaration<'a>,
    ) {
        if signature.return_type.is_none() {
            self.push(Kind::ImplicitAnyConstructReturn, signature.span, None);
        }
        self.check_signature_parameters(&signature.params);
        self.check_implicit_any_signature_parameters(&signature.params, false);
        oxc_ast_visit::walk::walk_ts_construct_signature_declaration(self, signature);
    }

    fn visit_ts_function_type(&mut self, function: &oxc_ast::ast::TSFunctionType<'a>) {
        self.check_implicit_any_signature_parameters(&function.params, true);
        oxc_ast_visit::walk::walk_ts_function_type(self, function);
    }

    fn visit_ts_constructor_type(&mut self, constructor: &oxc_ast::ast::TSConstructorType<'a>) {
        self.check_implicit_any_signature_parameters(&constructor.params, false);
        oxc_ast_visit::walk::walk_ts_constructor_type(self, constructor);
    }

    fn visit_ts_method_signature(&mut self, method: &TSMethodSignature<'a>) {
        self.check_signature_parameters(&method.params);
        self.check_implicit_any_signature_parameters(&method.params, true);
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
        let accessors: Vec<AccessorRecord> = object
            .properties
            .iter()
            .filter_map(|property| match property {
                ObjectPropertyKind::ObjectProperty(property)
                    if matches!(property.kind, PropertyKind::Get | PropertyKind::Set) =>
                {
                    let Expression::FunctionExpression(function) = &property.value else {
                        return None;
                    };
                    let is_getter = property.kind == PropertyKind::Get;
                    Some(AccessorRecord {
                        key: property_key_name(&property.key)?,
                        display: accessor_display_name(&property.key, property.computed),
                        is_static: false,
                        is_getter,
                        annotated: if is_getter {
                            function.return_type.is_some()
                        } else {
                            setter_parameter_annotated(&function.params)
                        },
                        has_body: function.body.is_some(),
                        private: false,
                        span: property.key.span(),
                    })
                }
                _ => None,
            })
            .collect();
        self.check_untyped_setters(&accessors, false);
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
        if statement.r#await
            && self.in_top_level_context()
            && let Some(offset) = self.source_text[statement.span.start as usize..].find("await")
        {
            let start = statement.span.start + offset as u32;
            self.push(Kind::Ts(1309), Span::new(start, start + 5), None);
        }
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
        // The parser's nameless placeholder for a computed name (TS1164).
        TSEnumMemberName::Identifier(identifier) if identifier.name.is_empty() => None,
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

/// The constants tsc's `checkAmbientInitializer` accepts: a string, numeric,
/// bigint or boolean literal (a numeric or bigint one possibly negated) or a
/// literal enum reference. Whether a member access is enum-typed is the
/// checker's to know (`isInitializerSimpleLiteralEnumReference`); the walker
/// accepts every access of the enum-reference shape.
fn is_ambient_constant_initializer(expression: &Expression<'_>) -> bool {
    fn is_string_or_number(expression: &Expression<'_>) -> bool {
        match expression {
            Expression::StringLiteral(_) | Expression::NumericLiteral(_) => true,
            Expression::TemplateLiteral(template) => template.expressions.is_empty(),
            _ => false,
        }
    }
    fn is_entity_name(expression: &Expression<'_>) -> bool {
        match expression {
            Expression::Identifier(_) => true,
            Expression::StaticMemberExpression(member) => is_entity_name(&member.object),
            _ => false,
        }
    }
    match expression {
        Expression::BooleanLiteral(_) | Expression::BigIntLiteral(_) => true,
        Expression::UnaryExpression(unary) => {
            unary.operator == UnaryOperator::UnaryNegation
                && matches!(unary.argument, Expression::NumericLiteral(_) | Expression::BigIntLiteral(_))
        }
        Expression::StaticMemberExpression(_) => true,
        Expression::ComputedMemberExpression(member) => {
            is_string_or_number(&member.expression) && is_entity_name(&member.object)
        }
        other => is_string_or_number(other),
    }
}
