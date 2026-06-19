#[derive(Debug, Clone, PartialEq)]
pub struct ParsedSource {
    pub file_name: String,
    pub statements: Vec<ParsedStatement>,
    pub parser_errors: Vec<String>,
    pub is_module: bool,
    /// Leading `/// <reference types="..." />` directives, in source order.
    pub reference_type_directives: Vec<ReferenceTypeDirective>,
}

/// A leading `/// <reference types="..." />` directive. Only the `types` form is
/// modeled; `path`/`lib` references are not collected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceTypeDirective {
    /// The referenced type-package specifier, e.g. `node` or `@scope/pkg`.
    pub value: String,
    /// Byte span of the specifier inside its quotes, used for TS2688 locations.
    pub value_span: TextSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParsedStatement {
    VariableDeclaration(Box<ParsedVariableDeclaration>),
    Assignment(Box<ParsedAssignment>),
    FunctionDeclaration(Box<ParsedFunctionDeclaration>),
    Call(Box<ParsedCall>),
    Expression(Box<ParsedExpression>),
    TypeAliasDeclaration(Box<ParsedTypeAliasDeclaration>),
    InterfaceDeclaration(Box<ParsedInterfaceDeclaration>),
    ClassDeclaration(Box<ParsedClassDeclaration>),
    ImportDeclaration(Box<ParsedImportDeclaration>),
    ExportDeclaration(Box<ParsedExportDeclaration>),
    DeclareModuleDeclaration(Box<ParsedDeclareModuleDeclaration>),
    /// An identifier-named namespace/module block such as `declare namespace JSX { ... }`.
    /// String-named `declare module "pkg"` blocks use [`ParsedDeclareModuleDeclaration`].
    NamespaceDeclaration(Box<ParsedNamespaceDeclaration>),
    UnsupportedDeclaration {
        span: Option<TextSpan>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedDeclareModuleDeclaration {
    pub module_specifier: String,
    pub module_specifier_span: Option<TextSpan>,
    pub statements: Vec<ParsedStatement>,
    pub span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedNamespaceDeclaration {
    /// The namespace identifier, e.g. `JSX`. Nested names (`A.B`) are joined with `.`.
    pub name: String,
    pub name_span: Option<TextSpan>,
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
    Never,
    StringLiteral(String),
    NumberLiteral(String),
    BooleanLiteral(bool),
    Object(ParsedObjectType),
    Array(Box<ParsedType>),
    Tuple(Vec<ParsedType>),
    Union(Vec<ParsedType>),
    Intersection(Vec<ParsedType>),
    Function(ParsedFunctionType),
    Named(ParsedNamedType),
    TypeOf(ParsedTypeOfType),
    KeyOf(Box<ParsedType>),
    IndexedAccess(ParsedIndexedAccessType),
    Mapped(ParsedMappedType),
    Conditional(ParsedConditionalType),
    TemplateLiteral(ParsedTemplateLiteralType),
}

/// A template literal type in type position, e.g. `` `/${Entity}/${Action}` ``.
///
/// `quasis` holds the literal string segments and always has exactly one more
/// element than `interpolations` (the head, the text between each
/// interpolation, and the tail). `interpolations[i]` sits between `quasis[i]`
/// and `quasis[i + 1]`. A template with no interpolations (`` `hello` ``) has a
/// single quasi and no interpolations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTemplateLiteralType {
    pub quasis: Vec<String>,
    pub interpolations: Vec<ParsedType>,
    pub span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedConditionalType {
    pub check_type: Box<ParsedType>,
    pub extends_type: Box<ParsedType>,
    pub true_type: Box<ParsedType>,
    pub false_type: Box<ParsedType>,
    pub span: Option<TextSpan>,
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
    /// Dotted member path following the base `name` for qualified queries such
    /// as `typeof NS.Root` (`members == ["Root"]`). Empty for a plain `typeof x`.
    pub members: Vec<String>,
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
    /// A `this: T` fake parameter. It carries typing metadata but is not a real
    /// call parameter, so it is excluded from arity and argument matching.
    pub is_this: bool,
    /// A `...rest: T[]` parameter. Its annotation is the array type; lowering
    /// stores the element type and marks the signature variadic.
    pub rest: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParsedBindingName {
    Identifier {
        name: String,
        span: Option<TextSpan>,
    },
    ObjectPattern(ParsedObjectBindingPattern),
    ArrayPattern(ParsedArrayBindingPattern),
    Unsupported {
        span: Option<TextSpan>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedArrayBindingPattern {
    /// Each position binds the corresponding element; `None` is an elision hole
    /// (`[, b]`). The element type is the source tuple element at that index, or
    /// the array element type for a non-tuple source.
    pub elements: Vec<Option<ParsedBindingName>>,
    /// The `...rest` binding of `[a, ...rest]`, if present.
    pub rest: Option<Box<ParsedBindingName>>,
    pub span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedObjectBindingPattern {
    pub elements: Vec<ParsedObjectBindingElement>,
    /// The `...rest` binding of an object destructuring pattern, if present.
    /// `{ a, ...rest }` binds `rest` to the remaining properties.
    pub rest: Option<Box<ParsedBindingName>>,
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
    /// Value type of a string/number index signature (`[key: string]: T`), if
    /// present. The key type is not modelled separately; both string and number
    /// index signatures map here.
    pub string_index_type: Option<ParsedType>,
    /// A bare call signature (`(value?: any): number`) on the interface, making
    /// values of this type callable without `new` (e.g. `NumberConstructor`).
    pub call_signature: Option<ParsedFunctionType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedInterfaceMember {
    pub name: String,
    pub name_span: Option<TextSpan>,
    pub optional: bool,
    pub ty: ParsedType,
}

/// A `class` declaration. The instance side (fields + methods) is modelled as a
/// type; the constructor/static side is modelled as a value. Unsupported members
/// (getters/setters, index signatures, static blocks) are dropped during parsing
/// rather than causing a fatal parse error.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedClassDeclaration {
    pub is_declare: bool,
    pub name: String,
    pub name_span: Option<TextSpan>,
    pub type_parameters: Vec<ParsedTypeParameter>,
    /// Base classes named in an `extends` clause. A class has at most one, but
    /// this is modelled as a list so the instance side can reuse the interface
    /// heritage-merge path. A non-identifier base (e.g. a mixin call) is dropped.
    pub extends: Vec<ParsedNamedType>,
    pub members: Vec<ParsedClassMember>,
    pub span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParsedClassMember {
    Property(ParsedClassProperty),
    Method(ParsedClassMethod),
    Accessor(ParsedClassAccessor),
    Constructor(ParsedClassConstructor),
}

/// A `get`/`set` accessor pair, collapsed into a single member keyed by name.
/// Either side may be absent (getter-only or setter-only). The instance side
/// lowers this to a property whose type is the getter return type when present,
/// otherwise the setter parameter type.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedClassAccessor {
    pub name: String,
    pub name_span: Option<TextSpan>,
    pub is_static: bool,
    pub getter_return_type: Option<ParsedType>,
    pub setter_param_type: Option<ParsedType>,
    pub has_getter: bool,
    pub has_setter: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedClassProperty {
    pub name: String,
    pub name_span: Option<TextSpan>,
    pub is_static: bool,
    pub optional: bool,
    pub readonly: bool,
    pub declared_type: Option<ParsedType>,
    pub initializer: Option<ParsedExpression>,
    pub initializer_span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedClassMethod {
    pub name: String,
    pub name_span: Option<TextSpan>,
    pub is_static: bool,
    pub type_parameters: Vec<ParsedTypeParameter>,
    pub parameters: Vec<ParsedFunctionParameter>,
    pub return_type: Option<ParsedType>,
    pub body: Vec<ParsedFunctionBodyStatement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedClassConstructor {
    pub parameters: Vec<ParsedFunctionParameter>,
    pub body: Vec<ParsedFunctionBodyStatement>,
    pub span: Option<TextSpan>,
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
    /// `import local = require("specifier")` — declaration-lite CommonJS import
    /// equals against a package/module entrypoint. Only the
    /// `require(...)` (external module reference) form is represented here;
    /// entity-name references (`import x = a.b`) remain `Unsupported`.
    Equals {
        local_name: String,
        name_span: Option<TextSpan>,
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
    /// `export = identifier` — declaration-lite CommonJS export assignment.
    /// Only a bare identifier target is represented here; any other expression
    /// target remains `Unsupported`.
    Equals {
        exported_name: String,
        exported_name_span: Option<TextSpan>,
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
    Class(ParsedClassDeclaration),
    Expression(ParsedExpression),
    Unsupported { span: Option<TextSpan> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedObjectType {
    pub properties: Vec<ParsedObjectTypeProperty>,
    /// A bare call signature (`(value?: any): number`) on the object type,
    /// making values of this type callable without `new`.
    pub call_signature: Option<Box<ParsedFunctionType>>,
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
    NullLiteral,
    Identifier {
        name: String,
        span: Option<TextSpan>,
    },
    /// The `this` keyword. Resolves to the enclosing class instance (in instance
    /// methods/constructors) or static side (in static methods); elsewhere it is
    /// typed conservatively rather than reported as an unresolved identifier.
    This {
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
    /// Element access on an arbitrary object expression (`expr[index]`), as
    /// opposed to [`IndexAccess`] whose object is a bare identifier. Currently
    /// produced when desugaring array destructuring of a non-identifier
    /// initializer (`const [a, b] = useState()` -> `b = useState()[1]`).
    ElementAccess {
        object: Box<ParsedExpression>,
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
    /// A JSX element such as `<div id="x">child</div>` or `<Button />`. Parsed in
    /// `.tsx` mode only; the checker types it conservatively (see [`ParsedJsxChild`]).
    JsxElement {
        /// The tag name exactly as written, e.g. `div`, `Button`, `UI.Button`.
        /// Used for diagnostics/debugging only.
        tag_name: String,
        tag_name_span: Option<TextSpan>,
        /// Set when the tag refers to a value that must resolve in scope: the head
        /// identifier of a component (`Button`) or member tag (`UI.Button`).
        /// `None` for intrinsic lowercase elements (`div`), which are not value
        /// references.
        component_name: Option<String>,
        component_span: Option<TextSpan>,
        attributes: Vec<ParsedJsxAttribute>,
        children: Vec<ParsedJsxChild>,
        span: Option<TextSpan>,
    },
    /// A JSX fragment, `<>...</>`.
    JsxFragment {
        children: Vec<ParsedJsxChild>,
        span: Option<TextSpan>,
    },
    ArrowFunction(Box<ParsedArrowFunction>),
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedJsxAttribute {
    /// Attribute name, e.g. `id`. Empty for a `{...spread}` attribute.
    pub name: String,
    pub name_span: Option<TextSpan>,
    /// The expression inside a `{...}` attribute value, a JSX element value, or the
    /// argument of a `{...spread}` attribute. `None` for string-literal values
    /// (`id="x"`) and boolean shorthand (`enabled`); their type is recovered from
    /// [`ParsedJsxAttribute::value_kind`] instead.
    pub value: Option<ParsedExpression>,
    pub value_span: Option<TextSpan>,
    /// Classifies values that carry no [`ParsedExpression`] so the checker can type
    /// `id="x"` as `string` and the `disabled` shorthand as `true`.
    pub value_kind: ParsedJsxAttributeValueKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParsedJsxAttributeValueKind {
    /// The value is the `value` expression (`name={expr}`), an empty container
    /// (`name={}`), or a `{...spread}` argument.
    #[default]
    Expression,
    /// A string-literal value, `name="literal"` — typed as `string`.
    StringLiteral,
    /// Boolean shorthand, `name` — equivalent to `name={true}`.
    BooleanShorthand,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParsedJsxChild {
    /// Plain text content; nothing to type-check.
    Text,
    /// A `{expression}` (or `{...spread}`) container child. An empty `{}` container
    /// carries `None`.
    Expression {
        expression: Option<ParsedExpression>,
        span: Option<TextSpan>,
    },
    /// A nested JSX element or fragment (itself a [`ParsedExpression`]).
    Element(ParsedExpression),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedObjectProperty {
    pub name: String,
    pub name_span: Option<TextSpan>,
    pub value: ParsedExpression,
    pub value_span: Option<TextSpan>,
    pub span: Option<TextSpan>,
    /// True when this property originates from method shorthand (`{ foo(arg): R { ... } }`).
    /// The `value` is lowered to an arrow function so it reuses arrow checking, but the
    /// declared parameter/return types must be honored when inferring the property type.
    pub is_method: bool,
    /// True for a spread element (`{ ...source }`). `name` is empty and `value`
    /// is the spread argument expression; inference merges the argument's own
    /// object properties into the result.
    pub is_spread: bool,
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
    Exponential,
    ShiftLeft,
    ShiftRight,
    ShiftRightZeroFill,
    BitwiseOR,
    BitwiseXOR,
    BitwiseAnd,
    /// The `in` operator (`"prop" in obj`). Evaluates to `boolean`; used as a
    /// property-presence type guard for narrowing.
    In,
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
    pub return_type_span: Option<TextSpan>,
    pub body: Vec<ParsedFunctionBodyStatement>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParsedFunctionBodyStatement {
    VariableDeclaration(Box<ParsedVariableDeclaration>),
    Return(Box<ParsedReturnStatement>),
    Throw(Box<ParsedThrowStatement>),
    Assignment(Box<ParsedAssignment>),
    /// A `this.<property> = <value>` assignment inside a class method or
    /// constructor body. Checked against the instance property's declared type.
    ThisPropertyAssignment(Box<ParsedThisPropertyAssignment>),
    Expression(Box<ParsedExpression>),
    Block(Vec<ParsedFunctionBodyStatement>),
    If(Box<ParsedIfStatement>),
    While(Box<ParsedWhileStatement>),
    ForOf(Box<ParsedForOfStatement>),
    Switch(Box<ParsedSwitchStatement>),
    Try(Box<ParsedTryStatement>),
    /// `continue;` — diverts straight-line flow back to the enclosing loop head.
    /// Carries no label/target; modelled only so flow analysis knows the branch
    /// does not fall through (enabling post-guard narrowing of `if (c) continue;`).
    Continue,
    /// `break;` — exits the enclosing loop/switch. Modelled for the same
    /// flow-divergence reason as [`ParsedFunctionBodyStatement::Continue`].
    Break,
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
    pub declared_type: Option<ParsedType>,
    pub body: Vec<ParsedFunctionBodyStatement>,
    pub span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedThisPropertyAssignment {
    pub property_name: String,
    pub property_span: Option<TextSpan>,
    pub value: ParsedExpression,
    pub value_span: Option<TextSpan>,
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
pub struct ParsedForOfStatement {
    pub binding_name: ParsedBindingName,
    pub iterable: ParsedExpression,
    pub iterable_span: Option<TextSpan>,
    pub body: Vec<ParsedFunctionBodyStatement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedFunctionParameter {
    pub binding_name: ParsedBindingName,
    pub declared_type: Option<ParsedType>,
    pub initializer: Option<ParsedExpression>,
    pub initializer_span: Option<TextSpan>,
    pub optional: bool,
    /// `...args` rest parameter. Marks the signature variadic so arity checks
    /// accept any number of trailing arguments.
    pub rest: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedArrowFunction {
    pub type_parameters: Vec<ParsedTypeParameter>,
    pub parameters: Vec<ParsedFunctionParameter>,
    pub return_type: Option<ParsedType>,
    pub is_async: bool,
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
