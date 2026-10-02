class C { constructor(public readonly x: number) {} }
const c = new C(1);
c.x = 2;
export { c };
