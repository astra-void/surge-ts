#[derive(Debug, Clone, PartialEq)]
pub struct ParsedSource {
    pub file_name: String,
    pub statements: Vec<ParsedStatement>,
    pub parser_errors: Vec<String>,
    pub is_module: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParsedStatement {
    VariableDeclaration(ParsedVariableDeclaration),
    Assignment(ParsedAssignment),
    FunctionDeclaration(ParsedFunctionDeclaration),
    Call(ParsedCall),
    Expression(ParsedExpression),
    TypeAliasDeclaration(ParsedTypeAliasDeclaration),
    InterfaceDeclaration(ParsedInterfaceDeclaration),
    ImportDeclaration(ParsedImportDeclaration),
    ExportDeclaration(ParsedExportDeclaration),
    DeclareModuleDeclaration(ParsedDeclareModuleDeclaration),
    UnsupportedDeclaration { span: Option<TextSpan> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedDeclareModuleDeclaration {
    pub module_specifier: String,
    pub module_specifier_span: Option<TextSpan>,
    pub statements: Vec<ParsedStatement>,
    pub span: Option<TextSpan>,
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
    Array(Box<ParsedType>),
    Tuple(Vec<ParsedType>),
    Union(Vec<ParsedType>),
    Function(ParsedFunctionType),
    Named(ParsedNamedType),
    TypeOf(ParsedTypeOfType),
    KeyOf(Box<ParsedType>),
    IndexedAccess(ParsedIndexedAccessType),
    Mapped(ParsedMappedType),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedMappedType {
    pub key_name: String,
    pub key_span: Option<TextSpan>,
    pub constraint: Box<ParsedType>,
    pub value_type: Box<ParsedType>,
    pub optional: bool,
    pub span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedIndexedAccessType {
    pub object_type: Box<ParsedType>,
    pub index_type: Box<ParsedType>,
    pub span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTypeOfType {
    pub name: String,
    pub name_span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTypeParameter {
    pub name: String,
    pub name_span: Option<TextSpan>,
    pub constraint: Option<ParsedType>,
    pub default_type: Option<ParsedType>,
    pub span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedFunctionType {
    pub parameters: Vec<ParsedFunctionTypeParameter>,
    pub return_type: Box<ParsedType>,
    pub type_parameters: Vec<ParsedTypeParameter>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedFunctionTypeParameter {
    pub name: Option<String>,
    pub name_span: Option<TextSpan>,
    pub ty: ParsedType,
    pub optional: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParsedBindingName {
    Identifier {
        name: String,
        span: Option<TextSpan>,
    },
    ObjectPattern(ParsedObjectBindingPattern),
    Unsupported {
        span: Option<TextSpan>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedObjectBindingPattern {
    pub elements: Vec<ParsedObjectBindingElement>,
    pub span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedObjectBindingElement {
    pub property_name: String,
    pub binding_name: ParsedBindingName,
    pub name_span: Option<TextSpan>,
    pub has_default: bool,
    pub span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedNamedType {
    pub name: String,
    pub span: Option<TextSpan>,
    pub type_arguments: Vec<ParsedType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTypeAliasDeclaration {
    pub is_declare: bool,
    pub name: String,
    pub name_span: Option<TextSpan>,
    pub type_parameters: Vec<ParsedTypeParameter>,
    pub ty: ParsedType,
    pub type_span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedInterfaceDeclaration {
    pub is_declare: bool,
    pub name: String,
    pub name_span: Option<TextSpan>,
    pub type_parameters: Vec<ParsedTypeParameter>,
    pub extends: Vec<ParsedNamedType>,
    pub members: Vec<ParsedInterfaceMember>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedInterfaceMember {
    pub name: String,
    pub name_span: Option<TextSpan>,
    pub optional: bool,
    pub ty: ParsedType,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedImportDeclaration {
    pub kind: ParsedImportKind,
    pub module_specifier: String,
    pub module_specifier_span: Option<TextSpan>,
    pub span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParsedImportKind {
    Named {
        is_type_only: bool,
        specifiers: Vec<ParsedImportSpecifier>,
    },
    DefaultAndNamed {
        local_name: String,
        name_span: Option<TextSpan>,
        is_type_only: bool,
        specifiers: Vec<ParsedImportSpecifier>,
    },
    Default {
        local_name: String,
        name_span: Option<TextSpan>,
    },
    Namespace {
        local_name: String,
        name_span: Option<TextSpan>,
        is_type_only: bool,
    },
    SideEffect,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedImportSpecifier {
    pub imported_name: String,
    pub local_name: String,
    pub name_span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParsedExportDeclaration {
    Statement {
        declaration: Box<ParsedStatement>,
        is_type_only: bool,
    },
    Named {
        is_type_only: bool,
        specifiers: Vec<ParsedExportSpecifier>,
        module_specifier: Option<String>,
        module_specifier_span: Option<TextSpan>,
        span: Option<TextSpan>,
    },
    Default {
        declaration: ParsedDefaultExportDeclaration,
        span: Option<TextSpan>,
    },
    All {
        module_specifier: String,
        module_specifier_span: Option<TextSpan>,
        span: Option<TextSpan>,
    },
    Namespace {
        exported_name: String,
        exported_name_span: Option<TextSpan>,
        module_specifier: String,
        module_specifier_span: Option<TextSpan>,
        span: Option<TextSpan>,
    },
    Empty {
        span: Option<TextSpan>,
    },
    Unsupported {
        span: Option<TextSpan>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedExportSpecifier {
    pub local_name: String,
    pub exported_name: String,
    pub name_span: Option<TextSpan>,
    pub is_type_only: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParsedDefaultExportDeclaration {
    Function(ParsedFunctionDeclaration),
    Class { span: Option<TextSpan> },
    Expression(ParsedExpression),
    Unsupported { span: Option<TextSpan> },
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
    Identifier {
        name: String,
        span: Option<TextSpan>,
    },
    ObjectLiteral {
        properties: Vec<ParsedObjectProperty>,
        span: Option<TextSpan>,
    },
    ArrayLiteral {
        elements: Vec<ParsedArrayElement>,
        span: Option<TextSpan>,
    },
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
        object: Box<ParsedExpression>,
        object_span: Option<TextSpan>,
        property_name: String,
        property_span: Option<TextSpan>,
    },
    IndexAccess {
        object_name: String,
        object_span: Option<TextSpan>,
        index: Box<ParsedExpression>,
        index_span: Option<TextSpan>,
    },
    Call {
        callee_name: String,
        callee_span: Option<TextSpan>,
        type_arguments: Vec<ParsedType>,
        arguments: Vec<ParsedCallArgument>,
    },
    New {
        callee: Box<ParsedExpression>,
        callee_span: Option<TextSpan>,
        type_arguments: Vec<ParsedType>,
        arguments: Vec<ParsedCallArgument>,
    },
    PropertyCall {
        object: Box<ParsedExpression>,
        object_span: Option<TextSpan>,
        property_name: String,
        property_span: Option<TextSpan>,
        call_span: Option<TextSpan>,
        type_arguments: Vec<ParsedType>,
        arguments: Vec<ParsedCallArgument>,
    },
    TypeAssertion {
        expression: Box<ParsedExpression>,
        expression_span: Option<TextSpan>,
        ty: ParsedType,
        type_span: Option<TextSpan>,
    },
    SatisfiesExpression {
        expression: Box<ParsedExpression>,
        target_type: ParsedType,
        span: Option<TextSpan>,
        target_span: Option<TextSpan>,
    },
    OptionalPropertyAccess {
        object: Box<ParsedExpression>,
        object_span: Option<TextSpan>,
        property_name: String,
        property_span: Option<TextSpan>,
    },
    OptionalPropertyCall {
        object: Box<ParsedExpression>,
        object_span: Option<TextSpan>,
        property_name: String,
        property_span: Option<TextSpan>,
        call_span: Option<TextSpan>,
        type_arguments: Vec<ParsedType>,
        arguments: Vec<ParsedCallArgument>,
    },
    OptionalIndexAccess {
        object: Box<ParsedExpression>,
        object_span: Option<TextSpan>,
        index: Box<ParsedExpression>,
        index_span: Option<TextSpan>,
    },
    OptionalCall {
        callee: Box<ParsedExpression>,
        callee_span: Option<TextSpan>,
        type_arguments: Vec<ParsedType>,
        arguments: Vec<ParsedCallArgument>,
    },
    NullishCoalescing {
        left: Box<ParsedExpression>,
        left_span: Option<TextSpan>,
        right: Box<ParsedExpression>,
        right_span: Option<TextSpan>,
    },
    NonNullAssertion {
        expression: Box<ParsedExpression>,
        span: Option<TextSpan>,
        /// Indicates if this assertion removes the `| undefined` component from an optional chain.
        /// If false, the optional chain wrapper's undefined short-circuit may still be kept.
        in_optional_chain: bool,
    },
    ConstAssertion {
        expression: Box<ParsedExpression>,
        span: Option<TextSpan>,
    },
    ArrowFunction(Box<ParsedArrowFunction>),
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedObjectProperty {
    pub name: String,
    pub name_span: Option<TextSpan>,
    pub value: ParsedExpression,
    pub value_span: Option<TextSpan>,
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

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedVariableDeclaration {
    pub is_declare: bool,
    pub kind: ParsedVariableKind,
    pub name: String,
    pub name_span: Option<TextSpan>,
    pub declared_type: Option<ParsedType>,
    pub initializer: Option<ParsedExpression>,
    pub initializer_span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedAssignment {
    pub target_name: String,
    pub target_span: Option<TextSpan>,
    pub value: ParsedExpression,
    pub value_span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedFunctionDeclaration {
    pub is_declare: bool,
    pub name: String,
    pub name_span: Option<TextSpan>,
    pub type_parameters: Vec<ParsedTypeParameter>,
    pub parameters: Vec<ParsedFunctionParameter>,
    pub return_type: Option<ParsedType>,
    pub body: Vec<ParsedFunctionBodyStatement>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParsedFunctionBodyStatement {
    VariableDeclaration(ParsedVariableDeclaration),
    Return(ParsedReturnStatement),
    Throw(ParsedThrowStatement),
    Assignment(ParsedAssignment),
    Expression(ParsedExpression),
    Block(Vec<ParsedFunctionBodyStatement>),
    If(ParsedIfStatement),
    While(ParsedWhileStatement),
    Switch(ParsedSwitchStatement),
    Try(ParsedTryStatement),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedThrowStatement {
    pub expression: ParsedExpression,
    pub expression_span: Option<TextSpan>,
    pub span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedSwitchCase {
    pub test: Option<ParsedExpression>,
    pub test_span: Option<TextSpan>,
    pub consequent: Vec<ParsedFunctionBodyStatement>,
    pub span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedSwitchStatement {
    pub discriminant: ParsedExpression,
    pub discriminant_span: Option<TextSpan>,
    pub cases: Vec<ParsedSwitchCase>,
    pub span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedTryStatement {
    pub block: Vec<ParsedFunctionBodyStatement>,
    pub handler: Option<ParsedCatchClause>,
    pub finalizer: Vec<ParsedFunctionBodyStatement>,
    pub span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedCatchClause {
    pub binding_name: Option<ParsedBindingName>,
    pub body: Vec<ParsedFunctionBodyStatement>,
    pub span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedReturnStatement {
    pub expression: Option<ParsedExpression>,
    pub expression_span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedIfStatement {
    pub condition: ParsedExpression,
    pub condition_span: Option<TextSpan>,
    pub then_body: Vec<ParsedFunctionBodyStatement>,
    pub else_body: Vec<ParsedFunctionBodyStatement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedWhileStatement {
    pub condition: ParsedExpression,
    pub condition_span: Option<TextSpan>,
    pub body: Vec<ParsedFunctionBodyStatement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedFunctionParameter {
    pub binding_name: ParsedBindingName,
    pub declared_type: Option<ParsedType>,
    pub initializer: Option<ParsedExpression>,
    pub initializer_span: Option<TextSpan>,
    pub optional: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedArrowFunction {
    pub type_parameters: Vec<ParsedTypeParameter>,
    pub parameters: Vec<ParsedFunctionParameter>,
    pub return_type: Option<ParsedType>,
    pub body: ParsedArrowFunctionBody,
    pub span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParsedArrowFunctionBody {
    Expression(Box<ParsedExpression>),
    Block(Vec<ParsedFunctionBodyStatement>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedCall {
    pub callee_name: String,
    pub callee_span: Option<TextSpan>,
    pub span: Option<TextSpan>,
    pub type_arguments: Vec<ParsedType>,
    pub arguments: Vec<ParsedCallArgument>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedCallArgument {
    pub expression: ParsedExpression,
    pub span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedArrayElement {
    pub expression: ParsedExpression,
    pub span: Option<TextSpan>,
}
