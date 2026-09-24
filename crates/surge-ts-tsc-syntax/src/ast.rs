//! The syntax tree: an arena of uniform nodes. A node keeps, by name, only the
//! children something reads back after it is built (`checkJSSyntax`, the JSX
//! tag matcher, the few parser decisions that look at a finished node); the
//! rest go to `children`/`lists` in factory argument order.

use crate::flags::NodeFlags;
use crate::kind::Kind;

pub type NodeId = u32;

#[derive(Clone, Default, Debug)]
pub struct NodeList {
    pub pos: usize,
    pub end: usize,
    pub nodes: Vec<NodeId>,
    /// `createMissingList`: the opening token of the list was not found.
    pub missing: bool,
}

impl NodeList {
    pub fn new(pos: usize, end: usize, nodes: Vec<NodeId>) -> Self {
        Self { pos, end, nodes, missing: false }
    }

}

#[derive(Clone, Default, Debug)]
pub struct Node {
    pub kind: Kind,
    pub pos: usize,
    pub end: usize,
    pub flags: NodeFlags,
    pub name: Option<NodeId>,
    pub ty: Option<NodeId>,
    pub body: Option<NodeId>,
    pub expression: Option<NodeId>,
    pub initializer: Option<NodeId>,
    /// `questionToken`, or a property's `postfixToken` (`?` or `!`).
    pub question_token: Option<NodeId>,
    pub import_clause: Option<NodeId>,
    pub modifiers: Option<NodeList>,
    pub type_parameters: Option<NodeList>,
    pub type_arguments: Option<NodeList>,
    pub parameters: Option<NodeList>,
    pub children: Vec<Option<NodeId>>,
    pub lists: Vec<Option<NodeList>>,
    /// An identifier's or literal's text.
    pub text: String,
    /// The operator, keyword or token kind a node records (a binary
    /// expression's operator, a heritage clause's `extends`/`implements`,
    /// a module declaration's keyword, a prefix/postfix operator).
    pub op: Kind,
    pub token_flags: crate::flags::TokenFlags,
    pub is_type_only: bool,
    pub is_export_equals: bool,
}

impl Node {
    pub fn new(kind: Kind) -> Self {
        Self { kind, ..Self::default() }
    }

    pub fn name(mut self, name: impl Into<Option<NodeId>>) -> Self {
        self.name = name.into();
        self
    }

    pub fn ty(mut self, ty: impl Into<Option<NodeId>>) -> Self {
        self.ty = ty.into();
        self
    }

    pub fn body(mut self, body: impl Into<Option<NodeId>>) -> Self {
        self.body = body.into();
        self
    }

    pub fn expression(mut self, expression: impl Into<Option<NodeId>>) -> Self {
        self.expression = expression.into();
        self
    }

    pub fn initializer(mut self, initializer: impl Into<Option<NodeId>>) -> Self {
        self.initializer = initializer.into();
        self
    }

    pub fn question(mut self, token: impl Into<Option<NodeId>>) -> Self {
        self.question_token = token.into();
        self
    }

    pub fn import_clause(mut self, clause: impl Into<Option<NodeId>>) -> Self {
        self.import_clause = clause.into();
        self
    }

    pub fn modifiers(mut self, modifiers: Option<NodeList>) -> Self {
        self.modifiers = modifiers;
        self
    }

    pub fn type_parameters(mut self, list: Option<NodeList>) -> Self {
        self.type_parameters = list;
        self
    }

    pub fn type_arguments(mut self, list: Option<NodeList>) -> Self {
        self.type_arguments = list;
        self
    }

    pub fn parameters(mut self, list: impl Into<Option<NodeList>>) -> Self {
        self.parameters = list.into();
        self
    }

    pub fn child(mut self, child: impl Into<Option<NodeId>>) -> Self {
        self.children.push(child.into());
        self
    }

