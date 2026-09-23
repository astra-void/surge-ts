export {};
class A {}
class B extends A { x = 1; constructor() { console.log(this); super(); } }
class C extends A { x = 1; constructor(flag: boolean) { if (flag) { super(); } else { super(); } } }
class D extends A { constructor(private y: number) { const z = 1; super(); } }
class E extends A { constructor(private y: number) { super(); } }
class F extends A { #p = 1; constructor() { const q = () => this; super(); } }
class G extends A { x = 1; constructor() { super(); console.log(this); } }
class H extends A { x = 1; constructor() { (super()); } }
class I extends A { x = 1; constructor() { const q = super(); } }
class J extends A { static s = 1; constructor() { console.log(this); super(); } }
class K extends A { x = 1; constructor() { console.log(1); super(); } }
class L extends A { x = 1; constructor() { console.log(A); this.x; super(); } }
