var x: string | number;

var r1 = typeof x === "string" && typeof x === "string" ? x.substr : x.toFixed;

var r2 = !(typeof x === "string" && typeof x === "string") ? x.toFixed : x.substr;

var r3 = typeof x === "string" || typeof x === "string" ? x.substr : x.toFixed;

var r4 = !(typeof x === "string" || typeof x === "string") ? x.toFixed : x.substr;

var single: string | number;
var r5 = typeof single === "string" ? single.substr : single.toFixed;

var branch: string | number;
if (typeof branch === "string") {
  branch.substr;
} else {
  branch.toFixed;
}

declare var ambient: string | number;
var r6 = typeof ambient === "string" && typeof ambient === "string" ? ambient.substr : ambient.toFixed;

var assigned: string | number;
assigned = Math.random() ? "a" : 1;
var r7 = typeof assigned === "string" ? assigned.substr : assigned.toFixed;
