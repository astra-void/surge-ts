type M = { name: "E1"; message: string } | { name: "Correct"; id: string };
type WT = { thing?: M };
type WU = { thing: M | undefined };

function bang1(w: WT) {
    if (w.thing!.name !== "Correct") { w.thing!.message; w.thing!.id; } else { w.thing!.id; w.thing!.message; }
}
function bang2(t: M | undefined) {
    if (t!.name === "Correct") { t!.id; t!.message; }
}
function and1(w: WT) {
    if (w.thing && w.thing.name === "Correct") { w.thing.id; w.thing.message; }
}
function and2(w: WU) {
    if (w.thing !== undefined && w.thing.name !== "Correct") { w.thing.message; w.thing.id; }
}
function or1(w: WT) {
    if (!w.thing || w.thing.name !== "Correct") { return; }
    w.thing.id;
    w.thing.message;
}
function deep(o: { a: { b?: M } }) {
    if (o.a.b && o.a.b.name === "E1") { o.a.b.message; o.a.b.id; }
}

declare const config: { [key: string]: boolean | { prop: string } };
function idx1() {
    if (typeof config["works"] !== "boolean") { config.works.prop = "test"; config["works"].prop = "test"; }
    config.works.prop;
}
function idx2(c: { [key: string]: string | undefined }) {
    if (c.a) { c.a.length; }
    if (c.b !== undefined) { c.b.length; c.other.length; }
    c.a.length;
}
function idx3(c: { [key: string]: M | undefined }) {
    if (c.x && c.x.name === "Correct") { c.x.id; c.x.message; }
}

declare const env: { [key: string]: string | undefined; known: string };
declare const proc: { env: { [key: string]: string | undefined } };
function fromIndex() {
    if (env.VERCEL_URL) { return `https://${env.VERCEL_URL}`; }
    if (proc.env.APP_URL) { const s: string = proc.env.APP_URL; return s; }
    if (env.known) { return env.known; }
    if (env["X"]) { const t: string = env["X"]; return t; }
    return proc.env.OTHER;
}
