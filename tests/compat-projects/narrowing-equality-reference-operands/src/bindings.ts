export {};
interface I1 { p1: number }
interface I2 extends I1 { p2: number }
interface I3 { p3: number }

function intersections(y: I1 & I3, z: I2) {
  if (y === z || z === y) {
  } else if (y !== z || z !== y) {
  } else if (y == z || z == y) {
  }
  if (y !== z) {
    return;
  }
  const bothNever: never = y;
  const alsoNever: never = z;
}

function literalValues(x: string | number, y: "a" | "b", n: number) {
  if (x === y) {
    const ok: "a" | "b" = x;
    const bad: number = x;
  }
  if (y === x) {
    const ok: "a" | "b" = x;
  }
  if (x !== y) {
    const still: string | number = x;
    const bad: string = x;
  }
  if (x === n) {
    const num: number = x;
    const bad: string = x;
  }
}

function unknownSubjects(u: unknown, s: string, o: { a: number }) {
  if (u === s) {
    const t: string = u;
  }
  if (u === o) {
    const t: object = u;
    const bad: { a: number } = u;
  }
}

function operands(a: string | number, b: string, c: boolean) {
  if (c && a === b) {
    const t: string = a;
  }
  const length = a === b && a.length;
  const other = a !== b || a.length;
}
