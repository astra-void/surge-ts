declare const record: { name: string; value: number; ab: number };
export const r1 = record.nme;
export const r2 = record.valeu;
export const r3 = record.Name;
export const r4 = record.a;
export const r5 = record.xyz;
export const r6 = record["valu"];

interface Settings {
  longPropertyName: number;
}
declare const settings: Settings;
export const s1 = settings.lngPropertyNme;

declare const text: string;
export const t1 = text.lenght;
export const t2 = text.toUppercase();

const list = [1];
export const l1 = list.lenght;
list.pushh(2);

export class Greeter {
  greet() {}
}
new Greeter().gret();
