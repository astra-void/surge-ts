# module-element-wrong-context

tsc's `checkGrammarModuleElementContext`: a namespace (TS1235), an ambient
module or `declare global` (TS1234), an import (TS1232), an export
declaration (TS1233), an export assignment (TS1231) or an `export default`
expression (TS1258) must sit directly in a file, a namespace body or a dotted
namespace. Anywhere else the error goes on the element's first token — the
`export` of an exported one — and tsc checks nothing else about it, so an
exported namespace in a function is not also TS1184. A default-exported class
or function is a declaration whose `export` is a misplaced modifier (TS1184).
