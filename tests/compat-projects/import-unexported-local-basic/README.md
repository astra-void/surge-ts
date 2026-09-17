# import-unexported-local-basic

Importing a name the target module declares at its top level without exporting
it is TS2459 ("declares … locally, but it is not exported"), or TS2614 when the
module has a default export. surge's module export tables also carry a module's
local values, so the import silently bound the local — including its type.

The verdict is taken from the module's syntax only when that is conclusive: a
source file whose exports include no `export *`, `export =`, or unparsed form.
Exported names, including one exported under another name, import cleanly. Not
covered yet: a module with an `export *`, whose local names tsc still reports
when no re-export supplies them.
