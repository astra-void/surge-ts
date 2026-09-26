//! tsc's `checkExpressionWithTypeArguments`: `f<T>` written without an
//! argument list — an instantiation expression, in a value or in a `typeof`
//! query.

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{ParsedExpression, ParsedType, TextSpan as SyntaxTextSpan};
use surge_ts_types::{FunctionType, Type};

use crate::checks::expr::evaluate_expression;
use crate::context::CheckerContext;
use crate::infer::InferredExpression;
use crate::spans::diagnostic_with_syntax_span;
use crate::symbols::{FunctionSignatureInfo, SymbolTable};

/// An instantiation expression in a value position: its type arguments are
/// checked for what they name, the expression is evaluated as written, and
/// its type is judged by [`instantiation_expression_diagnostic`]. The value
/// keeps its signatures uninstantiated.
pub(crate) fn evaluate_instantiation_expression(
    expression: &ParsedExpression,
    expression_span: Option<SyntaxTextSpan>,
    type_arguments: &[ParsedType],
    type_arguments_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let saved_symbols = std::mem::replace(&mut ctx.symbols, symbols.clone());
    super::check_untyped_call_type_arguments(type_arguments, ctx);
    ctx.symbols = saved_symbols;
    let result = evaluate_expression(expression, expression_span, symbols, ctx);
    if let InferredExpression::Known(ty) = &result {
        if let ParsedExpression::Identifier { name, .. } = expression
            && class_value_lost_its_constructor(name, ty, ctx)
        {
            return result;
        }
        // A declared function's own declarations say what each of its
        // signatures takes; its value type folds the overloads together.
        let declared = match expression {
            ParsedExpression::Identifier { name, .. } => symbols.get(name).and_then(|symbol| {
                symbol
                    .function_signature
                    .clone()
                    .filter(|_| symbol.ty == *ty)
            }),
            _ => None,
        };
        if let Some(diagnostic) = instantiation_expression_diagnostic(
            ty,
            declared.as_deref(),
            type_arguments.len(),
            type_arguments_span,
            &ctx.file_name,
        ) {
            ctx.push(diagnostic);
        }
    }
    result
}

/// tsgo's `getInstantiationExpressionType`, for what it reports: TS2635 on
/// the expression's type when none of its signatures takes the written number
/// of type arguments, or, when some do, on the first union member that has
/// signatures of which none does. `declared` is the value's own function
/// declaration. A part whose signatures' type-parameter counts surge does not
/// know, or a type it did not model, leaves the expression unjudged.
pub(crate) fn instantiation_expression_diagnostic(
    ty: &Type,
    declared: Option<&FunctionSignatureInfo>,
    type_argument_count: usize,
    type_arguments_span: Option<SyntaxTextSpan>,
    file_name: &str,
) -> Option<Diagnostic> {
    // An empty list (`f<>`) is TS1099's, and every generic signature takes it.
    if type_argument_count == 0 {
        return None;
    }
    let mut scan = ApplicabilityScan::new(type_argument_count);
    match declared {
        Some(signature)
            if signature.construct_signatures.is_none() && matches!(ty, Type::Function(_)) =>
        {
            let mut arities = vec![super::declared_type_argument_arity(&signature.type_parameters)];
            if signature.overloaded {
                arities.extend(
                    signature
                        .overload_alternatives
                        .iter()
                        .map(|overload| super::declared_type_argument_arity(&overload.type_parameters)),
                );
            }
            let mut part = PartSignatures::default();
            scan.judge_arities(&arities, &mut part);
            scan.close(ty, part);
        }
        _ => scan.judge(ty),
    }
    let error_type = scan.error_type(ty)?;
    // tsgo spans the arguments themselves, inside the angle brackets.
    let span = type_arguments_span.map(|span| SyntaxTextSpan {
        start: span.start + 1,
        end: span.end.saturating_sub(1).max(span.start + 1),
    });
    Some(diagnostic_with_syntax_span(
        Diagnostic::ts2635(error_type.name(), file_name),
        span,
    ))
}

