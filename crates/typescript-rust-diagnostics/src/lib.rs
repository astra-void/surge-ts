use std::fmt;

#[derive(Debug, Clone)]
pub enum DiagnosticCode {
    TypeScript(u32),
    Custom(&'static str),
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiagnosticCode::TypeScript(code) => write!(f, "TS{code}"),
            DiagnosticCode::Custom(code) => f.write_str(code),
        }
    }
}

/// Stable internal catalog for TypeScript-compatible diagnostics.
///
/// New diagnostics should be added here so code/message mappings stay centralized.
/// This keeps the crate ready for future generated metadata without changing the
/// public convenience constructors used by the checker today.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypeScriptDiagnosticKind {
    CannotFindName,
    TypeNotAssignable,
    PropertyDoesNotExist,
    ArgumentNotAssignableToParameter,
    ParameterImplicitlyHasAny,
    ThisExpressionIsNotCallable,
    CannotRedeclareBlockScopedVariable,
    ExpectedArguments,
    CannotAssignToConstant,
    LeftHandSideOfArithmeticOperationMustBeNumberLike,
    RightHandSideOfArithmeticOperationMustBeNumberLike,
    ArithmeticOperandMustBeNumberLike,
    OperatorCannotBeAppliedToTypes,
    ComparisonAppearsUnintentionalNoOverlap,
}

impl TypeScriptDiagnosticKind {
    pub fn code(self) -> u32 {
        match self {
            TypeScriptDiagnosticKind::CannotFindName => 2304,
            TypeScriptDiagnosticKind::TypeNotAssignable => 2322,
            TypeScriptDiagnosticKind::PropertyDoesNotExist => 2339,
            TypeScriptDiagnosticKind::ArgumentNotAssignableToParameter => 2345,
            TypeScriptDiagnosticKind::ParameterImplicitlyHasAny => 7006,
            TypeScriptDiagnosticKind::ThisExpressionIsNotCallable => 2349,
            TypeScriptDiagnosticKind::CannotRedeclareBlockScopedVariable => 2451,
            TypeScriptDiagnosticKind::ExpectedArguments => 2554,
            TypeScriptDiagnosticKind::CannotAssignToConstant => 2588,
            TypeScriptDiagnosticKind::LeftHandSideOfArithmeticOperationMustBeNumberLike => 2362,
            TypeScriptDiagnosticKind::RightHandSideOfArithmeticOperationMustBeNumberLike => 2363,
            TypeScriptDiagnosticKind::ArithmeticOperandMustBeNumberLike => 2356,
            TypeScriptDiagnosticKind::OperatorCannotBeAppliedToTypes => 2365,
            TypeScriptDiagnosticKind::ComparisonAppearsUnintentionalNoOverlap => 2367,
        }
    }

