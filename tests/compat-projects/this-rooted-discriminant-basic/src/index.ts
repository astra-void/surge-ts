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
