// zod's `$constructor`, whose base is a union of constructors surge does not
// model: the derived type stays open instead of reporting an unresolved name.
declare const Cls: new (..._args: any[]) => { a: number };
declare const Other: new (..._args: any[]) => { b: string };
export function $constructor(flag: boolean) {
const Parent = flag ? Cls : Other;
class Definition extends Parent {}
return Definition;
}
