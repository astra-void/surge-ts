export {};
type S = { done: boolean; value: number };
type T = { done: true; value: number } | { done: false; value: number };
declare let s: S;
let t: T = s;

interface Blue { color: "blue" }
interface Yellow { color?: "yellow" }
function draw(val: Blue | Yellow) {}
function drawWithColor(currentColor: "blue" | "yellow" | undefined) {
  return draw({ color: currentColor });
}

type Foo = { kind: "a" | "b"; value: number } | { kind: "a"; value: undefined } | { kind: "b"; value: undefined };
function test(obj: { kind: "a" | "b"; value: number | undefined }) {
  let x1: Foo = obj;
}

type Pair = { k: "x"; v: number } | { k: "y"; v: string };
declare let wrong: { k: "x" | "y"; v: number };
let p: Pair = wrong;
