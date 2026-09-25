const o = {};
o.a = 1;
Object.defineProperty(o, "b", { value: "x" });
o.a.toFixed();
o.b.toFixed();
o.c;
module.exports = o;
