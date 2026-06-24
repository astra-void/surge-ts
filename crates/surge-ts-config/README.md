# surge-ts-config

`tsconfig.json` parsing and source-file discovery for the
[surge-ts](https://github.com/astra-void/surge-ts) checker.

Parses `tsconfig.json` (including JSONC and `extends` chains), normalizes compiler
options, and resolves the set of files a project includes. Used by the surge-ts
CLI to drive project-mode checks.

## License

Licensed under either of [MIT](../../LICENSE-MIT) or
[Apache-2.0](../../LICENSE-APACHE) at your option.
