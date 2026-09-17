//! Generated diagnostic catalog. Do not edit by hand.

use crate::{
    Diagnostic, DiagnosticArg, DiagnosticCategory, DiagnosticDescriptor, DiagnosticSource,
    DiagnosticSupport,
};

pub const TS1029: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1029",
    number: Some(1029),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "'{0}' modifier must precede '{1}' modifier.",
    argument_count: 2,
    support: DiagnosticSupport::Emitted,
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

pub const TS5102: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS5102",
    number: Some(5102),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Option '{0}' has been removed. Please remove it from your configuration.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS5097: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS5097",
    number: Some(5097),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "An import path can only end with a '{0}' extension when 'allowImportingTsExtensions' is enabled.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS5108: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS5108",
    number: Some(5108),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Option '{0}={1}' has been removed. Please remove it from your configuration.",
    argument_count: 2,
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

pub const TS1361: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1361",
    number: Some(1361),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "'{0}' cannot be used as a value because it was imported using 'import type'.",
    argument_count: 1,
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

pub const TS2717: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2717",
    number: Some(2717),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Subsequent property declarations must have the same type.  Property '{0}' must be of type '{1}', but here has type '{2}'.",
    argument_count: 3,
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

pub const TS2614: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2614",
    number: Some(2614),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Module '{0}' has no exported member '{1}'. Did you mean to use 'import {1} from {0}' instead?",
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

pub const TS2732: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2732",
    number: Some(2732),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Cannot find module '{0}'. Consider using '--resolveJsonModule' to import module with '.json' extension.",
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

pub const TS2418: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2418",
    number: Some(2418),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Type of computed property's value is '{0}', which is not assignable to type '{1}'.",
    argument_count: 2,
    support: DiagnosticSupport::Emitted,
};

pub const TS2820: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2820",
    number: Some(2820),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Type '{0}' is not assignable to type '{1}'. Did you mean '{2}'?",
    argument_count: 3,
    support: DiagnosticSupport::Emitted,
};

pub const TS18046: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS18046",
    number: Some(18046),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "'{0}' is of type 'unknown'.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS2532: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2532",
    number: Some(2532),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Object is possibly 'undefined'.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS18048: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS18048",
    number: Some(18048),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "'{0}' is possibly 'undefined'.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS2571: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2571",
    number: Some(2571),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Object is of type 'unknown'.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS18050: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS18050",
    number: Some(18050),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "The value '{0}' cannot be used here.",
    argument_count: 1,
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

pub const TS2540: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2540",
    number: Some(2540),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Cannot assign to '{0}' because it is a read-only property.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS2542: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2542",
    number: Some(2542),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Index signature in type '{0}' only permits reading.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS2514: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2514",
    number: Some(2514),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "A tuple type cannot be indexed with a negative value.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS7015: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS7015",
    number: Some(7015),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Element implicitly has an 'any' type because index expression is not of type 'number'.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS2862: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2862",
    number: Some(2862),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Type '{0}' is generic and can only be indexed for reading.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS4104: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS4104",
    number: Some(4104),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "The type '{0}' is 'readonly' and cannot be assigned to the mutable type '{1}'.",
    argument_count: 2,
    support: DiagnosticSupport::Emitted,
};

pub const TS2536: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2536",
    number: Some(2536),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Type '{0}' cannot be used to index type '{1}'.",
    argument_count: 2,
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
    support: DiagnosticSupport::Emitted,
};

pub const TS2552: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2552",
    number: Some(2552),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Cannot find name '{0}'. Did you mean '{1}'?",
    argument_count: 2,
    support: DiagnosticSupport::Emitted,
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

pub const TS2555: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2555",
    number: Some(2555),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Expected at least {0} arguments, but got {1}.",
    argument_count: 2,
    support: DiagnosticSupport::Emitted,
};

pub const TS2576: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2576",
    number: Some(2576),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Property '{0}' does not exist on type '{1}'. Did you mean to access the static member '{2}' instead?",
    argument_count: 3,
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

