# callable-object-function-members

An object type with a call or construct signature reads the members it does
not declare from the global `Function` interface (tsc's `getPropertyOfType`
falls back to `globalFunctionType`): `arguments`, `caller`, `length`, `name`,
`prototype`, `call`, `bind` — on an interface as on an object literal type. A
member neither the type nor `Function` declares is still TS2339.
