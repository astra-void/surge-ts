# surge-ts-checker

Embeddable, `tsc`-compatible TypeScript type checker. This is the library entry
point for [surge-ts](https://github.com/astra-void/surge-ts).

## Usage

```rust
use surge_ts_checker::{Checker, SourceFileInput};

let result = Checker::new()
    .no_implicit_any(true)
    .check(vec![SourceFileInput {
        file_name: "index.ts".to_string(),
        source_text: "const x: number = 1;".to_string(),
    }]);

assert!(result.diagnostics.is_empty());
```

`Checker` is a builder: configure options (`no_implicit_any`, `diagnostic_profile`,
`jobs`, or the full `CheckerOptions` set), then call `check` for a multi-file
program or `check_source` for a single in-memory file. A check returns the emitted
diagnostics together with `tsc`-compatibility stats.

Lower-level building blocks (default-lib loading and resolution) live in the
`lowlevel` module and are not covered by the stable-API guarantees.

## License

Licensed under either of [MIT](../../LICENSE-MIT) or
[Apache-2.0](../../LICENSE-APACHE) at your option.
