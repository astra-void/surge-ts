// `let` and `const` stay block-scoped.
export function f() {
{
let scoped = 1;
const alsoScoped = 2;
scoped;
alsoScoped;
}
return scoped + alsoScoped;
}