pub const TS2580: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2580",
    number: Some(2580),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Cannot find name '{0}'. Do you need to install type definitions for node? Try `npm i --save-dev @types/node`.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS2591: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2591",
    number: Some(2591),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Cannot find name '{0}'. Do you need to install type definitions for node? Try `npm i --save-dev @types/node` and then add 'node' to the types field in your tsconfig.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS2688: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2688",
    number: Some(2688),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Cannot find type definition file for '{0}'.",
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

pub const TS2686: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2686",
    number: Some(2686),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "'{0}' refers to a UMD global, but the current file is a module. Consider adding an import instead.",
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

pub const TS2745: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2745",
    number: Some(2745),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "This JSX tag's '{0}' prop expects type '{1}' which requires multiple children, but only a single child was provided.",
    argument_count: 2,
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

pub const TS2869: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2869",
    number: Some(2869),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Right operand of ?? is unreachable because the left operand is never nullish.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS2447: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2447",
    number: Some(2447),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "The '{0}' operator is not allowed for boolean types. Consider using '{1}' instead.",
    argument_count: 2,
    support: DiagnosticSupport::Emitted,
};

pub const TS2459: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2459",
    number: Some(2459),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Module '{0}' declares '{1}' locally, but it is not exported.",
    argument_count: 2,
    support: DiagnosticSupport::Emitted,
};

pub const TS2469: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2469",
    number: Some(2469),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "The '{0}' operator cannot be applied to type 'symbol'.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS2632: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2632",
    number: Some(2632),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Cannot assign to '{0}' because it is an import.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS2731: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2731",
    number: Some(2731),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Implicit conversion of a 'symbol' to a 'string' will fail at runtime. Consider wrapping this expression in 'String(...)'.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS2736: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2736",
    number: Some(2736),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Operator '{0}' cannot be applied to type '{1}'.",
    argument_count: 2,
    support: DiagnosticSupport::Emitted,
};

pub const TS2769: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2769",
    number: Some(2769),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "No overload matches this call.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS2774: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2774",
    number: Some(2774),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "This condition will always return true since this function is always defined. Did you mean to call it instead?",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS2839: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2839",
    number: Some(2839),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "This condition will always return '{0}' since JavaScript compares objects by reference, not value.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS2845: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2845",
    number: Some(2845),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "This condition will always return '{0}'.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS2871: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2871",
    number: Some(2871),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "This expression is always nullish.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
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

pub const TS7016: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS7016",
    number: Some(7016),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Could not find a declaration file for module '{0}'. '{1}' implicitly has an 'any' type.",
    argument_count: 2,
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

pub const TS4111: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS4111",
    number: Some(4111),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Property '{0}' comes from an index signature, so it must be accessed with ['{0}'].",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS6133: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS6133",
    number: Some(6133),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "'{0}' is declared but its value is never read.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS6198: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS6198",
    number: Some(6198),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "All destructured elements are unused.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS6196: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS6196",
    number: Some(6196),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "'{0}' is declared but never used.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS4112: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS4112",
    number: Some(4112),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "This member cannot have an 'override' modifier because its containing class '{0}' does not extend another class.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS4113: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS4113",
    number: Some(4113),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "This member cannot have an 'override' modifier because it is not declared in the base class '{0}'.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS4114: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS4114",
    number: Some(4114),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "This member must have an 'override' modifier because it overrides a member in the base class '{0}'.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS7029: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS7029",
    number: Some(7029),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Fallthrough case in switch.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS7030: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS7030",
    number: Some(7030),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Not all code paths return a value.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS2534: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2534",
    number: Some(2534),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "A function returning 'never' cannot have a reachable end point.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
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
    support: DiagnosticSupport::Emitted,
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

pub const TS1117: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1117",
    number: Some(1117),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "An object literal cannot have multiple properties with the same name.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS1155: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1155",
    number: Some(1155),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "'const' declarations must be initialized.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS2390: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2390",
    number: Some(2390),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Constructor implementation is missing.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS2391: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2391",
    number: Some(2391),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Function implementation is missing or not immediately following the declaration.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS2528: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2528",
    number: Some(2528),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "A module cannot have multiple default exports.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS2739: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2739",
    number: Some(2739),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Type '{0}' is missing the following properties from type '{1}': {2}",
    argument_count: 3,
    support: DiagnosticSupport::Emitted,
};

