//! TypeScript's names for the children this port's [`crate::ast::Node`]
//! stores positionally, in the order tsc's `forEachChild` visits them.
//!
//! [`child_properties`] covers the node-valued properties. The scalar
//! properties TypeScript's API reports are kept in other fields of the node:
//!
//! | Kind | TypeScript property | Field |
//! |---|---|---|
//! | `Identifier`, `PrivateIdentifier` | `text` (`escapedText` adds tsc's leading-`__` escape) | `text` |
//! | literals, template heads, middles and tails, `JsxText` | `text` | `text` |
//! | the same | `isUnterminated`, `hasExtendedUnicodeEscape` | `token_flags` (`Unterminated`, `ExtendedUnicodeEscape`) |
//! | `StringLiteral` | `singleQuote` | `token_flags` (`SingleQuote`) |
//! | `PrefixUnaryExpression`, `PostfixUnaryExpression`, `TypeOperator` | `operator` | `op` |
//! | `MetaProperty` | `keywordToken` | `op` |
//! | `HeritageClause`, `ImportAttributes` | `token` | `op` |
//! | `ImportClause` | `phaseModifier` | `op` (`Unknown` when there is none) |
//! | `ImportClause`, `ImportEqualsDeclaration`, `ImportSpecifier`, `ExportSpecifier`, `ExportDeclaration` | `isTypeOnly` | `is_type_only` |
//! | `ImportType` | `isTypeOf` | `is_type_only` |
//! | `ExportAssignment` | `isExportEquals` | `is_export_equals` |
//! | `ModuleDeclaration` | the `Namespace` and `GlobalAugmentation` node flags | `op` (`ModuleKeyword`, `NamespaceKeyword` or `GlobalKeyword`) |
//! | `VariableDeclarationList` | the `Let`, `Const`, `Using` and `AwaitUsing` node flags | `flags` |
//!
//! Not kept: a template literal's `rawText` and `JsxText`'s
//! `containsOnlyTriviaWhiteSpaces` (both follow from the source text), the
//! `multiLine` of blocks, array and object literals and import attributes, and
//! `NodeArray.hasTrailingComma`.

use crate::ast::{NodeId, NodeList};
use crate::kind::Kind;
use crate::tree::SyntaxTree;

/// Where a TypeScript property's value lives on this port's Node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Name,
    Type,
    Body,
    Expression,
    Initializer,
    /// `question_token`, unless it holds a `!`.
    QuestionToken,
    /// `question_token` when it holds a `!`. A property declaration, a method
    /// and an object literal member keep typescript-go's single postfix token
    /// there, which TypeScript splits by kind into `questionToken` and
    /// `exclamationToken`.
    ExclamationToken,
    ImportClause,
    Modifiers,
    TypeParameters,
    TypeArguments,
    Parameters,
    /// `children[i]`.
    Child(u8),
    /// `lists[i]`.
    List(u8),
}

