# umd-global-binding

tsc's binder declares `export as namespace X` of a declaration module in the
global scope as an alias of the module (`resolveExternalModuleSymbol`): the
`export =` entity's value, type and namespace members (`WidgetLib`, a class
with a merged namespace; `Tools`, a module-local namespace of the same name),
or the module namespace (`Lib`). A script reads the value freely; a module may
use the alias in type positions, and a value use is TS2686, which holds while
every declaration of the global is a UMD export.
