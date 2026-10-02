// Two sibling loops each declare `i`; the lowering keeps the initializer in a
// block of its own so they do not collide.
export function f() {
  for (let i = 0; i < 2; i++) {}
  for (let i = 0; i < 2; i++) {}
}
