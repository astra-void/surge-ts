declare function apply(seed: number, step: (value: number) => number, last: number): number;
interface Callable {
  (value: number): number;
}
declare function applyCallable(step: Callable): void;

apply(1, (value) => "", 2);
apply(1, function (value) {
  return "";
}, 2);
apply(1, (value) => value + 1, "late");
applyCallable((value) => "");

export const assigned: (value: number) => number = (value) => "";
export const held: { step: (value: number) => number } = { step: (value) => "" };
