// @module: preserve
// @surge-args: --diagnosticProfile native
// @surge-expect: TS2307
// @filename: types/globals.d.ts
// `import x = require("specifier")` is supported; a missing module surfaces
// the existing missing-module diagnostic instead of unsupported-declaration.
import Foo = require("foo");
