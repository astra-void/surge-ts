//! What a file's JSX refers to without naming it: tsc's
//! `markJsxAliasReferenced` resolves a factory at every element and fragment,
//! chosen by the file's leading `@jsx`/`@jsxFrag` pragmas when it has them.

use oxc_ast::ast::{JSXFragment, JSXOpeningElement, Program};
use oxc_ast_visit::{Visit, walk};

use crate::JsxFactoryUses;

pub(crate) fn collect_jsx_factory_uses(program: &Program<'_>, source_text: &str) -> JsxFactoryUses {
    let mut presence = JsxPresence::default();
    presence.visit_program(program);
    let mut uses = JsxFactoryUses {
        has_elements: presence.elements,
        has_fragments: presence.fragments,
        ..JsxFactoryUses::default()
    };
    if !uses.has_elements && !uses.has_fragments {
        return uses;
    }
    for comment in leading_block_comments(source_text) {
        collect_pragmas(comment, &mut uses);
    }
    uses
}

#[derive(Default)]
struct JsxPresence {
    elements: bool,
    fragments: bool,
}

impl<'a> Visit<'a> for JsxPresence {
    fn visit_jsx_opening_element(&mut self, element: &JSXOpeningElement<'a>) {
        self.elements = true;
        walk::walk_jsx_opening_element(self, element);
    }

    fn visit_jsx_fragment(&mut self, fragment: &JSXFragment<'a>) {
        self.fragments = true;
        walk::walk_jsx_fragment(self, fragment);
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
                "jsximportsource" => uses.import_source_pragma = true,
                "jsxruntime" => uses.runtime_pragma = Some(argument.to_string()),
                _ => {}
            }
        }
        pos = line_end;
    }
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
