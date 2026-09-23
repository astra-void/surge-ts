export {};
interface C1 {
  foo: string;
  c: string;
  bar1: number;
}
interface C2 {
  foo: string;
  c: string;
  bar2: number;
}
interface A {
  foo: string;
}
interface CConstructor {
  new (value: string): C1;
  new (value: number): C2;
}
declare var C: CConstructor;

declare var either: C1 | A;
if (either instanceof C) {
  either.c;
  either.bar1;
  either.bar2;
}

declare var anything: any;
if (anything instanceof C) {
  anything.foo;
  anything.bar1;
}

interface D {
  foo: string;
}
declare var D: { new (): D };
declare var fromAny: any;
if (fromAny instanceof D) {
  fromAny.foo;
  fromAny.bar;
} else {
  fromAny.bar;
}

declare var viaObject: any;
if (viaObject instanceof Object) {
  viaObject.anything;
}
declare var viaFunction: any;
if (viaFunction instanceof Function) {
  viaFunction.anything;
}