pub const TS2740: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2740",
    number: Some(2740),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Type '{0}' is missing the following properties from type '{1}': {2}, and {3} more.",
    argument_count: 4,
    support: DiagnosticSupport::Emitted,
};

pub const TS7008: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS7008",
    number: Some(7008),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Member '{0}' implicitly has an '{1}' type.",
    argument_count: 2,
    support: DiagnosticSupport::Emitted,
};

pub const TS7010: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS7010",
    number: Some(7010),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "'{0}', which lacks return-type annotation, implicitly has an '{1}' return type.",
    argument_count: 2,
    support: DiagnosticSupport::Emitted,
};

pub const TS2377: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2377",
    number: Some(2377),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Constructors for derived classes must contain a 'super' call.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS2392: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2392",
    number: Some(2392),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Multiple constructor implementations are not allowed.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS2695: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2695",
    number: Some(2695),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Left side of comma operator is unused and has no side effects.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS1015: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1015",
    number: Some(1015),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Parameter cannot have question mark and initializer.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS1016: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1016",
    number: Some(1016),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "A required parameter cannot follow an optional parameter.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS1039: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1039",
    number: Some(1039),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Initializers are not allowed in ambient contexts.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS1049: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1049",
    number: Some(1049),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "A 'set' accessor must have exactly one parameter.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS1095: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1095",
    number: Some(1095),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "A 'set' accessor cannot have a return type annotation.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS1119: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1119",
    number: Some(1119),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "An object literal cannot have property and accessor with the same name.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS1244: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1244",
    number: Some(1244),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Abstract methods can only appear within an abstract class.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS1253: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1253",
    number: Some(1253),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Abstract properties can only appear within an abstract class.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS2369: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2369",
    number: Some(2369),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "A parameter property is only allowed in a constructor implementation.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS2371: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2371",
    number: Some(2371),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "A parameter initializer is only allowed in a function or constructor implementation.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS18004: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS18004",
    number: Some(18004),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "No value exists in scope for the shorthand property '{0}'. Either declare one or provide an initializer.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS2678: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2678",
    number: Some(2678),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Type '{0}' is not comparable to type '{1}'.",
    argument_count: 2,
    support: DiagnosticSupport::Emitted,
};

pub const TS2515: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2515",
    number: Some(2515),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Non-abstract class '{0}' does not implement inherited abstract member {1} from class '{2}'.",
    argument_count: 3,
    support: DiagnosticSupport::Emitted,
};

pub const TS2654: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2654",
    number: Some(2654),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Non-abstract class '{0}' is missing implementations for the following members of '{1}': {2}.",
    argument_count: 3,
    support: DiagnosticSupport::Emitted,
};

pub const TS2655: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2655",
    number: Some(2655),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Non-abstract class '{0}' is missing implementations for the following members of '{1}': {2} and {3} more.",
    argument_count: 4,
    support: DiagnosticSupport::Emitted,
};

pub const TS2511: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2511",
    number: Some(2511),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Cannot create an instance of an abstract class.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS2420: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2420",
    number: Some(2420),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Class '{0}' incorrectly implements interface '{1}'.",
    argument_count: 2,
    support: DiagnosticSupport::Emitted,
};

