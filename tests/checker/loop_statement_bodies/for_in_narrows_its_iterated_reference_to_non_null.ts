// tsc (flow.go): "for (const _ in ref) acts as a nonnull on ref".
declare const o: { [k: string]: number } | undefined;
export function f() {
  for (const k in o) { o[k].toFixed(1); }
}
