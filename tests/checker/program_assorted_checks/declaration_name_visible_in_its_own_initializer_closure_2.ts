// The declared name resolves inside a nested function body in its own
// initializer; a direct self-read stays a temporal-dead-zone error.
export function f() {
const x = x + 1;
return x;
}
