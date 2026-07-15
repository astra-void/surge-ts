# Module Resolution

Audit and architecture reference for every place surge-ts resolves a module
specifier. Kept current alongside resolver changes; the fixture list at the end
pins the behavior described here.

## Resolver call-site inventory

Resolution happens in two layers with different jobs:

* **Loader layer** (`surge-ts` crate, `Project::check`): decides which files to
  read from disk and builds the project-wide `resolved_modules` map handed to
  the checker. Runs against the real filesystem.
* **Checker layer** (`surge-ts-checker`): binds import/export symbols between
  already-loaded program files. Never reads new files from disk; relative
  resolution matches candidates against the loaded-file identity map.

| # | Call site | Syntax forms | Importer context used | Mode awareness | Resolver | Cache | Result | Failure → diagnostic | Feeds graph | Feeds binding |
|---|-----------|--------------|----------------------|----------------|----------|-------|--------|---------------------|-------------|---------------|
| 1 | `import_graph::expand_project_inputs` | static `import`, `import type`, `export {} from`, `export * from`, `export * as ns from` | importer file dir (relative only) | none | `resolve_relative_candidate` / `resolve_paths_alias_candidate` | per-call probe cache (path→is_file) | `PathBuf` loaded into inputs | no (silent skip) | yes | indirectly (loads the file) |
| 2 | `package_declarations::resolve_package_declaration_entrypoints_with_cache` | same statement set, bare/`#` specifiers only | importer dir + importer file (ESM inference) | condition set only (`import`/`require` from importer format) | `resolve_package_entrypoint` | `package_json_cache`, `entrypoint_cache` (importer-dir-aware), per-importer-scope resolved map | canonical file string per (scope, specifier) | no (checker reports later) | yes | via `resolved_modules` |
| 3 | `path_mapping::resolve_path_mappings` | same statement set, non-relative specifiers | none (correct: `paths`/`baseUrl` are importer-independent in tsc) | none | shared `surge_ts_config::paths_candidates` + candidate probe against loaded files | none | specifier→file map | no | no | via `resolved_modules` |
| 4 | `package_declarations::resolve_type_packages` | `compilerOptions.types` | project root | n/a | `resolve_type_directive_in_roots` | package_json cache | loaded `.d.ts` files + effective names | TS2688 | yes | ambient |
| 5 | `ReferenceTypeDirectiveResolver` | `/// <reference types>` / `/// <reference path>` | referencing file dir (path form) | n/a | same type-root logic | per-name cache | loaded files | TS2688 (gated by skipLibCheck for .d.ts) | yes | ambient |
| 6 | checker `modules/imports.rs::try_resolve_module` | all import kinds incl. side-effect, `import = require` | `ctx.file_name` (relative only) | none | `resolved_modules` map → ambient modules → `resolve_relative_module` | thread-local `(importer, specifier)` memo | `ModuleExportTable` + file index | TS2307/TS2882/TS2305/TS2614 | no | yes |
| 7 | checker `modules/exports/table.rs::try_resolve_module_export_table` | `export ... from`, `export * from`, `export * as ns` | `ctx.file_name` | none | same chain as 6 | same memo | export table + index | via consumers | no | yes |
| 8 | checker `modules/diagnostics.rs` (`module_has_default_export` etc.) | default-import diagnostics | none (substring match on file name!) | none | `resolved_modules` + name heuristics | none | bool | shapes TS2613/TS2614 choice | no | no |
| 9 | checker `driver.rs::is_runtime_js_only_module` | import-time stub decisions | none | none | `resolved_modules` | none | bool | shapes stub behavior | no | no |
| 10 | JSX automatic runtime | `jsx: react-jsx` | none | none | **not resolved as a module** — boolean `jsx_automatic_runtime` flag; ambient React types must come from loaded `@types` | n/a | n/a | n/a | no | no |
| 11 | default lib loading | `lib` entries | project root | n/a | `load_default_lib_inputs` (physical TS package discovery) | own io cache | lib file set | warning only | yes | ambient |

Not represented anywhere (parser gaps, deferred): dynamic `import()`
expressions parse to `ParsedExpression::Unknown`; `import("pkg").T` type nodes
and `typeof import("pkg")` are not modeled (`parser/types.rs` maps them to
unmodeled types). They contribute nothing to graph expansion or binding.
`require("pkg")` bare calls are not module-bearing; `import x = require("m")`
is supported and flows through call site 6.

## Current flow

