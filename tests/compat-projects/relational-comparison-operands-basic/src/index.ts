declare const n: number;
declare const b: bigint;
declare const s: string;
declare const d: Date;
declare const o: { a: number };
declare const flag: boolean;
declare const mixed: number | bigint;
enum Level {
  Low,
}
declare const level: Level;

export const valid = [n < b, b <= n, d < d, o < o, s < "a", flag < flag, mixed < n, level < n, 1 < 2n];
export const invalid = [n < s, d < n, o < s, s < d];
