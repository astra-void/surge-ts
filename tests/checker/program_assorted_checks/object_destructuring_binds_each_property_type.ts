// @surge-compare: messages
// The binding used to receive the *source* type, so every use compared
// against the whole object (`for (const { schema } of items) …`).
const items = [{ a: 1, b: "x" }];
export function f() {
  for (const { a, b } of items) {
    const bad: string = a;
    return [bad, b];
  }
  return [];
}