```
Project::check
├─ read tsconfig file list
├─ load default libs
├─ loop until fixpoint:
│  ├─ package_declarations  (bare + # specifiers → load .d.ts entrypoints)
│  │    └─ per-importer-scope resolved map → merged into resolved_modules
│  ├─ import_graph          (relative + paths aliases → load .ts sources)
│  └─ reference-type resolver (/// <reference …>)
├─ path_mapping             (paths/baseUrl → resolved_modules, loaded files only)
└─ Checker::check(inputs, CheckerOptions { resolved_modules, … })
     └─ per-file import/export binding:
        resolved_modules lookup → ambient modules → relative resolution
```

## Duplicated implementations (and their status)

1. **Relative candidate generation** existed in three places with drifting
   policies: `import_graph.rs`, checker `modules/resolution.rs`, and
   `path_mapping.rs::path_resolution_candidates`. Now unified in
   `surge_ts_checker::modules::candidates` (re-exported through
   `lowlevel::resolution_candidates`); all three call sites consume it.
2. **`paths` pattern matching** existed in `import_graph.rs` and
   `path_mapping.rs` with different target policies (import graph required
   `./` targets and anchored at the config dir; path mapping accepted any
   target anchored at `baseUrl`). Now unified in
   `surge_ts_config::select_path_mapping_targets` with tsc pattern-priority
   rules; both call sites consume it.
3. **`exports`/`imports`/`typesVersions` selection** was already shared
   (`package_resolution.rs`, pure functions). Kept.
4. **Canonicalization caches**: `surge_ts_config::paths` and checker
   `src/paths.rs` each keep a thread-local canonicalize memo. Deliberate (crate
   boundaries, different value types); both are cleared per run.

## Extension-substitution matrix

Centralized in `surge_ts_checker::modules::candidates`. "Sub" = specifier
extension replaced in place; the path never becomes a directory lookup.

| Specifier shape | Candidates (in order) | Directory index probing |
|-----------------|----------------------|------------------------|
| extensionless `./x` | `x.ts`, `x.tsx`, `x.d.ts` | `x/index.ts`, `x/index.tsx`, `x/index.d.ts` |
| `./x.js` | `x.js` (exact, loaded-graph only), `x.ts`, `x.tsx`, `x.d.ts` | **never** |
| `./x.jsx` | `x.jsx` (exact), `x.ts`, `x.tsx`, `x.d.ts` | **never** |
| `./x.mjs` | `x.mjs` (exact), `x.mts`, `x.d.mts` | **never** |
| `./x.cjs` | `x.cjs` (exact), `x.cts`, `x.d.cts` | **never** |
| `./x.ts` | `x.ts` exactly | never |
| `./x.tsx`, `./x.mts`, `./x.cts`, `./x.d.ts`, `./x.d.mts`, `./x.d.cts`, `./x.json` | unsupported (pinned; see deferred list) | never |
| `.` / `..` | directory-index set of the target directory | yes |

Rules deliberately **not** implemented because tsc does not have them:

* `.js` never substitutes `.mts`/`.cts`; `.mjs` never substitutes `.ts`;
  `.cjs` never substitutes `.ts`.
* Extensionless specifiers never probe `.mts`/`.cts`/`.d.mts`/`.d.cts`
  (tsc's default `tryAddingExtensions` case tries `.ts`/`.tsx`/`.d.ts` only;
  the m/c flavors are reachable only through explicit `.mjs`/`.cjs`
  specifiers).
* Directory index probing tries `index.ts`/`index.tsx`/`index.d.ts` only —
  no `index.mts`/`index.cts`.

Historical bugs this replaced: explicit `.js`/`.mjs`/`.cjs` specifiers used to
fall through to `<stripped>/index.*` directory candidates, and extensionless
probing included the full m/c flavor set (both diverged from tsc).

## Relative-resolution rules by moduleResolution

Supported kinds: `node16`, `node20`, `nodenext`, `bundler`. Legacy `classic` /
`node` / `node10` are rejected at config load with a diagnostic and downgraded
to `bundler` (deferred; see below).

* **bundler** — extensionless relative imports resolve; directory index
  probing allowed; `.js`/`.jsx`/`.mjs`/`.cjs` substitution allowed.
* **node16/node20/nodenext** — condition selection distinguishes the
  importer's module format (`importer_is_esm`: extension `.mts`/`.cts` wins,
  else nearest `package.json` `"type"`). ESM-relative extension *enforcement*
  (TS2835 for extensionless ESM imports) is not yet implemented — extensionless
  relative imports currently resolve as in bundler mode (deferred, documented
  below).

## `paths` and `baseUrl`

Selection rules (in `surge_ts_config::paths`):

* An exact (starless) pattern equal to the specifier wins outright.
* Otherwise the matching single-`*` pattern with the longest literal prefix
  wins; first-in-config order breaks prefix-length ties (tsc
  `findBestPatternMatch`). JSON insertion order never decides between
  patterns of different specificity.
