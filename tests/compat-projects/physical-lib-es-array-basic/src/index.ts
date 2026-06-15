const values = [1, 2, 3];
const mapped = values.map((value) => value.toString());

const ok: string[] = mapped;
const bad: number[] = mapped;
