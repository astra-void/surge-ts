//! Generated diagnostic catalog. Do not edit by hand.

use crate::{
    Diagnostic, DiagnosticArg, DiagnosticCategory, DiagnosticDescriptor, DiagnosticSource,
    DiagnosticSupport,
};

pub const TS5112: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS5112",
    number: Some(5112),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "tsconfig.json is present but will not be loaded if files are specified on commandline. Use '--ignoreConfig' to skip this error.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS1360: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1360",
    number: Some(1360),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Type '{0}' does not satisfy the expected type '{1}'.",
    argument_count: 2,
    support: DiagnosticSupport::Emitted,
};

pub const TS2304: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2304",
    number: Some(2304),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Cannot find name '{0}'.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS2300: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2300",
    number: Some(2300),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Duplicate identifier '{0}'.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS2305: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2305",
    number: Some(2305),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Module '{0}' has no exported member '{1}'.",
    argument_count: 2,
    support: DiagnosticSupport::Emitted,
};

pub const TS2306: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2306",
    number: Some(2306),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "File '{0}' is not a module.",
    argument_count: 1,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS2307: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2307",
    number: Some(2307),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Cannot find module '{0}' or its corresponding type declarations.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS2882: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2882",
    number: Some(2882),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Cannot find module or type declarations for side-effect import of '{0}'.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS2314: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2314",
    number: Some(2314),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Generic type '{0}' requires {1} type argument(s).",
    argument_count: 2,
    support: DiagnosticSupport::Emitted,
};

pub const TS2315: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2315",
    number: Some(2315),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Type '{0}' is not generic.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS2322: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2322",
    number: Some(2322),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Type '{0}' is not assignable to type '{1}'.",
    argument_count: 2,
    support: DiagnosticSupport::Emitted,
};

pub const TS2339: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2339",
    number: Some(2339),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Property '{0}' does not exist on type '{1}'.",
    argument_count: 2,
    support: DiagnosticSupport::Emitted,
};

pub const TS2344: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2344",
    number: Some(2344),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Type '{0}' does not satisfy the constraint '{1}'.",
    argument_count: 2,
    support: DiagnosticSupport::Emitted,
};

pub const TS2345: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2345",
    number: Some(2345),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Argument of type '{0}' is not assignable to parameter of type '{1}'.",
    argument_count: 2,
    support: DiagnosticSupport::Emitted,
};

pub const TS2349: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2349",
    number: Some(2349),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "This expression is not callable.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS2351: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2351",
    number: Some(2351),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "This expression is not constructable.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS2352: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2352",
    number: Some(2352),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Conversion of type '{0}' to type '{1}' may be a mistake because neither type sufficiently overlaps with the other. If this was intentional, convert the expression to 'unknown' first.",
    argument_count: 2,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS2353: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2353",
    number: Some(2353),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Object literal may only specify known properties, and '{0}' does not exist in type '{1}'.",
    argument_count: 2,
    support: DiagnosticSupport::Emitted,
};

pub const TS2355: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2355",
    number: Some(2355),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "A function whose declared type is neither 'undefined', 'void', nor 'any' must return a value.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS2356: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2356",
    number: Some(2356),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "An arithmetic operand must be of type 'any', 'number', 'bigint' or an enum type.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS2362: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2362",
    number: Some(2362),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "The left-hand side of an arithmetic operation must be of type 'any', 'number', 'bigint' or an enum type.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS2363: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2363",
    number: Some(2363),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "The right-hand side of an arithmetic operation must be of type 'any', 'number', 'bigint' or an enum type.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS2365: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2365",
    number: Some(2365),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Operator '{0}' cannot be applied to types '{1}' and '{2}'.",
    argument_count: 3,
    support: DiagnosticSupport::Emitted,
};

pub const TS2366: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2366",
    number: Some(2366),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Function lacks ending return statement and return type does not include 'undefined'.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS2367: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2367",
    number: Some(2367),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "This comparison appears to be unintentional because the types '{0}' and '{1}' have no overlap.",
    argument_count: 2,
    support: DiagnosticSupport::Emitted,
};

