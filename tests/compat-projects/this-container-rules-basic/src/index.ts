export {};

function standalone(): this {
  return null!;
}
class Members {
  static make(): this {
    return null!;
  }
  instance(): this {
    return this;
  }
  literal(): { self: this } {
    return null!;
  }
  field: this = null!;
  arrow = (): this => this;
  static factory = (): this => null!;
  method() {
    return new.target;
  }
  get accessor() {
    return new.target;
  }
  constructor() {
    new.target;
    const inner = () => new.target;
  }
}
interface Shape {
  clone(): this;
  nested: { of: this };
}
type Alias = this;
const topLevel = new.target;
const arrowTarget = () => new.target;
function declared() {
  return new.target;
}
const objectMethod = {
  m(): this {
    return null!;
  },
};

type Loose = infer U;
type Element<T> = T extends Array<infer U> ? U : never;
function inferParam(x: infer V) {}

interface string {}
type number = 1;
class any {}
enum symbol {}
function reserved<boolean>() {}
