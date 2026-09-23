# export-import-require-alias-basic

`export import local = require("m")` declares the alias `local` in the module
and exports it: the module's own code reads `local` as the module namespace
(value and qualified types), and importers reach it by name
(`import { local }`) or through the module namespace (`core.local`). A member
the aliased module lacks is still TS2339.
