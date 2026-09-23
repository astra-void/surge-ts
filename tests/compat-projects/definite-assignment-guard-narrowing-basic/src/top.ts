class C { p = 1; }
var sn: string | number;
var s: string;
var c: C | string;
var n: number;
var o: { a: number } | null;
var b: boolean;

if (typeof sn === "string") { s = sn; } else { n = sn as number; sn; }
sn;
var r1 = typeof sn === "string" && sn.length;
var r2 = typeof sn !== "string" || sn.length;
var r3 = typeof sn === "number" ? sn : 0;
var r4 = typeof sn === "number" ? 0 : sn;
if (c instanceof C) { c.p; } else { c; }
if (s) { s; } else { s; }
if (o === null) { o; } else { o; }
if (o == null) { o; } else { o; }
if (typeof s === "undefined") { s; } else { s; }
var t1 = true ? n : s;
var t2 = false ? s : n;
var t3 = b || s;
if (false) { s; }
if (true) { } else { n; }
