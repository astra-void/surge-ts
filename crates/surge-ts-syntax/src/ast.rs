/// A parse failure, kept in the shape tsc reports rather than flattened to a
/// rendered string. oxc already computes a TS code and a span for the
/// TypeScript-specific failures (`ts_error` in its `diagnostics` module), and
/// both are needed to report one the way tsc does.
#[derive(Debug, Clone, PartialEq)]
pub struct ParserError {
    /// The TypeScript error number when oxc classified the failure as one
    /// (`TS1172` -> `1172`); `None` for a generic parse failure.
    pub code: Option<u32>,
    /// oxc's own rendering, used when the code is unknown or uncatalogued.
    pub message: String,
    pub span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedSource {
    pub file_name: String,
    pub statements: Vec<ParsedStatement>,
    pub parser_errors: Vec<ParserError>,
    pub is_module: bool,
    /// Leading `/// <reference types="..." />` directives, in source order.
    pub reference_type_directives: Vec<ReferenceTypeDirective>,
    /// Every value- and type-position identifier name referenced anywhere in the
    /// module (including export specifiers, collected from the full oxc AST).
    /// Backs unused-import / unused-local diagnostics (TS6133): a top-level
    /// binding whose name never appears here and is not exported is unused.
    pub module_reads: Vec<String>,
    /// Byte ranges of lines suppressed by an `@ts-expect-error`/`@ts-ignore`
    /// directive on the preceding line. Diagnostics starting inside one are
    /// dropped, matching tsc.
    pub suppressed_ranges: Vec<TextSpan>,
    /// Module specifiers written as `import("...")` — type-position import
    /// types and dynamic import expressions — deduplicated in source order.
    /// They belong to the module graph exactly like declaration specifiers do,
    /// but the lossy `Parsed*` tree does not model either form.
    pub import_call_specifiers: Vec<String>,
    /// Grammar-level findings collected in one walk of the full oxc AST (see
    /// `parser::grammar`): a `const` with no initializer, a duplicate
    /// object-literal key, an overload group with no implementation, a member
    /// with no annotation. Empty for declaration and non-TypeScript files,
    /// which surge does not report on.
    pub grammar_diagnostics: Vec<ParsedGrammarDiagnostic>,
    /// For a `.json` file: the type of the value it holds, and the marker that
    /// this *is* a JSON module. Nothing in a JSON file is code, so it is never
    /// parsed as TypeScript and `statements` is empty; this carries its whole
    /// meaning. `None` for every other file. A `.json` file whose contents do
    /// not parse is still a module, with the degradation sentinel for a value.
    pub json_module_type: Option<ParsedType>,
}

/// One [`ParsedSource::grammar_diagnostics`] finding: what was wrong, where,
/// and the name the message needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedGrammarDiagnostic {
    pub kind: ParsedGrammarDiagnosticKind,
    pub span: TextSpan,
    /// The member/function name for the kinds whose message names one.
    pub name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsedGrammarDiagnosticKind {
    /// `const x;` outside an ambient context — TS1155.
    ConstNotInitialized,
    /// `delete someBinding` in strict-mode code — TS1102. Only a *direct*
    /// reference to a binding is a syntax error; `delete o.p` is the legal form.
    DeleteOnIdentifierInStrictMode,
    /// An enum member initializer naming a member declared after it — TS2651.
    EnumForwardReference,
    /// An enum member with no initializer after one whose value is not a
    /// number — TS1061.
    EnumMemberInitializerRequired,
    /// A `const enum` member initializer that is not a constant expression —
    /// TS2474.
    ConstEnumInitializerNotConstant,
    /// An ambient enum member initializer that is not a constant expression —
    /// TS1066.
    AmbientEnumInitializerNotConstant,
    /// A second property of the same name in one object literal — TS1117.
    DuplicateObjectLiteralProperty,
    /// An overload group with no implementation — TS2391.
    FunctionImplementationMissing,
    /// A class whose constructor signatures have no implementation — TS2390.
    ConstructorImplementationMissing,
    /// More than one `export default` in a module — TS2528.
    MultipleDefaultExports,
    /// A property with neither annotation nor initializer — TS7008, reported
    /// only under `noImplicitAny`.
    ImplicitAnyMember,
    /// A signature with neither a body nor a return type — TS7010, reported
    /// only under `noImplicitAny`.
    ImplicitAnyReturn,
    /// Two members of one class, interface, or object literal declaring the
    /// same name where neither is an overload of the other — TS2300.
    DuplicateMember,
    /// Two implementations of the same method — TS2393.
    DuplicateImplementation,
    /// Two constructor implementations — TS2392.
    MultipleConstructorImplementations,
    /// A derived class constructor with no `super` call — TS2377.
    MissingSuperCall,
    /// A comma operator whose left side is discarded and cannot have an
    /// effect — TS2695.
    UnusedCommaOperand,
    /// A tested expression whose syntax makes it always truthy — TS2872.
    AlwaysTruthyExpression,
    /// A tested expression whose syntax makes it always falsy — TS2873.
    AlwaysFalsyExpression,
    /// A `??` left operand whose syntax is never nullish — TS2869.
    NeverNullishCoalesceOperand,
    /// A `??` left operand whose syntax is always nullish — TS2871.
    AlwaysNullishCoalesceOperand,
    /// A class member modifier written after one it must precede — TS1029.
    /// `name` holds the two modifiers, the one that must come first separated
    /// from the other by a NUL.
    ModifierMustPrecede,
    /// An `async` function whose written return type is not `Promise<T>` —
    /// TS1064. `name` holds the written type, which the message wraps.
    AsyncReturnTypeNotPromise,
    /// A parameter written both optional and with a default — TS1015.
    OptionalParameterWithInitializer,
    /// A required parameter after an optional one — TS1016.
    RequiredParameterAfterOptional,
    /// An initializer in an ambient context — TS1039.
    AmbientInitializer,
    /// A `set` accessor whose parameter list is not exactly one — TS1049.
    SetAccessorParameterCount,
    /// A `set` accessor with a return type annotation — TS1095.
    SetAccessorReturnType,
    /// A `get` accessor whose body can complete without returning — TS2378.
    GetAccessorWithoutReturn,
    /// A `get` accessor less accessible than its `set` accessor — TS2808,
    /// reported on both.
    GetAccessorLessAccessible,
    /// A `get`/`set` pair of which only one is `abstract` — TS2676, reported on
    /// both.
    AccessorAbstractMismatch,
    /// One name written in an object literal as both an accessor and a plain
    /// property — TS1119.
    ObjectLiteralPropertyAndAccessor,
    /// An `abstract` method in a class that is not abstract — TS1244.
    AbstractMethodOutsideAbstractClass,
    /// An `abstract` property in a class that is not abstract — TS1253.
    AbstractPropertyOutsideAbstractClass,
    /// A parameter property on a constructor signature — TS2369.
    ParameterPropertyOutsideImplementation,
    /// A parameter default on a signature with no body — TS2371.
    ParameterInitializerOutsideImplementation,
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
    /// A module-scope write through a member or element access
    /// (`Component.displayName = "X"`, `registry[key] = value`). Checked with
    /// the same code a function body uses, over a scope stack rooted at the
    /// module's symbol table.
    MemberAssignment(Box<ParsedMemberAssignment>),
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
    /// A module-scope `if`. Its branches are function-body statements, the
    /// same lowering a function body uses, so the checker can decide whether a
    /// branch leaves the module (`if (isCancel(x)) process.exit(0)`) and narrow
    /// the statements that follow.
    If(Box<ParsedIfStatement>),
    /// A module-scope loop, block, `switch`, `try` or labelled statement,
    /// lowered as a function body lowers it so its statements are checked the
    /// same way.
    Block(Vec<ParsedFunctionBodyStatement>),
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

#[derive(Debug, PartialEq, Eq)]
pub enum ParsedType {
    String,
    Number,
    Boolean,
    BigInt,
    Symbol,
    Undefined,
    Void,
    Any,
    /// A type the source itself does not resolve (an unresolved name, or a
    /// name imported from a module that does not resolve). tsc models this as
    /// `errorType`; see [`surge_ts_types::Type::ErrorType`].
    ErrorType,
    Unknown,
    /// The genuine `unknown` keyword, kept distinct from [`ParsedType::Unknown`]
    /// (which doubles as surge's conservative degrade target for `intrinsic`
    /// and unparseable annotations). Only this
    /// variant lowers to [`Type::GenuineUnknown`], so the checker can emit
    /// `TS18046` on a genuinely-`unknown`-typed receiver without flagging a
    /// merely-degraded one.
    UnknownKeyword,
    Never,
    StringLiteral(String),
    NumberLiteral(String),
    BooleanLiteral(bool),
    Object(std::sync::Arc<ParsedObjectType>),
    Array(std::sync::Arc<ParsedType>),
    Tuple(std::sync::Arc<Vec<ParsedType>>),
    /// A tuple carrying a spread element (`[a, ...b]`, `[...a, ...b]`,
    /// `[infer head, ...infer tail]`). Kept distinct from [`ParsedType::Tuple`]
    /// because its length is only known once the spread operands resolve: a
    /// spread of a concrete tuple flattens into a fixed one, and anything else
    /// falls back to the length-less array lowering.
    VariadicTuple(std::sync::Arc<Vec<ParsedTupleElement>>),
    /// `readonly T[]` / `readonly [A, B]`. The operand is an array or tuple
    /// shape; the modifier survives so a readonly array is not assignable to a
    /// mutable one and the two are not identical.
    Readonly(std::sync::Arc<ParsedType>),
    Union(std::sync::Arc<Vec<ParsedType>>),
    Intersection(std::sync::Arc<Vec<ParsedType>>),
    Function(std::sync::Arc<ParsedFunctionType>),
    Named(std::sync::Arc<ParsedNamedType>),
    TypeOf(std::sync::Arc<ParsedTypeOfType>),
    KeyOf(std::sync::Arc<ParsedType>),
    IndexedAccess(std::sync::Arc<ParsedIndexedAccessType>),
    Mapped(std::sync::Arc<ParsedMappedType>),
    Conditional(std::sync::Arc<ParsedConditionalType>),
    TemplateLiteral(std::sync::Arc<ParsedTemplateLiteralType>),
    /// An `infer X` capture inside a conditional type's `extends` clause. Modelled
    /// so a conditional that uses it (e.g. React's `ComponentProps<T>`) survives
    /// parsing instead of degrading the whole conditional to `Unknown`.
    Infer(String),
    /// A type-predicate return annotation (`x is T`, `this is T`, `asserts x`,
    /// `asserts x is T`). Resolves to `boolean` in type position; the guard
    /// narrowing consumes the payload to narrow the tested argument.
    Predicate(std::sync::Arc<ParsedPredicateType>),
}

/// One element of a [`ParsedType::VariadicTuple`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedTupleElement {
    Fixed(ParsedType),
    /// `...T` — spreads every element of another tuple or array type.
    Rest(ParsedType),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedPredicateType {
    /// The tested parameter's name (`"this"` for a `this is T` predicate).
    pub parameter_name: String,
    /// `None` for a bare `asserts x` assertion with no type.
    pub ty: Option<ParsedType>,
    pub asserts: bool,
}

impl Clone for ParsedType {
    fn clone(&self) -> Self {
        crate::clone_census::record_parsed_type_clone(self.census_variant());
        match self {
            Self::String => Self::String,
            Self::Number => Self::Number,
            Self::Boolean => Self::Boolean,
            Self::BigInt => Self::BigInt,
            Self::Symbol => Self::Symbol,
            Self::Undefined => Self::Undefined,
            Self::Void => Self::Void,
            Self::Any => Self::Any,
            Self::Unknown => Self::Unknown,
            Self::ErrorType => Self::ErrorType,
            Self::UnknownKeyword => Self::UnknownKeyword,
            Self::Never => Self::Never,
            Self::StringLiteral(value) => Self::StringLiteral(value.clone()),
            Self::NumberLiteral(value) => Self::NumberLiteral(value.clone()),
            Self::BooleanLiteral(value) => Self::BooleanLiteral(*value),
            Self::Object(payload) => Self::Object(payload.clone()),
            Self::Array(payload) => Self::Array(payload.clone()),
            Self::Tuple(payload) => Self::Tuple(payload.clone()),
            Self::VariadicTuple(payload) => Self::VariadicTuple(payload.clone()),
            Self::Readonly(payload) => Self::Readonly(payload.clone()),
            Self::Union(payload) => Self::Union(payload.clone()),
            Self::Intersection(payload) => Self::Intersection(payload.clone()),
            Self::Function(payload) => Self::Function(payload.clone()),
            Self::Named(payload) => Self::Named(payload.clone()),
            Self::TypeOf(payload) => Self::TypeOf(payload.clone()),
            Self::KeyOf(payload) => Self::KeyOf(payload.clone()),
            Self::IndexedAccess(payload) => Self::IndexedAccess(payload.clone()),
            Self::Mapped(payload) => Self::Mapped(payload.clone()),
            Self::Conditional(payload) => Self::Conditional(payload.clone()),
            Self::TemplateLiteral(payload) => Self::TemplateLiteral(payload.clone()),
            Self::Infer(name) => Self::Infer(name.clone()),
            Self::Predicate(payload) => Self::Predicate(payload.clone()),
        }
    }
}

impl ParsedType {
    fn census_variant(&self) -> usize {
        match self {
            Self::String
            | Self::Number
            | Self::Boolean
            | Self::BigInt
            | Self::Symbol
            | Self::Undefined
            | Self::Void
            | Self::Any
            | Self::ErrorType
            | Self::Unknown
            | Self::UnknownKeyword
            | Self::Never
            | Self::BooleanLiteral(_) => 0,
            Self::StringLiteral(_) => 1,
            Self::NumberLiteral(_) => 2,
            Self::Object(_) => 3,
            Self::Array(_) | Self::KeyOf(_) | Self::Readonly(_) => 4,
            Self::Tuple(_)
            | Self::VariadicTuple(_)
            | Self::Union(_)
            | Self::Intersection(_) => 5,
            Self::Function(_) => 6,
            Self::Named(_) => 7,
            Self::TypeOf(_) => 8,
            Self::IndexedAccess(_) => 9,
            Self::Mapped(_) | Self::Conditional(_) => 10,
            Self::TemplateLiteral(_) | Self::Infer(_) | Self::Predicate(_) => 11,
        }
    }
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

/// A mapped type's optionality modifier. `-?` makes every mapped property
/// required even when the homomorphic source property is optional, so the
/// three states cannot collapse to a bool: `Keep` inherits the source's
/// optionality, `Add` forces optional, `Remove` forces required.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappedOptionality {
    Keep,
    Add,
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedMappedType {
    pub key_name: String,
    pub key_span: Option<TextSpan>,
    pub constraint: Box<ParsedType>,
    pub value_type: Box<ParsedType>,
    pub optional: MappedOptionality,
    /// The `readonly` modifier, with the same three states: `readonly` adds it,
    /// `-readonly` removes it, and no modifier keeps the source property's.
    pub readonly: MappedOptionality,
    /// The `as` clause (`[K in keyof T as Rename<K>]`): each key is mapped
    /// through it, `never` drops the key, a union of literals fans it out.
    pub name_type: Option<Box<ParsedType>>,
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
    /// `typeof import("spec")`: the module specifier the query reads the
    /// namespace value of. `name` then carries the rendered `import("spec")`
    /// and `members` the qualifier written after it.
    pub import_specifier: Option<String>,
    /// The instantiation expression's arguments: `typeof f<A, B>` binds the
    /// generic value's type parameters in type position, exactly as a call with
    /// explicit type arguments does. Empty for a plain `typeof f`. Dropping
    /// these left `ReturnType<typeof withTRPC<TRouter, TSSRContext>>` — the one
    /// member that degrades tRPC's exported client — at the sentinel.
    pub type_arguments: Vec<ParsedType>,
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
    /// `{ a }` rather than `{ a: a }`. Unused-binding reporting keys on it:
    /// tsc exempts an `_`-prefixed local only when it renames a property.
    pub shorthand: bool,
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
    /// The enum this alias stands for, when it was synthesized by lowering an
    /// `enum` declaration (both the enum's own alias and one per member). An
    /// enum type is nominal in tsc and displayed by the enum's name, which the
    /// literal-union body cannot express on its own.
    pub enum_name: Option<String>,
    /// Whether that `enum` was exported. tsc qualifies an exported enum's type
    /// as `import("<module>").Color` and names a file-local one bare.
    pub enum_exported: bool,
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
    /// See [`ParsedObjectType::number_index_type`].
    pub number_index_type: Option<ParsedType>,
    /// A bare call signature (`(value?: any): number`) on the interface, making
    /// values of this type callable without `new` (e.g. `NumberConstructor`).
    pub call_signature: Option<ParsedFunctionType>,
    /// Construct signatures (`new <T>(executor): Promise<T>`) on the interface,
    /// making values of this type usable with `new` (e.g. `PromiseConstructor`,
    /// `SetConstructor`). One entry per overload; the resolver merges them into a
    /// single permissive signature. Each carries its own `type_parameters`.
    pub construct_signatures: Vec<ParsedFunctionType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedInterfaceMember {
    pub name: String,
    pub name_span: Option<TextSpan>,
    pub optional: bool,
    pub is_abstract: bool,
    /// Declared with method syntax (`m(): T`). tsc checks such a member's
    /// parameters bivariantly even under `strictFunctionTypes`.
    pub is_method: bool,
    pub ty: ParsedType,
    /// Declared `readonly`, or a getter with no matching setter — tsc's
    /// `isReadonlySymbol`, which turns a write into TS2540 before any
    /// assignability check runs.
    pub readonly: bool,
    /// What a *write* to this member is checked against, when that differs from
    /// `ty`. tsc: "Distinct write types come only from set accessors"
    /// (`getWriteTypeOfSymbol`), so this is the setter's parameter type of an
    /// accessor pair whose getter declares something else. `None` everywhere
    /// else, where reading and writing share a type.
    pub write_ty: Option<ParsedType>,
}

/// A `class` declaration. The instance side (fields + methods) is modelled as a
/// type; the constructor/static side is modelled as a value. Unsupported members
/// (getters/setters, index signatures, static blocks) are dropped during parsing
/// rather than causing a fatal parse error.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedClassDeclaration {
    pub is_declare: bool,
    /// `abstract class C {}`. A non-abstract class must implement every
    /// abstract member it inherits, and only an abstract class may declare one.
    pub is_abstract: bool,
    pub name: String,
    pub name_span: Option<TextSpan>,
    pub type_parameters: Vec<ParsedTypeParameter>,
    /// Base classes named in an `extends` clause. A class has at most one, but
    /// this is modelled as a list so the instance side can reuse the interface
    /// heritage-merge path. A non-identifier base (e.g. a mixin call) is dropped.
    pub extends: Vec<ParsedNamedType>,
    /// Interfaces named in an `implements` clause. They contribute nothing to
    /// the instance type — the class has to declare the members itself, which
    /// is what makes an unimplemented one reportable.
    pub implements: Vec<ParsedNamedType>,
    pub members: Vec<ParsedClassMember>,
    /// Value types of the instance side's string and number index signatures
    /// (`[key: string]: T`), as on [`ParsedInterfaceDeclaration`].
    pub string_index_type: Option<ParsedType>,
    pub number_index_type: Option<ParsedType>,
    pub span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParsedClassMember {
    Property(ParsedClassProperty),
    Method(ParsedClassMethod),
    Accessor(ParsedClassAccessor),
    Constructor(ParsedClassConstructor),
    /// `static { … }`: code run once with `this` bound to the class.
    StaticBlock(ParsedClassStaticBlock),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedClassStaticBlock {
    pub body: Vec<ParsedFunctionBodyStatement>,
    /// See [`ParsedFunctionDeclaration::body_reads`].
    pub body_reads: Vec<String>,
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
    pub is_override: bool,
    pub is_abstract: bool,
    pub getter_return_type: Option<ParsedType>,
    pub setter_param_type: Option<ParsedType>,
    pub has_getter: bool,
    pub has_setter: bool,
    /// The written `get`/`set` declarations this member merges, each with its
    /// own body to check.
    pub declarations: Vec<ParsedAccessorDeclaration>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedAccessorDeclaration {
    pub is_getter: bool,
    /// A setter's single parameter; empty for a getter.
    pub parameters: Vec<ParsedFunctionParameter>,
    pub return_type_span: Option<TextSpan>,
    pub body: Vec<ParsedFunctionBodyStatement>,
    /// See [`ParsedFunctionDeclaration::body_reads`].
    pub body_reads: Vec<String>,
    /// False for an abstract or ambient accessor.
    pub has_body: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedClassProperty {
    pub name: String,
    pub name_span: Option<TextSpan>,
    pub is_static: bool,
    pub is_override: bool,
    pub is_abstract: bool,
    /// `declare x: T` — the property is declared elsewhere, so it carries no
    /// initialization obligation of its own.
    pub is_declare: bool,
    /// `x!: T` — asserted to be initialized outside the constructor, which is
    /// what exempts it from the initialization check.
    pub has_definite_assertion: bool,
    pub optional: bool,
    pub readonly: bool,
    pub declared_type: Option<ParsedType>,
    pub initializer: Option<ParsedExpression>,
    pub initializer_span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedClassMethod {
    /// See [`ParsedFunctionDeclaration::body_reads`].
    pub body_reads: Vec<String>,
    pub name: String,
    pub name_span: Option<TextSpan>,
    pub is_static: bool,
    pub is_override: bool,
    pub is_abstract: bool,
    pub type_parameters: Vec<ParsedTypeParameter>,
    pub parameters: Vec<ParsedFunctionParameter>,
    pub return_type: Option<ParsedType>,
    /// See [`ParsedFunctionDeclaration::return_type_span`]. tsc reports a
    /// missing return on the written return type, falling back to the name.
    pub return_type_span: Option<TextSpan>,
    pub body: Vec<ParsedFunctionBodyStatement>,
    /// See [`ParsedFunctionDeclaration::has_body`].
    pub has_body: bool,
    /// See [`ParsedFunctionDeclaration::is_generator`].
    pub is_generator: bool,
    pub is_async: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedClassConstructor {
    /// See [`ParsedFunctionDeclaration::body_reads`].
    pub body_reads: Vec<String>,
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
    /// `import type Foo from "specifier"` — a default import that binds only in
    /// type space. Kept distinct from `Default` so the value side stays unbound
    /// while the name still counts as declared (shadowing, unused-locals).
    TypeOnlyDefault {
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
        /// `export type * from "x"` re-exports the target's TYPES only. Binding
        /// its values too would invent names the module does not have, masking
        /// the TS2304 a consumer should get.
        is_type_only: bool,
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
    /// `export as namespace Foo` — the UMD global declaration. Names the global
    /// this module is exposed under when loaded via a script tag.
    NamespaceExport {
        exported_name: String,
        exported_name_span: Option<TextSpan>,
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
    /// A string/number index signature (`[k: string]: T`).
    pub string_index_type: Option<Box<ParsedType>>,
    /// `[key: number]: T`. Kept apart from the string index: a numeric key
    /// prefers it, and a string key a number-only type cannot answer is an
    /// implicit `any` rather than a resolved member.
    pub number_index_type: Option<Box<ParsedType>>,
    /// A bare call signature (`(value?: any): number`) on the object type,
    /// making values of this type callable without `new`.
    pub call_signature: Option<Box<ParsedFunctionType>>,
    /// Every call signature as written, in source order, kept only when the
    /// type literal declares more than one. `call_signature` above stays the
    /// permissive fold every other consumer reads; this list exists for
    /// `infer` capture binding, which must pair the pattern's signatures with
    /// the check type's overloads one for one — the fold widens a slot the
    /// overloads disagree on, and an `infer` capture in that slot is exactly
    /// what gets widened away.
    pub call_signature_overloads: Vec<ParsedFunctionType>,
    /// A construct signature. Carries the lowering of a constructor *type*
    /// (`new (args) => T`, `abstract new (args) => T`), which is an object type
    /// with only this signature — modelled distinctly from a call signature so
    /// a plain function does not satisfy `T extends new (…) => …`.
    pub construct_signature: Option<Box<ParsedFunctionType>>,
    /// The `object` keyword: every non-primitive. Its member surface is the
    /// empty object, but a primitive does not satisfy it, which `{}` cannot say.
    pub non_primitive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedObjectTypeProperty {
    pub name: String,
    pub name_span: Option<TextSpan>,
    pub ty: ParsedType,
    pub optional: bool,
    /// See [`ParsedInterfaceMember::is_method`].
    pub is_method: bool,
    /// See [`ParsedInterfaceMember::readonly`].
    pub readonly: bool,
    /// See [`ParsedInterfaceMember::write_ty`].
    pub write_ty: Option<ParsedType>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParsedExpression {
    StringLiteral(String),
    NumberLiteral(String),
    /// `10n`. Typed as `bigint`: surge has no bigint literal type.
    BigIntLiteral(String),
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
    /// A template literal (`` `a${x}b` ``).
    TemplateLiteral {
        expressions: Vec<ParsedExpression>,
        span: Option<TextSpan>,
        /// The cooked text around the interpolations (`None` for an invalid
        /// escape), one more than `expressions` for an untagged template.
        quasis: Vec<Option<String>>,
        /// Where each interpolation is written, in `expressions` order.
        expression_spans: Vec<Option<TextSpan>>,
        /// A tagged template lowers here too, with the tag as the first
        /// expression; its type is the tag call's, not a string.
        is_tagged: bool,
    },
    Unary {
        operator: ParsedUnaryOperator,
        operator_span: Option<TextSpan>,
        operand: Box<ParsedExpression>,
        operand_span: Option<TextSpan>,
    },
    /// `x++` / `x--` / `++x` / `--x`. All four forms check their operand the
    /// same way and produce the same result type, so neither the operator nor
    /// its position is recorded.
    Update {
        operand: Box<ParsedExpression>,
        operand_span: Option<TextSpan>,
    },
    /// `await x`. The operand is kept so the checker can unwrap the awaited
    /// type; erasing the `await` at parse time left every `await` expression
    /// typed as the promise itself.
    Await {
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
        /// See [`ParsedIfStatement::unreferenced_truthiness_tests`]; the guarded
        /// code is `when_true`.
        truthiness_tests: Vec<ParsedTruthinessTest>,
    },
    PropertyAccess {
        object: Box<ParsedExpression>,
        object_span: Option<TextSpan>,
        property_name: String,
        property_span: Option<TextSpan>,
        /// True when the source wrote `obj["key"]` (lowered here to a property
        /// access for lookup reuse) rather than `obj.key`. Only dotted accesses
        /// are subject to TS4111 (`noPropertyAccessFromIndexSignature`).
        is_bracketed: bool,
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
        /// The whole `new C(…)` expression, which is what tsc underlines for a
        /// diagnostic about the instantiation itself rather than the callee.
        span: Option<TextSpan>,
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
        /// See [`ParsedExpression::PropertyAccess::is_bracketed`].
        is_bracketed: bool,
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
    /// A call whose callee is neither a bare identifier nor a static member —
    /// an IIFE (`(() => { … })()`), a call on a call, a parenthesized
    /// expression. Without it the whole call (and everything written inside the
    /// callee) parsed to `Unknown` and was never checked.
    ExpressionCall {
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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ParsedJsxAttributeValueKind {
    /// The value is the `value` expression (`name={expr}`), an empty container
    /// (`name={}`), or a `{...spread}` argument.
    #[default]
    Expression,
    /// A string-literal value, `name="literal"` — typed as the string literal
    /// (tsc keeps the literal, so `type="submit"` satisfies a literal union).
    StringLiteral(String),
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
    /// True for shorthand (`{ value }`), where the property name is also the
    /// identifier read. tsc reports an unresolved shorthand name as TS18004
    /// rather than as a plain missing name.
    pub is_shorthand: bool,
    /// True for a `get`/`set` accessor (`{ get value() { … } }`). The `value` is
    /// lowered to an arrow like method shorthand, but the property's type is the
    /// accessor's *value* type — the getter's return type, or the setter's
    /// parameter type — not the accessor function itself.
    pub is_accessor: bool,
    /// The key expression of a computed name that is not itself a literal
    /// (`{ [key]: v }`). `name` holds its written path; the checker names the
    /// property by the key's literal type once it is known.
    pub computed_key: Option<Box<ParsedExpression>>,
    /// On a getter, the `set` accessor of the same name: its body is checked,
    /// but the getter decides the property's type.
    pub paired_setter: Option<Box<ParsedArrowFunction>>,
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
    /// The `instanceof` operator (`x instanceof Ctor`). Evaluates to `boolean`;
    /// used as a type guard for narrowing a union by class membership.
    Instanceof,
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
    /// `~`, typed like unary `-`.
    BitwiseNot,
    Typeof,
    /// `delete o.p`: its operand must be a property reference, and that
    /// property must be optional and writable.
    Delete,
    Void,
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
    /// Whether this binding came from a destructuring pattern rather than a
    /// plain `const x = …`. tsc exempts an `_`-prefixed *destructured* binding
    /// from `noUnusedLocals` (the idiom for dropping properties out of a rest
    /// spread) but not a plain one, so the two must stay distinguishable after
    /// the pattern is flattened into one declaration per binding.
    pub from_binding_pattern: bool,
    /// `let x!: T` — a definite-assignment assertion. The binding is asserted to
    /// be initialized elsewhere, so definite-assignment analysis skips it.
    pub has_definite_assertion: bool,
    /// The innermost array pattern this binding is an element of, whose source
    /// must be iterable (TS2488 is reported on the pattern).
    pub array_pattern_span: Option<TextSpan>,
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
    /// `function f(this: T)` — oxc keeps the `this` parameter out of the
    /// parameter list, so its presence has to be carried separately. Only the
    /// implicit-`this` check reads it; nothing about the signature depends on it.
    pub has_this_parameter: bool,
    /// The annotation of that `this` parameter, which types `this` in the body.
    pub this_parameter_type: Option<ParsedType>,
    /// All value-position identifier names read anywhere in the body (including
    /// nested functions, spreads, for-in, and object methods), collected from the
    /// full oxc AST during parsing. Backs unused-binding diagnostics (TS6133).
    ///
    /// Sorted and deduplicated: the unused-binding checks binary-search it, so a
    /// producer that builds this list by hand must preserve that order or those
    /// checks start missing reads (a false TS6133/TS6196).
    pub body_reads: Vec<String>,
    pub is_declare: bool,
    pub name: String,
    pub name_span: Option<TextSpan>,
    pub type_parameters: Vec<ParsedTypeParameter>,
    pub parameters: Vec<ParsedFunctionParameter>,
    pub return_type: Option<ParsedType>,
    pub return_type_span: Option<TextSpan>,
    pub body: Vec<ParsedFunctionBodyStatement>,
    /// False for an overload signature (no body block); its parameters are not
    /// subject to TS6133.
    pub has_body: bool,
    /// `function*` / `async function*`. A generator's declared return type
    /// describes what it *yields*, so tsc does not require it to `return` a
    /// value — the missing-return checks skip it.
    pub is_generator: bool,
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
    MemberAssignment(Box<ParsedMemberAssignment>),
    Expression(Box<ParsedExpression>),
    Block(Vec<ParsedFunctionBodyStatement>),
    /// A nested `function` declaration. Retained so identifier reads inside its
    /// body (e.g. a captured outer parameter) stay visible to use-tracking; the
    /// enclosing function's control-flow analysis treats it as inert.
    Function(Box<ParsedFunctionDeclaration>),
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
    /// A body-local `type` alias. Bound ahead of the statement loop so a
    /// forward reference from an earlier statement still resolves; inert for
    /// control flow.
    TypeAlias(Box<ParsedTypeAliasDeclaration>),
    /// A body-local `interface`. Bound and treated like
    /// [`ParsedFunctionBodyStatement::TypeAlias`].
    Interface(Box<ParsedInterfaceDeclaration>),
    /// A body-local `class`. Contributes both a type and a value binding; its
    /// member bodies are not separately checked.
    Class(Box<ParsedClassDeclaration>),
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

/// `o.p = v` / `o.a.b = v` — an assignment whose target is a member of
/// something other than `this`. The target keeps its full expression so the
/// checker can recover the reference path (`o` plus `["a", "b"]`) it narrows.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedMemberAssignment {
    pub target: ParsedExpression,
    pub target_span: Option<TextSpan>,
    pub value: ParsedExpression,
    pub value_span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedThisPropertyAssignment {
    pub property_name: String,
    pub property_span: Option<TextSpan>,
    /// The whole `this.<property>` target, which a mismatch is reported on.
    pub target_span: Option<TextSpan>,
    pub value: ParsedExpression,
    pub value_span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedReturnStatement {
    pub expression: Option<ParsedExpression>,
    pub expression_span: Option<TextSpan>,
    /// The whole statement, where tsc reports a returned value that does not
    /// fit the return type.
    pub span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedIfStatement {
    pub condition: ParsedExpression,
    pub condition_span: Option<TextSpan>,
    pub then_body: Vec<ParsedFunctionBodyStatement>,
    pub else_body: Vec<ParsedFunctionBodyStatement>,
    /// References the condition tests for truthiness that neither the then
    /// branch nor the rest of their `&&` chain mentions again. tsc reports such a
    /// reference when its type is a function (TS2774); whether it is used is a
    /// question about the source, so it is answered here.
    pub unreferenced_truthiness_tests: Vec<ParsedTruthinessTest>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedTruthinessTest {
    pub expression: ParsedExpression,
    pub span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedWhileStatement {
    pub condition: ParsedExpression,
    pub condition_span: Option<TextSpan>,
    pub body: Vec<ParsedFunctionBodyStatement>,
    /// The body runs before the condition can stop it — a lowered `do … while
    /// (c)`, or a `while (true)`. Assignments the body makes are definite
    /// afterwards, and the condition may read what the body assigned.
    pub runs_at_least_once: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedForOfStatement {
    pub binding_name: ParsedBindingName,
    pub iterable: ParsedExpression,
    pub iterable_span: Option<TextSpan>,
    pub body: Vec<ParsedFunctionBodyStatement>,
    /// `for (k in o)` rather than `for (k of o)`: the binding is the property
    /// key (always `string`), not the iterated element, and the right-hand side
    /// need not be iterable.
    pub keys_only: bool,
    /// `for await (x of xs)`, which may iterate an async iterable.
    pub is_await: bool,
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
    /// Constructor parameter property: the parameter carries an accessibility
    /// (`public`/`private`/`protected`) or `readonly` modifier, which declares a
    /// class instance member of the same name and type.
    pub is_parameter_property: bool,
    /// The parameter property was declared `readonly`, so the member it
    /// declares rejects writes (tsc's `isReadonlySymbol`).
    pub is_readonly_parameter_property: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedArrowFunction {
    /// What `this` means inside the body. A `function` expression and an
    /// object-literal method both lower to this shape, and neither inherits
    /// `this` the way a real arrow does.
    pub this_binding: ParsedThisBinding,
    /// See [`ParsedFunctionDeclaration::body_reads`].
    pub body_reads: Vec<String>,
    pub type_parameters: Vec<ParsedTypeParameter>,
    pub parameters: Vec<ParsedFunctionParameter>,
    pub return_type: Option<ParsedType>,
    pub return_type_span: Option<TextSpan>,
    pub is_async: bool,
    /// A `function*` / `async function*` expression lowered to this shape. Its
    /// return type is a `Generator`/`AsyncGenerator`, never the body's
    /// completion value — inferring the latter typed `async function* () {
    /// yield 'a' }` as `void`.
    pub is_generator: bool,
    pub body: ParsedArrowFunctionBody,
    /// The expression of an expression-bodied arrow, which tsc anchors a
    /// mismatched return on (`elaborateArrowFunction`).
    pub body_span: Option<TextSpan>,
    pub span: Option<TextSpan>,
}

/// Where a lowered function body's `this` comes from. An arrow does not bind
/// `this` at all, so it sees the enclosing function's; everything else lowered
/// to [`ParsedArrowFunction`] binds its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsedThisBinding {
    /// A real arrow: `this` is whatever the enclosing function bound, which is
    /// why `function f() { return () => this }` reports on the *arrow's* `this`.
    Inherited,
    /// An object-literal method or accessor, or a `function` expression that
    /// annotates `this`: the body has a `this` of its own.
    Own,
    /// A `function` expression with no `this` parameter: `this` has no type.
    ImplicitAny,
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
    /// `f(...xs)`. The count this contributes depends on the spread's own type,
    /// so a call carrying one has no statically known argument count.
    pub spread: bool,
    /// The argument without a spread's `...`.
    pub expression_span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedArrayElement {
    pub expression: ParsedExpression,
    pub span: Option<TextSpan>,
    /// `[...xs]`. The element stands for however many elements `xs` holds, and
    /// what it contributes is `xs`'s *element* type, not `xs` itself.
    pub spread: bool,
}

/// Census-only estimated owned-heap size of a parsed type tree, used by the
/// retained-memory instrumentation to attribute parsed-annotation retention.
/// Shallow-struct sizes plus owned string/vec heap; not allocator ground truth.
impl ParsedType {
    pub fn estimated_heap_bytes(&self) -> u64 {
        let own = std::mem::size_of::<ParsedType>() as u64;
        own + match self {
            ParsedType::StringLiteral(value) | ParsedType::NumberLiteral(value) => {
                value.capacity() as u64
            }
            ParsedType::Object(object) => {
                let mut bytes = 0u64;
                for property in &object.properties {
                    bytes += property.name.capacity() as u64
                        + std::mem::size_of::<ParsedObjectTypeProperty>() as u64
                        + property.ty.estimated_heap_bytes();
                }
                if let Some(call) = object.call_signature.as_ref() {
                    bytes += call.estimated_heap_bytes();
                }
                if let Some(construct) = object.construct_signature.as_ref() {
                    bytes += construct.estimated_heap_bytes();
                }
                bytes
            }
            ParsedType::Array(element) | ParsedType::KeyOf(element) => {
                element.estimated_heap_bytes()
            }
            ParsedType::Tuple(elements)
            | ParsedType::Union(elements)
            | ParsedType::Intersection(elements) => elements
                .iter()
                .map(ParsedType::estimated_heap_bytes)
                .sum::<u64>(),
            ParsedType::Function(function) => function.estimated_heap_bytes(),
            ParsedType::Named(named) => named.estimated_heap_bytes(),
            ParsedType::TypeOf(type_of) => {
                type_of.name.capacity() as u64
                    + type_of
                        .members
                        .iter()
                        .map(|member| {
                            member.capacity() as u64 + std::mem::size_of::<String>() as u64
                        })
                        .sum::<u64>()
            }
            ParsedType::IndexedAccess(indexed) => {
                indexed.object_type.estimated_heap_bytes()
                    + indexed.index_type.estimated_heap_bytes()
            }
            ParsedType::Mapped(mapped) => {
                mapped.key_name.capacity() as u64
                    + mapped.constraint.estimated_heap_bytes()
                    + mapped.value_type.estimated_heap_bytes()
            }
            ParsedType::Conditional(conditional) => {
                conditional.check_type.estimated_heap_bytes()
                    + conditional.extends_type.estimated_heap_bytes()
                    + conditional.true_type.estimated_heap_bytes()
                    + conditional.false_type.estimated_heap_bytes()
            }
            ParsedType::TemplateLiteral(template) => {
                template
                    .quasis
                    .iter()
                    .map(|quasi| quasi.capacity() as u64 + std::mem::size_of::<String>() as u64)
                    .sum::<u64>()
                    + template
                        .interpolations
                        .iter()
                        .map(ParsedType::estimated_heap_bytes)
                        .sum::<u64>()
            }
            ParsedType::Infer(name) => name.capacity() as u64,
            _ => 0,
        }
    }
}

impl ParsedFunctionType {
    pub fn estimated_heap_bytes(&self) -> u64 {
        let mut bytes = std::mem::size_of::<ParsedFunctionType>() as u64;
        for parameter in &self.parameters {
            bytes += std::mem::size_of::<ParsedFunctionTypeParameter>() as u64;
            bytes += parameter
                .name
                .as_ref()
                .map_or(0, |name| name.capacity() as u64);
            bytes += parameter.ty.estimated_heap_bytes();
        }
        bytes += self.return_type.estimated_heap_bytes();
        for parameter in &self.type_parameters {
            bytes += parameter.estimated_heap_bytes();
        }
        bytes
    }
}

impl ParsedTypeParameter {
    pub fn estimated_heap_bytes(&self) -> u64 {
        let mut bytes = (std::mem::size_of::<ParsedTypeParameter>() + self.name.capacity()) as u64;
        if let Some(constraint) = self.constraint.as_ref() {
            bytes += constraint.estimated_heap_bytes();
        }
        if let Some(default_type) = self.default_type.as_ref() {
            bytes += default_type.estimated_heap_bytes();
        }
        bytes
    }
}

impl ParsedNamedType {
    pub fn estimated_heap_bytes(&self) -> u64 {
        let mut bytes = (std::mem::size_of::<ParsedNamedType>() + self.name.capacity()) as u64;
        for argument in &self.type_arguments {
            bytes += argument.estimated_heap_bytes();
        }
        bytes
    }
}

impl ParsedExpression {
    /// Whether this expression is a later link of an optional chain, so a
    /// member access on it short-circuits with the chain instead of being a
    /// non-optional access on a possibly-`undefined` receiver: the `.c` of
    /// `a?.b.c` never runs when `a?.b` is `undefined`.
    pub fn continues_optional_chain(&self) -> bool {
        match self {
            ParsedExpression::OptionalPropertyAccess { .. }
            | ParsedExpression::OptionalIndexAccess { .. }
            | ParsedExpression::OptionalPropertyCall { .. }
            | ParsedExpression::OptionalCall { .. } => true,
            ParsedExpression::PropertyAccess { object, .. }
            | ParsedExpression::ElementAccess { object, .. }
            | ParsedExpression::PropertyCall { object, .. } => object.continues_optional_chain(),
            ParsedExpression::ExpressionCall { callee, .. } => callee.continues_optional_chain(),
            ParsedExpression::NonNullAssertion {
                in_optional_chain, ..
            } => *in_optional_chain,
            _ => false,
        }
    }
}
