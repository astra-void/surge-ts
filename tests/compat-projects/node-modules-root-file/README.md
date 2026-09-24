# node-modules-root-file

tsc's `loadModuleFromSpecificNodeModulesDirectory` loads a bare package name
as a file inside `node_modules` (`node_modules/flat-types.d.ts`) before
looking for a package directory, at every ancestor `node_modules`. The import
resolves, so its bindings are typed (`greet` returns `string`, TS2322 on the
`number` annotation); a name with neither a file nor a directory is TS2307.
