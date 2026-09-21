type Test = { type: "a"; name: string } | { type: "b"; value: number };
export class K {
  test!: Test;
  direct() {
    if (this.test.type === "a") { const n: number = this.test.name; return n; }
    const v: string = this.test.value;
    return v;
  }
  aliased() {
    const t = this.test.type;
    if (t === "a") { const n: number = this.test.name; return n; }
    return this.test.name;
  }
  viaSwitch() {
    switch (this.test.type) {
      case "a": return this.test.value;
      case "b": return this.test.value;
    }
  }
}

export function withThisParameter(this: { test: Test }) {
  const kind = this.test.type;
  if (kind === "a") {
    const n: number = this.test.name;
    return n;
  }
  return this.test.name;
}

export function deepPath(p: { a: { b: Test } }, q: { a: { b: { c: Test } } }) {
  if (p.a.b.type === "a") {
    const n: number = p.a.b.name;
    return n;
  }
  if (q.a.b.c.type === "b") {
    const n: string = q.a.b.c.value;
    return n;
  }
  return 0;
}

export function deepTernary(r: { a: { b: { c: Test } } }) {
  const viaTernary = r.a.b.c.type === "a" ? r.a.b.c.name : r.a.b.c.name;
  return viaTernary;
}

export class Deep {
  state!: { inner: Test };
  read() {
    if (this.state.inner.type === "a") {
      return this.state.inner.name;
    }
    return this.state.inner.name;
  }
}
