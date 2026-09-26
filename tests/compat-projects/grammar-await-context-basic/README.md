# grammar-await-context-basic

tsc's parser clears the await context for a class field's initializer
(`parsePropertyDeclaration`) and an enum's members (`parseEnumDeclaration`),
so an `await` there is TS1308 even inside an `async` function; a computed
field name and an `async` arrow in the initializer keep their own context. A
static block's body is parsed in an await context, so an `await` in one is
only TS18037, never also TS1308 (`checkGrammarAwaitOrAwaitUsing` reports the
static block first).
