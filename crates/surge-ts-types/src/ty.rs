use crate::{FunctionType, ObjectType, TypeReference, UnionType};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumberLiteralType {
    pub value: String,
}

/// The payload of [`Type::TypeParameter`]: the parameter's declared name.
///
/// Deliberately only the name. A substitution binds a declaration's **own**
/// parameter names, so two references to `QueryBehavior<TQueryFnData, …>` from
/// different consumers produce the same argument tuple — which is why the whole
/// project has 2,343 distinct tuples rather than one per reference site. Adding
/// a per-declaration identity here would make those tuples stop colliding and
/// the cache hit *less*. Any consumer that keys on this must therefore establish
/// that two declarations' same-named parameters cannot meet inside one key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeParameterType {
    pub name: std::sync::Arc<str>,
}

/// A tuple with a rest slot: `[1, ...number[]]`, `[...string[], 'end']`,
/// `[a, ...b[], c]`. `leading` and `trailing` are the fixed slots on either
/// side of the rest, `rest` the rest slot's *element* type. Kept distinct from
/// [`Type::Tuple`] because its length is unknown, and from [`Type::Array`]
/// because it is not: `number[]` does not satisfy `[number, ...number[]]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenTupleType {
    pub leading: Vec<Type>,
    pub rest: Box<Type>,
    pub trailing: Vec<Type>,
}

impl OpenTupleType {
    /// Every element type the tuple can hold, as one union.
    pub fn element_union(&self) -> Type {
        let mut members = self.leading.clone();
        members.push(self.rest.as_ref().clone());
        members.extend(self.trailing.iter().cloned());
        crate::union_type(members)
    }

    pub fn fixed_len(&self) -> usize {
        self.leading.len() + self.trailing.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    String,
    Number,
    Boolean,
    BigInt,
    Symbol,
    Undefined,
    Void,
    Any,
    Unknown,
    /// The genuine `unknown` type as written with the `unknown` keyword (or
    /// flowing from a declaration that uses it). Behaves identically to
    /// [`Type::Unknown`] in every type operation — assignability, display,
    /// narrowing — and is matched together with it via [`Type::is_unknown`].
    /// The sole distinction is provenance: `Unknown` is also surge's
    /// graceful-degradation sentinel for types it cannot resolve, whereas
    /// `GenuineUnknown` is only produced by an actual `unknown` annotation. The
    /// checker uses that distinction to emit `TS18046` ('x' is of type
    /// 'unknown') on a genuine-unknown receiver while staying silent on a
    /// degraded one, matching tsc's no-cascade behavior.
    GenuineUnknown,
    /// A type tsc itself has no answer for — an unresolved type name or a type
    /// imported from a module that does not resolve. tsc models this as
    /// `errorType`, which is `any` with its own identity
    /// (`newIntrinsicType(TypeFlagsAny, "error")`), so it stays permissive
    /// everywhere and is only consulted where the *rule* differs.
    ///
    /// Behaves identically to [`Type::Unknown`] in every type operation and is
    /// matched with it via [`Type::is_unknown`]; the distinction is provenance,
    /// exactly as [`Type::GenuineUnknown`] carries it for the written keyword.
    /// `Unknown` means "surge could not model this" and must stay silent to
    /// avoid cascading its own gaps; `ErrorType` means "the source is genuinely
    /// broken", which is where tsc keeps reporting — a callback parameter
    /// contextually typed by it is still an implicit `any` (TS7006).
    ErrorType,
    /// An unsubstituted type parameter standing in for itself, as bound by a
    /// signature or declaration pre-pass (`substitution.insert_placeholder`).
    ///
    /// Behaves identically to [`Type::Unknown`] in every type operation —
    /// assignability, display, narrowing, interning — and is matched together
    /// with it via [`Type::is_unknown`]. Like [`Type::GenuineUnknown`] the sole
    /// distinction is provenance, and it exists for one reason: `Type::Unknown`
    /// doubles as surge's graceful-degradation sentinel, so a cache that sees it
    /// as a type argument cannot tell "this resolution failed" (which the
    /// memory-lifetime rules forbid caching) from "this argument is the
    /// declaration's own parameter" (which is perfectly cacheable). Carrying the
    /// distinction in the type is what lets the interface instantiation key
    /// admit a placeholder argument instead of refusing it.
    TypeParameter(TypeParameterType),
    Never,
    StringLiteral(String),
    NumberLiteral(NumberLiteralType),
    BooleanLiteral(bool),
    Function(FunctionType),
    Object(ObjectType),
    Array(Box<Type>),
    Tuple(Vec<Type>),
    OpenTuple(OpenTupleType),
    Union(UnionType),
    /// A lazy, nominal reference to a named type instantiation (`Box<string>`,
    /// `User`, …). The structural shape is resolved on demand and memoized,
    /// rather than eagerly expanded at every use site. See [`TypeReference`].
    Reference(TypeReference),
}

fn function_type(
    parameters: Vec<Type>,
    return_type: Type,
    is_variadic: bool,
    required_parameter_count: usize,
) -> Type {
    Type::Function(FunctionType::new(
        parameters,
        return_type,
        is_variadic,
        required_parameter_count,
    ))
}

/// A reference resolver that yields a fixed pre-built structural type.
#[derive(Debug)]
struct FixedResolved(Type);

impl crate::ResolveReference for FixedResolved {
    fn resolve(&self) -> Type {
        self.0.clone()
    }
}

/// Builds an `ArrayIterator<T>` reference for the result of `Array.prototype`'s
/// `values()`/`keys()`/`entries()`. It carries the yielded element as its single
/// type argument (so `for…of` derives the element via the iterator-reference
/// path) and resolves structurally to a minimal iterator object exposing
/// `next(): { value, done }`, so direct iterator-protocol use does not report a
/// missing member. `Array`-valued modelling was rejected: it would falsely reject
/// `arr.values().next()`.
fn array_iterator_type(yields: Type) -> Type {
    use crate::{ObjectProperty, PropertyMap};
    let display = format!("ArrayIterator<{}>", yields.name());

    let mut result_props = PropertyMap::default();
    // The protocol's terminal result carries `value: undefined`, so the merged
    // `next()` result type is `T | undefined` — matching tsc and keeping the
    // possibly-absent value sound.
    result_props.insert(
        "value".into(),
        ObjectProperty::required(crate::union_type(vec![yields.clone(), Type::Undefined])),
    );
    result_props.insert("done".into(), ObjectProperty::required(Type::Boolean));
    let result = Type::Object(ObjectType::new(result_props, None));

    let mut props = PropertyMap::default();
    props.insert(
        "next".into(),
        ObjectProperty::required(function_type(vec![], result, false, 0)),
    );
    let body = Type::Object(ObjectType::new(props, None));

    Type::Reference(TypeReference::new(
        "\u{0}ArrayIterator",
        display,
        vec![yields],
        std::sync::Arc::new(FixedResolved(body)),
    ))
}

impl Type {
    /// Whether this is the `unknown` type, regardless of provenance — both the
    /// degradation sentinel [`Type::Unknown`] and the genuine
    /// [`Type::GenuineUnknown`]. Use this in place of `== Type::Unknown` at every
    /// site that cares about unknown-ness rather than provenance.
    /// The placeholder a pre-pass binds an unsubstituted type parameter to.
    /// See [`Type::TypeParameter`] for why this is not just [`Type::Unknown`].
    pub fn type_parameter(name: &str) -> Type {
        Type::TypeParameter(TypeParameterType {
            name: std::sync::Arc::from(name),
        })
    }

