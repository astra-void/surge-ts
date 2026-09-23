# import-require-in-namespace-grammar

An `import x = require()` inside a namespace is TS1147 on the module name, and
tsc stops there (`checkExternalImportOrExportDeclaration`): even exported and
under an ECMAScript `module` it is not also TS1202.
