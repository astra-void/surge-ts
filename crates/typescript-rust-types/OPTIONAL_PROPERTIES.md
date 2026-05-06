# Optional Object Properties

Current semantics:
- missing optional property is allowed
- present optional property is type-checked
- property access returns `T | undefined` for optional properties
- declared property types remain separate from read types
- `get_property_type(...)` is for writes/contextual checks
- `get_property_access_type(...)` is for reads
- mapped types support optional mapped properties via the `?` modifier
- `Partial<T>` lowering in v0.81 reuses the same optional-property model: concrete object/interface properties are copied and marked optional while preserving their declared value types

Why optionality lives on `ObjectProperty`:
- optionality is a property-level attribute, not a type-level wrapper
- keeping it on the property lets required and optional members coexist in one object type
- it keeps future `undefined`/union support localized to lookup and assignability

Limitations:
- no strict-null analysis
- optional property presence is still checked against the declared property type
- no exactOptionalPropertyTypes

Future direction:
- assignment rules can then be refined for exact optional property semantics if needed

Smoke fixture groups:
- `object_type_optional_variable_*`
- `object_type_optional_assignment_*`
- `object_type_optional_call_argument_*`
- `object_type_optional_return_*`
- `object_type_optional_conditional_branch_*`
- `object_type_optional_property_access_*`
- `union_*`
