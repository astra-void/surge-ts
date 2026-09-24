interface Callable { (): void; }
interface Newable { new (): object; }
interface Hybrid { (x: number): string; prop: number; }
declare const callable: Callable;
declare const newable: Newable;
declare const hybrid: Hybrid;
declare const literal: { (): number };

export const reads = [
    callable.arguments,
    callable.caller,
    callable.length,
    callable.name,
    callable.prototype,
    newable.arguments,
    newable.length,
    hybrid.name,
    hybrid.prop,
    literal.caller,
    callable.call(null),
    callable.bind(null),
];

callable.notAMember;
hybrid.notAMember;
