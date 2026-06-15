const map = new Map<string, number>();
map.set("a", 1);

const value = map.get("a");
const bad: string = value!;
const ok: number = value!;
