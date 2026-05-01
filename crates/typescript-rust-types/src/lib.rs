use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionType {
    pub parameters: Vec<Type>,
    pub return_type: Box<Type>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectType {
    pub properties: BTreeMap<String, Type>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    String,
    Number,
    Boolean,
    Any,
    Unknown,
    Function(FunctionType),
    Object(ObjectType),
}

impl Type {
    pub fn name(&self) -> String {
        match self {
            Type::String => "string".to_string(),
            Type::Number => "number".to_string(),
            Type::Boolean => "boolean".to_string(),
            Type::Any => "any".to_string(),
            Type::Unknown => "unknown".to_string(),
            Type::Function(_) => "function".to_string(),
            Type::Object(object) => {
                let properties = object
                    .properties
                    .iter()
                    .map(|(name, ty)| format!("{name}: {}", ty.name()))
                    .collect::<Vec<_>>()
                    .join("; ");

                if properties.is_empty() {
                    "{}".to_string()
                } else {
                    format!("{{ {}; }}", properties)
                }
            }
        }
    }
}

pub fn is_assignable_to(from: &Type, to: &Type) -> bool {
    from == to
        || matches!(from, Type::Any)
        || matches!(to, Type::Any)
        || matches!(to, Type::Unknown)
}
