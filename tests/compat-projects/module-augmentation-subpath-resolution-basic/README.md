# module-augmentation-subpath-resolution-basic

A `declare module "pkg/sub"` augmentation name resolves like an import of that name, including an explicit `.ts` extension that finds the `.d.ts` beside it under `allowImportingTsExtensions` (tsc's `tryAddingExtensions`). tsc resolves augmentation names but only loads the files imports reach, so a target that resolves but nothing imports is still TS2664, as is a name that does not resolve.