    pub fn is_unknown(&self) -> bool {
        matches!(
            self,
            Type::Unknown | Type::GenuineUnknown | Type::ErrorType | Type::TypeParameter(_)
        )
    }

    pub fn base_primitive(&self) -> Option<Type> {
        match self {
            Type::String | Type::StringLiteral(_) => Some(Type::String),
            Type::Number | Type::NumberLiteral(_) => Some(Type::Number),
            Type::Boolean | Type::BooleanLiteral(_) => Some(Type::Boolean),
            Type::BigInt => Some(Type::BigInt),
            Type::Symbol => Some(Type::Symbol),
            Type::Reference(reference) => reference.resolve().base_primitive(),
            // A union whose members share one base primitive behaves like that
            // primitive for operand checks. `number | 0` arises from `x ?? 0`
            // (TypeScript reduces it to `number`; surge keeps the literal), and
            // rejecting it made `(x ?? 0) > 0` a false `TS2365`.
            Type::Union(union) => {
                let mut base: Option<Type> = None;
                for member in union.types() {
                    // Every member must share the base — a member without one
                    // (`undefined`) makes the union something else entirely.
                    let member_base = member.base_primitive()?;
                    match &base {
                        Some(existing) if *existing != member_base => return None,
                        Some(_) => {}
                        None => base = Some(member_base),
                    }
                }
                base
            }
            _ => None,
        }
    }

    /// Returns the structural form of a [`Type::Reference`] (resolving lazily and
    /// recursively), or a clone of `self` otherwise. Use at sites that
    /// structurally inspect a type — e.g. matching `Type::Object` to read its
    /// properties or index signature — so a nominal reference stays transparent.
    pub fn peeled(&self) -> Type {
        match self {
            Type::Reference(reference) => reference.resolve().peeled(),
            other => other.clone(),
        }
    }

    /// Whether `name` resolves only through a string index signature rather than
    /// a declared property — the condition for TS4111 under
    /// `noPropertyAccessFromIndexSignature`. References are peeled; other type
    /// shapes (no index signature) answer `false`.
    ///
    /// A checker-injected openness index (`synthetic_open_index`) does not count:
    /// there the property really lives on an intersection operand surge could not
    /// enumerate, and tsc — which does model that operand — sees a declared
    /// property and reports nothing.
    pub fn property_only_from_string_index(&self, name: &str) -> bool {
        match self {
            Type::Object(object) => {
                object.get_property(name).is_none()
                    && object.allows_string_index_access()
                    && !object.synthetic_open_index
            }
            Type::Reference(reference) => reference.resolve().property_only_from_string_index(name),
            // tsc looks the name up on the union's synthetic property set and
            // then for an index signature every member carries; an `a?.b`
            // receiver still carries the chain's `undefined`, which is not a
            // member of the lookup.
            Type::Union(union) => {
                let members: Vec<&Type> = union
                    .types()
                    .iter()
                    .filter(|member| !matches!(member, Type::Undefined | Type::Void))
                    .collect();
                !members.is_empty()
                    && members
                        .iter()
                        .all(|member| member.property_only_from_string_index(name))
            }
            _ => false,
        }
    }