* Within the winning pattern, substitutions are tried in author order.
* Substitution targets need not start with `./`; they resolve against
  `baseUrl` when set, else the config directory. Oracle note (tsc 7.0.2):
  `baseUrl` itself is flagged TS5102 and non-relative targets TS5090 at the
  *config* level, but resolution still succeeds against the config directory
  — surge matches the resolution behavior; the config diagnostics are
  deferred.
* When no pattern matches and `baseUrl` is set, the bare specifier resolves
  against `baseUrl` directly.

Both the import-graph expander (loads files from disk) and the path-mapping
pass (maps specifiers to already-loaded files for the checker) consume the
same selection function, so graph expansion and checker binding agree.

## Package resolution

`package_declarations` + `package_resolution` (see module docs there). Key
properties:

* Walks `dir/node_modules/<pkg>` ancestor chain from the importer directory,
  with `@types/<mangled>` fallback per level; `exports` (when present and
  honored) is authoritative — a non-matching subpath is blocked, no file-probe
  fallback.
* `imports` (`#alias`) resolves against the importer's nearest enclosing
  `package.json`; results are scoped per package directory.
* Self-name imports resolve through the enclosing package's own `exports`.
* Condition membership: mode condition (`import`/`require`), `types`, `node`
  (non-bundler only), then `customConditions`; priority is package-author key
  order.
* `typesVersions`: first matching range key wins; exact paths beat patterns;
  longest-prefix pattern wins.

## Cache-key audit

| Cache | Key | Sound? |
|-------|-----|--------|
| `entrypoint_cache` | (canonical importer dir, package name, subpath, is_imports, importer_is_esm) | yes — options are per-resolver-instance |
| `package_json_cache` | absolute path | yes |
| loader per-scope resolved map | (importer package scope, specifier) | yes — scope = nearest enclosing package dir for bare/`#` imports |
| checker `RELATIVE_MODULE_CACHE` | (importer file name, specifier), cleared per run | yes — file set fixed within a run |
| `resolved_modules` (checker options) | importer-scope-aware: scoped map consulted with the importing file's scope first, then the project-root scope map | yes for package/`#` entries; `paths`/`baseUrl` entries are importer-independent by construction |
| import-graph `probe_cache` | absolute candidate path → is_file | yes (per call) |
| config/checker canonicalize caches | path, cleared per load/run | yes while FS stable |

## Determinism

Inputs that could order-depend and how they are pinned:

* `paths` pattern choice: longest-prefix rule, not map iteration order.
* `exports`/`imports` condition choice: package-author key order
  (serde_json `preserve_order` feature keeps it).
* Wildcard `@types` discovery: directory names sorted before use.
* Package resolution queue: BFS in source order; per-scope maps keep
  first-resolution-wins within one (scope, specifier) key only.

## Symlinks and casing

* `canonicalize_if_exists` (realpath) is applied to every loaded file and to
  resolution results before identity comparison, so two symlink paths to one
  real file collapse to a single program file. `preserveSymlinks` is **not**
  supported (no option surface); pnpm virtual-store layouts work through
  realpath collapse.
* Case: file identity is the canonicalized path string, byte-compared.
  Extension checks use ASCII-lowercase. On case-insensitive filesystems
  (default macOS) `canonicalize` yields the on-disk casing, so a wrong-case
  import resolves like tsc-on-macOS does (tsc additionally reports
  forceConsistentCasingInFileNames drift — not modeled, deferred).

## Deferred / unsupported (intentional)

* `classic` / `node` / `node10` moduleResolution kinds (config downgrade +
  diagnostic).
* Node16/NodeNext ESM relative-extension enforcement (TS2835) and
  import-vs-require *syntax* mode distinction for relative imports. Condition
  selection does distinguish importer format for package `exports`.
* Dynamic `import()`, `import("m").T`, `typeof import("m")` — parser gaps;
  no graph expansion or binding.
* Explicit `.tsx`/`.mts`/`.cts`/`.d.*`/`.json` relative specifiers
  (`allowImportingTsExtensions`, `resolveJsonModule` unsupported).
* `preserveSymlinks`, `rootDirs`, `forceConsistentCasingInFileNames`.
* Full runtime `main` JS entrypoint parity (declaration-side only).
* `resolution-mode` import-attribute / `/// <reference types … resolution-mode>`.
* `paths`-match authority: in tsc a *matched* pattern whose targets all fail
  ends resolution (no `node_modules` fallback). surge's `paths` pass and
  package-declaration pass are independent, so such a specifier can still
  resolve as a package. Pre-existing divergence, kept for now.
* Config-level diagnostics TS5090 (non-relative `paths` target without
  `baseUrl`) and TS5102 (`baseUrl` removed in TS7).
