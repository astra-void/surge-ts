// @noImplicitReturns: true
// tsc never applies noImplicitReturns to constructors (they implicitly
// return `this`). A constructor with a conditional `return` value must not
// produce TS7030.
export class C { constructor(x: number) { if (x > 0) { return; } this.y = x; } y = 0; }
