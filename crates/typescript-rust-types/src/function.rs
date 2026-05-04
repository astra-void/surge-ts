use crate::Type;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionType {
    pub parameters: Vec<Type>,
    pub return_type: Box<Type>,
    pub is_variadic: bool,
}

impl FunctionType {
    pub fn name(&self) -> String {
        let mut parameters = self.parameters.iter().map(Type::name).collect::<Vec<_>>();

        if self.is_variadic {
            parameters.push("...args: any[]".to_string());
        }

        let parameters = parameters.join(", ");

        format!("({parameters}) => {}", self.return_type.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn function_type_name_no_params() {
        let ty = FunctionType {
            parameters: vec![],
            return_type: Box::new(Type::String),
            is_variadic: false,
        };

        assert_eq!(ty.name(), "() => string");
    }

    #[test]
    fn function_type_name_one_param() {
        let ty = FunctionType {
            parameters: vec![Type::String],
            return_type: Box::new(Type::Number),
            is_variadic: false,
        };

        assert_eq!(ty.name(), "(string) => number");
    }

    #[test]
    fn function_type_name_multiple_params() {
        let ty = FunctionType {
            parameters: vec![Type::String, Type::Number, Type::Boolean],
            return_type: Box::new(Type::Void),
            is_variadic: false,
        };

        assert_eq!(ty.name(), "(string, number, boolean) => void");
    }

    #[test]
    fn function_type_name_nested_parameter() {
        let ty = FunctionType {
            parameters: vec![Type::Function(FunctionType {
                parameters: vec![Type::String],
                return_type: Box::new(Type::Number),
                is_variadic: false,
            })],
            return_type: Box::new(Type::Void),
            is_variadic: false,
        };

        assert_eq!(ty.name(), "((string) => number) => void");
    }

    #[test]
    fn function_type_name_nested_return() {
        let ty = FunctionType {
            parameters: vec![Type::String],
            return_type: Box::new(Type::Function(FunctionType {
                parameters: vec![Type::Number],
                return_type: Box::new(Type::Boolean),
                is_variadic: false,
            })),
            is_variadic: false,
        };

        assert_eq!(ty.name(), "(string) => (number) => boolean");
    }
}
