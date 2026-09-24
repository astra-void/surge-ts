function handler(this: { n: number }) {}

const target: { n: number; run?: (this: { n: number }) => void; label: string | number } = {
  n: 1,
  label: 0,
};
target.run = handler;
target.run();
(target.run)();
target.label = "text";
export const upper = target.label.toUpperCase();

export function later() {
  target.run();
}

declare function assertDefined(value: unknown): asserts value;
declare const maybeText: string | undefined;
assertDefined(maybeText);
export const size = maybeText.length;
export function readLater() {
  return maybeText.length;
}
export const readArrow = () => maybeText.length;
