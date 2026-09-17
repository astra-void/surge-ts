# template-expression-type-basic

tsc's `checkTemplateExpression`: an untagged template is a `string` — the fresh
string literal of its text when every interpolation is a constant — and a
possibly-symbol interpolation is TS2731 at the interpolation. surge typed
templates as unknown, so `const n: number = \`x\`` went unreported.
