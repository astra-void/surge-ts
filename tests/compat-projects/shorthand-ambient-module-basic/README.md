# shorthand-ambient-module-basic

`declare module "x";` declares a module with no body. tsc's
`isShorthandAmbientModuleSymbol` makes every binding imported from it —
default, named, namespace, `import =`, or re-exported with `export { … } from`
— the module symbol itself, which is `any` as a value: no TS2307, no TS2305,
no TS1192. A wildcard shorthand (`declare module "*.css";`) covers every
matching specifier, and a shorthand declared first stays shorthand when a
later block merges members into it. A module with a body still reports a
member it lacks (TS2305), and an undeclared module is still TS2307.
