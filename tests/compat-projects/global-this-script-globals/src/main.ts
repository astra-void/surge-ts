var shared = 1;
const fixed = 2;
let mutable = 3;
function helper() { return 1; }
class Widget { }

globalThis.shared;
globalThis.fixed;
globalThis.mutable;
globalThis['shared'];
globalThis['fixed'];
globalThis.helper();
globalThis.Widget;
globalThis.missing;

declare let viaType: (typeof globalThis)['shared'];
