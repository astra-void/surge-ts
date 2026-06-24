# surge-ts-types

Type representation and the assignability engine for the
[surge-ts](https://github.com/astra-void/surge-ts) TypeScript checker.

This crate defines the `Type` model (primitives, unions, objects, references,
tuples, and friends) and the assignability rules that decide whether one type is
assignable to another. It is consumed by `surge-ts-checker`; most embedders should
depend on the checker rather than this crate directly.

## License

Licensed under either of [MIT](../../LICENSE-MIT) or
[Apache-2.0](../../LICENSE-APACHE) at your option.
