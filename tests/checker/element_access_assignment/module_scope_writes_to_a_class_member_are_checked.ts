class C { static s: string = ""; annotated: number = 0; }
C.s = 1;
const c = new C();
c.annotated = "s";
export { C, c };
