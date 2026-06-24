# surge-ts

Embeddable, `tsc`-compatible TypeScript noEmit checker for Rust.

`surge-ts` is the high-level entry point: it bundles the type checker
(`surge-ts-checker`), `tsconfig.json` handling (`surge-ts-config`), diagnostics
(`surge-ts-diagnostics`), and the project-resolution layer (package `exports`/
`imports`, path mappings, and import-graph expansion) into a single dependency for
checking in-memory sources or whole `tsconfig` projects.

For just the in-memory checking API, depend on
[`surge-ts-checker`](https://crates.io/crates/surge-ts-checker) directly.

## License

Licensed under either of [MIT](../../LICENSE-MIT) or
[Apache-2.0](../../LICENSE-APACHE) at your option.
