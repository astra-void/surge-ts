export function normalized(flag: boolean, names: string[] | undefined) {
  const viaTernary = flag ? { a: 1 } : { a: 1, b: "x" };
  const readTernary: number = viaTernary.b;

  let viaLet = flag ? { kind: "one" } : { kind: "two", extra: true };
  const readLet: number = viaLet.extra;

  const list = [{ a: 0 }, { a: 1, b: "x" }];
  const readList: number = list[0].b;

  const result = flag
    ? { type: "success" as const, id: 1 }
    : { type: "error" as const, message: "fail" };
  const kind: "other" = result.type;
  const id: string = result.id;
  if (result.type === "success") {
    const narrowed: string = result.id;
    return [readTernary, readLet, readList, kind, id, narrowed];
  }

  const keyed = names ? { names } : { fallback: ["there"] };
  const viaTruthy = keyed.names ? keyed.names.join() : keyed.fallback.join();
  return [viaTruthy];
}

export function notNormalized(flag: boolean) {
  const one = { a: 1 };
  const two = { a: 1, b: "x" };
  const viaVariables = flag ? one : two;
  const declared: { a: number } | { a: number; b: string } = flag ? one : two;
  return [viaVariables.b, declared.b];
}
