class Base { prop = 1; m() { return 1; } constructor(_a?: number) {} }
export class A extends Base { constructor() { this.prop = 1; this.m(); super(); } }
export class B extends Base { constructor() { const f = () => this.prop; super(); f(); } }
export class C extends Base { constructor() { const g = function (this: any) { return this; }; super(); g(); } }
export class D extends Base { constructor() { super.m(); super(); } }
export class E extends Base { constructor() { super(this.prop); } }
export class F extends Base { constructor(x: boolean) { if (x) { super(); } else { super(); } this.prop = 2; } }
export class G extends Base { constructor() { super(); this.prop = 3; } }
export class H { constructor() { this; } }
