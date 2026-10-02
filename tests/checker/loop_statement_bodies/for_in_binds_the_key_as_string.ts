export function f(o: { [k: string]: number }) {
  for (const k in o) { k.toFixed(1); }
}
