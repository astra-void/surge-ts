// The generated `.d.ts` fallback (parsed, not a Rust snapshot table) must
// provide the named `Array`/`ReadonlyArray` globals and their `.length`/
// `.map`/`.find` members, mirroring the physical-lib path.
const values: Array<number> = [1, 2, 3];
const readonlyValues: ReadonlyArray<number> = values;
const size: number = values.length;
const doubled: number[] = values.map((value) => value * 2);
const found: number | undefined = values.find((value) => value > 1);
void readonlyValues; void size; void doubled; void found;
