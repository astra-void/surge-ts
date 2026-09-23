const f = function (n: number) { return n; };
f.x = 1;
const h = (n: number) => n;
h.y = "";
function readsExpando(a: typeof f.x) { return a; }
readsExpando(f.x);
const k = 1;
const k = 2;
