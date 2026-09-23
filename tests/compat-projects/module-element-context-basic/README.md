# module-element-context-basic

tsc's `checkGrammarModuleElementContext`: a statement that must be a module
element written inside a function or block — an import (TS1232, including
`import x = ...`), an export list or `export *` (TS1233), an ambient module or
`global` augmentation (TS1234), a namespace (TS1235), and `export default
<expression>` (TS1258). A default-exported declaration there is a misplaced
`export` modifier instead (TS1184). Nested namespace bodies and namespace
members are module elements.
