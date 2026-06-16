# Declaration Ingestion Foundation

v0.65 hardens the v0.64 declaration ingestion foundation. v0.69 supports narrow bare package declaration entrypoints, v0.69.1 hardens this support, v0.70 supports exact package declaration subpaths and `exports["."].types`, v0.84 hardens declaration-file export visibility for already-loaded modules without adding full declaration semantics, and v0.85 adds a generated default-lib foundation that loads a small supported subset from the local TypeScript package as ambient declarations. The real physical `node_modules/typescript/lib/lib*.d.ts` graph is now the **default** source of default libs in project mode (the `--physicalLibs` flag is no longer required); the generated subset is a fallback used only when the TypeScript package cannot be found. tsc-style lib selection is honored: `noLib`, `compilerOptions.lib`, the target's `lib.<target>.full.d.ts` seed when `lib` is absent, and recursive `/// <reference lib="..." />` resolution deduped by canonical path in dependency-first order. Diagnostics originating inside physical lib files are suppressed so unsupported internal lib syntax does not reach user diagnostics.

## Capabilities

- Loads `.d.ts` files from tsconfig `files` and `include`.
- Parses a narrow ambient declaration subset from loaded declaration files.
- Registers global `declare type`, `declare interface`, `declare const/let/var`, and `declare function` into a shared ambient global namespace.
- Supports exact `declare module "pkg"` blocks for ambient external modules.
- Resolves package `.d.ts` entrypoints via `types`, `typings`, exact `exports["."].types` / `exports["./x"].types`, or `index.d.ts` fallback.
- Resolves exact package declaration subpaths.
- Preserves export visibility through loaded declaration files for `export { ... } from ...`, `export type { ... } from ...`, `export * from ...`, and `export * as ns from ...` when the target declaration file is already loaded or resolved.
- Checks ambient external modules and resolved package entrypoints before package import stubbing fallback is invoked.
- Supports default exports, namespace imports, named re-exports, type-only re-exports, and star re-exports within the exact ambient-module subset.
- Merges duplicate `interface` declarations in the same type namespace: within a file, across global script/declaration files, in reopened `declare module "pkg"` blocks, and inside `declare global`. Conflicting property types report TS2717 and the first declaration's type wins. Duplicate ambient `var`/`const`/`function` globals remain first-wins / pinned. Ambient global type declarations are collected and merged across all declaration files *before* any `declare var`/`function`/`class` value type is lowered, so a value typed by a split global interface (e.g. `declare var Array: ArrayConstructor` with `from`/`of` added in a later lib file) sees every merged member.
- Lowers interface and object-type call signatures (`(value?: any): number`) so callable values such as `Number`/`String`/`Boolean`/`Array` invoke without `new`; same-named function members (overloads, including those gained through declaration merging) collapse into one permissive signature so a call matching any overload's arity is accepted; rest parameters in interface/object method signatures and type-predicate return types (`x is T`) are captured rather than dropped.
- Supports `declare global { ... }` inside module files, merging interface declarations into the global type namespace and adding supported global `var`/`function` declarations.
- Supports module augmentation: a `declare module "pkg"` block in a module file merges its exported interfaces and adds new exported functions/types into an already-resolved target module.
- Supports a narrow `declare class` (instance members, static side, and constructor signature) and merges a same-named `interface` into the class's instance members.
- Resolves a single `extends` base for both `class` and `declare class`: the base's instance members (own and inherited) merge into the derived instance type via the shared interface-heritage path, so inherited fields/methods are visible on property access and property calls. A base that resolves to `any`, fails to resolve inside a declaration file, or is an empty unmodelled lib/dependency stub keeps the derived type open (no cascade) rather than flagging every inherited access; an unresolved base in user source stays closed and reports the missing-member error. Unresolved/circular heritage degrades without cascading or recursing.
- Lowers `get`/`set` accessor members (on `class` and `declare class`) to instance properties through the existing object/interface property path: a getter contributes its return type, a setter-only accessor its parameter type, and missing annotations degrade to `any`.
- Declaration files remain symbol sources even when `skipLibCheck` suppresses their diagnostics in dependency graphs.

## Limitations

- The physical `lib*.d.ts` graph is loaded and lowered through the normal ambient pipeline, but the checker still models only a subset of TypeScript's type system, so full upstream `lib.d.ts` semantic parity is not guaranteed (e.g. precise overload selection, indexed-access types over imported declarations, and some nested generic assignability remain approximate). The generated subset fallback is narrower still. `@types` discovery is supported through configured `compilerOptions.types`/`typeRoots`, but is not full automatic ambient `@types` resolution.
- Modern package declaration resolution (conditional and pattern `exports`, `imports`, `typesVersions`, package self-name, exact subpaths) is supported on the declaration side; full runtime/JS entrypoint resolution and `main` parity remain out of scope.
- No runtime JS entrypoint resolution, or `import` / `require` runtime conditions beyond the declaration-side resolver.
- Declaration merging is narrow: full `namespace` value/runtime semantics, `enum` merging, class/`namespace` merging, and overload ordering across merged signatures remain unsupported.
- Module augmentation parity is narrow: augmenting an unresolved target keeps the TypeScript-like no-cascade policy (TS2307) rather than synthesizing the module.
- Unsupported syntax such as `declare namespace` value semantics and other unsupported declaration forms stays pinned with `typescript-rust::unsupported-declaration`.
- No `import = require()` / `export =`.
