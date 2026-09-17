enum Direction { Up, Down }
enum Sparse { Low = 1, High = 4 }
declare const computed: number;
declare function flag(): number;
enum Computed { A = flag(), B = 2 }

const upByValue: Direction = 0;
const downByValue: Direction = 1;
const outOfRange: Direction = 5;
const sparseHit: Sparse = 4;
const sparseGap: Sparse = 2;
const fromNumber: Sparse = computed;
const bitFlags: Sparse = Sparse.Low | Sparse.High;
const memberMatch: Direction.Up = 0;
const memberMismatch: Direction.Up = 1;
const anyComputed: Computed = 99;

function takes(value: Sparse): void {}
takes(4);
takes(3);

let reassigned: Direction = Direction.Up;
reassigned = 7;

declare const maybe: number | undefined;
declare const inRange: 0 | 1;
declare const partlyOutOfRange: 0 | 5;
const defaulted: Direction = maybe ?? 0;
const unionInRange: Direction = inRange;
const unionOutOfRange: Direction = partlyOutOfRange;