/// Whether `name` is a class whose value surge holds without the construct
/// signatures tsgo instantiates: a generic class merged with a namespace is
/// its namespace object alone, and lacking signatures there says nothing.
pub(crate) fn class_value_lost_its_constructor(name: &str, ty: &Type, ctx: &CheckerContext) -> bool {
    matches!(
        ctx.lookup_type_declaration(name),
        Some(crate::symbols::TypeDeclarationInfo::Interface(info)) if info.is_class_instance
    ) && !matches!(ty.peeled(), Type::Object(object) if object.construct_signature().is_some())
}

/// What `getInstantiationExpressionType` gathers over the parts of the
/// expression's type.
struct ApplicabilityScan {
    type_argument_count: usize,
    some_applicable: bool,
    non_applicable: Option<Type>,
    undecided: bool,
}

/// One `getInstantiatedType` call's own flags: whether its part has
/// signatures, and whether one of them takes the type arguments.
#[derive(Default)]
struct PartSignatures {
    any: bool,
    applicable: bool,
}

impl ApplicabilityScan {
    fn new(type_argument_count: usize) -> Self {
        Self {
            type_argument_count,
            some_applicable: false,
            non_applicable: None,
            undecided: false,
        }
    }

    /// `getInstantiatedType`: the whole type, or one member of a union.
    fn judge(&mut self, ty: &Type) {
        let mut part = PartSignatures::default();
        self.judge_part(ty, &mut part);
        self.close(ty, part);
    }

    fn close(&mut self, ty: &Type, part: PartSignatures) {
        self.some_applicable |= part.applicable;
        if part.any && !part.applicable && self.non_applicable.is_none() {
            self.non_applicable = Some(ty.clone());
        }
    }

    /// `getInstantiatedTypePart`.
    fn judge_part(&mut self, ty: &Type, part: &mut PartSignatures) {
        match ty {
            Type::Union(union) => {
                for member in union.types() {
                    self.judge(member);
                }
            }
            Type::Reference(_) => self.judge_part(&ty.peeled(), part),
            Type::Function(function) => self.judge_group(function, part),
            Type::Object(object) => {
                // An open shape is one surge could not enumerate, and a merged
                // intersection folds its operands' signatures into one without
                // keeping them apart.
                if object.synthetic_open_index
                    || (object.is_intersection
                        && (object.call_signature().is_some() || object.construct_signature().is_some()))
                {
                    self.undecided = true;
                    return;
                }
                for group in [object.call_signature(), object.construct_signature()]
                    .into_iter()
                    .flatten()
                {
                    self.judge_group(group, part);
                }
            }
            // surge's sentinel and its permissive `any`, the error type, a type
            // variable whose constraint is not at hand, and a `never` surge may
            // have narrowed to: none says which signatures tsgo's type has.
            Type::Unknown | Type::Any | Type::ErrorType | Type::TypeParameter(_) | Type::Never => {
                self.undecided = true;
            }
            _ => {}
        }
    }

    fn judge_group(&mut self, group: &FunctionType, part: &mut PartSignatures) {
        let mut signatures = Vec::new();
        group.push_overload_members(&mut signatures);
        let mut arities = Vec::with_capacity(signatures.len());
        for signature in &signatures {
            let Some(arity) = super::signature_type_argument_arity(signature, false) else {
                self.undecided = true;
                return;
            };
            arities.push(arity);
        }
        self.judge_arities(&arities, part);
    }

    /// `hasCorrectTypeArgumentArity`, over the generic signatures only.
    fn judge_arities(&self, arities: &[(usize, usize)], part: &mut PartSignatures) {
        part.any |= !arities.is_empty();
        part.applicable |= arities.iter().any(|&(minimum, maximum)| {
            maximum > 0 && self.type_argument_count >= minimum && self.type_argument_count <= maximum
        });
    }

    fn error_type(&self, ty: &Type) -> Option<Type> {
        if self.undecided {
            return None;
        }
        if self.some_applicable {
            self.non_applicable.clone()
        } else {
            Some(ty.clone())
        }
    }
}
