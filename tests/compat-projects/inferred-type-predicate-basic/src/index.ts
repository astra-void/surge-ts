declare const nums: (number | null)[];
declare const mixed: (string | number)[];

export function inferred(): void {
  const a: number[] = nums.filter((x) => x !== null);
  const b: number[] = nums.filter((x) => x != null);
  const c: number[] = nums.filter((x) => typeof x === "number");
  const d: number[] = nums.filter((x) => {
    return x !== null;
  });
  const e: string[] = mixed.filter((x) => typeof x === "string");
}

export function notPredicates(): void {
  const a: number[] = nums.filter((x) => !!x);
  const b: number[] = nums.filter((x) => Boolean(x));
  const c: number[] = nums.filter((x) => x !== null && x > 2);
  const d: number[] = nums.filter((x, index) => x !== null && index > 0);
}