pub const TS2393: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2393",
    number: Some(2393),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Duplicate function implementation.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS2394: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2394",
    number: Some(2394),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "This overload signature is not compatible with its implementation signature.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS2448: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2448",
    number: Some(2448),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Block-scoped variable '{0}' used before its declaration.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS2451: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2451",
    number: Some(2451),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Cannot redeclare block-scoped variable '{0}'.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS2454: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2454",
    number: Some(2454),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Variable '{0}' is used before being assigned.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS2493: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2493",
    number: Some(2493),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Tuple type '{0}' of length '{1}' has no element at index '{2}'.",
    argument_count: 3,
    support: DiagnosticSupport::Emitted,
};

pub const TS2538: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2538",
    number: Some(2538),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Type '{0}' cannot be used as an index type.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS2551: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2551",
    number: Some(2551),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Property '{0}' does not exist on type '{1}'. Did you mean '{2}'?",
    argument_count: 3,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS2554: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2554",
    number: Some(2554),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Expected {0} arguments, but got {1}.",
    argument_count: 2,
    support: DiagnosticSupport::Emitted,
};

pub const TS2588: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2588",
    number: Some(2588),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Cannot assign to '{0}' because it is a constant.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS2693: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2693",
    number: Some(2693),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "'{0}' only refers to a type, but is being used as a value here.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS2741: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2741",
    number: Some(2741),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Property '{0}' is missing in type '{1}' but required in type '{2}'.",
    argument_count: 3,
    support: DiagnosticSupport::Emitted,
};

pub const TS2749: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2749",
    number: Some(2749),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "'{0}' refers to a value, but is being used as a type here. Did you mean 'typeof {0}'?",
    argument_count: 1,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS2872: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2872",
    number: Some(2872),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "This kind of expression is always truthy.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS2873: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2873",
    number: Some(2873),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "This kind of expression is always falsy.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS7005: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS7005",
    number: Some(7005),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Variable '{0}' implicitly has an '{1}' type.",
    argument_count: 2,
    support: DiagnosticSupport::Emitted,
};

pub const TS7006: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS7006",
    number: Some(7006),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Parameter '{0}' implicitly has an 'any' type.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS7019: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS7019",
    number: Some(7019),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Rest parameter '{0}' implicitly has an 'any[]' type.",
    argument_count: 1,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS7030: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS7030",
    number: Some(7030),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Not all code paths return a value.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS7031: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS7031",
    number: Some(7031),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Binding element '{0}' implicitly has an '{1}' type.",
    argument_count: 2,
    support: DiagnosticSupport::Emitted,
};