    pub fn get_property_access_type(&self, name: &str) -> Option<Type> {
        match self {
            Type::Object(object) => object.get_property_access_type(name).or_else(|| {
                // A callable or constructable object *is* a `Function`, so it
                // carries `name`, `length`, `call`/`apply`/`bind` — `typeof C`
                // and `new (...args: any[]) => T` both answer `.name`.
                let signature = object
                    .call_signature()
                    .or_else(|| object.construct_signature())?;
                function_property_access_type(signature, name)
            }),
            Type::Array(element) => array_property_access_type(name, element.as_ref()),
            // A tuple is an array, so it carries every `Array.prototype` method
            // (`includes`, `map`, …) over the union of its element types — not just
            // `length`. `["get","post"] as const` must answer `.includes`.
            Type::Tuple(elements) => tuple_property_access_type(name, elements),
            Type::OpenTuple(tuple) => {
                if name == "length" {
                    return Some(Type::Number);
                }
                array_property_access_type(name, &tuple.element_union())
            }
            Type::String | Type::StringLiteral(_) => string_property_access_type(name),
            Type::Number | Type::NumberLiteral(_) => number_property_access_type(name),
            Type::BigInt => bigint_property_access_type(name),
            Type::Symbol => symbol_property_access_type(name),
            Type::Function(function) => function_property_access_type(function, name),
            // Any property of `any` is `any`. A lazy reference can resolve to `any`
            // (e.g. `Promise<any>` collapses to its awaited `any`); without this arm
            // the access falls through to `None` and is misreported as a missing
            // property, where the old eager `any` shape emitted nothing.
            Type::Any => Some(Type::Any),
            Type::Reference(reference) => {
                if reference.is_readonly_array() && MUTATING_ARRAY_MEMBERS.contains(&name) {
                    return None;
                }
                reference.resolve().get_property_access_type(name)
            }
            // Every member must declare the property, and the read is their
            // union. Without this a nominal reference that *resolves* to a union
            // answered `None` for every property, which the callers report as a
            // missing member (zod's `$ZodInternalIssue<T>`, a union of 12
            // `Identity<…>` references that all carry `path`).
            Type::Union(union) => {
                let mut members = Vec::with_capacity(union.types().len());
                for member in union.types().iter() {
                    if matches!(member, Type::Undefined) {
                        members.push(Type::Undefined);
                        continue;
                    }
                    members.push(member.get_property_access_type(name)?);
                }
                (!members.is_empty()).then(|| crate::union_type(members))
            }
            _ => None,
        }
    }

    pub fn builtin_constructor_result_type(name: &str) -> Option<Type> {
        match name {
            "Date" => Some(Type::Any),
            "Array" => Some(Type::Array(Box::new(Type::Any))),
            "Uint8Array" => Some(Type::Array(Box::new(Type::Number))),
            "Map" => Some(Type::Object(ObjectType::new(
                {
                    let mut properties = crate::PropertyMap::default();
                    properties.insert(
                        "get".into(),
                        crate::ObjectProperty::required(function_type(
                            vec![Type::Any],
                            Type::Any,
                            false,
                            1,
                        )),
                    );
                    properties.insert(
                        "set".into(),
                        crate::ObjectProperty::required(function_type(
                            vec![Type::Any, Type::Any],
                            Type::Any,
                            false,
                            2,
                        )),
                    );
                    properties.insert(
                        "has".into(),
                        crate::ObjectProperty::required(function_type(
                            vec![Type::Any],
                            Type::Boolean,
                            false,
                            1,
                        )),
                    );
                    properties.insert(
                        "delete".into(),
                        crate::ObjectProperty::required(function_type(
                            vec![Type::Any],
                            Type::Boolean,
                            false,
                            1,
                        )),
                    );
                    properties.insert(
                        "clear".into(),
                        crate::ObjectProperty::required(function_type(
                            vec![],
                            Type::Void,
                            false,
                            0,
                        )),
                    );
                    properties.insert("size".into(), crate::ObjectProperty::required(Type::Number));
                    properties
                },
                None,
            ))),
            _ => None,
        }
    }

    pub fn name(&self) -> String {
        match self {
            Type::String => "string".to_string(),
            Type::Number => "number".to_string(),
            Type::Boolean => "boolean".to_string(),
            Type::BigInt => "bigint".to_string(),
            Type::Symbol => "symbol".to_string(),
            Type::Undefined => "undefined".to_string(),
            Type::Void => "void".to_string(),
            Type::Any => "any".to_string(),
            Type::Unknown | Type::GenuineUnknown | Type::ErrorType | Type::TypeParameter(_) => {
                "unknown".to_string()
            }
            Type::Never => "never".to_string(),
            Type::StringLiteral(value) => format!("{value:?}"),
            Type::NumberLiteral(value) => value.value.clone(),
            Type::BooleanLiteral(value) => value.to_string(),
            Type::Function(function) => function.name(),
            Type::Object(object) => {
                if let Some(alias_name) = &object.alias_name {
                    return alias_name.to_string();
                }

                return object_structural_name(object);
            }
            Type::Array(element) => format!("{}[]", array_element_name(element)),
            Type::Tuple(elements) => {
                let elements = elements
                    .iter()
                    .map(Type::name)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{elements}]")
            }
            Type::OpenTuple(tuple) => {
                let mut parts = tuple.leading.iter().map(Type::name).collect::<Vec<_>>();
                parts.push(format!("...{}[]", array_element_name(&tuple.rest)));
                parts.extend(tuple.trailing.iter().map(Type::name));
                format!("[{}]", parts.join(", "))
            }
            Type::Union(union) => union.name(),
            Type::Reference(reference) if reference.render_structurally => {
                match reference.resolve() {
                    Type::Object(object) => object_structural_name(&object),
                    peeled => peeled.name(),
                }
            }
            Type::Reference(reference) => reference.display.to_string(),
        }
    }
}

