interface Box {
  anyOf: number[];
}
interface Holder {
  items: boolean | Box;
  label: string;
}

export function fill(o: Holder, rest: number): void {
  o.items = { anyOf: [1] };
  if (rest) {
    o.items.anyOf.push(rest);
  }
}

export function mistyped(o: Holder, count: number): void {
  o.label = count;
}
