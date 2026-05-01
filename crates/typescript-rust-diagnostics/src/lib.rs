//! Diagnostic types, catalog, constructors, and rendering helpers.

mod catalog;
mod category;
mod code;
mod diagnostic;
mod render;

pub use catalog::*;
pub use category::*;
pub use code::*;
pub use diagnostic::*;
pub use render::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_root_reexports_still_work() {
        let diagnostic = Diagnostic::ts2304("value", "example.ts");
        assert_eq!(diagnostic.code.to_string(), "TS2304");
        assert_eq!(
            render_diagnostics(&[diagnostic], "const value = 1;"),
            "error[TS2304]: Cannot find name 'value'.\n --> example.ts"
        );
    }
}
