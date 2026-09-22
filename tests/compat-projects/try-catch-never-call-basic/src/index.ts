declare function g(): Promise<{ a: number }>;
declare const process: { exit(code?: number): never };
declare function die(): never;
async function m() {
  let doc: Awaited<ReturnType<typeof g>>;
  try { doc = await g(); } catch (e) { process.exit(1); }
  return doc.a;
}
function n(f: () => number) {
  let v: number;
  try { v = f(); } catch { die(); }
  const w: string = v;
  let u: number;
  try { u = f(); } catch { console.log("x"); }
  return u + w.length;
}
function o(f: () => number) {
  let v: number;
  try { v = f(); } catch { if (Math.random()) die(); }
  return v;
}
export {};