    pub fn message_template(self) -> &'static str {
        match self {
            TypeScriptDiagnosticKind::CannotFindName => "Cannot find name '{0}'.",
            TypeScriptDiagnosticKind::TypeNotAssignable => {
                "Type '{0}' is not assignable to type '{1}'."
            }
            TypeScriptDiagnosticKind::PropertyDoesNotExist => {
                "Property '{0}' does not exist on type '{1}'."
            }
            TypeScriptDiagnosticKind::ArgumentNotAssignableToParameter => {
                "Argument of type '{0}' is not assignable to parameter of type '{1}'."
            }
            TypeScriptDiagnosticKind::ParameterImplicitlyHasAny => {
                "Parameter '{0}' implicitly has an 'any' type."
            }
            TypeScriptDiagnosticKind::ThisExpressionIsNotCallable => {
                "This expression is not callable."
            }
            TypeScriptDiagnosticKind::CannotRedeclareBlockScopedVariable => {
                "Cannot redeclare block-scoped variable '{0}'."
            }
            TypeScriptDiagnosticKind::ExpectedArguments => "Expected {0} arguments, but got {1}.",
            TypeScriptDiagnosticKind::CannotAssignToConstant => {
                "Cannot assign to '{0}' because it is a constant."
            }
            TypeScriptDiagnosticKind::LeftHandSideOfArithmeticOperationMustBeNumberLike => {
                "The left-hand side of an arithmetic operation must be of type 'any', 'number', 'bigint' or an enum type."
            }
            TypeScriptDiagnosticKind::RightHandSideOfArithmeticOperationMustBeNumberLike => {
                "The right-hand side of an arithmetic operation must be of type 'any', 'number', 'bigint' or an enum type."
            }
            TypeScriptDiagnosticKind::ArithmeticOperandMustBeNumberLike => {
                "An arithmetic operand must be of type 'any', 'number', 'bigint' or an enum type."
            }
            TypeScriptDiagnosticKind::OperatorCannotBeAppliedToTypes => {
                "Operator '{0}' cannot be applied to types '{1}' and '{2}'."
            }
            TypeScriptDiagnosticKind::ComparisonAppearsUnintentionalNoOverlap => {
                "This comparison appears to be unintentional because the types '{0}' and '{1}' have no overlap."
            }
        }
    }
}

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

    pub fn ts2304(name: &str, file_name: impl Into<String>) -> Self {
        Self::typescript(
            TypeScriptDiagnosticKind::CannotFindName,
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

pub fn render_diagnostics(diagnostics: &[Diagnostic], source_text: &str) -> String {
    if diagnostics.is_empty() {
        return "No errors.".to_string();
    }

    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.render(source_text))
        .collect::<Vec<_>>()
        .join("\n\n")
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "error[{}]: {}", self.code, self.message)?;
        write!(f, " --> {}", self.file_name)
    }
}

fn render_with_span(diagnostic: &Diagnostic, source_text: &str, span: TextSpan) -> String {
    let (line, column) = line_col_from_offset(source_text, span.start);
    let source_line = line_text_at_offset(source_text, span.start);
    let line_number_width = line.to_string().len();
    let line_padding = " ".repeat(line_number_width);
    let caret_width = span.end.saturating_sub(span.start).max(1);
    let caret_line = format!(
        "{}{}",
        " ".repeat(column.saturating_sub(1)),
        "^".repeat(caret_width)
    );

    format!(
        "error[{}]: {}\n --> {}:{}:{}\n  |\n{line:>width$} | {source_line}\n{sep:>width$} | {caret_line}",
        diagnostic.code,
        diagnostic.message,
        diagnostic.file_name,
        line,
        column,
        width = line_number_width,
        sep = line_padding,
    )
}

fn line_col_from_offset(source_text: &str, offset: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut column = 1usize;
    let target = offset.min(source_text.len());

    for (byte_index, ch) in source_text.char_indices() {
        if byte_index >= target {
            break;
        }

        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }

    (line, column)
}

