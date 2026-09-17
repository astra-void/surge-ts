export {}
function annotatedThis(this: { v: number }) { return this.v }
declare function declaredThis(this: void): void;
const objectMethod = { v: 1, m() { return this.v } };
const objectGetter = { get g() { return this } };
class WithClassThis { v = 1; m() { return this.v } n = () => this.v }
const moduleArrow = () => this;
function noThisAtAll() { return 1 }

function plainFunction() { return this.anything }
function insideBranch(c: boolean) { if (c) { return this.y } return 0 }
function throughArrow() { return () => this }
