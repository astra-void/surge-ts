# module-es2015-import-forms

Under `module: es2015` an import attribute clause on any import or export
declaration is TS2823 (`checkImportAttributes`, which stops there), and a
dynamic `import(…)` is TS1323 whatever its arguments
(`checkGrammarImportCallExpression`, which stops before counting them) —
`import()` and `import(a, b, c)` included, which tsc's parser keeps.