fn object_structural_name(object: &crate::ObjectType) -> String {
    {
                let mut parts = object
                    .properties
                    .iter()
                    .map(|(name, property)| {
                        if property.is_optional() {
                            format!("{name}?: {}", optional_property_display(&property.ty))
                        } else {
                            format!("{name}: {}", property.ty.name())
                        }
                    })
                    .collect::<Vec<_>>();

                if let Some(index_type) = &object.string_index_type {
                    parts.push(format!("[key: string]: {}", index_type.name()));
                }

                if let Some(index_type) = &object.number_index_type {
                    parts.push(format!("[key: number]: {}", index_type.name()));
                }

                let properties = parts.join("; ");

                if properties.is_empty() {
                    "{}".to_string()
                } else {
                    format!("{{ {}; }}", properties)
                }
    }
}

fn string_property_access_type(name: &str) -> Option<Type> {
    match name {
        "length" => Some(Type::Number),
        ITERATION_PROTOCOL_MEMBER => Some(iteration_protocol_type(&Type::String)),
        // `searchValue` may be a string or a `RegExp`, so it stays permissive.
        // The replacement is a string *or* a replacer function; spelling that as
        // a union rather than `Any` keeps argument checking just as permissive
        // while giving an inline replacer its contextual parameter types.
        "replace" | "replaceAll" => Some(function_type(
            vec![
                Type::Any,
                crate::union_type(vec![
                    Type::String,
                    function_type(vec![Type::String], Type::String, true, 1),
                ]),
            ],
            Type::String,
            false,
            2,
        )),
        // `(searchString, position?)` — the optional second argument is a number,
        // so declaring only the string parameter made every positional call
        // (`s.includes(v, from)`) a false TS2345.
        "indexOf" | "lastIndexOf" => Some(function_type(
            vec![Type::String, Type::Number],
            Type::Number,
            false,
            1,
        )),
        "search" => Some(function_type(vec![Type::Any], Type::Number, false, 1)),
        // `match`/`matchAll` return regex match data we do not model; `Any` keeps
        // any downstream access conservative rather than cascading.
        "match" | "matchAll" => Some(function_type(vec![Type::Any], Type::Any, false, 1)),
        "split" => Some(function_type(
            vec![Type::Any],
            Type::Array(Box::new(Type::String)),
            true,
            1,
        )),
        "slice" | "substring" | "substr" => {
            Some(function_type(vec![Type::Number], Type::String, true, 1))
        }
        "startsWith" | "endsWith" | "includes" => Some(function_type(
            vec![Type::String, Type::Number],
            Type::Boolean,
            false,
            1,
        )),
        "toLowerCase" | "toUpperCase" | "toLocaleLowerCase" | "toLocaleUpperCase" | "trim"
        | "trimStart" | "trimEnd" | "trimLeft" | "trimRight" => {
            Some(function_type(vec![], Type::String, false, 0))
        }
        // `normalize(form?)` takes the optional Unicode normalization form.
        "normalize" => Some(function_type(vec![Type::String], Type::String, false, 0)),
        "toString" | "valueOf" => Some(function_type(vec![], Type::String, false, 0)),
        "repeat" => Some(function_type(vec![Type::Number], Type::String, false, 1)),
        "concat" => Some(function_type(vec![Type::String], Type::String, true, 0)),
        "charAt" => Some(function_type(vec![Type::Number], Type::String, true, 0)),
        "at" => Some(function_type(
            vec![Type::Number],
            Type::Union(UnionType::new(vec![Type::String, Type::Undefined])),
            false,
            1,
        )),
        "padStart" | "padEnd" => Some(function_type(
            vec![Type::Number, Type::String],
            Type::String,
            true,
            1,
        )),
        "charCodeAt" | "codePointAt" => {
            Some(function_type(vec![Type::Number], Type::Number, false, 1))
        }
        "localeCompare" => Some(function_type(vec![Type::String], Type::Number, true, 1)),
        _ => None,
    }
}

fn number_property_access_type(name: &str) -> Option<Type> {
    match name {
        "toString" => Some(function_type(vec![Type::Number], Type::String, true, 0)),
        "toFixed" | "toPrecision" | "toExponential" => {
            Some(function_type(vec![Type::Number], Type::String, true, 0))
        }
        "toLocaleString" => Some(function_type(vec![], Type::String, true, 0)),
        "valueOf" => Some(function_type(vec![], Type::Number, false, 0)),
        _ => None,
    }
}

