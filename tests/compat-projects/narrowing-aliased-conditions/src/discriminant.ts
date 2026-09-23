export {};
type Obj = { kind: 'foo', foo: string } | { kind: 'bar', bar: number };
function viaSwitch(obj: Obj) {
  const { kind } = obj;
  switch (kind) {
    case 'foo': obj.foo; break;
    case 'bar': obj.bar; break;
  }
}
function viaSwitchAccess(obj: Obj) {
  const kind = obj.kind;
  switch (kind) {
    case 'foo': obj.foo; break;
    case 'bar': obj.bar; break;
  }
}
function viaAliasOperand(obj: { kind: 'foo', foo?: string } | { kind: 'bar', bar?: number }) {
  const { kind } = obj;
  const isFoo = kind == 'foo';
  if (isFoo && obj.foo) {
    let t: string = obj.foo;
  }
}