fn line_text_at_offset(source_text: &str, offset: usize) -> &str {
    let target = offset.min(source_text.len());
    let mut line_start = 0usize;
    let mut line_end = source_text.len();

    for (byte_index, ch) in source_text.char_indices() {
        if byte_index >= target {
            break;
        }

        if ch == '\n' {
            line_start = byte_index + ch.len_utf8();
        }
    }

    for (byte_index, ch) in source_text[line_start..].char_indices() {
        if ch == '\n' {
            line_end = line_start + byte_index;
            break;
        }
    }

    &source_text[line_start..line_end]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_kind_codes_are_stable() {
        assert_eq!(TypeScriptDiagnosticKind::CannotFindName.code(), 2304);
        assert_eq!(TypeScriptDiagnosticKind::TypeNotAssignable.code(), 2322);
        assert_eq!(TypeScriptDiagnosticKind::PropertyDoesNotExist.code(), 2339);
        assert_eq!(
            TypeScriptDiagnosticKind::ArgumentNotAssignableToParameter.code(),
            2345
        );
        assert_eq!(
            TypeScriptDiagnosticKind::ParameterImplicitlyHasAny.code(),
            7006
        );
        assert_eq!(
            TypeScriptDiagnosticKind::ThisExpressionIsNotCallable.code(),
            2349
        );
        assert_eq!(
            TypeScriptDiagnosticKind::CannotRedeclareBlockScopedVariable.code(),
            2451
        );
        assert_eq!(TypeScriptDiagnosticKind::ExpectedArguments.code(), 2554);
        assert_eq!(
            TypeScriptDiagnosticKind::CannotAssignToConstant.code(),
            2588
        );
        assert_eq!(
            TypeScriptDiagnosticKind::LeftHandSideOfArithmeticOperationMustBeNumberLike.code(),
            2362
        );
        assert_eq!(
            TypeScriptDiagnosticKind::RightHandSideOfArithmeticOperationMustBeNumberLike.code(),
            2363
        );
        assert_eq!(
            TypeScriptDiagnosticKind::ArithmeticOperandMustBeNumberLike.code(),
            2356
        );
        assert_eq!(
            TypeScriptDiagnosticKind::OperatorCannotBeAppliedToTypes.code(),
            2365
        );
        assert_eq!(
            TypeScriptDiagnosticKind::ComparisonAppearsUnintentionalNoOverlap.code(),
            2367
        );
    }

    #[test]
    fn diagnostic_wrappers_format_messages() {
        let diagnostic = Diagnostic::ts2322("number", "string", "example.ts");
        assert_eq!(
            diagnostic.message,
            "Type 'number' is not assignable to type 'string'."
        );
        assert_eq!(diagnostic.code.to_string(), "TS2322");

        let diagnostic = Diagnostic::ts2304("value", "example.ts");
        assert_eq!(diagnostic.message, "Cannot find name 'value'.");
        assert_eq!(diagnostic.code.to_string(), "TS2304");

        let diagnostic = Diagnostic::ts2554(1, 2, "example.ts");
        assert_eq!(diagnostic.message, "Expected 1 arguments, but got 2.");
        assert_eq!(diagnostic.code.to_string(), "TS2554");

        let diagnostic = Diagnostic::ts7006("value", "example.ts");
        assert_eq!(
            diagnostic.message,
            "Parameter 'value' implicitly has an 'any' type."
        );
        assert_eq!(diagnostic.code.to_string(), "TS7006");

        let diagnostic = Diagnostic::ts2362("example.ts");
        assert_eq!(
            diagnostic.message,
            "The left-hand side of an arithmetic operation must be of type 'any', 'number', 'bigint' or an enum type."
        );
        assert_eq!(diagnostic.code.to_string(), "TS2362");

        let diagnostic = Diagnostic::ts2363("example.ts");
        assert_eq!(
            diagnostic.message,
            "The right-hand side of an arithmetic operation must be of type 'any', 'number', 'bigint' or an enum type."
        );
        assert_eq!(diagnostic.code.to_string(), "TS2363");

        let diagnostic = Diagnostic::ts2356("example.ts");
        assert_eq!(
            diagnostic.message,
            "An arithmetic operand must be of type 'any', 'number', 'bigint' or an enum type."
        );
        assert_eq!(diagnostic.code.to_string(), "TS2356");

        let diagnostic = Diagnostic::ts2365("+", "boolean", "number", "example.ts");
        assert_eq!(
            diagnostic.message,
            "Operator '+' cannot be applied to types 'boolean' and 'number'."
        );
        assert_eq!(diagnostic.code.to_string(), "TS2365");

        let diagnostic = Diagnostic::ts2367("string", "number", "example.ts");
        assert_eq!(
            diagnostic.message,
            "This comparison appears to be unintentional because the types 'string' and 'number' have no overlap."
        );
        assert_eq!(diagnostic.code.to_string(), "TS2367");
    }

    #[test]
    fn generic_typescript_constructor_uses_catalog_metadata() {
        let diagnostic = Diagnostic::typescript(
            TypeScriptDiagnosticKind::ThisExpressionIsNotCallable,
            Vec::<DiagnosticArg>::new(),
            "example.ts",
        );

        assert_eq!(diagnostic.code.to_string(), "TS2349");
        assert_eq!(diagnostic.message, "This expression is not callable.");
    }
}