impl Slot {
    const fn holds_list(self) -> bool {
        matches!(self, Slot::Modifiers | Slot::TypeParameters | Slot::TypeArguments | Slot::Parameters | Slot::List(_))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Property {
    /// TypeScript's property name, e.g. "expression", "arguments", "operatorToken".
    pub name: &'static str,
    pub slot: Slot,
    /// The property is a NodeArray (forEachChild hands it to `cbNodes`).
    pub list: bool,
}

impl Property {
    const fn new(name: &'static str, slot: Slot) -> Property {
        Property { name, slot, list: slot.holds_list() }
    }
}

/// A property's value: a node for a single-node slot, a list for a list slot.
#[derive(Clone, Copy, Debug)]
pub enum SlotValue<'a> {
    Node(Option<NodeId>),
    List(Option<&'a NodeList>),
}

impl SlotValue<'_> {
    /// The nodes the value holds, in order.
    pub fn nodes(&self) -> &[NodeId] {
        match self {
            SlotValue::Node(Some(id)) => std::slice::from_ref(id),
            SlotValue::List(Some(list)) => &list.nodes,
            SlotValue::Node(None) | SlotValue::List(None) => &[],
        }
    }
}

/// Reads `slot` off node `id`.
pub fn resolve(tree: &SyntaxTree, id: NodeId, slot: Slot) -> SlotValue<'_> {
    let node = tree.node(id);
    let is_exclamation = |token: &NodeId| tree.kind(*token) == Kind::ExclamationToken;
    match slot {
        Slot::Name => SlotValue::Node(node.name),
        Slot::Type => SlotValue::Node(node.ty),
        Slot::Body => SlotValue::Node(node.body),
        Slot::Expression => SlotValue::Node(node.expression),
        Slot::Initializer => SlotValue::Node(node.initializer),
        Slot::QuestionToken => SlotValue::Node(node.question_token.filter(|token| !is_exclamation(token))),
        Slot::ExclamationToken => SlotValue::Node(node.question_token.filter(is_exclamation)),
        Slot::ImportClause => SlotValue::Node(node.import_clause),
        Slot::Modifiers => SlotValue::List(node.modifiers.as_ref()),
        Slot::TypeParameters => SlotValue::List(node.type_parameters.as_ref()),
        Slot::TypeArguments => SlotValue::List(node.type_arguments.as_ref()),
        Slot::Parameters => SlotValue::List(node.parameters.as_ref()),
        Slot::Child(index) => SlotValue::Node(node.children.get(usize::from(index)).copied().flatten()),
        Slot::List(index) => SlotValue::List(node.lists.get(usize::from(index)).and_then(Option::as_ref)),
    }
}

macro_rules! properties {
    ($($name:literal: $slot:expr),* $(,)?) => {
        const { &[$(Property::new($name, $slot)),*] }
    };
}