fn bigint_property_access_type(name: &str) -> Option<Type> {
    match name {
        "toString" => Some(function_type(vec![Type::Number], Type::String, true, 0)),
        "toLocaleString" => Some(function_type(vec![], Type::String, true, 0)),
        "valueOf" => Some(function_type(vec![], Type::BigInt, false, 0)),
        _ => None,
    }
}

fn symbol_property_access_type(name: &str) -> Option<Type> {
    match name {
        "toString" => Some(function_type(vec![], Type::String, false, 0)),
        "valueOf" => Some(function_type(vec![], Type::Symbol, false, 0)),
        "description" => Some(Type::Union(UnionType::new(vec![
            Type::String,
            Type::Undefined,
        ]))),
        _ => None,
    }
}

fn tuple_property_access_type(name: &str, elements: &[Type]) -> Option<Type> {
    if name == "length" {
        return Some(Type::Number);
    }
    let element = if elements.is_empty() {
        Type::Never
    } else {
        // Flatten and dedup (a tuple element may itself be a union, and
        // repeated literals are common in `as const` tables); a raw nested
        // union fails member-wise assignability against its flat equivalent.
        crate::union_type(elements.to_vec())
    };
    array_property_access_type(name, &element)
}

/// The members every function value carries via `Function`/`CallableFunction`
/// (tsc reports none of these as missing). Under `strictBindCallApply` `call`
/// and `apply` yield the function's own return type; modelled as variadic with
/// no required parameters so an arbitrary `thisArg`/args list is accepted. `bind`
/// yields a bound function, approximated by the original function type so a later
/// call still resolves.
fn function_property_access_type(function: &FunctionType, name: &str) -> Option<Type> {
    match name {
        "call" | "apply" => Some(function_type(
            vec![],
            function.return_type().clone(),
            true,
            0,
        )),
        "bind" => Some(function_type(
            vec![],
            Type::Function(function.clone()),
            true,
            0,
        )),
        "length" => Some(Type::Number),
        "name" => Some(Type::String),
        "prototype" => Some(Type::Any),
        "toString" | "toLocaleString" => Some(function_type(vec![], Type::String, false, 0)),
        _ => None,
    }
}

/// The `(element, index, array)` callback signature shared by the array
/// iteration methods. Modelling all three parameters (rather than just the
/// element) lets a `(v, i) => …` callback type its index parameter as `number`
/// and stay assignable, instead of cascading into `TS7006`/`TS2345`.
fn array_iteration_callback(element: &Type, return_type: Type) -> Type {
    function_type(
        vec![
            element.clone(),
            Type::Number,
            Type::Array(Box::new(element.clone())),
        ],
        return_type,
        false,
        1,
    )
}

/// The member name `[Symbol.iterator]` parses to; see `computed_key_name` in the
/// parser, which renders a computed key under a name no written identifier can
/// spell.
pub const ITERATION_PROTOCOL_MEMBER: &str = "[Symbol.iterator]";

/// The iteration protocol an array, tuple or string carries through the lib's
/// `[Symbol.iterator](): ArrayIterator<T>` / `StringIterator<T>`. surge models
/// these as their own [`Type`] variants rather than as `Array`/`String`
/// interface instances, so nothing supplied that member and no array ever
/// satisfied `Iterable<T>`: every `take([1, 2, 3])` against
/// `take<T>(items: Iterable<T>)` was a false `TS2345`.
///
/// Only what the relation reads is modelled. `next()` yields `{ value: E }`,
/// the least `IteratorYieldResult<T>` accepts — `done` is optional there — so
/// the element is still checked and `number[]` stays rejected against
/// `Iterable<string>`.
fn iteration_protocol_type(element: &Type) -> Type {
    let mut yielded = crate::PropertyMap::default();
    yielded.insert(
        std::sync::Arc::from("value"),
        crate::ObjectProperty::required(element.clone()),
    );
    let yielded = Type::Object(ObjectType::new(yielded, None));

    let mut iterator = crate::PropertyMap::default();
    iterator.insert(
        std::sync::Arc::from("next"),
        crate::ObjectProperty::required(function_type(vec![], yielded, false, 0)),
    );
    let iterator = Type::Object(ObjectType::new(iterator, None));

    function_type(vec![], iterator, false, 0)
}

fn element_or_undefined(element: &Type) -> Type {
    crate::union_type(vec![element.clone(), Type::Undefined])
}

/// Every member name [`array_property_access_type`] answers. An interface that
/// `extends Array<T>`/`ReadonlyArray<T>` inherits a structural member set, and
/// the array surface is modelled by a name lookup rather than a property map —
/// this is what lets the heritage merge materialize it.
/// `Array.prototype` members `ReadonlyArray` does not declare.
pub const MUTATING_ARRAY_MEMBERS: &[&str] = &[
    "push", "pop", "shift", "unshift", "splice", "sort", "reverse", "fill", "copyWithin",
];

pub fn array_property_names() -> &'static [&'static str] {
    &[
        "length", "map", "find", "findLast", "findIndex", "findLastIndex", "filter", "some",
        "every", "forEach", "flatMap", "flat", "reduce", "reduceRight", "join", "concat", "slice",
        "sort", "reverse", "fill", "splice", "push", "pop", "shift", "unshift", "at", "indexOf",
        "lastIndexOf", "includes", "values", "keys", "entries", "toString", "toLocaleString",
    ]
}

