use std::fmt;

use crate::render::render_with_span;
use crate::{DiagnosticCode, TypeScriptDiagnosticKind};

#[derive(Debug, Clone)]
pub enum DiagnosticArg {
    Str(String),
    Usize(usize),
}

impl fmt::Display for DiagnosticArg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiagnosticArg::Str(value) => f.write_str(value),
            DiagnosticArg::Usize(value) => write!(f, "{value}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextSpan {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub message: String,
    pub file_name: String,
    pub span: Option<TextSpan>,
}

impl Diagnostic {
    pub fn new(
        code: DiagnosticCode,
        message: impl Into<String>,
        file_name: impl Into<String>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            file_name: file_name.into(),
            span: None,
        }
    }

    pub fn with_span(mut self, span: TextSpan) -> Self {
        self.span = Some(span);
        self
    }

    pub fn ts2322(source_type: &str, target_type: &str, file_name: impl Into<String>) -> Self {
        Self::typescript(
            TypeScriptDiagnosticKind::TypeNotAssignable,
            vec![
                DiagnosticArg::Str(source_type.to_string()),
                DiagnosticArg::Str(target_type.to_string()),
            ],
            file_name,
        )
    }

    pub fn ts2353(property_name: &str, target_type: &str, file_name: impl Into<String>) -> Self {
        Self::typescript(
            TypeScriptDiagnosticKind::ObjectLiteralMayOnlySpecifyKnownProperties,
            vec![
                DiagnosticArg::Str(property_name.to_string()),
                DiagnosticArg::Str(target_type.to_string()),
            ],
            file_name,
        )
    }

    pub fn ts2339(property_name: &str, object_type: &str, file_name: impl Into<String>) -> Self {
        Self::typescript(
            TypeScriptDiagnosticKind::PropertyDoesNotExist,
            vec![
                DiagnosticArg::Str(property_name.to_string()),
                DiagnosticArg::Str(object_type.to_string()),
            ],
            file_name,
        )
    }

    pub fn ts2741(
        property_name: &str,
        source_type: &str,
        target_type: &str,
        file_name: impl Into<String>,
    ) -> Self {
        Self::typescript(
            TypeScriptDiagnosticKind::PropertyIsMissingInTypeButRequiredInType,
            vec![
                DiagnosticArg::Str(property_name.to_string()),
                DiagnosticArg::Str(source_type.to_string()),
                DiagnosticArg::Str(target_type.to_string()),
            ],
            file_name,
        )
    }

    pub fn ts2345(source_type: &str, target_type: &str, file_name: impl Into<String>) -> Self {
        Self::typescript(
            TypeScriptDiagnosticKind::ArgumentNotAssignableToParameter,
            vec![
                DiagnosticArg::Str(source_type.to_string()),
                DiagnosticArg::Str(target_type.to_string()),
            ],
            file_name,
        )
    }

    pub fn ts7006(parameter_name: &str, file_name: impl Into<String>) -> Self {
        Self::typescript(
            TypeScriptDiagnosticKind::ParameterImplicitlyHasAny,
            vec![DiagnosticArg::Str(parameter_name.to_string())],
            file_name,
        )
    }

    pub fn ts2349(file_name: impl Into<String>) -> Self {
        Self::typescript(
            TypeScriptDiagnosticKind::ThisExpressionIsNotCallable,
            Vec::<DiagnosticArg>::new(),
            file_name,
        )
    }

    pub fn ts2554(expected: usize, actual: usize, file_name: impl Into<String>) -> Self {
        Self::typescript(
            TypeScriptDiagnosticKind::ExpectedArguments,
            vec![DiagnosticArg::Usize(expected), DiagnosticArg::Usize(actual)],
            file_name,
        )
    }

    pub fn ts2314(type_name: &str, arity: usize, file_name: impl Into<String>) -> Self {
        Self::typescript(
            TypeScriptDiagnosticKind::GenericTypeRequiresTypeArguments,
            vec![
                DiagnosticArg::Str(type_name.to_string()),
                DiagnosticArg::Usize(arity),
            ],
            file_name,
        )
    }

    pub fn ts2315(type_name: &str, file_name: impl Into<String>) -> Self {
        Self::typescript(
            TypeScriptDiagnosticKind::TypeIsNotGeneric,
            vec![DiagnosticArg::Str(type_name.to_string())],
            file_name,
        )
    }

    pub fn ts2304(name: &str, file_name: impl Into<String>) -> Self {
        Self::typescript(
            TypeScriptDiagnosticKind::CannotFindName,
            vec![DiagnosticArg::Str(name.to_string())],
            file_name,
        )
    }

    pub fn ts2300(name: &str, file_name: impl Into<String>) -> Self {
        Self::typescript(
            TypeScriptDiagnosticKind::DuplicateIdentifier,
            vec![DiagnosticArg::Str(name.to_string())],
            file_name,
        )
    }

    pub fn ts2588(name: &str, file_name: impl Into<String>) -> Self {
        Self::typescript(
            TypeScriptDiagnosticKind::CannotAssignToConstant,
            vec![DiagnosticArg::Str(name.to_string())],
            file_name,
        )
    }

    pub fn ts2451(name: &str, file_name: impl Into<String>) -> Self {
        Self::typescript(
            TypeScriptDiagnosticKind::CannotRedeclareBlockScopedVariable,
            vec![DiagnosticArg::Str(name.to_string())],
            file_name,
        )
    }

    pub fn ts2448(name: &str, file_name: impl Into<String>) -> Self {
        Self::typescript(
            TypeScriptDiagnosticKind::BlockScopedVariableUsedBeforeItsDeclaration,
            vec![DiagnosticArg::Str(name.to_string())],
            file_name,
        )
    }

    pub fn ts2454(name: &str, file_name: impl Into<String>) -> Self {
        Self::typescript(
            TypeScriptDiagnosticKind::VariableUsedBeforeBeingAssigned,
            vec![DiagnosticArg::Str(name.to_string())],
            file_name,
        )
    }

    pub fn ts2393(file_name: impl Into<String>) -> Self {
        Self::typescript(
            TypeScriptDiagnosticKind::DuplicateFunctionImplementation,
            Vec::<DiagnosticArg>::new(),
            file_name,
        )
    }

    pub fn ts2355(file_name: impl Into<String>) -> Self {
        Self::typescript(
            TypeScriptDiagnosticKind::FunctionMustReturnAValue,
            Vec::<DiagnosticArg>::new(),
            file_name,
        )
    }

    pub fn ts2366(file_name: impl Into<String>) -> Self {
        Self::typescript(
            TypeScriptDiagnosticKind::FunctionLacksEndingReturnStatement,
            Vec::<DiagnosticArg>::new(),
            file_name,
        )
    }

    pub fn ts7005(name: &str, implicit_type: &str, file_name: impl Into<String>) -> Self {
        Self::typescript(
            TypeScriptDiagnosticKind::VariableImplicitlyHasAnyType,
            vec![
                DiagnosticArg::Str(name.to_string()),
                DiagnosticArg::Str(implicit_type.to_string()),
            ],
            file_name,
        )
    }

    pub fn ts2362(file_name: impl Into<String>) -> Self {
        Self::typescript(
            TypeScriptDiagnosticKind::LeftHandSideOfArithmeticOperationMustBeNumberLike,
            Vec::<DiagnosticArg>::new(),
            file_name,
        )
    }

    pub fn ts2363(file_name: impl Into<String>) -> Self {
        Self::typescript(
            TypeScriptDiagnosticKind::RightHandSideOfArithmeticOperationMustBeNumberLike,
            Vec::<DiagnosticArg>::new(),
            file_name,
        )
    }

    pub fn ts2356(file_name: impl Into<String>) -> Self {
        Self::typescript(
            TypeScriptDiagnosticKind::ArithmeticOperandMustBeNumberLike,
            Vec::<DiagnosticArg>::new(),
            file_name,
        )
    }

    pub fn ts2365(
        operator: &str,
        left_type: &str,
        right_type: &str,
        file_name: impl Into<String>,
    ) -> Self {
        Self::typescript(
            TypeScriptDiagnosticKind::OperatorCannotBeAppliedToTypes,
            vec![
                DiagnosticArg::Str(operator.to_string()),
                DiagnosticArg::Str(left_type.to_string()),
                DiagnosticArg::Str(right_type.to_string()),
            ],
            file_name,
        )
    }

    pub fn ts2367(left_type: &str, right_type: &str, file_name: impl Into<String>) -> Self {
        Self::typescript(
            TypeScriptDiagnosticKind::ComparisonAppearsUnintentionalNoOverlap,
            vec![
                DiagnosticArg::Str(left_type.to_string()),
                DiagnosticArg::Str(right_type.to_string()),
            ],
            file_name,
        )
    }

    pub fn ts2872(file_name: impl Into<String>) -> Self {
        Self::typescript(
            TypeScriptDiagnosticKind::ThisKindOfExpressionIsAlwaysTruthy,
            Vec::<DiagnosticArg>::new(),
            file_name,
        )
    }

    pub fn ts2873(file_name: impl Into<String>) -> Self {
        Self::typescript(
            TypeScriptDiagnosticKind::ThisKindOfExpressionIsAlwaysFalsy,
            Vec::<DiagnosticArg>::new(),
            file_name,
        )
    }

    pub fn typescript(
        kind: TypeScriptDiagnosticKind,
        args: impl Into<Vec<DiagnosticArg>>,
        file_name: impl Into<String>,
    ) -> Self {
        let args = args.into();
        Self::new(
            DiagnosticCode::TypeScript(kind.code()),
            format_message(kind.message_template(), &args),
            file_name,
        )
    }

    pub fn render(&self, source_text: &str) -> String {
        match self.span {
            Some(span) => render_with_span(self, source_text, span),
            None => self.to_string(),
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "error[{}]: {}", self.code, self.message)?;
        write!(f, " --> {}", self.file_name)
    }
}

fn format_message(template: &str, args: &[DiagnosticArg]) -> String {
    let mut formatted = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '{' {
            formatted.push(ch);
            continue;
        }

        let mut index_text = String::new();
        while let Some(next) = chars.peek().copied() {
            if next == '}' {
                chars.next();
                break;
            }

            if next.is_ascii_digit() {
                index_text.push(next);
                chars.next();
                continue;
            }

            formatted.push('{');
            formatted.push_str(&index_text);
            formatted.push(next);
            chars.next();
            index_text.clear();
            continue;
        }

        if index_text.is_empty() {
            formatted.push('{');
            continue;
        }

        match index_text.parse::<usize>() {
            Ok(index) => match args.get(index) {
                Some(arg) => formatted.push_str(&arg.to_string()),
                None => {
                    formatted.push('{');
                    formatted.push_str(&index_text);
                    formatted.push('}');
                }
            },
            Err(_) => {
                formatted.push('{');
                formatted.push_str(&index_text);
                formatted.push('}');
            }
        }
    }

    formatted
}
