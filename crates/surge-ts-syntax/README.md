# surge-ts-syntax

TypeScript parsing layer for the
[surge-ts](https://github.com/astra-void/surge-ts) checker, built on
[oxc](https://oxc.rs).

This crate wraps oxc's allocator, parser, and AST into the syntax surface the
checker consumes. It is an internal building block; most embedders should depend
on `surge-ts-checker` rather than this crate directly.

## License

Licensed under either of [MIT](../../LICENSE-MIT) or
[Apache-2.0](../../LICENSE-APACHE) at your option.
