export {};
type Obj = { kind: 'foo', foo: string } | { kind: 'bar', bar: number };
function annotated(obj: Obj) {
  const isFoo: boolean = obj.kind === 'foo';
  if (isFoo) {
    obj.foo;
  } else {
    obj.bar;
  }
}
function mutableAlias(obj: Obj) {
  let isFoo = obj.kind === 'foo';
  if (isFoo) {
    obj.foo;
  }
}
