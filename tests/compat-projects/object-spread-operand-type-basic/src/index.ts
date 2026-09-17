export {}
declare const n: number;
declare const s: string;
declare const b: boolean;
declare const sym: symbol;
declare const u: unknown;
declare const nu: null;
declare const un: undefined;
declare const o: { a: number };
declare const arr: number[];
declare const a: any;
declare const maybe: { a: number } | undefined;
declare const eitherObj: { a: number } | { b: number };
declare const mixed: { a: number } | number;

const ok1 = { ...o };
const ok2 = { ...arr };
const ok3 = { ...a };
const ok4 = { ...maybe };
const ok5 = { ...eitherObj };

const bad1 = { ...n };
const bad2 = { ...s };
const bad3 = { ...b };
const bad4 = { ...sym };
const bad5 = { ...u };
const bad6 = { ...nu };
const bad7 = { ...un };
const bad8 = { ...mixed };

declare const holder: { input?: unknown };

const ok6 = { ...(u ?? {}) };
const ok7 = { ...(holder.input ?? {}) };
const ok8 = { ...(u ?? { k: 1 }) };
