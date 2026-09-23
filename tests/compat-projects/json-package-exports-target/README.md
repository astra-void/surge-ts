# json-package-exports-target

Under `resolveJsonModule`, a package `exports` target naming a `.json` file
resolves to it: tsgo strips the extension and tries `.json` among its fallback
extensions (`tryAddingExtensions`), and the file joins the program as a JSON
module. surge probed only declaration and JavaScript extensions.

- `actually-json` resolves to `index.json`, typed `{ version: number }`.
- `actually-json/missing` targets a file that does not exist: TS2307.
