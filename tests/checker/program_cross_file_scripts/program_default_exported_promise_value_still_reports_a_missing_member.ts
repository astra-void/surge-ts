// @filename: node_modules/lib/index.d.ts
// The collapse must not invent members: a name the awaited value does not
// declare still reports.
export interface Res { ok: boolean }
export default function grab(url: string): Promise<Res>;
// @filename: src/index.ts
import grab from 'lib';
export async function read() {
const res = await grab('u');
return res.missing;
}
