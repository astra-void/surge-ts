export {};
declare let x: string | null;
declare let y: string | null | undefined;
if (x !== null) { const a: string = x; }
if (x != null) { const a: string = x; }
if (x) { const a: string = x; }
if (x === null) { const a: null = x; } else { const b: string = x; }
if (y !== undefined) { const a: string = y; }
if (y != undefined) { const a: string = y; }
if (y !== null && y !== undefined) { const a: string = y; }
if (typeof x === "string") { const a: string = x; }
if (typeof x === "object") { const a: null = x; }
if (!x) { const a: string = x; }
function h(p: string | null): string { if (p === null) return ""; return p; }
function h2(p: string | null): string { if (!p) { return ""; } return p; }
function h3(p: string | null): string { return p === null ? "" : p; }
function h4(p: string | null) { if (p == null) { throw new Error(); } const s: string = p; }
const len = x?.length;
const len2: number = len;

interface WithError { error: Error; data: null }
interface WithoutError<Data> { error: null; data: Data }
type DataCarrier<Data> = WithError | WithoutError<Data>;
function carry<Data>(carrier: DataCarrier<Data>) {
    if (carrier.error === null) {
        const error: null = carrier.error;
    } else {
        const error: Error = carrier.error;
        const data: null = carrier.data;
    }
}
