const values: number[] = [1, 2, 3];

const mapped = values.map((value) => value + 1);
const found = values.find((value) => value === 2);
const joined = values.join(",");
const hasTwo = values.includes(2);
const pushed = values.push(4);
const readonlyValues: ReadonlyArray<number> = values;

const n = Math.floor(1.2);
const now = Date.now();
const json = JSON.stringify({ ok: true });
const parsed = JSON.parse(json);
const bad: string = Math.floor(1.2);