pub const SURGE_PARSER_ERROR: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "surge::parser-error",
    number: None,
    source: DiagnosticSource::TypeScriptRust,
    category: DiagnosticCategory::Error,
    message_template: "{0}",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const SURGE_DUPLICATE_TYPE_PARAMETER: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "surge::duplicate-type-parameter",
    number: None,
    source: DiagnosticSource::TypeScriptRust,
    category: DiagnosticCategory::Error,
    message_template: "Duplicate type parameter '{0}'.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const SURGE_UNSUPPORTED_MODULE_SYNTAX: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "surge::unsupported-module-syntax",
    number: None,
    source: DiagnosticSource::TypeScriptRust,
    category: DiagnosticCategory::Error,
    message_template: "Unsupported module syntax.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const SURGE_UNSUPPORTED_DECLARATION: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "surge::unsupported-declaration",
    number: None,
    source: DiagnosticSource::TypeScriptRust,
    category: DiagnosticCategory::Error,
    message_template: "Unsupported declaration syntax.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const SURGE_TYPE_ALIAS_CYCLE: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "surge::type-alias-cycle",
    number: None,
    source: DiagnosticSource::TypeScriptRust,
    category: DiagnosticCategory::Error,
    message_template: "Type alias '{0}' circularly references itself.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const SURGE_TYPE_DECLARATION_CYCLE: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "surge::type-declaration-cycle",
    number: None,
    source: DiagnosticSource::TypeScriptRust,
    category: DiagnosticCategory::Error,
    message_template: "Type declaration '{0}' circularly references itself.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS2703: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2703",
    number: Some(2703),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "The operand of a 'delete' operator must be a property reference.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS2704: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2704",
    number: Some(2704),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "The operand of a 'delete' operator cannot be a read-only property.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS2790: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2790",
    number: Some(2790),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "The operand of a 'delete' operator must be optional.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS18011: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS18011",
    number: Some(18011),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "The operand of a 'delete' operator cannot be a private identifier.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS1102: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1102",
    number: Some(1102),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "'delete' cannot be called on an identifier in strict mode.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS2358: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2358",
    number: Some(2358),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "The left-hand side of an 'instanceof' expression must be of type 'any', an object type or a type parameter.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS2488: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2488",
    number: Some(2488),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Type '{0}' must have a '[Symbol.iterator]()' method that returns an iterator.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS2698: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2698",
    number: Some(2698),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Spread types may only be created from object types.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS2416: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2416",
    number: Some(2416),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Property '{0}' in type '{1}' is not assignable to the same property in base type '{2}'.",
    argument_count: 3,
    support: DiagnosticSupport::Emitted,
};

pub const TS2430: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2430",
    number: Some(2430),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Interface '{0}' incorrectly extends interface '{1}'.",
    argument_count: 2,
    support: DiagnosticSupport::Emitted,
};

pub const TS2564: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2564",
    number: Some(2564),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Property '{0}' has no initializer and is not definitely assigned in the constructor.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS2403: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2403",
    number: Some(2403),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Subsequent variable declarations must have the same type.  Variable '{0}' must be of type '{1}', but here has type '{2}'.",
    argument_count: 3,
    support: DiagnosticSupport::Emitted,
};

pub const TS2449: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2449",
    number: Some(2449),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Class '{0}' used before its declaration.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS2651: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2651",
    number: Some(2651),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "A member initializer in a enum declaration cannot reference members declared after it, including members defined in other enums.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS2724: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2724",
    number: Some(2724),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "'{0}' has no exported member named '{1}'. Did you mean '{2}'?",
    argument_count: 3,
    support: DiagnosticSupport::Emitted,
};

pub const TS2729: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2729",
    number: Some(2729),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Property '{0}' is used before its initialization.",
    argument_count: 1,
    support: DiagnosticSupport::Emitted,
};

