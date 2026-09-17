declare const n: number;
declare const o: { a: number };
declare const text: string;
declare const maybe: number[] | undefined;
declare const unk: unknown;
declare const map: Map<string, number>;

export const a1 = [...n];
export const a2 = [...o];
export const a3 = [...text, ...map];
export const a4 = [...maybe];
export const a5 = [...unk];

function sum(...values: number[]): number {
  return values.length;
}
sum(...n);

for (const x of n) {
  void x;
}
for (const x of o) {
  void x;
}
for (const x of text) {
  void x;
}

declare function sized(size: number, message?: string): void;
export const min: (size: number, message?: string) => void = (...args) => sized(...args);

export async function drain(source: AsyncIterable<number>) {
  for await (const x of source) {
    void x;
  }
}
