declare function take(value: string): void;
declare const input: string | number;

// A module-scope `if` over a `const` alias narrows by the aliased condition.
const isText = typeof input === "string";
if (isText) {
  take(input);
}
if (!isText) {
} else {
  take(input);
}
export const exported = input !== undefined && typeof input === "string";
if (exported) {
  take(input);
}

// A `let` is not an alias: it may have been reassigned.
let mutable = typeof input === "string";
if (mutable) {
  take(input);
}
