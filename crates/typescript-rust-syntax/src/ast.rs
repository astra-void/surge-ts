#[derive(Debug, Clone)]
pub struct ParsedSource {
    pub file_name: String,
    pub statements: Vec<ParsedStatement>,
    pub parser_errors: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum ParsedStatement {
    VariableDeclaration(ParsedVariableDeclaration),
    Assignment(ParsedAssignment),
    FunctionDeclaration(ParsedFunctionDeclaration),
    Call(ParsedCall),
    Expression(ParsedExpression),
    TypeAliasDeclaration(ParsedTypeAliasDeclaration),
    InterfaceDeclaration(ParsedInterfaceDeclaration),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsedVariableKind {
    Var,
    Let,
    Const,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedType {
    String,
    Number,
    Boolean,
    Undefined,
    Void,
    Any,
    Unknown,
    StringLiteral(String),
    NumberLiteral(String),
    BooleanLiteral(bool),
    Object(ParsedObjectType),
    Union(Vec<ParsedType>),
    Function(ParsedFunctionType),
    Named(ParsedNamedType),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedFunctionType {
    pub parameters: Vec<ParsedFunctionTypeParameter>,
    pub return_type: Box<ParsedType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedFunctionTypeParameter {
    pub name: Option<String>,
    pub name_span: Option<TextSpan>,
    pub ty: ParsedType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedNamedType {
    pub name: String,
    pub span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTypeAliasDeclaration {
    pub name: String,
    pub name_span: Option<TextSpan>,
    pub ty: ParsedType,
    pub type_span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedInterfaceDeclaration {
    pub name: String,
    pub name_span: Option<TextSpan>,
    pub members: Vec<ParsedInterfaceMember>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedInterfaceMember {
    pub name: String,
    pub name_span: Option<TextSpan>,
    pub optional: bool,
    pub ty: ParsedType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedObjectType {
    pub properties: Vec<ParsedObjectTypeProperty>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedObjectTypeProperty {
    pub name: String,
    pub name_span: Option<TextSpan>,
    pub ty: ParsedType,
    pub optional: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParsedExpression {
    StringLiteral(String),
    NumberLiteral(String),
    BooleanLiteral(bool),
    UndefinedLiteral,
    Identifier(String),
    ObjectLiteral(Vec<ParsedObjectProperty>),
    Unary {
        operator: ParsedUnaryOperator,
        operator_span: Option<TextSpan>,
        operand: Box<ParsedExpression>,
        operand_span: Option<TextSpan>,
    },
    Binary {
        left: Box<ParsedExpression>,
        left_span: Option<TextSpan>,
        operator: ParsedBinaryOperator,
        operator_span: Option<TextSpan>,
        right: Box<ParsedExpression>,
        right_span: Option<TextSpan>,
    },
    Logical {
        left: Box<ParsedExpression>,
        left_span: Option<TextSpan>,
        operator: ParsedLogicalOperator,
        operator_span: Option<TextSpan>,
        right: Box<ParsedExpression>,
        right_span: Option<TextSpan>,
    },
    Conditional {
        condition: Box<ParsedExpression>,
        condition_span: Option<TextSpan>,
        when_true: Box<ParsedExpression>,
        when_true_span: Option<TextSpan>,
        when_false: Box<ParsedExpression>,
        when_false_span: Option<TextSpan>,
    },
    PropertyAccess {
        object_name: String,
        object_span: Option<TextSpan>,
        property_name: String,
        property_span: Option<TextSpan>,
    },
    Call {
        callee_name: String,
        callee_span: Option<TextSpan>,
        arguments: Vec<ParsedCallArgument>,
    },
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedObjectProperty {
    pub name: String,
    pub value: ParsedExpression,
    pub span: Option<TextSpan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsedBinaryOperator {
    StrictEquals,
    StrictNotEquals,
    Equals,
    NotEquals,
    LessThan,
    LessThanEquals,
    GreaterThan,
    GreaterThanEquals,
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsedLogicalOperator {
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsedUnaryOperator {
    Not,
    Plus,
    Minus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextSpan {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone)]
pub struct ParsedVariableDeclaration {
    pub kind: ParsedVariableKind,
    pub name: String,
    pub name_span: Option<TextSpan>,
    pub declared_type: Option<ParsedType>,
    pub initializer: Option<ParsedExpression>,
    pub initializer_span: Option<TextSpan>,
}

#[derive(Debug, Clone)]
pub struct ParsedAssignment {
    pub target_name: String,
    pub target_span: Option<TextSpan>,
    pub value: ParsedExpression,
    pub value_span: Option<TextSpan>,
}

#[derive(Debug, Clone)]
pub struct ParsedFunctionDeclaration {
    pub name: String,
    pub name_span: Option<TextSpan>,
    pub parameters: Vec<ParsedFunctionParameter>,
    pub return_type: Option<ParsedType>,
    pub body: Vec<ParsedFunctionBodyStatement>,
}

#[derive(Debug, Clone)]
pub enum ParsedFunctionBodyStatement {
    VariableDeclaration(ParsedVariableDeclaration),
    Return(ParsedReturnStatement),
    Assignment(ParsedAssignment),
    Expression(ParsedExpression),
    Block(Vec<ParsedFunctionBodyStatement>),
    If(ParsedIfStatement),
    While(ParsedWhileStatement),
}

#[derive(Debug, Clone)]
pub struct ParsedReturnStatement {
    pub expression: Option<ParsedExpression>,
    pub expression_span: Option<TextSpan>,
}

#[derive(Debug, Clone)]
pub struct ParsedIfStatement {
    pub condition: ParsedExpression,
    pub condition_span: Option<TextSpan>,
    pub then_body: Vec<ParsedFunctionBodyStatement>,
    pub else_body: Vec<ParsedFunctionBodyStatement>,
}

#[derive(Debug, Clone)]
pub struct ParsedWhileStatement {
    pub condition: ParsedExpression,
    pub condition_span: Option<TextSpan>,
    pub body: Vec<ParsedFunctionBodyStatement>,
}

#[derive(Debug, Clone)]
pub struct ParsedFunctionParameter {
    pub name: String,
    pub name_span: Option<TextSpan>,
    pub declared_type: Option<ParsedType>,
}

#[derive(Debug, Clone)]
pub struct ParsedCall {
    pub callee_name: String,
    pub callee_span: Option<TextSpan>,
    pub arguments: Vec<ParsedCallArgument>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedCallArgument {
    pub expression: ParsedExpression,
    pub span: Option<TextSpan>,
}
