//! Private names (`#x`) are scoped to the class body that declares them: two
//! classes' `#x` are different members even when one extends the other (tsc
//! keys the symbol `GetSymbolNameForPrivateIdentifier(class, "#x")`). A
//! declared private member is lowered under a key naming its class, and every
//! `#x` an expression writes is resolved against the class bodies around it
//! (`lookupSymbolForPrivateIdentifierDeclaration`), so each name-keyed lookup
//! the checker makes reaches exactly the member tsc's does. The key layout is
//! read back by `surge_ts_types::private_name`.

use std::cell::RefCell;

use oxc_ast::ast::{Class, ClassElement, PropertyKey};

thread_local! {
    static FILE_NAME: RefCell<String> = const { RefCell::new(String::new()) };
    static SCOPES: RefCell<Vec<ClassScope>> = const { RefCell::new(Vec::new()) };
}

/// The private names one class body declares.
struct ClassScope {
    /// Each declaration's spelling (without `#`), whether it is static, and
    /// its key.
    names: Vec<(String, bool, String)>,
}

impl ClassScope {
    /// tsc looks a name up among the instance members (`symbol.Members`)
    /// before the statics (`symbol.Exports`).
    fn resolve(&self, spelling: &str) -> Option<&str> {
        let declared = |is_static: bool| {
            self.names
                .iter()
                .find(|(name, static_member, _)| name == spelling && *static_member == is_static)
                .map(|(_, _, key)| key.as_str())
        };
        declared(false).or_else(|| declared(true))
    }
}

/// Lowers one file: its name is part of every key its classes declare.
pub(crate) fn with_file<R>(file_name: &str, f: impl FnOnce() -> R) -> R {
    struct Restore(String, Vec<ClassScope>);
    impl Drop for Restore {
        fn drop(&mut self) {
            FILE_NAME.with(|file| *file.borrow_mut() = std::mem::take(&mut self.0));
            SCOPES.with(|scopes| *scopes.borrow_mut() = std::mem::take(&mut self.1));
        }
    }
    let previous_file = FILE_NAME.with(|file| file.replace(file_name.to_string()));
    let previous_scopes = SCOPES.with(|scopes| std::mem::take(&mut *scopes.borrow_mut()));
    let _restore = Restore(previous_file, previous_scopes);
    f()
}

/// Lowers what is lexically inside `class`'s body — everything but the
/// decorators on the class itself, which tsc resolves outside it
/// (`getContainingClassExcludingClassDecorators`). `name` is the name the
/// class is lowered under, which the diagnostics about its members print.
pub(crate) fn with_class<R>(class: &Class<'_>, name: &str, f: impl FnOnce() -> R) -> R {
    struct Pop;
    impl Drop for Pop {
        fn drop(&mut self) {
            SCOPES.with(|scopes| {
                scopes.borrow_mut().pop();
            });
        }
    }
    let file = FILE_NAME.with(|file| file.borrow().clone());
    let body = class.body.span;
    let names = class
        .body
        .body
        .iter()
        .filter_map(|element| {
            let (key, is_static) = match element {
                ClassElement::MethodDefinition(method) => (&method.key, method.r#static),
                ClassElement::PropertyDefinition(property) => (&property.key, property.r#static),
                ClassElement::AccessorProperty(property) => (&property.key, property.r#static),
                _ => return None,
            };
            let PropertyKey::PrivateIdentifier(identifier) = key else {
                return None;
            };
            let spelling = identifier.name.as_str();
            let suffix = if is_static { "\0static" } else { "" };
            let key = format!("#{spelling}\0{name}\0{file}\0{}\0{}{suffix}", body.start, body.end);
            Some((spelling.to_string(), is_static, key))
        })
        .collect();
    SCOPES.with(|scopes| scopes.borrow_mut().push(ClassScope { names }));
    let _pop = Pop;
    f()
}

/// The key a private member the innermost class body declares is lowered
/// under.
pub(crate) fn member_key(spelling: &str, is_static: bool) -> String {
    SCOPES.with(|scopes| {
        scopes
            .borrow()
            .last()
            .and_then(|scope| {
                scope
                    .names
                    .iter()
                    .find(|(name, static_member, _)| name == spelling && *static_member == is_static)
                    .map(|(_, _, key)| key.clone())
            })
            .unwrap_or_else(|| undeclared_key(spelling, false))
    })
}

/// The key `#spelling` written in an expression reads: the member of the
/// innermost enclosing class body that declares it.
pub(crate) fn access_key(spelling: &str) -> String {
    SCOPES.with(|scopes| {
        let scopes = scopes.borrow();
        scopes
            .iter()
            .rev()
            .find_map(|scope| scope.resolve(spelling))
            .map(str::to_string)
            .unwrap_or_else(|| undeclared_key(spelling, scopes.is_empty()))
    })
}

fn undeclared_key(spelling: &str, outside_class_body: bool) -> String {
    if outside_class_body {
        format!("#{spelling}\0\0")
    } else {
        format!("#{spelling}\0")
    }
}
