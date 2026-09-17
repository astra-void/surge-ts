export const record = {
  get size(): number {},
  count(): number {},
};
export const expression = function (): number {};
export const arrow = (): number => {};
export const awaited = async (): Promise<number> => {};
export const nothing = async (): Promise<void> => {};
export const partial = (flag: boolean): number => {
  if (flag) {
    return 1;
  }
};
export const impossible = (): never => {};
export const generator = function* (): Generator<number> {};
export const thrower = (): number => {
  throw new Error("no");
};
declare function take(callback: () => number): void;
take((): number => {});
