export {};
type Obj = { kind: 'foo', foo: string } | { kind: 'bar', bar: number };
function reassignedParameter(obj: { readonly x: string | number }) {
  const isString = typeof obj.x === 'string';
  obj = { x: 42 };
  if (isString) {
    let s: string = obj.x;
  }
}
function reassignedLater(obj: Obj) {
  const isFoo = obj.kind === 'foo';
  if (isFoo) {
    obj.foo;
  }
  obj = obj;
}
function mutableProperty(outer: { obj: Obj }) {
  const isFoo = outer.obj.kind === 'foo';
  if (isFoo) {
    outer.obj.foo;
  }
}
function readonlyProperty(outer: { readonly obj: Obj }) {
  const isFoo = outer.obj.kind === 'foo';
  if (isFoo) {
    outer.obj.foo;
  }
}
function letNeverAssigned(arg: Obj) {
  let obj = arg;
  const isFoo = obj.kind === 'foo';
  if (isFoo) {
    obj.foo;
  }
}
function readonlyTuple(obj: readonly [string | number]) {
  const isString = typeof obj[0] === 'string';
  if (isString) {
    let s: string = obj[0];
  }
}
function reassignedTuple(obj: readonly [string | number]) {
  const isString = typeof obj[0] === 'string';
  obj = [42];
  if (isString) {
    let s: string = obj[0];
  }
}
class Constructed {
  constructor(readonly x: string | number) {
    const thisIsString = typeof this.x === 'string';
    const paramIsString = typeof x === 'string';
    if (thisIsString && paramIsString) {
      let s: string;
      s = this.x;
      s = x;
    } else {
      this.x = 10;
      x = 10;
    }
  }
}
function varBinding() {
  var v: string | number = Math.random() ? "a" : 1;
  const isString = typeof v === "string";
  if (isString) {
    const s: string = v;
  }
}
