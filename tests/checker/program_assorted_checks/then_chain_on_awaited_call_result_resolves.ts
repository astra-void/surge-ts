// `Promise<T>` is modeled as its awaited `T`, so a chained `.then`/`.catch` must
// still resolve instead of reporting the member missing on the value type.
async function load(): Promise<number | undefined> { return 1; }
export function use() {
return load().then(() => 2);
}
export async function useCatch() {
await load().catch(() => undefined);
}
