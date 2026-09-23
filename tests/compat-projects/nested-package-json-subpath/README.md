# nested-package-json-subpath

`foo/bar` with a `node_modules/foo/bar/package.json` resolves through that
nested manifest's `types`/`typings`/`main` when the package root has no
`exports` to redirect around it (tsgo's
`loadModuleFromSpecificNodeModulesDirectory`). surge only probed the subpath
as a file and as `bar/index`.

- `foo/bar` (nested `types`) and `foo/@scoped` (nested `typings`) resolve.
- `gated/inner` is TS2307: `gated` has `exports`, which blocks the subpath.
- `plain/sub` has no nested manifest and resolves to its `index.d.ts`.
