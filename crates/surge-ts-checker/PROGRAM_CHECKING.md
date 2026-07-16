# Program Checking

`surge-ts-checker` exposes a program-level entry point for checking multiple files while distinguishing global script files from module files:

- `check_program(files: Vec<SourceFileInput>)`
- `check_program_with_options(files: Vec<SourceFileInput>, options: CheckerOptions)`

The API is intentionally narrow. v0.57.1 hardens relative module resolution-lite for loaded program files while keeping the single-file APIs unchanged, v0.59/v0.59.1 add a small generic syntax surface on top of the existing declaration prepass, v0.61 expands the module surface to cover default imports/exports, namespace imports, named re-exports, type-only re-exports, and star re-exports over loaded relative `.ts` files, v0.65 hardens the ambient declaration path for loaded `.d.ts` files, and v0.69/v0.69.1/v0.70 add and harden bare package declaration entrypoint and subpath support. v0.84 hardens already-loaded source and declaration export visibility, including namespace re-exports and exact declaration-only package `types` entrypoints, while staying out of full package/runtime resolution. v0.85 introduced a generated default-lib foundation (a small supported subset generated from the local TypeScript package), which is now the fallback rather than the primary source. v0.74 adds a narrow optional chaining and nullish coalescing compatibility foundation for expression evaluation. v0.74.1 hardens nested optional property/call chains, adds optional element access for arrays and tuples, and maintains ?? conservative undefined-removal. v0.82 is the project visibility hardening pass: directory-style `include` roots are treated recursively, `.tsx` files are visible without implying JSX semantics, and project mode now emits an explicit custom diagnostic when it discovers zero source files. v0.83 adds parser-safe binding-pattern parameter diagnostics and arrow-function expression support for `TS7031` on object binding elements without claiming full destructuring or callback contextual typing. v0.86 added the physical `lib.d.ts` loading path, which is now the default in project mode: it discovers the installed `typescript` package's `lib/` directory, follows the `/// <reference lib="..." />` graph, and lowers the real ES/DOM declarations through the normal ambient pipeline with default-lib interface declaration merging, `declare var`/`function` globals, `new X()` instance resolution, string index signatures, `readonly`/`this`-returning members, and a pragmatic `Promise<T>` -> `T` model for async/await. The generated subset is the fallback when the `typescript` package cannot be found, and `noLib: true` disables both (keeping standard/DOM globals unavailable). The `--physicalLibs` flag, a `.physicalLibs` marker file, and `SURGE_PHYSICAL_LIBS` no longer toggle loading; they are retained only as a debug aid that warns when physical loading was requested but the package was absent. See [PHYSICAL_LIBS.md](PHYSICAL_LIBS.md) for supported/unsupported surface. Full byte-for-byte TypeScript lib semantics remain in progress (notably overload resolution, `Awaited<T>`, call/construct signatures, and contextual callback typing).

## Public API

The checker crate keeps the existing single-file APIs and adds program-level wrappers:

- `check_source(source_text, file_name)`
- `check_source_with_options(source_text, file_name, options)`
- `check_program(files)`
- `check_program_with_options(files, options)`
- `SourceFileInput`
- `CheckerOptions`

These APIs remain stable in this phase. v0.58 adds compatibility-report
instrumentation in the CLI on top of these APIs without changing the checker
surface. None of these semantic checker APIs emit TS5112; TS5112 is strictly a
CLI/config-level diagnostic emitted prior to running semantic checking.

`CheckerOptions` is shared as a single `Arc<CheckerOptions>` across every
context in a run (the options carry the project-wide module-resolution
tables); internal shadow contexts go through
`CheckerContext::new_with_shared_options` rather than deep-cloning the
options.

## Checking Execution Model

The orchestrator lives in `src/program/mod.rs`. Each
`check_program_with_stats_and_jobs` run creates a fresh canonical
`ProgramTypeStore` and installs it thread-locally for the run (workers install
the same store on their threads); all program caches and the store are torn
down at end of run (`clear_program_type_caches` + `ProgramTypeStore::clear`).

