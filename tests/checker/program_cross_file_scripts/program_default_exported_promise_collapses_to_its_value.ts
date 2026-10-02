// @filename: node_modules/lib/index.d.ts
// A default-exported promise-returning function is collapsed to its awaited
// value like every other `Promise<T>`, so a member read on the awaited result
// resolves instead of landing on a synthetic promise stand-in.
export interface Res { ok: boolean; status: number }
export default function grab(url: string): Promise<Res>;
// @filename: src/index.ts
import grab from 'lib';
export async function read() {
const res = await grab('u');
return res.status;
}
export function chain() {
return grab('u').then((res) => res.ok);
}
