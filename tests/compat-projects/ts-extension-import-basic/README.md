# ts-extension-import-basic

An import path that resolves *by* its written TypeScript extension
(`'./util.ts'`) is `TS5097` unless `allowImportingTsExtensions` is on. A
`type`-only import is exempt, as is an import written in a declaration file.
surge resolved the specifier and reported nothing.
