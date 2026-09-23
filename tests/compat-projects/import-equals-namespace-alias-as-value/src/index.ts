import N = require("./ns");
N.T;
let t: N.T = 1;
let tBad: N.T = "no";

import M = require("merged");
declare var m: M;
m.bar("hello");
M.bar("hello");
let a: M.A = { a: 1 };

import I = require("./iface");
I.x;
let i: I = { x: 1 };