pub const TS2683: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2683",
    number: Some(2683),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "'this' implicitly has type 'any' because it does not have a type annotation.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS1019: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1019",
    number: Some(1019),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "An index signature parameter cannot have a question mark.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS1021: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1021",
    number: Some(1021),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "An index signature must have a type annotation.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS1024: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1024",
    number: Some(1024),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "'readonly' modifier can only appear on a property declaration or index signature.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS1028: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1028",
    number: Some(1028),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Accessibility modifier already seen.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS1047: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1047",
    number: Some(1047),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "A rest parameter cannot be optional.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS1051: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1051",
    number: Some(1051),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "A 'set' accessor cannot have an optional parameter.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS1092: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1092",
    number: Some(1092),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Type parameters cannot appear on a constructor declaration.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS1093: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1093",
    number: Some(1093),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Type annotation cannot appear on a constructor declaration.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS1096: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1096",
    number: Some(1096),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "An index signature must have exactly one parameter.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS1098: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1098",
    number: Some(1098),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Type parameter list cannot be empty.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS1099: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1099",
    number: Some(1099),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Type argument list cannot be empty.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS1108: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1108",
    number: Some(1108),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "A 'return' statement can only be used within a function body.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS1141: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1141",
    number: Some(1141),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "String literal expected.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS1164: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1164",
    number: Some(1164),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Computed property names are not allowed in enums.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS1172: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1172",
    number: Some(1172),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "'extends' clause already seen.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS1173: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1173",
    number: Some(1173),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "'extends' clause must precede 'implements' clause.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS1174: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1174",
    number: Some(1174),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Classes can only extend a single class.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS1175: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1175",
    number: Some(1175),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "'implements' clause already seen.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS1176: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1176",
    number: Some(1176),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Interface declaration cannot have 'implements' clause.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS1183: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1183",
    number: Some(1183),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "An implementation cannot be declared in ambient contexts.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS1184: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1184",
    number: Some(1184),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Modifiers cannot appear here.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS1187: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1187",
    number: Some(1187),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "A parameter property may not be declared using a binding pattern.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS1249: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1249",
    number: Some(1249),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "A decorator can only decorate a method implementation, not an overload.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS1257: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1257",
    number: Some(1257),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "A required element cannot follow an optional element.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS1263: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1263",
    number: Some(1263),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Declarations with initializers cannot also have definite assignment assertions.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS1264: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1264",
    number: Some(1264),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Declarations with definite assignment assertions must also have type annotations.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS1265: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1265",
    number: Some(1265),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "A rest element cannot follow another rest element.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS1266: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1266",
    number: Some(1266),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "An optional element cannot follow a rest element.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS1276: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1276",
    number: Some(1276),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "An 'accessor' property cannot be declared optional.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS1318: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1318",
    number: Some(1318),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "An abstract accessor cannot have an implementation.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS1354: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1354",
    number: Some(1354),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "'readonly' type modifier is only permitted on array and tuple literal types.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS1363: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1363",
    number: Some(1363),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "A type-only import can specify a default import or named bindings, but not both.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS1477: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1477",
    number: Some(1477),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "An instantiation expression cannot be followed by a property access.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS1490: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1490",
    number: Some(1490),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "File appears to be binary.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS1502: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1502",
    number: Some(1502),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "The Unicode (u) flag and the Unicode Sets (v) flag cannot be set simultaneously.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS1545: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1545",
    number: Some(1545),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "'using' declarations are not allowed in ambient contexts.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS1546: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS1546",
    number: Some(1546),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "'await using' declarations are not allowed in ambient contexts.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS2206: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2206",
    number: Some(2206),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "The 'type' modifier cannot be used on a named import when 'import type' is used on its import statement.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS2207: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2207",
    number: Some(2207),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "The 'type' modifier cannot be used on a named export when 'export type' is used on its export statement.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS2452: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2452",
    number: Some(2452),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "An enum member cannot have a numeric name.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS2499: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2499",
    number: Some(2499),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "An interface can only extend an identifier/qualified-name with optional type arguments.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS2566: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2566",
    number: Some(2566),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "A rest element cannot have a property name.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS2681: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2681",
    number: Some(2681),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "A constructor cannot have a 'this' parameter.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS2730: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2730",
    number: Some(2730),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "An arrow function cannot have a 'this' parameter.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS2784: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS2784",
    number: Some(2784),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "'get' and 'set' accessors cannot declare 'this' parameters.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS5085: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS5085",
    number: Some(5085),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "A tuple member cannot be both optional and rest.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS5086: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS5086",
    number: Some(5086),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "A labeled tuple element is declared as optional with a question mark after the name and before the colon, rather than after the type.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS5087: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS5087",
    number: Some(5087),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "A labeled tuple element is declared as rest with a '...' before the name, rather than before the type.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS8002: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS8002",
    number: Some(8002),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "'import ... =' can only be used in TypeScript files.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS8005: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS8005",
    number: Some(8005),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "'implements' clauses can only be used in TypeScript files.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS8012: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS8012",
    number: Some(8012),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Parameter modifiers can only be used in TypeScript files.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS8016: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS8016",
    number: Some(8016),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Type assertion expressions can only be used in TypeScript files.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS8037: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS8037",
    number: Some(8037),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Type satisfaction expressions can only be used in TypeScript files.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS17000: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS17000",
    number: Some(17000),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "JSX attributes must only be assigned a non-empty 'expression'.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS18010: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS18010",
    number: Some(18010),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "An accessibility modifier cannot be used with a private identifier.",
    argument_count: 0,
    support: DiagnosticSupport::Emitted,
};

