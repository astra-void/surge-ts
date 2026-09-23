# node-modules-root-file-esm

Under node16/nodenext an ESM-mode import skips the file probe of a package
root (`loadModuleFromSpecificNodeModulesDirectory` only loads
`node_modules/<name>` as a file when `rest != "" || !esmMode`), so the `.mts`
importer cannot see `node_modules/flat-types.d.ts` and reports TS2307.