    pub fn list(mut self, list: impl Into<Option<NodeList>>) -> Self {
        self.lists.push(list.into());
        self
    }

    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
    }

    pub fn op(mut self, op: Kind) -> Self {
        self.op = op;
        self
    }

    pub fn token_flags(mut self, flags: crate::flags::TokenFlags) -> Self {
        self.token_flags = flags;
        self
    }

    pub fn type_only(mut self, value: bool) -> Self {
        self.is_type_only = value;
        self
    }

    pub fn export_equals(mut self, value: bool) -> Self {
        self.is_export_equals = value;
        self
    }

    pub fn flags(mut self, flags: NodeFlags) -> Self {
        self.flags |= flags;
        self
    }

    pub fn modifier_nodes(&self) -> &[NodeId] {
        self.modifiers.as_ref().map_or(&[], |list| &list.nodes)
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum OperatorPrecedence {
    Invalid = -1,
    Comma = 0,
    Spread,
    Yield,
    Assignment,
    Conditional,
    LogicalOR,
    LogicalAND,
    BitwiseOR,
    BitwiseXOR,
    BitwiseAND,
    Equality,
    Relational,
    Shift,
    Additive,
    Multiplicative,
    Exponentiation,
    Unary,
    Update,
    LeftHandSide,
    OptionalChain,
    Member,
    Primary,
    Parentheses,
}

#[allow(non_upper_case_globals, dead_code)]
impl OperatorPrecedence {
    pub const Lowest: OperatorPrecedence = OperatorPrecedence::Comma;
    pub const Highest: OperatorPrecedence = OperatorPrecedence::Parentheses;
    pub const DisallowComma: OperatorPrecedence = OperatorPrecedence::Yield;
    pub const Coalesce: OperatorPrecedence = OperatorPrecedence::LogicalOR;
}

pub fn get_binary_operator_precedence(kind: Kind) -> OperatorPrecedence {
    use OperatorPrecedence as P;
    match kind {
        Kind::QuestionQuestionToken => P::Coalesce,
        Kind::BarBarToken => P::LogicalOR,
        Kind::AmpersandAmpersandToken => P::LogicalAND,
        Kind::BarToken => P::BitwiseOR,
        Kind::CaretToken => P::BitwiseXOR,
        Kind::AmpersandToken => P::BitwiseAND,
        Kind::EqualsEqualsToken
        | Kind::ExclamationEqualsToken
        | Kind::EqualsEqualsEqualsToken
        | Kind::ExclamationEqualsEqualsToken => P::Equality,
        Kind::LessThanToken
        | Kind::GreaterThanToken
        | Kind::LessThanEqualsToken
        | Kind::GreaterThanEqualsToken
        | Kind::InstanceOfKeyword
        | Kind::InKeyword
        | Kind::AsKeyword
        | Kind::SatisfiesKeyword => P::Relational,
        Kind::LessThanLessThanToken
        | Kind::GreaterThanGreaterThanToken
        | Kind::GreaterThanGreaterThanGreaterThanToken => P::Shift,
        Kind::PlusToken | Kind::MinusToken => P::Additive,
        Kind::AsteriskToken | Kind::SlashToken | Kind::PercentToken => P::Multiplicative,
        Kind::AsteriskAsteriskToken => P::Exponentiation,
        _ => P::Invalid,
    }
}

pub fn is_keyword(token: Kind) -> bool {
    Kind::FirstKeyword <= token && token <= Kind::LastKeyword
}



pub fn is_reserved_word(token: Kind) -> bool {
    Kind::FirstReservedWord <= token && token <= Kind::LastReservedWord
}

pub fn is_modifier_kind(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::AbstractKeyword
            | Kind::AccessorKeyword
            | Kind::AsyncKeyword
            | Kind::ConstKeyword
            | Kind::DeclareKeyword
            | Kind::DefaultKeyword
            | Kind::ExportKeyword
            | Kind::InKeyword
            | Kind::PrivateKeyword
            | Kind::ProtectedKeyword
            | Kind::PublicKeyword
            | Kind::ReadonlyKeyword
            | Kind::OutKeyword
            | Kind::OverrideKeyword
            | Kind::StaticKeyword
    )
}

pub fn is_parameter_property_modifier(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::PublicKeyword | Kind::PrivateKeyword | Kind::ProtectedKeyword | Kind::ReadonlyKeyword | Kind::OverrideKeyword
    )
}

