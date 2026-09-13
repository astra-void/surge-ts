export const ready: boolean = true;

// A `const` with no initializer is a grammar error even with an annotation.
const pending: number;

declare const ambient: number;

declare namespace Ambient {
  const inside: number;
}

const numbers: number[] = [1, 2, 3];
for (const value of numbers) {
  void value;
}
for (const key in { a: 1 }) {
  void key;
}

export { pending, ambient, numbers };
