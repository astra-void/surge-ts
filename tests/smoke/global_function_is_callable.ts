export function invoke(fn: Function, name: string): unknown {
  return fn(name);
}

export function invokeGuarded(names: string[], fn: Function | undefined): void {
  for (const name of names) {
    if (typeof fn !== "function") continue;
    fn(name);
  }
}
