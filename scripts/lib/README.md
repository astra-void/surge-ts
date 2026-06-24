# Default Lib Generator

This script refreshes the checked-in fallback default-lib bundle by copying the
local TypeScript package's `lib.*.d.ts` files from `node_modules`.

## Inputs

- `node_modules/typescript/lib/lib.*.d.ts`

## Output

- `crates/surge-ts-checker/generated-libs/lib.*.d.ts`
- `crates/surge-ts-checker/generated-libs/manifest.json`

## Usage

```bash
pnpm run lib:generate
pnpm run lib:test
```

The generator fails clearly if the local TypeScript package or lib directory is
missing. It does not fetch anything from the network.