pub fn is_class_member_modifier(token: Kind) -> bool {
    is_parameter_property_modifier(token)
        || token == Kind::StaticKeyword
        || token == Kind::OverrideKeyword
        || token == Kind::AccessorKeyword
}

/// `ModifierToFlag(kind) & ModifierFlagsJavaScript != 0`.
pub fn is_javascript_modifier(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::ExportKeyword | Kind::StaticKeyword | Kind::AccessorKeyword | Kind::AsyncKeyword | Kind::DefaultKeyword
    )
}

pub fn is_assignment_operator(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::EqualsToken
            | Kind::PlusEqualsToken
            | Kind::MinusEqualsToken
            | Kind::AsteriskAsteriskEqualsToken
            | Kind::AsteriskEqualsToken
            | Kind::SlashEqualsToken
            | Kind::PercentEqualsToken
            | Kind::AmpersandEqualsToken
            | Kind::BarEqualsToken
            | Kind::CaretEqualsToken
            | Kind::LessThanLessThanEqualsToken
            | Kind::GreaterThanGreaterThanGreaterThanEqualsToken
            | Kind::GreaterThanGreaterThanEqualsToken
            | Kind::BarBarEqualsToken
            | Kind::AmpersandAmpersandEqualsToken
            | Kind::QuestionQuestionEqualsToken
    )
}

pub fn is_left_hand_side_expression_kind(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::PropertyAccessExpression
            | Kind::ElementAccessExpression
            | Kind::NewExpression
            | Kind::CallExpression
            | Kind::JsxElement
            | Kind::JsxSelfClosingElement
            | Kind::JsxFragment
            | Kind::TaggedTemplateExpression
            | Kind::ArrayLiteralExpression
            | Kind::ParenthesizedExpression
            | Kind::ObjectLiteralExpression
            | Kind::ClassExpression
            | Kind::FunctionExpression
            | Kind::Identifier
            | Kind::PrivateIdentifier
            | Kind::RegularExpressionLiteral
            | Kind::NumericLiteral
            | Kind::BigIntLiteral
            | Kind::StringLiteral
            | Kind::NoSubstitutionTemplateLiteral
            | Kind::TemplateExpression
            | Kind::FalseKeyword
            | Kind::NullKeyword
            | Kind::ThisKeyword
            | Kind::TrueKeyword
            | Kind::SuperKeyword
            | Kind::NonNullExpression
            | Kind::ExpressionWithTypeArguments
            | Kind::MetaProperty
            | Kind::ImportKeyword
            | Kind::MissingDeclaration
    )
}

pub fn is_function_like_declaration_kind(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::FunctionDeclaration
            | Kind::MethodDeclaration
            | Kind::Constructor
            | Kind::GetAccessor
            | Kind::SetAccessor
            | Kind::FunctionExpression
            | Kind::ArrowFunction
    )
}

pub fn is_function_like_kind(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::MethodSignature
            | Kind::CallSignature
            | Kind::JSDocSignature
            | Kind::ConstructSignature
            | Kind::IndexSignature
            | Kind::FunctionType
            | Kind::ConstructorType
    ) || is_function_like_declaration_kind(kind)
}

pub fn can_have_illegal_decorators(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::PropertyAssignment
            | Kind::ShorthandPropertyAssignment
            | Kind::FunctionDeclaration
            | Kind::Constructor
            | Kind::IndexSignature
            | Kind::ClassStaticBlockDeclaration
            | Kind::MissingDeclaration
            | Kind::VariableStatement
            | Kind::InterfaceDeclaration
            | Kind::TypeAliasDeclaration
            | Kind::EnumDeclaration
            | Kind::ModuleDeclaration
            | Kind::ImportEqualsDeclaration
            | Kind::ImportDeclaration
            | Kind::JSImportDeclaration
            | Kind::NamespaceExportDeclaration
            | Kind::ExportDeclaration
            | Kind::ExportAssignment
    )
}

pub fn can_have_decorators(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Parameter
            | Kind::PropertyDeclaration
            | Kind::MethodDeclaration
            | Kind::GetAccessor
            | Kind::SetAccessor
            | Kind::ClassExpression
            | Kind::ClassDeclaration
    )
}