pub const TS18058: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS18058",
    number: Some(18058),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Default imports are not allowed in a deferred import.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const TS18059: DiagnosticDescriptor = DiagnosticDescriptor {
    code: "TS18059",
    number: Some(18059),
    source: DiagnosticSource::TypeScript,
    category: DiagnosticCategory::Error,
    message_template: "Named imports are not allowed in a deferred import.",
    argument_count: 0,
    support: DiagnosticSupport::CatalogOnly,
};

pub const DIAGNOSTIC_CATALOG: &[DiagnosticDescriptor] = &[
    TS1029,
    TS5112,
    TS5102,
    TS5097,
    TS5108,
    TS1360,
    TS1361,
    TS2304,
    TS2300,
    TS2717,
    TS2305,
    TS2614,
    TS2306,
    TS2307,
    TS2732,
    TS2882,
    TS2314,
    TS2315,
    TS2322,
    TS2418,
    TS2820,
    TS18046,
    TS2532,
    TS18048,
    TS2571,
    TS18050,
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
    TS2540,
    TS2542,
    TS2514,
    TS7015,
    TS2862,
    TS4104,
    TS2536,
    TS2538,
    TS2551,
    TS2552,
    TS2554,
    TS2555,
    TS2576,
    TS2588,
    TS2580,
    TS2591,
    TS2688,
    TS2693,
    TS2686,
    TS2741,
    TS2745,
    TS2749,
    TS2869,
    TS2447,
    TS2459,
    TS2469,
    TS2632,
    TS2731,
    TS2736,
    TS2769,
    TS2774,
    TS2839,
    TS2845,
    TS2871,
    TS2872,
    TS2873,
    TS7005,
    TS7006,
    TS7016,
    TS7019,
    TS4111,
    TS6133,
    TS6198,
    TS6196,
    TS4112,
    TS4113,
    TS4114,
    TS7029,
    TS7030,
    TS2534,
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
    TS1117,
    TS1155,
    TS2390,
    TS2391,
    TS2528,
    TS2739,
    TS2740,
    TS7008,
    TS7010,
    TS2377,
    TS2392,
    TS2695,
    TS1015,
    TS1016,
    TS1039,
    TS1049,
    TS1095,
    TS1119,
    TS1244,
    TS1253,
    TS2369,
    TS2371,
    TS18004,
    TS2678,
    TS2515,
    TS2654,
    TS2655,
    TS2511,
    TS2420,
    SURGE_PARSER_ERROR,
    SURGE_DUPLICATE_TYPE_PARAMETER,
    SURGE_UNSUPPORTED_MODULE_SYNTAX,
    SURGE_UNSUPPORTED_DECLARATION,
    SURGE_TYPE_ALIAS_CYCLE,
    SURGE_TYPE_DECLARATION_CYCLE,
    TS2703,
    TS2704,
    TS2790,
    TS18011,
    TS1102,
    TS2358,
    TS2488,
    TS2698,
    TS2416,
    TS2430,
    TS2564,
    TS2403,
    TS2449,
    TS2651,
    TS2724,
    TS2729,
    TS2683,
    TS1019,
    TS1021,
    TS1024,
    TS1028,
    TS1047,
    TS1051,
    TS1092,
    TS1093,
    TS1096,
    TS1098,
    TS1099,
    TS1108,
    TS1141,
    TS1164,
    TS1172,
    TS1173,
    TS1174,
    TS1175,
    TS1176,
    TS1183,
    TS1184,
    TS1187,
    TS1249,
    TS1257,
    TS1263,
    TS1264,
    TS1265,
    TS1266,
    TS1276,
    TS1318,
    TS1354,
    TS1363,
    TS1477,
    TS1490,
    TS1502,
    TS1545,
    TS1546,
    TS2206,
    TS2207,
    TS2452,
    TS2499,
    TS2566,
    TS2681,
    TS2730,
    TS2784,
    TS5085,
    TS5086,
    TS5087,
    TS8002,
    TS8005,
    TS8012,
    TS8016,
    TS8037,
    TS17000,
    TS18010,
    TS18058,
    TS18059,
];

