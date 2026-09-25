import double = require("./mod");
import m1 = require("./obj");
double("s");
m1.count = "no";
let t: import("./types").Named = 1;
let u: import("./types").Nope;
declare const d: typeof import("./mod");
d("y");
function fn() {
    try { } catch (e) { console.log(e); }
    try { } catch (e) {
        var e: boolean;
    }
}
const s: string = "a";
s.toFixed();