pub const TS7034: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS7034",
    number: Some(7034),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Variable '{0}' implicitly has type '{1}' in some locations where its type cannot be determined.",
    argument_count: 2,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS7051: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS7051",
    number: Some(7051),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Parameter has a name but no type. Did you mean '{0}: {1}'?",
    argument_count: 2,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS7052: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS7052",
    number: Some(7052),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Element implicitly has an 'any' type because type '{0}' has no index signature. Did you mean to call '{1}'?",
    argument_count: 2,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS7053: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS7053",
    number: Some(7053),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Element implicitly has an 'any' type because expression of type '{0}' can't be used to index type '{1}'.",
    argument_count: 2,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS7054: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS7054",
    number: Some(7054),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "No index signature with a parameter of type '{0}' was found on type '{1}'.",
    argument_count: 2,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS7055: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS7055",
    number: Some(7055),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "'{0}', which lacks return-type annotation, implicitly has an '{1}' yield type.",
    argument_count: 2,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS7056: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS7056",
    number: Some(7056),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "The inferred type of this node exceeds the maximum length the compiler will serialize. An explicit type annotation is needed.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS7057: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS7057",
    number: Some(7057),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "'yield' expression implicitly results in an 'any' type because its containing generator lacks a return-type annotation.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS7058: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS7058",
    number: Some(7058),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "If the '{0}' package actually exposes this module, try adding a new declaration (.d.ts) file containing `declare module '{1}';`",
    argument_count: 2,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS7059: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS7059",
    number: Some(7059),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "This syntax is reserved in files with the .mts or .cts extension. Use an `as` expression instead.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS7060: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS7060",
    number: Some(7060),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "This syntax is reserved in files with the .mts or .cts extension. Add a trailing comma or explicit constraint.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS7061: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS7061",
    number: Some(7061),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "A mapped type may not declare properties or methods.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TYPESCRIPT_RUST_PARSER_ERROR: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "typescript-rust::parser-error",
    number: None,
    source: DiagnosticSource::TypeScriptRust,
    category: DiagnosticCategory::Error,
    message_template: "{0}",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TYPESCRIPT_RUST_DUPLICATE_TYPE_PARAMETER: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "typescript-rust::duplicate-type-parameter",
    number: None,
    source: DiagnosticSource::TypeScriptRust,
    category: DiagnosticCategory::Error,
    message_template: "Duplicate type parameter '{0}'.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TYPESCRIPT_RUST_DUPLICATE_DEFAULT_EXPORT: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "typescript-rust::duplicate-default-export",
    number: None,
    source: DiagnosticSource::TypeScriptRust,
    category: DiagnosticCategory::Error,
    message_template: "Duplicate default export.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TYPESCRIPT_RUST_UNSUPPORTED_MODULE_SYNTAX: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "typescript-rust::unsupported-module-syntax",
    number: None,
    source: DiagnosticSource::TypeScriptRust,
    category: DiagnosticCategory::Error,
    message_template: "Unsupported module syntax.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TYPESCRIPT_RUST_UNSUPPORTED_DECLARATION: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "typescript-rust::unsupported-declaration",
    number: None,
    source: DiagnosticSource::TypeScriptRust,
    category: DiagnosticCategory::Error,
    message_template: "Unsupported declaration syntax.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TYPESCRIPT_RUST_TYPE_ALIAS_CYCLE: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "typescript-rust::type-alias-cycle",
    number: None,
    source: DiagnosticSource::TypeScriptRust,
    category: DiagnosticCategory::Error,
    message_template: "Type alias '{0}' circularly references itself.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TYPESCRIPT_RUST_TYPE_DECLARATION_CYCLE: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "typescript-rust::type-declaration-cycle",
    number: None,
    source: DiagnosticSource::TypeScriptRust,
    category: DiagnosticCategory::Error,
    message_template: "Type declaration '{0}' circularly references itself.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const DIAGNOSTIC_CATALOG: &[DiagnosticDescriptor] = &[
    TS5112,
    TS1360,
    TS2304,
    TS2300,
    TS2305,
    TS2306,
    TS2307,
    TS2882,
    TS2314,
    TS2315,
    TS2322,
    TS2339,
    TS2344,
    TS2345,
    TS2349,
    TS2351,
    TS2352,
    TS2353,
    TS2355,
    TS2356,
    TS2362,
    TS2363,
    TS2365,
    TS2366,
    TS2367,
    TS2393,
    TS2394,
    TS2448,
    TS2451,
    TS2454,
    TS2493,
    TS2538,
    TS2551,
    TS2554,
    TS2588,
    TS2693,
    TS2741,
    TS2749,
    TS2872,
    TS2873,
    TS7005,
    TS7006,
    TS7019,
    TS7030,
    TS7031,
    TS7034,
    TS7051,
    TS7052,
    TS7053,
    TS7054,
    TS7055,
    TS7056,
    TS7057,
    TS7058,
    TS7059,
    TS7060,
    TS7061,
    TYPESCRIPT_RUST_PARSER_ERROR,
    TYPESCRIPT_RUST_DUPLICATE_TYPE_PARAMETER,
    TYPESCRIPT_RUST_DUPLICATE_DEFAULT_EXPORT,
    TYPESCRIPT_RUST_UNSUPPORTED_MODULE_SYNTAX,
    TYPESCRIPT_RUST_UNSUPPORTED_DECLARATION,
    TYPESCRIPT_RUST_TYPE_ALIAS_CYCLE,
    TYPESCRIPT_RUST_TYPE_DECLARATION_CYCLE,
];

