const values: Array<number> = [1, 2, 3];
const readonlyValues: ReadonlyArray<number> = values;
const size: number = values.length;
const doubled: number[] = values.map((value) => value * 2);
const found: number | undefined = values.find((value) => value > 1);

export { values, readonlyValues, size, doubled, found };
