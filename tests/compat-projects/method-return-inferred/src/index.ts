class Counter {
  private count = 0;
  label(prefix: string) {
    return prefix + this.count;
  }
  next() {
    this.count++;
    return this.count;
  }
  maybe(flag: boolean) {
    if (flag) {
      return "set";
    }
  }
  reset() {
    this.count = 0;
  }
  twice() {
    return [this.next(), this.next()];
  }
  static create(start: number) {
    const counter = new Counter();
    counter.count = start;
    return counter;
  }
  static describe() {
    return { kind: "counter", version: 1 };
  }
}

const counter = Counter.create(1);
const a: number = counter.label("n");
const b: string = counter.next();
const c: string = counter.maybe(true);
const d: number = counter.reset();
const e: string[] = counter.twice();
const f: string = Counter.create(2);
const g: { kind: number } = Counter.describe();

abstract class Strategy {
  abstract mode(): "explicit" | "all";
  abstract fixed(): "all";
}
class Chosen extends Strategy {
  constructor(private global: boolean) {
    super();
  }
  mode() {
    return this.global ? "all" : "explicit";
  }
  fixed() {
    return "all" as const;
  }
}

declare const text: string;
let chosen = new Chosen(true).mode();
chosen = text;
const exact: "all" | "explicit" = new Chosen(false).mode();
const wrong: "explicit" = new Chosen(false).fixed();