pub fn array_member_type(name: &str, element: &Type) -> Option<Type> {
    array_property_access_type(name, element)
}

fn array_property_access_type(name: &str, element: &Type) -> Option<Type> {
    match name {
        "length" => Some(Type::Number),
        ITERATION_PROTOCOL_MEMBER => Some(iteration_protocol_type(element)),
        "map" => Some(function_type(
            vec![array_iteration_callback(element, Type::Any)],
            Type::Array(Box::new(Type::Any)),
            false,
            1,
        )),
        "find" => Some(function_type(
            vec![array_iteration_callback(element, Type::Boolean)],
            element_or_undefined(element),
            false,
            1,
        )),
        "findLast" => Some(function_type(
            vec![array_iteration_callback(element, Type::Boolean)],
            element_or_undefined(element),
            false,
            1,
        )),
        "findIndex" | "findLastIndex" => Some(function_type(
            vec![array_iteration_callback(element, Type::Boolean)],
            Type::Number,
            false,
            1,
        )),
        // The predicate's return is `unknown` in the lib, not `boolean`: any
        // truthy value filters (`lines.filter((x) => x)`), so demanding `boolean`
        // reports a `TS2345` tsc never emits.
        "filter" => Some(function_type(
            vec![array_iteration_callback(element, Type::Any)],
            Type::Array(Box::new(element.clone())),
            false,
            1,
        )),
        "some" | "every" => Some(function_type(
            vec![array_iteration_callback(element, Type::Any)],
            Type::Boolean,
            false,
            1,
        )),
        "forEach" => Some(function_type(
            vec![array_iteration_callback(element, Type::Any)],
            Type::Void,
            false,
            1,
        )),
        "flatMap" => Some(function_type(
            vec![array_iteration_callback(element, Type::Any)],
            Type::Array(Box::new(Type::Any)),
            false,
            1,
        )),
        "flat" => Some(function_type(
            vec![Type::Number],
            Type::Array(Box::new(Type::Any)),
            true,
            0,
        )),
        // `reduce`/`reduceRight` carry an accumulator type we do not infer, so
        // the accumulator and the result stay `Any` — but the callback's *shape*
        // is known, and modelling it is what contextually types an inline
        // reducer instead of reporting every parameter as an implicit any.
        "reduce" | "reduceRight" => Some(function_type(
            vec![
                function_type(
                    vec![
                        Type::Any,
                        element.clone(),
                        Type::Number,
                        Type::Array(Box::new(element.clone())),
                    ],
                    Type::Any,
                    false,
                    2,
                ),
                Type::Any,
            ],
            Type::Any,
            false,
            1,
        )),
        "join" => Some(function_type(vec![Type::String], Type::String, true, 0)),
        "toString" | "toLocaleString" => Some(function_type(vec![], Type::String, false, 0)),
        "concat" => Some(function_type(
            vec![Type::Array(Box::new(Type::Any))],
            Type::Array(Box::new(element.clone())),
            true,
            0,
        )),
        "slice" => Some(function_type(
            vec![Type::Number],
            Type::Array(Box::new(element.clone())),
            true,
            0,
        )),
        // `sort`'s optional comparator is `(a, b) => number`; modelling both
        // parameters lets `(left, right) => …` type them as the element type
        // instead of cascading into `TS7006`.
        "sort" => Some(function_type(
            vec![function_type(
                vec![element.clone(), element.clone()],
                Type::Number,
                false,
                2,
            )],
            Type::Array(Box::new(element.clone())),
            true,
            0,
        )),
        "reverse" => Some(function_type(
            vec![],
            Type::Array(Box::new(element.clone())),
            false,
            0,
        )),
        // `fill(value: T, start?: number, end?: number)` — positional, not
        // variadic; modelling it as variadic checked `start`/`end` against `T`.
        "fill" => Some(function_type(
            vec![element.clone(), Type::Number, Type::Number],
            Type::Array(Box::new(element.clone())),
            false,
            1,
        )),
        // `splice(start: number, deleteCount?: number, ...items: T[])`.
        "splice" => Some(function_type(
            vec![
                Type::Number,
                Type::Number,
                Type::Array(Box::new(element.clone())),
            ],
            Type::Array(Box::new(element.clone())),
            true,
            1,
        )),
        // `push`/`unshift` take `...items: T[]`: the rest parameter is stored as
        // the array it is written as, so an element type that is itself an
        // array or tuple (`string[][]`, `[string, string][]`) is not unwrapped
        // a second time by variadic argument checking.
        "push" | "unshift" => Some(function_type(
            vec![Type::Array(Box::new(element.clone()))],
            Type::Number,
            true,
            0,
        )),
        "pop" | "shift" => Some(function_type(
            vec![],
            element_or_undefined(element),
            false,
            0,
        )),
        "at" => Some(function_type(
            vec![Type::Number],
            element_or_undefined(element),
            false,
            1,
        )),
        "indexOf" | "lastIndexOf" => Some(function_type(
            vec![element.clone(), Type::Number],
            Type::Number,
            false,
            1,
        )),
        "includes" => Some(function_type(
            vec![element.clone(), Type::Number],
            Type::Boolean,
            false,
            1,
        )),
        // Iterator-producing methods. The result carries the yielded element so
        // `for…of arr.values()` / `.entries()` / `.keys()` derive the loop
        // variable type instead of degrading to `unknown`.
        "values" => Some(function_type(
            vec![],
            array_iterator_type(element.clone()),
            false,
            0,
        )),
        "keys" => Some(function_type(
            vec![],
            array_iterator_type(Type::Number),
            false,
            0,
        )),
        "entries" => Some(function_type(
            vec![],
            array_iterator_type(Type::Tuple(vec![Type::Number, element.clone()])),
            false,
            0,
        )),
        _ => None,
    }
}

