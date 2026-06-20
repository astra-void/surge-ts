type Tree = { child: Tree; value: number };

declare const t: Tree;
const a = t.child.missing;
const b: string = t.child.value;
