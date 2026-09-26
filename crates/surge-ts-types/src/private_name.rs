//! Private names (`#x`) as property keys.
//!
//! A private name belongs to the class body that declares it, so two classes'
//! `#x` are different members even when one extends the other (tsc keys the
//! symbol `GetSymbolNameForPrivateIdentifier(class, "#x")`). The syntax
//! lowering writes the key: `#x`, a NUL, then the declaring class — its name,
//! its file and the offsets of its body, NUL-separated — and a trailing
//! `\0static` for a static member. An access that no enclosing class body
//! declares is keyed `#x\0`, or `#x\0\0` outside every class body, so it can
//! never meet a declared member by accident. The layout here must stay in step
//! with `surge_ts_syntax`'s `private_names`.

const STATIC_SUFFIX: &str = "\0static";

/// Whether `key` is a private name (as opposed to a string key such as
/// `obj["#x"]`, which never carries a NUL).
pub fn is_private_name_key(key: &str) -> bool {
    key.starts_with('#') && key.contains('\0')
}

/// The name as written, `#x`: what every diagnostic prints.
pub fn display(key: &str) -> &str {
    if is_private_name_key(key) {
        key.split('\0').next().unwrap_or(key)
    } else {
        key
    }
}

/// A static member's key (tsc's `isStaticPrivateIdentifierProperty`): not
/// inherited by a derived class's constructor, and skipped when the source
/// of a relation lacks it.
pub fn is_static(key: &str) -> bool {
    is_private_name_key(key) && key.ends_with(STATIC_SUFFIX)
}

/// An access that no enclosing class body declares, written outside every
/// class body.
pub fn is_outside_class_body(key: &str) -> bool {
    is_private_name_key(key) && key.len() == display(key).len() + 2 && key.ends_with("\0\0")
}

/// The class body that declares the member a key names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeclaringClass<'a> {
    pub name: &'a str,
    pub file: &'a str,
    pub body_start: usize,
    pub body_end: usize,
}

impl DeclaringClass<'_> {
    /// Whether this class body is `other`'s or lexically encloses it.
    pub fn encloses(&self, other: &DeclaringClass<'_>) -> bool {
        self.file == other.file && self.body_start <= other.body_start && other.body_end <= self.body_end
    }
}

/// `None` for a key no class body declares (and for a string key).
pub fn declaring_class(key: &str) -> Option<DeclaringClass<'_>> {
    if !is_private_name_key(key) {
        return None;
    }
    let mut parts = key.split('\0').skip(1);
    let name = parts.next().filter(|name| !name.is_empty())?;
    let file = parts.next()?;
    let body_start = parts.next()?.parse().ok()?;
    let body_end = parts.next()?.parse().ok()?;
    match parts.next() {
        None | Some("static") => {}
        Some(_) => return None,
    }
    Some(DeclaringClass { name, file, body_start, body_end })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declared_keys_name_their_class() {
        let key = "#x\0Base\0/src/a.ts\010\040";
        assert!(is_private_name_key(key));
        assert_eq!(display(key), "#x");
        assert!(!is_static(key));
        assert!(!is_outside_class_body(key));
        let class = declaring_class(key).unwrap();
        assert_eq!((class.name, class.file, class.body_start, class.body_end), ("Base", "/src/a.ts", 10, 40));

        let nested = declaring_class("#x\0Inner\0/src/a.ts\020\030\0static").unwrap();
        assert!(class.encloses(&nested));
        assert!(!nested.encloses(&class));
        assert!(is_static("#x\0Inner\0/src/a.ts\020\030\0static"));
    }

    #[test]
    fn undeclared_and_string_keys() {
        assert!(is_private_name_key("#x\0"));
        assert!(declaring_class("#x\0").is_none());
        assert!(!is_outside_class_body("#x\0"));
        assert!(is_outside_class_body("#x\0\0"));
        assert!(declaring_class("#x\0\0").is_none());
        assert!(!is_private_name_key("#x"));
        assert_eq!(display("#x"), "#x");
        assert_eq!(display("name"), "name");
    }
}
