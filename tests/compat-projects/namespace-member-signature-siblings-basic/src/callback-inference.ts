declare function box<S>(initial: S | (() => S)): S;
declare function resultOf<T>(value: T | (() => T)): T;

export const lazily = box(() => 1);
export const eagerly = box("two");
export const wrong: string = box(() => 3);
export const fromValue: string = resultOf("four");

export const fromBlock = box(() => {
  return 5;
});
export const fromBlockLocal: string = box(() => {
  const rows = ["a", "b"];
  return rows;
});
