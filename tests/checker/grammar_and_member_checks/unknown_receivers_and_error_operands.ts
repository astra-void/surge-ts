declare const u: unknown;
u.toString();
let w: unknown = 1;
w++;
--w;
try { } catch (e) {
    e.toUpperCase();
}
function f(obj: Record<string, unknown>, key: string) {
    if (typeof obj[key] === "string") {
        obj[key].toUpperCase();
    }
}
var r = missing ** `x`;
