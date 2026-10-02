// @noImplicitAny: true
declare const items: string[];
export const a = items.reduce((acc, value) => acc + value.length, 0);
declare const s: string;
export const b = s.replace(/x/g, (c) => c.toUpperCase());
export const c = s.normalize("NFC");
export const d = items.filter((x) => x);
