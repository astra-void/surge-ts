use super::*;

pub(crate) const PROMISE_LIKE_VALUE_PROPERTY: &str = "\0surgePromiseValue";

pub(crate) fn promise_like_type(value_type: Type) -> Type {
    let mut properties = PropertyMap::default();
    properties.insert(
        PROMISE_LIKE_VALUE_PROPERTY.into(),
        ObjectProperty::required(value_type.clone()),
    );
    properties.insert(
        "then".into(),
        ObjectProperty::required(Type::Function(FunctionType::new(
            vec![Type::Function(FunctionType::new(
                vec![value_type],
                Type::Unknown,
                false,
                1,
            ))],
            Type::Unknown,
            true,
            1,
        ))),
    );
    properties.insert(
        "catch".into(),
        ObjectProperty::required(Type::Function(FunctionType::new(
            vec![Type::Any],
            Type::Unknown,
            true,
            0,
        ))),
    );
    properties.insert(
        "finally".into(),
        ObjectProperty::required(Type::Function(FunctionType::new(
            vec![Type::Any],
            Type::Unknown,
            true,
            0,
        ))),
    );
    Type::Object(crate::arena::alloc_object_type(properties, None))
}

pub(super) fn promise_value_type(
    return_type: &Option<ParsedType>,
    resolution_scope: Option<&Arc<TypeDeclarationScope>>,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let Some(ParsedType::Named(named)) = return_type else {
        return None;
    };
    if !matches!(named.name.as_str(), "Promise" | "PromiseLike") {
        return None;
    }
    let value_type = named.type_arguments.first()?;
    let saved_scope = ctx.type_declaration_scope.clone();
    let saved_type_declarations = if resolution_scope.is_some() {
        Some(std::mem::replace(
            &mut ctx.type_declarations,
            TypeDeclarationTable::new(),
        ))
    } else {
        None
    };
    if let Some(resolution_scope) = resolution_scope {
        ctx.type_declaration_scope = Some(resolution_scope.clone());
    }
    let ty = crate::infer::map_parsed_type(value_type.clone(), ctx);
    ctx.type_declaration_scope = saved_scope;
    if let Some(saved_type_declarations) = saved_type_declarations {
        ctx.type_declarations = saved_type_declarations;
    }
    Some(ty)
}