impl Diagnostic {
    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1029(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS1029,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts5112(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS5112, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts5102(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS5102,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts5097(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS5097,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts5108(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS5108,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
            ],
            file_name,
        )
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
    pub fn ts1361(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS1361,
            vec![DiagnosticArg::from(arg0.to_string())],
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
    pub fn ts2717(
        arg0: impl ToString,
        arg1: impl ToString,
        arg2: impl ToString,
        file_name: impl Into<String>,
    ) -> Self {
        Self::from_descriptor(
            &TS2717,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
                DiagnosticArg::from(arg2.to_string()),
            ],
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
    pub fn ts2614(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2614,
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
    pub fn ts2732(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2732,
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
    pub fn ts2418(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2418,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2820(
        arg0: impl ToString,
        arg1: impl ToString,
        arg2: impl ToString,
        file_name: impl Into<String>,
    ) -> Self {
        Self::from_descriptor(
            &TS2820,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
                DiagnosticArg::from(arg2.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts18046(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS18046,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2532(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2532, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts18048(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS18048,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2571(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2571, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts18050(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS18050,
            vec![DiagnosticArg::from(arg0.to_string())],
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
    pub fn ts2540(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2540,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2542(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2542,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2514(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2514, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts7015(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS7015, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2862(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2862,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts4104(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS4104,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2536(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2536,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
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
    pub fn ts2552(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2552,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
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
    pub fn ts2555(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2555,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2576(
        arg0: impl ToString,
        arg1: impl ToString,
        arg2: impl ToString,
        file_name: impl Into<String>,
    ) -> Self {
        Self::from_descriptor(
            &TS2576,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
                DiagnosticArg::from(arg2.to_string()),
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
    pub fn ts2580(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2580,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2591(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2591,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2688(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2688,
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
    pub fn ts2686(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2686,
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
    pub fn ts2745(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2745,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
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
    pub fn ts2869(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2869, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2447(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2447,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2459(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2459,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2469(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2469,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2632(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2632,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2731(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2731, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2736(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2736,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2769(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2769, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2774(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2774, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2839(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2839,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2845(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2845,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2871(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2871, Vec::<DiagnosticArg>::new(), file_name)
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
    pub fn ts7016(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS7016,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
            ],
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
    pub fn ts4111(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS4111,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts6133(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS6133,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts6198(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS6198, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts6196(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS6196,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts4112(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS4112,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts4113(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS4113,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts4114(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS4114,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts7029(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS7029, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts7030(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS7030, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2534(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2534, Vec::<DiagnosticArg>::new(), file_name)
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
    pub fn ts1117(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1117, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1155(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1155, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2390(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2390, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2391(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2391, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2528(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2528, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2739(
        arg0: impl ToString,
        arg1: impl ToString,
        arg2: impl ToString,
        file_name: impl Into<String>,
    ) -> Self {
        Self::from_descriptor(
            &TS2739,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
                DiagnosticArg::from(arg2.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2740(
        arg0: impl ToString,
        arg1: impl ToString,
        arg2: impl ToString,
        arg3: impl ToString,
        file_name: impl Into<String>,
    ) -> Self {
        Self::from_descriptor(
            &TS2740,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
                DiagnosticArg::from(arg2.to_string()),
                DiagnosticArg::from(arg3.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts7008(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS7008,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts7010(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS7010,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2377(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2377, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2392(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2392, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2695(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2695, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1015(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1015, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1016(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1016, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1039(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1039, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1049(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1049, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1095(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1095, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1119(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1119, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1244(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1244, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1253(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1253, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2369(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2369, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2371(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2371, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts18004(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS18004,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2678(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2678,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2515(
        arg0: impl ToString,
        arg1: impl ToString,
        arg2: impl ToString,
        file_name: impl Into<String>,
    ) -> Self {
        Self::from_descriptor(
            &TS2515,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
                DiagnosticArg::from(arg2.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2654(
        arg0: impl ToString,
        arg1: impl ToString,
        arg2: impl ToString,
        file_name: impl Into<String>,
    ) -> Self {
        Self::from_descriptor(
            &TS2654,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
                DiagnosticArg::from(arg2.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2655(
        arg0: impl ToString,
        arg1: impl ToString,
        arg2: impl ToString,
        arg3: impl ToString,
        file_name: impl Into<String>,
    ) -> Self {
        Self::from_descriptor(
            &TS2655,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
                DiagnosticArg::from(arg2.to_string()),
                DiagnosticArg::from(arg3.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2511(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2511, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2420(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2420,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn surge_parser_error(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &SURGE_PARSER_ERROR,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn surge_duplicate_type_parameter(
        arg0: impl ToString,
        file_name: impl Into<String>,
    ) -> Self {
        Self::from_descriptor(
            &SURGE_DUPLICATE_TYPE_PARAMETER,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn surge_unsupported_module_syntax(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &SURGE_UNSUPPORTED_MODULE_SYNTAX,
            Vec::<DiagnosticArg>::new(),
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn surge_unsupported_declaration(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &SURGE_UNSUPPORTED_DECLARATION,
            Vec::<DiagnosticArg>::new(),
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn surge_type_alias_cycle(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &SURGE_TYPE_ALIAS_CYCLE,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn surge_type_declaration_cycle(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &SURGE_TYPE_DECLARATION_CYCLE,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2703(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2703, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2704(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2704, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2790(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2790, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts18011(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS18011, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1102(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1102, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2358(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2358, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2488(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2488,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2698(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2698, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2416(
        arg0: impl ToString,
        arg1: impl ToString,
        arg2: impl ToString,
        file_name: impl Into<String>,
    ) -> Self {
        Self::from_descriptor(
            &TS2416,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
                DiagnosticArg::from(arg2.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2430(arg0: impl ToString, arg1: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2430,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2564(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2564,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2403(
        arg0: impl ToString,
        arg1: impl ToString,
        arg2: impl ToString,
        file_name: impl Into<String>,
    ) -> Self {
        Self::from_descriptor(
            &TS2403,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
                DiagnosticArg::from(arg2.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2449(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2449,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2651(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2651, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2724(
        arg0: impl ToString,
        arg1: impl ToString,
        arg2: impl ToString,
        file_name: impl Into<String>,
    ) -> Self {
        Self::from_descriptor(
            &TS2724,
            vec![
                DiagnosticArg::from(arg0.to_string()),
                DiagnosticArg::from(arg1.to_string()),
                DiagnosticArg::from(arg2.to_string()),
            ],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2729(arg0: impl ToString, file_name: impl Into<String>) -> Self {
        Self::from_descriptor(
            &TS2729,
            vec![DiagnosticArg::from(arg0.to_string())],
            file_name,
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2683(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2683, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1019(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1019, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1021(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1021, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1024(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1024, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1028(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1028, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1047(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1047, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1051(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1051, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1092(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1092, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1093(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1093, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1096(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1096, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1098(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1098, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1099(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1099, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1108(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1108, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1141(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1141, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1164(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1164, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1172(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1172, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1173(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1173, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1174(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1174, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1175(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1175, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1176(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1176, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1183(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1183, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1184(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1184, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1187(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1187, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1249(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1249, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1257(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1257, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1263(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1263, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1264(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1264, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1265(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1265, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1266(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1266, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1276(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1276, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1318(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1318, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1354(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1354, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1363(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1363, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1477(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1477, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1490(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1490, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1502(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1502, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1545(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1545, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts1546(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS1546, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2206(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2206, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2207(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2207, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2452(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2452, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2499(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2499, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2566(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2566, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2681(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2681, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2730(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2730, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts2784(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS2784, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts5085(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS5085, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts5086(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS5086, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts5087(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS5087, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts8002(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS8002, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts8005(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS8005, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts8012(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS8012, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts8016(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS8016, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts8037(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS8037, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts17000(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS17000, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts18010(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS18010, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts18058(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS18058, Vec::<DiagnosticArg>::new(), file_name)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn ts18059(file_name: impl Into<String>) -> Self {
        Self::from_descriptor(&TS18059, Vec::<DiagnosticArg>::new(), file_name)
    }
}
