import Card, { Exp, Arrow } from "./lib";

const a1: number = Exp.p1 + Exp.m(1);
const a2: string = Exp.p1;
const b1: string = Arrow.label;
const d1: number = Card.Header();
const d2: string = Card.Header();
Exp.nope;

function early(): number { return Hoisted.tag.length; }
function Hoisted(): number { return 1; }
Hoisted.tag = "t";

function Twice(): number { return 1; }
Twice.a = 1;
Twice.a = "s";
const twice: number = Twice.a;

function scope() {
    function H(): number { return 1; }
    H.count = 0;
    H.count++;
    const n: string = H.count;
    const K = () => 1;
    K.meta = { id: 1 };
    K.meta.id.toFixed();
    K.meta.nope;
}

let Mutable = () => 1;
Mutable.x = 1;
class C {}
C.y = 1;
const O = { f(): number { return 1; } };
O.z = 1;

function Branchy(): number { return 1; }
if (Math.random() > 0.5) {
    Branchy.q = false;
} else {
    Branchy.q = true;
}
const q: string = Branchy.q;

function Shadowed(): void {}
Shadowed.test = "foo";
const aliasTop = Shadowed;
if (Math.random()) {
    const Shadowed = function (): void {};
    Shadowed.test = 42;
    const topCheck: { (): void; test: string } = aliasTop;
    const block: { (): void; test: number } = Shadowed;
    const wrong: { (): void; test: number } = aliasTop;
}
