type Thing = { foo: string | number; bar(): number; baz: object; next?: Thing };

declare const moduleThing: { check(value: unknown): boolean } | undefined;
if (moduleThing?.check(1)) {
  moduleThing.check;
}

export function equalsValue(o: Thing | undefined, n: Thing | null, value: number, maybe: number | undefined) {
  if (o?.foo === value) {
    o.foo;
  }
  if (o?.["foo"] === value) {
    o["foo"];
  }
  if (o?.bar() === value) {
    o.bar;
  }
  if (o?.next?.bar() === 1) {
    o.next.bar;
  }
  if (n?.bar() == value) {
    n.bar;
  }
  if (o?.bar() !== undefined) {
    o.bar;
  }
  const viaTernary = o?.bar() === value ? o.foo : 0;
  const viaAnd = o?.bar() === value && o.foo;

  if (o?.foo === maybe) {
    o.foo;
  }
  if (o?.bar() !== value) {
    o.bar;
  }
  if (o?.bar() === undefined) {
    o.bar;
  }
  return [viaTernary, viaAnd];
}

export function typeofAndInstanceof(o: Thing | undefined) {
  if (typeof o?.["foo"] === "number") {
    o["foo"];
  }
  if (typeof o?.bar() === "number") {
    o.bar;
  }
  if (o?.baz instanceof Error) {
    o.baz;
  }
  if (typeof o?.bar() === "undefined") {
    o.bar;
  }
  if (typeof o?.bar() !== "number") {
    o.bar;
  }
}
