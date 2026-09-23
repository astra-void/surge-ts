# namespace-default-export-basic

`export default` inside a namespace is TS1319: on the `default` modifier of a
declaration (`checkGrammarModifiers`) and on the statement for an expression
(`checkExportAssignment`). An ambient module body is an ECMAScript-style
module, so a default export there is fine.
