export function Exp(n: number): number { return n; }
Exp.p1 = 111;
Exp.m = function (n: number): number { return n + 1; };

export const Arrow = (n: number) => n;
Arrow.label = "x";

function Card(): number { return 1; }
function Header(): number { return 2; }
Card.Header = Header;
export default Card;

const local1: number = Exp.p1;
const local2: number = Arrow.label;
const local3: string = Card.Header();