/// The node-valued properties of a node of `kind`, in the order TypeScript's
/// `forEachChild` visits them. Empty for tokens and leaves, and for the kinds
/// this parser never builds (JSDoc comments and tags, and what only
/// typescript-go's emitter or JavaScript reparser makes).
pub fn child_properties(kind: Kind) -> &'static [Property] {
    use Slot::*;
    match kind {
        // `endOfFileToken` is the token `SyntaxTree::parse` appends.
        Kind::SourceFile => properties!["statements": List(0), "endOfFileToken": Child(0)],
        Kind::QualifiedName => properties!["left": Child(0), "right": Child(1)],
        Kind::ComputedPropertyName
        | Kind::Decorator
        | Kind::ParenthesizedExpression
        | Kind::DeleteExpression
        | Kind::TypeOfExpression
        | Kind::VoidExpression
        | Kind::AwaitExpression
        | Kind::SpreadElement
        | Kind::NonNullExpression
        | Kind::ExpressionStatement
        | Kind::ReturnStatement
        | Kind::ThrowStatement
        | Kind::ExternalModuleReference
        | Kind::JsxSpreadAttribute
        | Kind::SpreadAssignment => properties!["expression": Expression],
        // tsc visits `default` before `expression`, a constraint that did not
        // parse as a type.
        Kind::TypeParameter => properties![
            "modifiers": Modifiers,
            "name": Name,
            "constraint": Child(0),
            "default": Child(1),
            "expression": Expression,
        ],
        Kind::Parameter => properties![
            "modifiers": Modifiers,
            "dotDotDotToken": Child(0),
            "name": Name,
            "questionToken": QuestionToken,
            "type": Type,
            "initializer": Initializer,
        ],
        Kind::PropertySignature => properties![
            "modifiers": Modifiers,
            "name": Name,
            "questionToken": QuestionToken,
            "type": Type,
            "initializer": Initializer,
        ],
        Kind::PropertyDeclaration => properties![
            "modifiers": Modifiers,
            "name": Name,
            "questionToken": QuestionToken,
            "exclamationToken": ExclamationToken,
            "type": Type,
            "initializer": Initializer,
        ],
        Kind::MethodSignature => properties![
            "modifiers": Modifiers,
            "name": Name,
            "questionToken": QuestionToken,
            "typeParameters": TypeParameters,
            "parameters": Parameters,
            "type": Type,
        ],
        Kind::MethodDeclaration => properties![
            "modifiers": Modifiers,
            "asteriskToken": Child(0),
            "name": Name,
            "questionToken": QuestionToken,
            "exclamationToken": ExclamationToken,
            "typeParameters": TypeParameters,
            "parameters": Parameters,
            "type": Type,
            "body": Body,
        ],
        Kind::ClassStaticBlockDeclaration => properties!["modifiers": Modifiers, "body": Body],
        // tsc's forEachChild also visits `name`, which no parser gives a
        // constructor.
        Kind::Constructor => properties![
            "modifiers": Modifiers,
            "typeParameters": TypeParameters,
            "parameters": Parameters,
            "type": Type,
            "body": Body,
        ],
        Kind::GetAccessor | Kind::SetAccessor => properties![
            "modifiers": Modifiers,
            "name": Name,
            "typeParameters": TypeParameters,
            "parameters": Parameters,
            "type": Type,
            "body": Body,
        ],
        Kind::CallSignature | Kind::ConstructSignature => {
            properties!["typeParameters": TypeParameters, "parameters": Parameters, "type": Type]
        }
        // tsc's forEachChild also visits `typeParameters`, which no parser
        // gives an index signature.
        Kind::IndexSignature => properties!["modifiers": Modifiers, "parameters": Parameters, "type": Type],
        Kind::TypePredicate => {
            properties!["assertsModifier": Child(0), "parameterName": Child(1), "type": Type]
        }
        Kind::TypeReference => properties!["typeName": Name, "typeArguments": TypeArguments],
        // tsc's forEachChild also visits `modifiers`, which no parser gives a
        // function type.
        Kind::FunctionType => {
            properties!["typeParameters": TypeParameters, "parameters": Parameters, "type": Type]
        }
        Kind::ConstructorType => properties![
            "modifiers": Modifiers,
            "typeParameters": TypeParameters,
            "parameters": Parameters,
            "type": Type,
        ],
        Kind::TypeQuery => properties!["exprName": Child(0), "typeArguments": TypeArguments],
        Kind::TypeLiteral => properties!["members": List(0)],
        Kind::ArrayType => properties!["elementType": Type],
        Kind::TupleType
        | Kind::ObjectBindingPattern
        | Kind::ArrayBindingPattern
        | Kind::ArrayLiteralExpression
        | Kind::NamedImports
        | Kind::NamedExports
        | Kind::ImportAttributes => properties!["elements": List(0)],
        Kind::OptionalType
        | Kind::RestType
        | Kind::ParenthesizedType
        | Kind::TypeOperator
        | Kind::JSDocNullableType
        | Kind::JSDocNonNullableType
        | Kind::JSDocOptionalType
        | Kind::JSDocVariadicType => properties!["type": Type],
        Kind::UnionType | Kind::IntersectionType | Kind::HeritageClause => properties!["types": List(0)],
        Kind::ConditionalType => properties![
            "checkType": Child(0),
            "extendsType": Child(1),
            "trueType": Child(2),
            "falseType": Child(3),
        ],
        Kind::InferType => properties!["typeParameter": Child(0)],
        Kind::IndexedAccessType => properties!["objectType": Child(0), "indexType": Child(1)],
        Kind::MappedType => properties![
            "readonlyToken": Child(0),
            "typeParameter": Child(1),
            "nameType": Child(2),
            "questionToken": QuestionToken,
            "type": Type,
            "members": List(0),
        ],
        Kind::LiteralType => properties!["literal": Child(0)],
        Kind::NamedTupleMember => properties![
            "dotDotDotToken": Child(0),
            "name": Name,
            "questionToken": QuestionToken,
            "type": Type,
        ],
        Kind::TemplateLiteralType | Kind::TemplateExpression => {
            properties!["head": Child(0), "templateSpans": List(0)]
        }
        Kind::TemplateLiteralTypeSpan => properties!["type": Type, "literal": Child(0)],
        Kind::TemplateSpan => properties!["expression": Expression, "literal": Child(0)],
        Kind::ImportType => properties![
            "argument": Type,
            "attributes": Child(0),
            "qualifier": Child(1),
            "typeArguments": TypeArguments,
        ],
        // The hole in an array binding pattern (`[, a]`) is a BindingElement
        // with nothing set; TypeScript builds an OmittedExpression there.
        Kind::BindingElement => properties![
            "dotDotDotToken": Child(0),
            "propertyName": Child(1),
            "name": Name,
            "initializer": Initializer,
        ],
        Kind::ObjectLiteralExpression | Kind::JsxAttributes => properties!["properties": List(0)],
        Kind::PropertyAccessExpression => {
            properties!["expression": Expression, "questionDotToken": Child(0), "name": Name]
        }
        Kind::ElementAccessExpression => properties![
            "expression": Expression,
            "questionDotToken": Child(0),
            "argumentExpression": Child(1),
        ],
        Kind::CallExpression => properties![
            "expression": Expression,
            "questionDotToken": Child(0),
            "typeArguments": TypeArguments,
            "arguments": List(0),
        ],
        // `children[0]` is a `questionDotToken` slot a `new` expression never
        // fills (tsc's forEachChild visits the always-absent property too).
        Kind::NewExpression => {
            properties!["expression": Expression, "typeArguments": TypeArguments, "arguments": List(0)]
        }
        Kind::TaggedTemplateExpression => properties![
            "tag": Expression,
            "questionDotToken": Child(0),
            "typeArguments": TypeArguments,
            "template": Body,
        ],
        Kind::TypeAssertionExpression => properties!["type": Type, "expression": Expression],
        Kind::AsExpression | Kind::SatisfiesExpression => properties!["expression": Expression, "type": Type],
        Kind::FunctionDeclaration | Kind::FunctionExpression => properties![
            "modifiers": Modifiers,
            "asteriskToken": Child(0),
            "name": Name,
            "typeParameters": TypeParameters,
            "parameters": Parameters,
            "type": Type,
            "body": Body,
        ],
        Kind::ArrowFunction => properties![
            "modifiers": Modifiers,
            "typeParameters": TypeParameters,
            "parameters": Parameters,
            "type": Type,
            "equalsGreaterThanToken": Child(0),
            "body": Body,
        ],
        Kind::PrefixUnaryExpression | Kind::PostfixUnaryExpression => properties!["operand": Expression],
        // typescript-go's `modifiers` slot, filled only by its JavaScript
        // reparser, stays empty.
        Kind::BinaryExpression => properties!["left": Child(0), "operatorToken": Child(1), "right": Child(2)],
        Kind::ConditionalExpression => properties![
            "condition": Child(0),
            "questionToken": Child(1),
            "whenTrue": Child(2),
            "colonToken": Child(3),
            "whenFalse": Child(4),
        ],
        Kind::YieldExpression => properties!["asteriskToken": Child(0), "expression": Expression],
        Kind::ClassDeclaration | Kind::ClassExpression | Kind::InterfaceDeclaration => properties![
            "modifiers": Modifiers,
            "name": Name,
            "typeParameters": TypeParameters,
            "heritageClauses": List(0),
            "members": List(1),
        ],
        Kind::ExpressionWithTypeArguments => properties!["expression": Expression, "typeArguments": TypeArguments],
        Kind::MetaProperty | Kind::NamespaceImport | Kind::NamespaceExport => properties!["name": Name],
        Kind::Block | Kind::ModuleBlock | Kind::DefaultClause => properties!["statements": List(0)],
        Kind::VariableStatement => properties!["modifiers": Modifiers, "declarationList": Child(0)],
        Kind::IfStatement => {
            properties!["expression": Expression, "thenStatement": Child(0), "elseStatement": Child(1)]
        }
        Kind::DoStatement => properties!["statement": Child(0), "expression": Expression],
        Kind::WhileStatement | Kind::WithStatement => properties!["expression": Expression, "statement": Child(0)],
        Kind::ForStatement => properties![
            "initializer": Initializer,
            "condition": Child(0),
            "incrementor": Child(1),
            "statement": Child(2),
        ],
        // `children[0]` is an `awaitModifier` slot a for-in statement never
        // fills.
        Kind::ForInStatement => {
            properties!["initializer": Initializer, "expression": Expression, "statement": Child(1)]
        }
        Kind::ForOfStatement => properties![
            "awaitModifier": Child(0),
            "initializer": Initializer,
            "expression": Expression,
            "statement": Child(1),
        ],
        Kind::ContinueStatement | Kind::BreakStatement => properties!["label": Child(0)],
        Kind::SwitchStatement => properties!["expression": Expression, "caseBlock": Child(0)],
        Kind::LabeledStatement => properties!["label": Child(0), "statement": Child(1)],
        Kind::TryStatement => {
            properties!["tryBlock": Child(0), "catchClause": Child(1), "finallyBlock": Child(2)]
        }
        Kind::VariableDeclaration => properties![
            "name": Name,
            "exclamationToken": Child(0),
            "type": Type,
            "initializer": Initializer,
        ],
        Kind::VariableDeclarationList => properties!["declarations": List(0)],
        Kind::TypeAliasDeclaration => {
            properties!["modifiers": Modifiers, "name": Name, "typeParameters": TypeParameters, "type": Type]
        }
        Kind::EnumDeclaration => properties!["modifiers": Modifiers, "name": Name, "members": List(0)],
        Kind::EnumMember => properties!["name": Name, "initializer": Initializer],
        // The body of `namespace A.B` is a ModuleDeclaration whose modifiers
        // hold a zero-width `export` typescript-go synthesizes (flagged
        // `Reparsed`); TypeScript gives that declaration no modifiers and flags
        // it `NestedNamespace` instead.
        Kind::ModuleDeclaration => properties!["modifiers": Modifiers, "name": Name, "body": Body],
        Kind::CaseBlock => properties!["clauses": List(0)],
        Kind::NamespaceExportDeclaration => properties!["modifiers": Modifiers, "name": Name],
        Kind::ImportEqualsDeclaration => {
            properties!["modifiers": Modifiers, "name": Name, "moduleReference": Child(0)]
        }
        Kind::ImportDeclaration => properties![
            "modifiers": Modifiers,
            "importClause": ImportClause,
            "moduleSpecifier": Child(0),
            "attributes": Child(1),
        ],
        Kind::ImportClause => properties!["name": Name, "namedBindings": Child(0)],
        Kind::ImportSpecifier | Kind::ExportSpecifier => properties!["propertyName": Child(0), "name": Name],
        Kind::ExportAssignment => properties!["modifiers": Modifiers, "expression": Expression],
        Kind::ExportDeclaration => properties![
            "modifiers": Modifiers,
            "exportClause": Child(0),
            "moduleSpecifier": Child(1),
            "attributes": Child(2),
        ],
        Kind::MissingDeclaration => properties!["modifiers": Modifiers],
        Kind::JsxElement => {
            properties!["openingElement": Child(0), "children": List(0), "closingElement": Child(1)]
        }
        Kind::JsxFragment => {
            properties!["openingFragment": Child(0), "children": List(0), "closingFragment": Child(1)]
        }
        Kind::JsxSelfClosingElement | Kind::JsxOpeningElement => {
            properties!["tagName": Name, "typeArguments": TypeArguments, "attributes": Child(0)]
        }
        Kind::JsxClosingElement => properties!["tagName": Name],
        Kind::JsxAttribute => properties!["name": Name, "initializer": Initializer],
        Kind::JsxExpression => properties!["dotDotDotToken": Child(0), "expression": Expression],
        Kind::JsxNamespacedName => properties!["namespace": Expression, "name": Name],
        Kind::CaseClause => properties!["expression": Expression, "statements": List(0)],
        Kind::CatchClause => properties!["variableDeclaration": Child(0), "block": Child(1)],
        Kind::ImportAttribute => properties!["name": Name, "value": Child(0)],
        // typescript-go's `ty` slot stays empty on a property assignment and a
        // shorthand one.
        Kind::PropertyAssignment => properties![
            "modifiers": Modifiers,
            "name": Name,
            "questionToken": QuestionToken,
            "exclamationToken": ExclamationToken,
            "initializer": Initializer,
        ],
        Kind::ShorthandPropertyAssignment => properties![
            "modifiers": Modifiers,
            "name": Name,
            "questionToken": QuestionToken,
            "exclamationToken": ExclamationToken,
            "equalsToken": Child(0),
            "objectAssignmentInitializer": Initializer,
        ],
        _ => &[],
    }
}
