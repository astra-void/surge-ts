//! What a file's JSX refers to without naming it: tsc's
//! `markJsxAliasReferenced` resolves a factory at every element and fragment,
//! chosen by the file's leading `@jsx`/`@jsxFrag` pragmas when it has them, and
//! the automatic runtime imports the module its pragmas and options name.

use oxc_ast::ast::{
    ArrowFunctionExpression, Function, JSXElement, JSXFragment, JSXOpeningElement,
    MethodDefinition, Program,
};
use oxc_ast_visit::{Visit, walk};
use oxc_span::Span;
use oxc_syntax::scope::ScopeFlags;

use super::spans::text_span_from_oxc_span;
use crate::{JsxFactoryUses, TextSpan};

pub(crate) fn collect_jsx_factory_uses(program: &Program<'_>, source_text: &str) -> JsxFactoryUses {
    let mut presence = JsxPresence::default();
    presence.visit_program(program);
    let mut uses = JsxFactoryUses {
        has_elements: presence.elements,
        has_fragments: presence.fragments,
        first_tag: presence.first_tag.map(|(_, span)| span),
        ..JsxFactoryUses::default()
    };
    // The runtime import pragmas apply to a JSX file with no JSX in it too.
    for comment in leading_block_comments(source_text) {
        collect_pragmas(comment, &mut uses);
    }
    uses
}

#[derive(Default)]
struct JsxPresence {
    elements: bool,
    fragments: bool,
    /// The first tag tsc checks, with how many function expressions enclose
    /// it: their bodies are checked after the code around them
    /// (`checkNodeDeferred`), a nested one after its parent's.
    first_tag: Option<(usize, TextSpan)>,
    deferred_depth: usize,
    /// A class method's function, which is checked with its class.
    method_value: Option<Span>,
}

impl JsxPresence {
    fn record_tag(&mut self, span: Span) {
        if self
            .first_tag
            .is_none_or(|(depth, _)| self.deferred_depth < depth)
        {
            self.first_tag = Some((self.deferred_depth, text_span_from_oxc_span(span)));
        }
    }
}

impl<'a> Visit<'a> for JsxPresence {
    fn visit_jsx_element(&mut self, element: &JSXElement<'a>) {
        self.record_tag(element.span);
        walk::walk_jsx_element(self, element);
    }

    fn visit_jsx_opening_element(&mut self, element: &JSXOpeningElement<'a>) {
        self.elements = true;
        walk::walk_jsx_opening_element(self, element);
    }

    fn visit_jsx_fragment(&mut self, fragment: &JSXFragment<'a>) {
        self.fragments = true;
        self.record_tag(fragment.opening_fragment.span);
        walk::walk_jsx_fragment(self, fragment);
    }

    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        self.deferred_depth += 1;
        walk::walk_arrow_function_expression(self, arrow);
        self.deferred_depth -= 1;
    }

    fn visit_method_definition(&mut self, method: &MethodDefinition<'a>) {
        self.method_value = Some(method.value.span);
        walk::walk_method_definition(self, method);
    }

    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        let deferred = function.is_expression() && self.method_value != Some(function.span);
        self.deferred_depth += usize::from(deferred);
        walk::walk_function(self, function, flags);
        self.deferred_depth -= usize::from(deferred);
    }
}

/// The `/* … */` comments before the file's first token
/// (`GetLeadingCommentRanges(text, 0)`), where JSX pragmas are read.
fn leading_block_comments(text: &str) -> Vec<&str> {
    let mut comments = Vec::new();
    let mut pos = if text.starts_with("#!") {
        text.find('\n').unwrap_or(text.len())
    } else {
        0
    };
    loop {
        pos += text[pos..]
            .char_indices()
            .find(|(_, c)| !c.is_whitespace() && *c != '\u{feff}')
            .map_or(text.len() - pos, |(offset, _)| offset);
        let rest = &text[pos..];
        if rest.starts_with("//") {
            pos += rest.find('\n').unwrap_or(rest.len());
        } else if rest.starts_with("/*") {
            let end = rest[2..].find("*/").map_or(rest.len(), |offset| offset + 4);
            comments.push(&rest[..end]);
            pos += end;
        } else {
            return comments;
        }
    }
}

