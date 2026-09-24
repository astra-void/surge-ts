# module-resolution-follows-module-kind

A tsconfig that writes `"module": "node16"` and no `moduleResolution`
resolves as node16 — tsgo's `GetModuleResolutionKind` derives the resolver
from the emit module kind (`node16`/`node18`/`node20` → node16, `nodenext` →
nodenext, anything else → bundler). surge fell back to bundler, so the `node`
export condition never matched and every import resolved with `import`.

- `inner/node-only` resolves through its `node` condition.
- `inner/browser-only` has no condition node16 matches: TS2307.
- `src/index.ts` sits in a `"type": "commonjs"` scope, so it resolves
  `inner` with `require` (TS2305 for the ESM-only `fromEsm`); `src/esm.mts`
  resolves it with `import` (TS2305 for `fromCjs`).
