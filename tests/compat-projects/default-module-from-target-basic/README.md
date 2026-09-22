# default-module-from-target-basic

With no `module` written, tsgo derives it from `target` (`GetEmitModuleKind`:
ES2022 for a modern target), so `export =` is still TS1203. surge's own
`module` default is `preserve`, which would have silenced it.