impl Diagnostic {
    #[allow(clippy::needless_pass_by_value)]
    pub fn ts5112(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS5112, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1360(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS1360,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2304(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2304,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2300(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2300,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2305(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2305,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2306(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2306,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2307(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2307,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2882(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2882,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2314(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2314,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2315(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2315,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2322(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2322,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2339(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2339,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2344(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2344,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2345(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2345,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2349(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2349, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2351(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2351, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2352(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2352,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2353(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2353,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2355(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2355, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2356(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2356, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2362(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2362, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2363(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2363, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2365(
        arg0: impl ToString,
        arg1: impl ToString,
        arg2: impl ToString,
        file_name: impl Into<String>,
    ) -> Self {
        Self::from_descriptor(
            &TS2365,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
                DiagnosticArg::from(arg2.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2366(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2366, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2367(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2367,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2393(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2393, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2394(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2394, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2448(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2448,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2451(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2451,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2454(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2454,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2493(
        arg0: impl ToString,
        arg1: impl ToString,
        arg2: impl ToString,
        file_name: impl Into<String>,
    ) -> Self {
        Self::from_descriptor(
            &TS2493,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
                DiagnosticArg::from(arg2.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2538(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2538,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2551(
        arg0: impl ToString,
        arg1: impl ToString,
        arg2: impl ToString,
        file_name: impl Into<String>,
    ) -> Self {
        Self::from_descriptor(
            &TS2551,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
                DiagnosticArg::from(arg2.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2554(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2554,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2588(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2588,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2693(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2693,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2741(
        arg0: impl ToString,
        arg1: impl ToString,
        arg2: impl ToString,
        file_name: impl Into<String>,
    ) -> Self {
        Self::from_descriptor(
            &TS2741,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
                DiagnosticArg::from(arg2.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2749(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2749,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2872(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2872, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2873(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2873, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts7005(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS7005,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts7006(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS7006,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts7019(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS7019,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts7030(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS7030, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts7031(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS7031,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts7034(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS7034,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts7051(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS7051,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts7052(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS7052,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts7053(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS7053,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts7054(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS7054,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts7055(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS7055,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts7056(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS7056, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts7057(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS7057, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts7058(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS7058,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts7059(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS7059, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts7060(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS7060, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts7061(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS7061, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn typescript_rust_parser_error(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TYPESCRIPT_RUST_PARSER_ERROR,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn typescript_rust_duplicate_type_parameter(
        arg0: impl ToString,
        file_name: impl Into<String>,
    ) -> Self {
        Self::from_descriptor(
            &TYPESCRIPT_RUST_DUPLICATE_TYPE_PARAMETER,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn typescript_rust_duplicate_default_export(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TYPESCRIPT_RUST_DUPLICATE_DEFAULT_EXPORT,
            Vec::<DiagnosticArg>::new(),
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn typescript_rust_unsupported_module_syntax(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TYPESCRIPT_RUST_UNSUPPORTED_MODULE_SYNTAX,
            Vec::<DiagnosticArg>::new(),
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn typescript_rust_unsupported_declaration(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TYPESCRIPT_RUST_UNSUPPORTED_DECLARATION,
            Vec::<DiagnosticArg>::new(),
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn typescript_rust_type_alias_cycle(
        arg0: impl ToString,
        file_name: impl Into<String>,
    ) -> Self {
        Self::from_descriptor(
            &TYPESCRIPT_RUST_TYPE_ALIAS_CYCLE,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn typescript_rust_type_declaration_cycle(
        arg0: impl ToString,
        file_name: impl Into<String>,
    ) -> Self {
        Self::from_descriptor(
            &TYPESCRIPT_RUST_TYPE_DECLARATION_CYCLE,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }
}
