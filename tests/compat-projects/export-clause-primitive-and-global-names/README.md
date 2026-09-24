# export-clause-primitive-and-global-names

tsc's `checkExportSpecifier` reports a local export clause naming something
that is not a local of the module (TS2661): a name that resolves to
`undefined`, `globalThis`, or a declaration of a global script file.
A primitive type name (`any`, `string`, `number`, `boolean`, `never`,
`unknown`) resolves to nothing, and `onFailedToResolveSymbol` reports it the
same way (`checkAndReportErrorForExportingPrimitiveType`) before falling back
to TS2304, which only `missingName` gets here. surge reported every one of
these as TS2304 except the ambient global.
