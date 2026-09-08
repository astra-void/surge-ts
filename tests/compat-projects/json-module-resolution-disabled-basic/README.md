# json-module-resolution-disabled-basic

With `resolveJsonModule` off, a `.json` specifier is not a missing module — it
is a module the option refuses to resolve, and tsc says so with its own code and
hint (`TS2732`), not the generic `TS2307`. surge reported `TS2307` because it
had no `.json` support at all.

The flag defaults on for every resolver tsc 7.0.2 still accepts except `node16`,
so this project has to turn it off explicitly. The import of a path that does
not exist pins that an ordinary missing module still reports `TS2307`.
