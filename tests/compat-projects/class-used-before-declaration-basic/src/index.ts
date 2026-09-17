export {}
declare class Ambient { x: number }

const okAmbient = new Ambient();
const okTypePosition: Later = null as any;
const okTypeQuery: typeof Later = Later2Holder.later;
const okFunctionExpr = function () { return new Later() };
const okArrow = () => new Later();
const okMethodShorthand = { m() { return new Later() } };
function okDeclaration() { return new Later() }

const badNew = new Later();
const badValue = Later;
const badStatic = Later2.staticThing;
const badInArray = [new Later3()];
const badInConditional = pick() ? new Later4() : 1;

class Later {}
class Later2 { static staticThing = 1 }
class Later3 {}
class Later4 {}
declare const Later2Holder: { later: typeof Later };
function pick() { return true }

const afterDeclaration = new Later();
export { Later as Exported };
