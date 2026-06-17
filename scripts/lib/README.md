# Default Lib Generator

This script regenerates the checked-in minimal default-lib layer from the local
TypeScript package installed in `node_modules`.

## Inputs

- `node_modules/typescript/lib/lib.es5.d.ts`
- `node_modules/typescript/lib/lib.dom.d.ts`

## Output

- `crates/surge-ts-checker/generated-libs/lib.es.generated.d.ts`
- `crates/surge-ts-checker/generated-libs/lib.dom.generated.d.ts`
- `crates/surge-ts-checker/generated-libs/manifest.json`

## Usage

```bash
pnpm run lib:generate
pnpm run lib:test
```

The generator fails clearly if the local TypeScript package or lib files are
missing. It does not fetch anything from the network. The WebAuthn transport
union is normalized to a canonical supported subset so the generated DOM lib
stays stable across local TypeScript revisions.
