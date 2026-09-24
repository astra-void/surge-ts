# import-equals-namespace-alias-as-value

tsc's `onFailedToResolveSymbol` tries `checkAndReportErrorForUsingNamespaceAsTypeOrValue`
before `checkAndReportErrorForUsingTypeAsValue`: a value use of a name whose
alias target is an uninstantiated namespace is TS2708 ("Cannot use namespace as
a value"), even when the namespace merges with an interface. `import N =
require("./ns")` over `export = N` is such an alias; surge answered TS2304 (a
namespace of types only) or TS2693 (merged with an interface). An interface
alone stays TS2693, and `export = N` re-exporting the alias is not a value use.
