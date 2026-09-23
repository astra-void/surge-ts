export {};
declare function* single(): any;
declare function* typed(): Iterable<number>;
declare namespace Space { function* inner(): any; }
declare class Shape { *method(): any; static *make(): any; }
declare global { function* globalGenerator(): any; }
function* concrete() { yield 1; }
class Concrete { *method() { yield 1; } }
const wrong: string = 1;