/// tsc's `extractPragmas` for a multi-line comment: the first `@name` on each
/// line, its argument the next run of non-blanks. The last pragma of a name wins.
fn collect_pragmas(comment: &str, uses: &mut JsxFactoryUses) {
    let text = comment.strip_suffix("*/").unwrap_or(comment);
    let is_blank = |c: u8| c == b' ' || c == b'\t';
    let is_line_break = |c: u8| c == b'\r' || c == b'\n';
    let bytes = text.as_bytes();
    let skip_blanks = |mut pos: usize| {
        while pos < bytes.len() && is_blank(bytes[pos]) {
            pos += 1;
        }
        pos
    };
    let skip_non_blanks = |mut pos: usize| {
        while pos < bytes.len() && !is_blank(bytes[pos]) && !is_line_break(bytes[pos]) {
            pos += 1;
        }
        pos
    };
    let mut pos = 2.min(bytes.len());
    while let Some(offset) = text[pos..].find('@') {
        let at = pos + offset;
        let name_end = skip_non_blanks(at + 1);
        if name_end == at + 1 {
            pos = at + 1;
            continue;
        }
        let line_end = text[at..]
            .find(['\r', '\n'])
            .map_or(text.len(), |offset| at + offset);
        let name = text[at + 1..name_end].to_ascii_lowercase();
        let argument_start = skip_blanks(name_end);
        let argument = &text[argument_start..skip_non_blanks(argument_start)];
        if !argument.is_empty() {
            match name.as_str() {
                "jsx" => uses.factory_pragma = entity_root(argument),
                "jsxfrag" => uses.fragment_pragma = entity_root(argument),
                "jsximportsource" => uses.import_source_pragma = Some(argument.to_string()),
                "jsxruntime" => uses.runtime_pragma = Some(argument.to_string()),
                _ => {}
            }
        }
        pos = line_end;
    }
}

/// The compiler options that decide a JSX file's implicit runtime import.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JsxRuntimeOptions {
    /// `jsx: react-jsx` or `react-jsxdev`.
    pub automatic: bool,
    /// `jsx: react-jsxdev`.
    pub development: bool,
    /// `jsxImportSource`.
    pub import_source: Option<String>,
}

/// tsc's `GetJSXRuntimeImport(GetJSXImplicitImportBase(options, file))`: the
/// module a JavaScript or `.tsx` file imports the automatic JSX runtime from —
/// `<jsxImportSource or "react">/jsx-runtime` (`jsx-dev-runtime` under
/// `react-jsxdev`) when the options or the file's pragmas select that runtime.
pub fn jsx_runtime_import(
    file_name: &str,
    uses: &JsxFactoryUses,
    options: &JsxRuntimeOptions,
) -> Option<String> {
    let extension = file_name.rsplit_once('.').map(|(_, extension)| extension)?;
    if !matches!(extension, "tsx" | "js" | "jsx" | "mjs" | "cjs") {
        return None;
    }
    let runtime = uses.runtime_pragma.as_deref();
    if runtime == Some("classic") {
        return None;
    }
    let import_source = options
        .import_source
        .as_deref()
        .filter(|source| !source.is_empty());
    let automatic = options.automatic
        || import_source.is_some()
        || uses.import_source_pragma.is_some()
        || runtime == Some("automatic");
    if !automatic {
        return None;
    }
    let base = uses
        .import_source_pragma
        .as_deref()
        .or(import_source)
        .unwrap_or("react");
    let module = if options.development {
        "jsx-dev-runtime"
    } else {
        "jsx-runtime"
    };
    Some(format!("{base}/{module}"))
}

/// The first identifier of an entity name (`h`, `React.createElement`), as
/// `parseIsolatedEntityName` reads it — reserved words included, so
/// `@jsxFrag null` names `null`. `None` when the text is not an entity name.
pub fn entity_root(text: &str) -> Option<String> {
    let is_identifier = |part: &str| {
        let mut chars = part.chars();
        chars
            .next()
            .is_some_and(|first| first.is_alphabetic() || first == '_' || first == '$')
            && chars.all(|c| c.is_alphanumeric() || c == '_' || c == '$')
    };
    let mut parts = text.split('.');
    let root = parts.next()?;
    (is_identifier(root) && parts.all(is_identifier)).then(|| root.to_string())
}
