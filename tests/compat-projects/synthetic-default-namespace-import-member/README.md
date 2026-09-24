# synthetic-default-namespace-import-member

tsc's `resolveESModuleSymbol` for `import * as ns`: when the module can have
a synthetic default and is callable (`make.cts` assigns a function), already
has a `default`, or is a CommonJS file an ESM file imports (`counter.cts`),
the namespace object carries that synthetic default as its `default` member.
Between two ESM files (`esm.mts`) `default` stays the module's own export.