### Serial vs parallel

- `jobs = 1` checks files serially; `jobs = 0` (the CLI's auto mode) selects a
  worker count from available cores capped by parsed statement count
  (`MIN_STATEMENTS_PER_WORKER`), so tiny programs stay serial; any other value
  requests that many workers (capped by file count).
- Parsing may also fan out to parse workers; loading, graph construction,
  declaration collection, and module binding remain serial.
- Parallel workers pull file indices from a shared atomic counter; results are
  merged by loaded-file order (`file_index`), never completion order, so
  diagnostics are deterministic across worker counts. Serial/parallel
  diagnostic equality is an asserted invariant.
- Before the parallel fan-out, every arena reachable by workers is frozen
  (`freeze_worker_reachable_arenas`): a late allocation on the
  non-thread-safe bump allocator panics deterministically instead of racing.
  Serial checking is exempt (single-threaded allocation is sound).

### Checker-context reuse and per-file reset

Both modes reuse one `CheckerContext` per worker — serial checking uses a
single reused context for the whole pass (cloning the large context per file
was a measured ~3% of check-phase time on tRPC). `check_program_file` starts
with `CheckerContext::begin_file_check`, the file-region reset:

- `resolved_named_types` is replaced with a fresh map (swapped, not cleared in
  place — resolutions depend on the consumer file's environment, and retained
  declaration environments may still reference the previous file's map);
- the diagnostic dedup index is cleared (diagnostics themselves were taken at
  the previous file's end);
- the `push_utility_diagnostic_once` keys use a baseline/overlay split: keys
  inherited from before the check phase form a shared baseline, and only the
  per-file overlay is cleared, so a reused context behaves byte-identically to
  a fresh clone without cloning the key set per file.

Any new per-file transient state must join this reset or it will leak across
files on a reused worker.

### Preliminary vs final analysis passes

Module analysis runs twice (`collect_module_analyses_with_bindings`): a
preliminary round makes types and import/export shapes available so the
export-table/import-binding/resolution-scope fixpoint can run, and a final
round re-analyzes every module against the settled bindings. Two rules follow
from the rounds being asymmetric:

- **Preliminary results must not install first-wins global state.**
  `declare global` augmentation *values* are lowered only in the final round
  (the `lower_global_augmentation_values` flag): insertion into
  `ambient_global_symbols` is first-wins, so a value typed against the
  incomplete preliminary environment would permanently shadow the correctly
  typed final-round value.
- Preliminary analyses, bindings, scopes, and per-round export tables are
  dropped at the `preliminary_release` boundary once the final round
  supersedes them (see MEMORY_REGIONS.md).

### Lexical declaration environments for dependency files

A declaration's body must resolve in its declaring module's scope, not the
consumer's:

- `module_scope_by_file` maps each module's source file to its resolution
  scope (local declarations + resolved imports). It is installed before the
  *final* analysis round — signature collection resolves parameter types
  through local aliases whose pre-attached scope carries no import layers —
  and refreshed before the check phase. The preliminary round deliberately
  runs without it (its outputs are superseded, and resolving the full import
  graph twice measurably regresses time/memory on large cyclic programs).
- `module_local_values_by_file` is the value analogue: `typeof <localValue>`
  inside an imported alias body resolves against the declaring module's
  values. It is built once before the (possibly parallel) check phase so all
  jobs share it read-only and results stay order-independent.
- While a dependency `.d.ts` body is expanded from another file, name lookup
  ignores the consuming module's local declaration table
  (`lookup_ignores_local_table` in `src/context.rs`) so a consumer-local name
  cannot shadow the dependency's own lexical scope.

## Global Script Model

Program mode treats the input files as one shared global script:

- Top-level `type` aliases are shared across files.
- Top-level `interface` declarations are shared across files.
- Top-level `typeof value` type queries resolve across files in a narrow type-position subset.
- Constraints are parsed and retained but are not enforced in this phase.
- Top-level function declarations are shared across files.
- Function bodies can reference shared declarations from earlier or later files.
- Top-level `let`, `const`, and `var` declarations remain file-local.
- Relative named imports, type-only named imports, default imports, namespace imports, side-effect imports, local named export lists, named re-exports, type-only re-exports, and star re-exports are resolved across loaded `.ts` files.
- Module files are isolated from the global-script prepass in this phase.
- Loaded `.d.ts` files can contribute the v0.64 ambient subset: simple global type/interface/value/function declarations and exact `declare module "pkg"` blocks.
- Package imports resolve to `.d.ts` entrypoints via `types`, `typings`, `exports["types"]`, or `index.d.ts` and behave like external modules.
- Ambient modules and resolved packages resolve before package import stubbing fallback.
- Default exports, namespace imports, named re-exports, type-only re-exports, and star re-exports inside exact ambient modules and resolved package modules are pinned in this phase.
- Duplicate ambient modules and duplicate ambient globals are first-wins / pinned rather than merged.
- Only declaration-oriented `node_modules` lookup is supported, and declaration files can still act as symbol sources even when `skipLibCheck` suppresses their diagnostics.
- Project mode supports focused declaration-side package resolution for package
  `types`/`typings`, full and pattern `exports` type targets (conditional
  objects, nested conditions, and a single `*` wildcard), `typesVersions`
  patterns, package-local `imports` (`#alias`), and package self-name imports.
  Condition selection follows package-author key order with the active set
  `import`/`require` (per importer module format; bundler is always `import`),
  `types`, `node` (node16/nodenext only), then `customConditions`. When `exports`
  is present it is authoritative: a non-matching subpath is blocked (TS2307)
  rather than falling back to file probing, matching node16/nodenext/bundler.
  `resolvePackageJsonExports: false` / `resolvePackageJsonImports: false` bypass
  the respective field.
- Targets are probed for declaration variants (`.d.ts`/`.d.mts`/`.d.cts`,
  including extension substitution around `.js`/`.mjs`/`.cjs`) and rejected if
  they escape the package root. Runtime JavaScript resolution and full Node
  loader parity remain out of scope; declaration-only entrypoints are found.
- Multiple `*` wildcards in a single `exports`/`imports`/`typesVersions` key are
  not supported; `typesVersions` version ranges are matched against the pinned
  TypeScript 6.0.

## What is shared

- Type declarations and function signatures are collected in a prepass before statement checking begins.
- Checker value symbol tables store shared `SymbolInfo` handles. Cloning a
  `SymbolTable` copies handles, not symbol payloads or nested function/union
  type payloads. v1.2.4 keeps that v1.2.3 storage model and reduces hot
  materialization by borrowing visible symbols for function-local variable
  checks, lazily cloning function-parameter scope tables only when parameter
  initializers need them, and restoring `ScopeStack` visible-symbol shadows on
  pop instead of rebuilding the whole flat visible map. v1.2.5 makes the
  `SymbolTable` map itself copy-on-write (`Arc<HashMap<..>>` with `Arc::make_mut`
  on mutation), so the multi-pass module-binding fixpoint's table clones share
  one map and only deep-copy when a shared table is actually mutated.
- v1.2.5 also adds per-run memoization for path canonicalization and relative
  module resolution, and gates the instrumentation-counter mutex behind
  `--timings`. These caches are thread-local and cleared at the start of each
  `check_program` run, so they never leak resolved indices or canonical paths
  across runs. They are pure performance changes with no effect on emitted
  diagnostics.
- `SymbolInfo` entries are treated as immutable once shared. Narrowing or
  assignment-style updates create a replacement `SymbolInfo` and swap the table
  handle instead of mutating a shared payload in place.
- Declaration diagnostics keep the declaration file name.
- Consuming expression diagnostics keep the consumer file name.
- Script files participate in the global prepass.
- Module files are checked with file-local type declarations and function signatures plus resolved module bindings.
- CLI project mode loads the physical `lib*.d.ts` graph by default and seeds the program with those ambient declarations. When the physical graph is unavailable, the generated default-lib subset is injected ahead of program files instead, so direct program-checker callers without physical-lib discovery still get an ambient core/DOM fallback.
- Loaded declaration files are checked as ambient inputs, not as normal runtime files.
- Single-file checking still does not read sibling files.

## Diagnostic Order

Program diagnostics are emitted in a fixed order:

1. Parser diagnostics in input-file order.
2. Global type-declaration diagnostics in input-file order.
3. Global function-signature diagnostics in input-file order.
4. Module export/import diagnostics in input-file order, including default-import/default-export checks, namespace imports, named re-exports, type-only re-exports, and star re-exports.
5. Per-file statement and function-body diagnostics in input-file order.

This ordering is part of the v0.55.1 compatibility contract.

## Type Declarations

- Type aliases and interfaces are stored in one shared namespace.
- Generic aliases and interfaces still live in that same namespace; arity errors are diagnosed on the type reference span and type-parameter references inside the declaration lower to the instantiated body.
- The first declaration wins.
- Later duplicates report TS2300 in the duplicate file.
- Duplicate declarations do not replace the original declaration.
- Named-type resolution uses the declaration file while resolving the declaration itself.
- Cycles and unknown names inside a declaration do not cascade into consumer files.

## Function Declarations

- Top-level function declarations are shared across files.
- The first declaration wins for calls.
- Later duplicates report TS2393 in the duplicate file.
- Duplicate function bodies are still checked against their own declared signatures.
- Duplicate function declarations do not replace the global callable symbol.
- Calls use the first declaration's signature.
- Function signature diagnostics are emitted in prepass order, before statement checking.
- Unannotated object binding elements in function and arrow parameters now emit TS7031 on the local binding name span when `noImplicitAny` is enabled.
- Arrow function expressions are parsed and checked as expression values so their parameter diagnostics still fire in variable initializers, call arguments, and expression statements.
- This phase still does not implement full destructuring, callback contextual typing, or generic callback inference.

## Modules

See [MODULES.md](./MODULES.md) for the import/export syntax surface, module-file boundary, and current limitations.

## Local Variables

- Top-level `let`, `const`, and `var` remain file-local in this phase.
- File-local variables do not leak into later files.
- File-local variables are visible to later statements in the same file under the current sequential checker policy.
- File-local variables can be visible inside function bodies when they were declared earlier in the same file.

## Parser Diagnostics

- Parser diagnostics preserve the parser file name.
- Parser errors do not stop type or function prepasses for other files.
- Parser diagnostics do not prevent statement checking of other files.

## CLI Project Mode

`surge-ts-cli --project` loads all files from the `tsconfig`, checks the program as a whole with `check_program_with_options(...)`, and renders diagnostics grouped by diagnostic file name.

- Diagnostics are grouped in loaded-file order.
- Diagnostics for files not present in the loaded list are rendered at the end when possible.
- `--jobs` is deterministic project-checking infrastructure only. Shared loading, graph construction, declaration collection, and module binding remain serial; parsing and the per-file checking phase can run in parallel. Worker results are merged by loaded-file order, not completion order (see "Checking Execution Model" above).
- `--showConfig` still prints the normalized config and exits successfully.
- `--showSpans` prints the diagnostic code and span metadata before the rendered excerpt.
- `--compatReport` prints a compatibility summary with loaded-file count,
  total diagnostic count, counts by code, counts by file, parser-error
  grouping, file-kind counts, suppressed totals, and a visibility warning
  when no source files were loaded. It is raw measurement, not a root-cause
  classifier.
- Generic type arguments on references are parsed and lowered for explicit
  alias/interface instantiation.
- v1.1 supports narrow generic indexed access after concrete substitution,
  including `T["key"]`, `T[K]`, and `T[keyof T]` when the receiver/key have
  been substituted to concrete types. Fully unresolved generic indexed access
  and constraint enforcement remain unsupported.
- Narrow generic call-site inference exists for simple direct calls, repeated-
  parameter calls, and array-element calls. Explicit type arguments are still
  preserved and applied when present, but full generic inference, overload
  resolution, generic classes, callback contextual typing, higher-order
  inference, and tuple-valued implicit generic returns remain unsupported.
- `--maxDiagnostics` limits rendered diagnostics but does not change the total
  counts in the compatibility summary.
- `.tsx` files have a parser-safe JSX foundation: JSX elements, fragments,
  attributes, and `{...}` expression containers parse in expression position, and
  JSX expressions infer a conservative `JSX.Element` stand-in (an opaque empty
  object that renders as `Element`). Child and attribute expression containers
  and capitalized component / member tags are walked for ordinary diagnostics
  (so unresolved names still report `TS2304`). `JSX` namespace resolution,
  `JSX.IntrinsicElements` prop validation, React globals, DOM globals, and the
  JSX transform remain out of scope.
- Positional single-file mode still uses the single-file checker APIs and does not resolve sibling files.
- Diagnostic span policy lives in [DIAGNOSTIC_SPANS.md](./DIAGNOSTIC_SPANS.md).
- v0.81 synthetic utility lowering covers `Record<K, T>`, `Partial<T>`, `Pick<T, K>`, and `Omit<T, K>` for concrete object/interface shapes and string-literal key unions.
- A narrow conditional-type evaluator handles `T extends U ? X : Y` (concrete evaluation plus distribution over a naked type parameter), which backs `Exclude`, `Extract`, and `NonNullable`; `ReturnType`/`Parameters` stay as synthetic lowerings over concrete function types. This does not imply physical `lib.d.ts`, `@types`, DOM/Node globals, conditional-type inference, nested/arbitrary `infer`, recursive conditionals, or the rest of the utility-type ecosystem; unsupported conditionals degrade to `unknown`, and full index signatures remain unsupported.
- A narrow template literal type evaluator expands a template into a deduped string-literal union when every interpolation resolves to a finite string/number/boolean literal union (including after generic substitution and over `keyof`). Broad or unresolved interpolations (e.g. `` `id:${string}` ``) degrade to `string` rather than cascading, which under-reports relative to tsc but never yields a false positive. Recursive expansion, in-template `infer`/pattern matching, and intrinsic string utilities (`Uppercase`, etc.) remain unsupported.

## Upstream Virtual Files

The compatibility harness still splits `// @filename:` comments into virtual files for a small subset of upstream TypeScript-Go fixtures.

- The resulting virtual files are passed to the program checker in `virtual_files` mode.
- This preserves multi-file diagnostic behavior for those fixtures.
- This is still not full upstream baseline compatibility.

## Next phase

The next phase should still be chosen from compatibility-report output rather
than reshaping the single-file APIs.

## Ambient Globals

Ambient globals from loaded `.d.ts` files are gathered into `ambient_global_symbols` and `ambient_global_type_declarations`, then mixed into modules and scripts.

## Diagnostic Profiles

The checker supports two diagnostic profiles, controlled via `CheckerOptions.diagnostic_profile` or the `--diagnosticProfile` CLI flag:

1. **`Tsc` (Default)**: Aims to match the TypeScript compiler (`tsc`) exactly. This includes applying targeted type widening (like on `satisfies`) and allowing known diagnostic cascades (like outer assignability failures) to appear if TypeScript emits them.
2. **`Native`**: Uses `surge-ts`-specific behaviors. For example, it aggressively returns `Unknown` from failed contextual checks (like `satisfies` failures) to suppress noisy downstream cascade errors. This produces a cleaner developer experience but diverges from the TypeScript compiler baseline.

The `compat-projects` oracle testing runs exclusively in `tsc` profile.
`CheckerOptions::default().diagnostic_profile` is `DiagnosticProfile::Tsc`, and the CLI default stays `tsc` unless `--diagnosticProfile native` is explicitly requested.
