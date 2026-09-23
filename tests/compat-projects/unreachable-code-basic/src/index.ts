function a() {
  return 1;
  console.log("dead");
  console.log("dead2");
  function hoisted() {}
  console.log("dead3");
}
function b(x: boolean) {
  if (x) { return 1; } else { return 2; }
  var v;
  var w = 1;
  let l = 1;
  interface I {}
  type T = string;
  const enum CE { A }
  enum E { A }
  class C {}
}
function c() {
  while (true) { }
  console.log("after infinite");
}
function d() {
  while (true) { break; }
  console.log("reachable");
  for (;;) { }
  console.log("after for");
}
function e() {
  if (false) { console.log("dead then"); }
  if (true) { } else { console.log("dead else"); }
  if (!true) { console.log("bound as opaque"); }
  if ((false)) { console.log("parens opaque"); }
  if (false && Math.random()) { console.log("dead and"); }
  if (true || Math.random()) { } else { console.log("dead or"); }
  if (!(false || Math.random() > 1)) { } else { console.log("reachable via not"); }
}
function f(x: number) {
  try { return 1; } finally { console.log("fin"); }
  console.log("after try finally");
}
function g(x: number) {
  try { return 1; } catch { console.log("c"); }
  console.log("reachable after catch");
  try { throw new Error(); } catch { return 2; }
  console.log("dead after both exit");
}
function h(x: number) {
  switch (x) { case 1: return 1; default: return 2; }
  console.log("dead after switch");
}
function i(x: number) {
  switch (x) { case 1: return 1; }
  console.log("reachable no default");
  switch (x) { case 1: break; default: return 2; }
  console.log("reachable via break");
}
function j() {
  outer: for (;;) { for (;;) { break outer; } }
  console.log("reachable via labeled break");
  lbl: { break lbl; }
  console.log("reachable via block label");
  do { continue; } while (false);
  console.log("reachable do-while");
  do { return; } while (Math.random() > 0.5);
  console.log("dead do-while returning body");
}
function k() {
  throw new Error("x");
  { console.log("nested dead"); { console.log("deeper dead"); } }
}
function l() {
  return;
  ;
  console.log("after empty");
  export_like();
}
function export_like() {}
function m() {
  (() => { throw new Error(); })();
  console.log("dead after iife");
}
function n() {
  (() => { return; })();
  console.log("reachable after returning iife");
  (async () => { throw new Error(); })();
  console.log("reachable after async iife");
}
function o(x: number) {
  for (const k of [1]) { return k; }
  console.log("reachable after for-of");
  for (; false;) { console.log("dead for body"); }
  while (false) { console.log("dead while body"); }
  console.log("still reachable");
}
namespace Outer {
  export namespace NI { export type X = number }
  export function f() {
    return 1;
  }
}
export {};