fn array_element_name(element: &Type) -> String {
    match element {
        Type::Union(_) | Type::Function(_) => format!("({})", element.name()),
        _ => element.name(),
    }
}

/// Renders the type of an optional property as tsc does in diagnostics: outside
/// `exactOptionalPropertyTypes`, an optional property's printed type gains
/// `| undefined`. `unknown`/`any` already absorb `undefined`, and a union that
/// already includes it needs no addition.
fn optional_property_display(ty: &Type) -> String {
    match ty {
        Type::Any | Type::Unknown | Type::GenuineUnknown | Type::TypeParameter(_) | Type::Undefined => ty.name(),
        Type::Union(union) if union.types().iter().any(|m| matches!(m, Type::Undefined)) => {
            ty.name()
        }
        _ => format!("{} | undefined", ty.name()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ObjectProperty;

    #[test]
    fn string_literal_type_name_quotes_value() {
        assert_eq!(Type::StringLiteral("ok".to_string()).name(), r#""ok""#);
    }

    // `toString`/`toLocaleString` come from `Object.prototype` and every array
    // answers them; leaving them out of the surface reported TS2339.
    #[test]
    fn array_answers_the_object_string_conversions() {
        for name in ["toString", "toLocaleString"] {
            assert!(array_property_names().contains(&name), "{name}");
            let member = array_member_type(name, &Type::String).expect(name);
            let Type::Function(function_type) = member else {
                panic!("{name} must be callable");
            };
            assert!(function_type.parameters().is_empty(), "{name}");
            assert_eq!(function_type.return_type(), &Type::String, "{name}");
        }
    }

    #[test]
    fn number_literal_type_name_is_stable() {
        assert_eq!(
            Type::NumberLiteral(NumberLiteralType {
                value: "1".to_string()
            })
            .name(),
            "1"
        );
    }

    #[test]
    fn boolean_literal_type_name_is_true_false() {
        assert_eq!(Type::BooleanLiteral(true).name(), "true");
        assert_eq!(Type::BooleanLiteral(false).name(), "false");
    }

    #[test]
    fn void_type_name_is_void() {
        assert_eq!(Type::Void.name(), "void");
    }

    #[test]
    fn array_type_name_string() {
        assert_eq!(Type::Array(Box::new(Type::String)).name(), "string[]");
    }

    #[test]
    fn array_length_property_is_number() {
        assert_eq!(
            Type::Array(Box::new(Type::String)).get_property_access_type("length"),
            Some(Type::Number)
        );
    }

    #[test]
    fn tuple_length_property_is_number() {
        assert_eq!(
            Type::Tuple(vec![Type::String, Type::Number]).get_property_access_type("length"),
            Some(Type::Number)
        );
    }

    #[test]
    fn tuple_unknown_property_is_unsupported() {
        assert_eq!(
            Type::Tuple(vec![Type::String, Type::Number])
                .get_property_access_type("definitelyNotAnArrayMember"),
            None
        );
    }

    #[test]
    fn tuple_exposes_array_methods_over_element_union() {
        let tuple = Type::Tuple(vec![Type::String, Type::Number]);
        // A tuple is an array, so array methods resolve over the element union.
        assert!(tuple.get_property_access_type("includes").is_some());
        assert!(tuple.get_property_access_type("map").is_some());
        assert_eq!(tuple.get_property_access_type("length"), Some(Type::Number));
    }

    #[test]
    fn array_type_name_number() {
        assert_eq!(Type::Array(Box::new(Type::Number)).name(), "number[]");
    }

    #[test]
    fn array_type_name_boolean() {
        assert_eq!(Type::Array(Box::new(Type::Boolean)).name(), "boolean[]");
    }

    #[test]
    fn array_type_name_undefined() {
        assert_eq!(Type::Array(Box::new(Type::Undefined)).name(), "undefined[]");
    }

    #[test]
    fn array_type_name_void() {
        assert_eq!(Type::Array(Box::new(Type::Void)).name(), "void[]");
    }

    #[test]
    fn array_type_name_literal() {
        assert_eq!(
            Type::Array(Box::new(Type::StringLiteral("ok".to_string()))).name(),
            r#""ok"[]"#
        );
    }

    #[test]
    fn array_type_name_union() {
        assert_eq!(
            Type::Array(Box::new(Type::Union(crate::UnionType::new(vec![
                Type::String,
                Type::Number,
            ]))))
            .name(),
            "(string | number)[]"
        );
    }

    #[test]
    fn array_type_name_function() {
        assert_eq!(
            Type::Array(Box::new(Type::Function(FunctionType::new(
                vec![],
                Type::String,
                false,
                0,
            ))))
            .name(),
            "(() => string)[]"
        );
    }

    #[test]
    fn array_type_name_object() {
        let mut properties = crate::PropertyMap::default();
        properties.insert("name".into(), ObjectProperty::required(Type::String));

        assert_eq!(
            Type::Array(Box::new(Type::Object(ObjectType::new(properties, None)))).name(),
            "{ name: string; }[]"
        );
    }

    #[test]
    fn array_type_name_nested_array() {
        assert_eq!(
            Type::Array(Box::new(Type::Array(Box::new(Type::String)))).name(),
            "string[][]"
        );
    }

    #[test]
    fn tuple_type_name_empty() {
        assert_eq!(Type::Tuple(vec![]).name(), "[]");
    }

    #[test]
    fn tuple_type_name_one_element() {
        assert_eq!(Type::Tuple(vec![Type::String]).name(), "[string]");
    }

    #[test]
    fn tuple_type_name_two_elements() {
        assert_eq!(
            Type::Tuple(vec![Type::String, Type::Number]).name(),
            "[string, number]"
        );
    }

    #[test]
    fn tuple_type_name_literal_element() {
        assert_eq!(
            Type::Tuple(vec![Type::StringLiteral("ok".to_string()), Type::Number]).name(),
            r#"["ok", number]"#
        );
    }

    #[test]
    fn tuple_type_name_union_element() {
        assert_eq!(
            Type::Tuple(vec![
                Type::Union(crate::UnionType::new(vec![Type::String, Type::Number,])),
                Type::Boolean,
            ])
            .name(),
            "[string | number, boolean]"
        );
    }

    #[test]
    fn tuple_type_name_function_element() {
        assert_eq!(
            Type::Tuple(vec![
                Type::Function(FunctionType::new(vec![], Type::Void, false, 0)),
                Type::String,
            ])
            .name(),
            "[() => void, string]"
        );
    }

    #[test]
    fn tuple_type_name_object_element() {
        let mut properties = crate::PropertyMap::default();
        properties.insert("name".into(), ObjectProperty::required(Type::String));

        assert_eq!(
            Type::Tuple(vec![
                Type::Object(ObjectType::new(properties, None)),
                Type::Number,
            ])
            .name(),
            "[{ name: string; }, number]"
        );
    }

    #[test]
    fn tuple_type_name_array_element() {
        assert_eq!(
            Type::Tuple(vec![Type::Array(Box::new(Type::String)), Type::Number]).name(),
            "[string[], number]"
        );
    }

    #[test]
    fn tuple_type_name_nested_tuple() {
        assert_eq!(
            Type::Tuple(vec![
                Type::Tuple(vec![Type::String, Type::Number]),
                Type::Boolean,
            ])
            .name(),
            "[[string, number], boolean]"
        );
    }

    fn iterator_element(method_result: &Type) -> Type {
        let Type::Function(function) = method_result else {
            panic!("expected a function-typed array method, got {method_result:?}");
        };
        let Type::Reference(reference) = function.return_type() else {
            panic!("expected an iterator reference return type");
        };
        assert_eq!(reference.id.as_ref(), "\u{0}ArrayIterator");
        reference.arguments[0].clone()
    }

    #[test]
    fn array_values_iterator_yields_element() {
        let result = array_property_access_type("values", &Type::Number).unwrap();
        assert_eq!(iterator_element(&result), Type::Number);
    }

    #[test]
    fn array_keys_iterator_yields_number() {
        let result = array_property_access_type("keys", &Type::String).unwrap();
        assert_eq!(iterator_element(&result), Type::Number);
    }

    #[test]
    fn array_entries_iterator_yields_index_value_tuple() {
        let result = array_property_access_type("entries", &Type::String).unwrap();
        assert_eq!(
            iterator_element(&result),
            Type::Tuple(vec![Type::Number, Type::String])
        );
    }

    #[test]
    fn array_iterator_next_value_is_element_or_undefined() {
        let result = array_property_access_type("values", &Type::Number).unwrap();
        let Type::Function(function) = &result else {
            panic!("expected function");
        };
        let next = function
            .return_type()
            .get_property_access_type("next")
            .expect("iterator exposes next()");
        let Type::Function(next_fn) = next else {
            panic!("next is callable");
        };
        let value = next_fn
            .return_type()
            .get_property_access_type("value")
            .expect("iterator result has value");
        assert_eq!(
            value,
            crate::union_type(vec![Type::Number, Type::Undefined])
        );
    }
}

/// Whether `ty` is the global `Function` interface. tsc treats it as callable
/// with any arguments (yielding `any`) and constructible, even though
/// `lib.d.ts` declares no call signature on it — so every consumer that asks
/// "is this callable?" has to special-case it. The `apply`/`call`/`bind` check
/// pins this to the real lib interface rather than any same-named user type.
pub fn is_global_function_interface(ty: &Type) -> bool {
    let Type::Reference(reference) = ty else {
        return false;
    };
    if reference.id.split('\u{0}').next_back() != Some("Function") {
        return false;
    }
    match ty.peeled() {
        Type::Object(object) => {
            object.call_signature().is_none()
                && ["apply", "call", "bind"]
                    .iter()
                    .all(|member| object.properties.get(*member).is_some())
        }
        _ => false,
    }
}
