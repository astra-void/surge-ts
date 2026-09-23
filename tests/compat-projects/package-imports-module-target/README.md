# package-imports-module-target

A package.json `imports` target that is not a `./` path names a module, which
tsgo resolves from the package scope as if it were written there
(`loadModuleFromTargetExportOrImport`): `"#self": "self"` reaches the
package's own `exports` by self-name, and `"#dep/*": "dep/*"` a dependency's
file. surge joined the bare target onto the package directory.

- `#self`, `#dep` and `#dep/sub` resolve.
- `#missing` names a package that is not installed and `#parent` a `../`
  target, which `imports` rejects: both TS2307.
